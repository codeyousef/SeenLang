//! Pipe communication between processes

use std::io::{Read, Write};
use seen_common::{SeenResult, SeenError};

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use std::os::unix::io::{RawFd, AsRawFd, FromRawFd};
    use nix::unistd::pipe;
    
    pub struct Pipe {
        read_fd: RawFd,
        write_fd: RawFd,
    }
    
    impl Pipe {
        pub fn new() -> SeenResult<Self> {
            let (read_fd, write_fd) = pipe()
                .map_err(|e| SeenError::runtime_error(format!("Failed to create pipe: {}", e)))?;
            
            Ok(Self {
                read_fd: read_fd.as_raw_fd(),
                write_fd: write_fd.as_raw_fd(),
            })
        }
        
        pub fn split(self) -> (PipeEnd, PipeEnd) {
            (
                PipeEnd::new(self.read_fd, false),
                PipeEnd::new(self.write_fd, true),
            )
        }
        
        pub fn read_end(&self) -> PipeEnd {
            PipeEnd::new(self.read_fd, false)
        }
        
        pub fn write_end(&self) -> PipeEnd {
            PipeEnd::new(self.write_fd, true)
        }
    }
    
    impl Drop for Pipe {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.read_fd);
                libc::close(self.write_fd);
            }
        }
    }
    
    pub struct PipeEnd {
        fd: RawFd,
        is_write: bool,
    }
    
    impl PipeEnd {
        pub fn new(fd: RawFd, is_write: bool) -> Self {
            Self { fd, is_write }
        }
        
        pub fn into_file(self) -> std::fs::File {
            unsafe { std::fs::File::from_raw_fd(self.fd) }
        }
        
        pub fn as_raw_fd(&self) -> RawFd {
            self.fd
        }
    }
    
    impl Read for PipeEnd {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.is_write {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot read from write end of pipe",
                ));
            }
            
            // Use libc directly for compatibility
            let result = unsafe {
                libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len())
            };
            
            if result < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(result as usize)
            }
        }
    }
    
    impl Write for PipeEnd {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if !self.is_write {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot write to read end of pipe",
                ));
            }
            
            // Use libc directly for compatibility
            let result = unsafe {
                libc::write(self.fd, buf.as_ptr() as *const libc::c_void, buf.len())
            };
            
            if result < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(result as usize)
            }
        }
        
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    
    impl AsRawFd for PipeEnd {
        fn as_raw_fd(&self) -> RawFd {
            self.fd
        }
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    
    pub struct Pipe {
        _marker: std::marker::PhantomData<()>,
    }
    
    impl Pipe {
        pub fn new() -> SeenResult<Self> {
            Err(SeenError::runtime_error("Pipes not yet implemented on Windows"))
        }
        
        pub fn split(self) -> (PipeEnd, PipeEnd) {
            unimplemented!("Pipes not yet implemented on Windows")
        }
        
        pub fn read_end(&self) -> PipeEnd {
            unimplemented!("Pipes not yet implemented on Windows")
        }
        
        pub fn write_end(&self) -> PipeEnd {
            unimplemented!("Pipes not yet implemented on Windows")
        }
    }
    
    pub struct PipeEnd {
        _marker: std::marker::PhantomData<()>,
    }
    
    impl PipeEnd {
        pub fn new(_fd: i32, _is_write: bool) -> Self {
            Self { _marker: std::marker::PhantomData }
        }
    }
    
    impl Read for PipeEnd {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Pipes not implemented on this platform",
            ))
        }
    }
    
    impl Write for PipeEnd {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Pipes not implemented on this platform",
            ))
        }
        
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(unix)]
pub use unix_impl::{Pipe, PipeEnd};

#[cfg(windows)]
pub use windows_impl::{Pipe, PipeEnd};

/// Create a pipe between two processes
pub fn create_pipe() -> SeenResult<(PipeEnd, PipeEnd)> {
    let pipe = Pipe::new()?;
    Ok(pipe.split())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[cfg(unix)]
    fn test_pipe_creation() {
        let pipe = Pipe::new().expect("Failed to create pipe");
        let (mut read_end, mut write_end) = pipe.split();
        
        // Write to the pipe
        write_end.write_all(b"Hello, pipe!").expect("Failed to write to pipe");
        
        // Read from the pipe
        let mut buffer = vec![0u8; 12];
        read_end.read_exact(&mut buffer).expect("Failed to read from pipe");
        
        assert_eq!(&buffer, b"Hello, pipe!");
    }
    
    #[test]
    #[cfg(unix)]
    fn test_pipe_end_restrictions() {
        let pipe = Pipe::new().expect("Failed to create pipe");
        let (mut read_end, mut write_end) = pipe.split();
        
        // Test that we can't write to read end
        assert!(write_end.read(&mut [0u8; 10]).is_err());
        
        // Test that we can't read from write end
        assert!(read_end.write(b"test").is_err());
    }
}