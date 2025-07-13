//! Common test utilities for the Seen language test suite

use tempfile::TempDir;
use std::path::{Path, PathBuf};
use std::fs;

// Include the new test utilities module
pub mod test_utils;

/// Represents the language variant for testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestLanguage {
    English,
    Arabic,
}

impl TestLanguage {
    pub fn to_string(&self) -> &'static str {
        match self {
            TestLanguage::English => "english",
            TestLanguage::Arabic => "arabic",
        }
    }
}

/// Test fixture builder for creating test projects
pub struct TestProjectBuilder {
    name: String,
    language: TestLanguage,
    files: Vec<(PathBuf, String)>,
}

impl TestProjectBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            language: TestLanguage::English,
            files: Vec::new(),
        }
    }
    
    pub fn language(mut self, language: TestLanguage) -> Self {
        self.language = language;
        self
    }
    
    pub fn add_file(mut self, path: impl AsRef<Path>, content: impl Into<String>) -> Self {
        self.files.push((path.as_ref().to_path_buf(), content.into()));
        self
    }
    
    pub fn build(self) -> Result<TestProject, std::io::Error> {
        let temp_dir = TempDir::new()?;
        let project_path = temp_dir.path().join(&self.name);
        fs::create_dir_all(&project_path)?;
        
        // Create seen.toml
        let config_content = format\!(
            "[project]\nname = \"{}\"\nversion = \"0.1.0\"\nlanguage = \"{}\"\n",
            self.name,
            self.language.to_string()
        );
        fs::write(project_path.join("seen.toml"), config_content)?;
        
        // Create source directory
        let src_path = project_path.join("src");
        fs::create_dir_all(&src_path)?;
        
        // Create files
        for (path, content) in self.files {
            let full_path = project_path.join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(full_path, content)?;
        }
        
        Ok(TestProject {
            _temp_dir: temp_dir,
            path: project_path,
        })
    }
}

/// Represents a test project with automatic cleanup
pub struct TestProject {
    _temp_dir: TempDir,
    path: PathBuf,
}

impl TestProject {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Run a test with both English and Arabic variants
pub fn test_both_languages<F>(test_name: &str, test_fn: F)
where
    F: Fn(TestLanguage),
{
    println\!("Testing {}: English variant", test_name);
    test_fn(TestLanguage::English);
    
    println\!("Testing {}: Arabic variant", test_name);
    test_fn(TestLanguage::Arabic);
}

/// Common test source code snippets
pub mod snippets {
    use super::TestLanguage;
    
    pub fn hello_world(lang: TestLanguage) -> &'static str {
        match lang {
            TestLanguage::English => {
                r#"func main() {
    val greeting = "Hello, World\!";
    println(greeting);
}"#
            }
            TestLanguage::Arabic => {
                r#"دالة رئيسية() {
    ثابت تحية = "مرحباً، يا عالم\!";
    اطبع(تحية);
}"#
            }
        }
    }
    
    pub fn simple_function(lang: TestLanguage) -> &'static str {
        match lang {
            TestLanguage::English => {
                r#"func add(a: Int, b: Int) -> Int {
    return a + b;
}"#
            }
            TestLanguage::Arabic => {
                r#"دالة جمع(أ: صحيح، ب: صحيح) -> صحيح {
    ارجع أ + ب;
}"#
            }
        }
    }
}

/// Assertion helpers
pub mod assertions {
    use std::fmt::Debug;
    
    pub fn assert_contains_subsequence<T: Debug + PartialEq>(haystack: &[T], needle: &[T]) {
        let mut needle_iter = needle.iter();
        let mut current_needle = needle_iter.next();
        
        for item in haystack {
            if let Some(needle_item) = current_needle {
                if item == needle_item {
                    current_needle = needle_iter.next();
                }
            }
        }
        
        assert!(
            current_needle.is_none(),
            "Subsequence {:?} not found in {:?}",
            needle,
            haystack
        );
    }
}
