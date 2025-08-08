//! Working directory management

use std::path::{Path, PathBuf};
use std::env;
use seen_common::{SeenResult, SeenError};

/// Normalize a path for consistent comparison across platforms
/// On Windows, this handles the \\?\ prefix and canonicalization differences
#[cfg(test)]
fn normalize_path_for_comparison(path: &Path) -> PathBuf {
    // Try to canonicalize first
    match path.canonicalize() {
        Ok(canonical) => {
            // On Windows, strip the \\?\ prefix if present for comparison
            #[cfg(windows)]
            {
                let path_str = canonical.to_string_lossy();
                if path_str.starts_with(r"\\?\") {
                    PathBuf::from(&path_str[4..])
                } else {
                    canonical
                }
            }
            #[cfg(not(windows))]
            canonical
        }
        Err(_) => path.to_path_buf(),
    }
}

/// Check if two paths are equivalent, handling Windows path normalization
#[cfg(test)]
fn paths_equivalent(path1: &Path, path2: &Path) -> bool {
    let normalized1 = normalize_path_for_comparison(path1);
    let normalized2 = normalize_path_for_comparison(path2);
    normalized1 == normalized2
}

/// Working directory manager
pub struct WorkingDirectory {
    original: PathBuf,
    current: PathBuf,
}

impl WorkingDirectory {
    /// Get the current working directory
    pub fn current() -> SeenResult<PathBuf> {
        env::current_dir()
            .map_err(|e| SeenError::runtime_error(format!("Failed to get current directory: {}", e)))
    }
    
    /// Create a new working directory manager
    pub fn new() -> SeenResult<Self> {
        let current = Self::current()?;
        Ok(Self {
            original: current.clone(),
            current,
        })
    }
    
    /// Change the working directory
    pub fn change(&mut self, path: impl AsRef<Path>) -> SeenResult<()> {
        let path = path.as_ref();
        
        // Resolve relative paths
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current.join(path)
        };
        
        // Canonicalize the path
        let canonical = absolute_path.canonicalize()
            .map_err(|e| SeenError::runtime_error(format!("Invalid directory path '{}': {}", path.display(), e)))?;
        
        // Actually change the directory
        env::set_current_dir(&canonical)
            .map_err(|e| SeenError::runtime_error(format!("Failed to change directory to '{}': {}", canonical.display(), e)))?;
        
        self.current = canonical;
        Ok(())
    }
    
    /// Get the current directory path
    pub fn get(&self) -> &Path {
        &self.current
    }
    
    /// Get the original directory path (where we started)
    pub fn original(&self) -> &Path {
        &self.original
    }
    
    /// Reset to the original directory
    pub fn reset(&mut self) -> SeenResult<()> {
        let original = self.original.clone();
        self.change(original)
    }
    
    /// Create a directory if it doesn't exist
    pub fn create_dir(&self, path: impl AsRef<Path>) -> SeenResult<PathBuf> {
        let path = path.as_ref();
        
        // Resolve relative paths
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current.join(path)
        };
        
        // Create the directory
        std::fs::create_dir_all(&absolute_path)
            .map_err(|e| SeenError::runtime_error(format!("Failed to create directory '{}': {}", absolute_path.display(), e)))?;
        
        Ok(absolute_path)
    }
    
    /// Check if a path exists
    pub fn exists(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref();
        
        // Resolve relative paths
        let absolute_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current.join(path)
        };
        
        absolute_path.exists()
    }
    
    /// Resolve a path relative to the current directory
    pub fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
        let path = path.as_ref();
        
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current.join(path)
        }
    }
}

impl Drop for WorkingDirectory {
    fn drop(&mut self) {
        // Try to restore the original directory
        let _ = env::set_current_dir(&self.original);
    }
}

/// Change the current working directory
pub fn change_dir(path: impl AsRef<Path>) -> SeenResult<()> {
    env::set_current_dir(path.as_ref())
        .map_err(|e| SeenError::runtime_error(format!("Failed to change directory: {}", e)))
}

/// Get the current working directory
pub fn current_dir() -> SeenResult<PathBuf> {
    WorkingDirectory::current()
}

/// Create a scoped directory change that automatically restores the original directory
pub struct ScopedDirectory {
    original: PathBuf,
}

impl ScopedDirectory {
    /// Change to a directory temporarily
    pub fn new(path: impl AsRef<Path>) -> SeenResult<Self> {
        let original = current_dir()?;
        change_dir(path)?;
        Ok(Self { original })
    }
}

impl Drop for ScopedDirectory {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.original);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_working_directory_current() {
        let current = WorkingDirectory::current().expect("Failed to get current directory");
        assert!(current.exists());
    }
    
    #[test]
    fn test_working_directory_change() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();
        
        let mut wd = WorkingDirectory::new().expect("Failed to create WorkingDirectory");
        let original = wd.original().to_path_buf();
        
        // Change to temp directory
        wd.change(temp_path).expect("Failed to change directory");
        assert!(paths_equivalent(wd.get(), &temp_path.canonicalize().unwrap()));
        
        // On Windows, add a small delay to allow file handles to be released
        #[cfg(windows)]
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Reset to original - on Windows, try multiple times if it fails due to file locks
        #[cfg(windows)]
        {
            let mut attempts = 0;
            loop {
                match wd.reset() {
                    Ok(_) => break,
                    Err(e) if attempts < 5 && e.to_string().contains("being used by another process") => {
                        attempts += 1;
                        std::thread::sleep(std::time::Duration::from_millis(50 * attempts));
                        continue;
                    }
                    Err(e) => panic!("Failed to reset directory after {} attempts: {}", attempts, e),
                }
            }
        }
        #[cfg(not(windows))]
        wd.reset().expect("Failed to reset directory");
        
        assert!(paths_equivalent(wd.get(), &original));
    }
    
    #[test]
    fn test_working_directory_create() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let wd = WorkingDirectory::new().expect("Failed to create WorkingDirectory");
        
        let new_dir = temp_dir.path().join("new_dir");
        assert!(!new_dir.exists());
        
        let created = wd.create_dir(&new_dir).expect("Failed to create directory");
        assert!(created.exists());
        assert!(new_dir.exists());
    }
    
    #[test]
    fn test_scoped_directory() {
        let original = current_dir().expect("Failed to get current directory");
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path().to_path_buf();
        
        {
            let _scoped = ScopedDirectory::new(&temp_path).expect("Failed to create scoped directory");
            let current = current_dir().expect("Failed to get current directory");
            assert!(paths_equivalent(&current, &temp_path.canonicalize().unwrap()));
        }
        
        // On Windows, add a small delay to allow file handles to be released
        #[cfg(windows)]
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Should be restored after scope
        let restored = current_dir().expect("Failed to get current directory");
        assert!(paths_equivalent(&restored, &original));
    }
}