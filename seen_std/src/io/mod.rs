//! I/O functionality for file and stream operations
//!
//! High-performance I/O with zero-copy where possible

use std::fs::{File as StdFile, OpenOptions as StdOpenOptions};
use std::io::{self, Read as StdRead, Write as StdWrite, Seek as StdSeek, SeekFrom};
use std::path::Path;
use crate::string::String;
use crate::collections::Vec;
// Use std Result for I/O operations
type Result<T> = std::result::Result<T, io::Error>;

/// File handle for reading and writing
pub struct File {
    inner: StdFile,
}

impl File {
    /// Opens a file in read-only mode
    pub fn open<P: AsRef<Path>>(path: P) -> Result<File> {
        StdFile::open(path)
            .map(|f| File { inner: f })
    }

    /// Creates a new file, truncating if it exists
    pub fn create<P: AsRef<Path>>(path: P) -> Result<File> {
        StdFile::create(path)
            .map(|f| File { inner: f })
    }

    /// Opens a file with specific options
    pub fn open_with_options<P: AsRef<Path>>(path: P, options: &OpenOptions) -> Result<File> {
        options.inner.open(path)
            .map(|f| File { inner: f })
    }

    /// Reads the entire file into a string
    pub fn read_to_string(&mut self) -> Result<String> {
        let mut buffer = std::string::String::new();
        self.inner.read_to_string(&mut buffer)?;
        Ok(String::from(&buffer))
    }

    /// Reads the entire file into bytes
    pub fn read_to_bytes(&mut self) -> Result<Vec<u8>> {
        let mut buffer = std::vec::Vec::new();
        self.inner.read_to_end(&mut buffer)?;
        
        let mut result = Vec::with_capacity(buffer.len());
        for byte in buffer {
            result.push(byte);
        }
        Ok(result)
    }

    /// Writes a string to the file
    pub fn write_string(&mut self, s: &String) -> Result<usize> {
        self.inner.write(s.as_str().as_bytes())
    }

    /// Writes bytes to the file
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<usize> {
        self.inner.write(bytes)
    }

    /// Flushes the file buffer
    pub fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }

    /// Seeks to a position in the file
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        self.inner.seek(pos)
    }

    /// Gets the current position in the file
    pub fn position(&mut self) -> Result<u64> {
        self.seek(SeekFrom::Current(0))
    }

    /// Gets the file size
    pub fn size(&self) -> Result<u64> {
        self.inner.metadata()
            .map(|m| m.len())
    }
}

/// Options for opening files
pub struct OpenOptions {
    inner: StdOpenOptions,
}

impl OpenOptions {
    /// Creates a new set of options
    pub fn new() -> Self {
        OpenOptions {
            inner: StdOpenOptions::new(),
        }
    }

    /// Sets read access
    pub fn read(mut self, read: bool) -> Self {
        self.inner.read(read);
        self
    }

    /// Sets write access
    pub fn write(mut self, write: bool) -> Self {
        self.inner.write(write);
        self
    }

    /// Sets append mode
    pub fn append(mut self, append: bool) -> Self {
        self.inner.append(append);
        self
    }

    /// Sets truncate mode
    pub fn truncate(mut self, truncate: bool) -> Self {
        self.inner.truncate(truncate);
        self
    }

    /// Sets create mode
    pub fn create(mut self, create: bool) -> Self {
        self.inner.create(create);
        self
    }

    /// Sets create_new mode
    pub fn create_new(mut self, create_new: bool) -> Self {
        self.inner.create_new(create_new);
        self
    }
}

impl Default for OpenOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffered reader for efficient reading
pub struct BufReader<R> {
    #[allow(dead_code)]
    inner: std::io::BufReader<R>,
}

impl<R: StdRead> BufReader<R> {
    /// Creates a new buffered reader
    pub fn new(reader: R) -> Self {
        BufReader {
            inner: std::io::BufReader::new(reader),
        }
    }

    /// Creates a buffered reader with specified capacity
    pub fn with_capacity(capacity: usize, reader: R) -> Self {
        BufReader {
            inner: std::io::BufReader::with_capacity(capacity, reader),
        }
    }
}

/// Buffered writer for efficient writing
pub struct BufWriter<W: StdWrite> {
    #[allow(dead_code)]
    inner: std::io::BufWriter<W>,
}

impl<W: StdWrite> BufWriter<W> {
    /// Creates a new buffered writer
    pub fn new(writer: W) -> Self {
        BufWriter {
            inner: std::io::BufWriter::new(writer),
        }
    }

    /// Creates a buffered writer with specified capacity
    pub fn with_capacity(capacity: usize, writer: W) -> Self {
        BufWriter {
            inner: std::io::BufWriter::with_capacity(capacity, writer),
        }
    }
}

/// Standard input stream
pub struct Stdin {
    #[allow(dead_code)]
    inner: io::Stdin,
}

impl Stdin {
    /// Creates a handle to standard input
    pub fn new() -> Self {
        Stdin {
            inner: io::stdin(),
        }
    }

    /// Reads a line from stdin
    pub fn read_line(&self) -> Result<String> {
        let mut buffer = std::string::String::new();
        let stdin = io::stdin();
        stdin.read_line(&mut buffer)?;
        Ok(String::from(&buffer))
    }
}

/// Standard output stream
pub struct Stdout {
    inner: io::Stdout,
}

impl Stdout {
    /// Creates a handle to standard output
    pub fn new() -> Self {
        Stdout {
            inner: io::stdout(),
        }
    }

    /// Writes a string to stdout
    pub fn write(&mut self, s: &String) -> Result<()> {
        self.inner.write_all(s.as_str().as_bytes())?;
        self.inner.flush()
    }

    /// Writes a line to stdout
    pub fn write_line(&mut self, s: &String) -> Result<()> {
        self.inner.write_all(s.as_str().as_bytes())?;
        self.inner.write_all(b"\n")?;
        self.inner.flush()
    }
}

/// Standard error stream
pub struct Stderr {
    inner: io::Stderr,
}

impl Stderr {
    /// Creates a handle to standard error
    pub fn new() -> Self {
        Stderr {
            inner: io::stderr(),
        }
    }

    /// Writes a string to stderr
    pub fn write(&mut self, s: &String) -> Result<()> {
        self.inner.write_all(s.as_str().as_bytes())?;
        self.inner.flush()
    }

    /// Writes a line to stderr
    pub fn write_line(&mut self, s: &String) -> Result<()> {
        self.inner.write_all(s.as_str().as_bytes())?;
        self.inner.write_all(b"\n")?;
        self.inner.flush()
    }
}

/// Convenience functions for common I/O operations

/// Reads an entire file to a string
pub fn read_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let mut file = File::open(path)?;
    file.read_to_string()
}

/// Writes a string to a file
pub fn write_file<P: AsRef<Path>>(path: P, content: &String) -> Result<()> {
    let mut file = File::create(path)?;
    file.write_string(content)?;
    file.flush()
}

/// Appends a string to a file
pub fn append_file<P: AsRef<Path>>(path: P, content: &String) -> Result<()> {
    let mut file = File::open_with_options(
        path,
        &OpenOptions::new().write(true).append(true).create(true)
    )?;
    file.write_string(content)?;
    file.flush()
}

/// Checks if a file exists
pub fn file_exists<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().exists()
}

/// Gets file size
pub fn file_size<P: AsRef<Path>>(path: P) -> Result<u64> {
    std::fs::metadata(path)
        .map(|m| m.len())
}

/// Copies a file
pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<u64> {
    std::fs::copy(from, to)
}

/// Removes a file
pub fn remove_file<P: AsRef<Path>>(path: P) -> Result<()> {
    std::fs::remove_file(path)
}

/// Creates a directory
pub fn create_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    std::fs::create_dir(path)
}

/// Creates a directory and all parent directories
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    std::fs::create_dir_all(path)
}

/// Removes a directory
pub fn remove_dir<P: AsRef<Path>>(path: P) -> Result<()> {
    std::fs::remove_dir(path)
}

/// Removes a directory and all its contents
pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> Result<()> {
    std::fs::remove_dir_all(path)
}

// Re-export commonly used types
pub use self::File as IoFile;
pub use self::Stdin as IoStdin;
pub use self::Stdout as IoStdout;
pub use self::Stderr as IoStderr;

#[cfg(test)]
mod tests;