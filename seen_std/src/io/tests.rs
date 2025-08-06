//! Tests for I/O functionality
//!
//! Comprehensive tests for file operations, streams, and buffered I/O

use std::fs;
use tempfile::TempDir;
use crate::io::*;
use crate::string::String;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_file(content: &str) -> (TempDir, std::path::PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::write(&file_path, content).unwrap();
        (temp_dir, file_path)
    }

    #[test]
    fn test_file_open_read() {
        let (_temp_dir, file_path) = create_temp_file("Hello World!");
        
        let mut file = File::open(&file_path).expect("Failed to open file");
        let content = file.read_to_string().expect("Failed to read file");
        
        assert_eq!(content.as_str(), "Hello World!");
    }

    #[test]
    fn test_file_create_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");
        
        let mut file = File::create(&file_path).expect("Failed to create file");
        let test_string = String::from("Test content");
        
        file.write_string(&test_string).expect("Failed to write string");
        file.flush().expect("Failed to flush");
        drop(file);
        
        // Verify content was written
        let content = fs::read_to_string(&file_path).expect("Failed to read back");
        assert_eq!(content, "Test content");
    }

    #[test]
    fn test_file_write_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("bytes_file.txt");
        
        let mut file = File::create(&file_path).expect("Failed to create file");
        let test_bytes = b"Binary data: \x00\x01\x02\xFF";
        
        file.write_bytes(test_bytes).expect("Failed to write bytes");
        file.flush().expect("Failed to flush");
        drop(file);
        
        // Verify content was written
        let content = fs::read(&file_path).expect("Failed to read back");
        assert_eq!(&content, test_bytes);
    }

    #[test]
    fn test_file_read_to_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("binary_file.bin");
        
        // Write binary data directly to file
        let test_data = b"Binary test data: \x00\x01\x02\xFF\xDE\xAD\xBE\xEF";
        fs::write(&file_path, test_data).unwrap();
        
        let mut file = File::open(&file_path).expect("Failed to open file");
        let bytes = file.read_to_bytes().expect("Failed to read bytes");
        
        assert_eq!(bytes.len(), test_data.len());
        for (i, &expected) in test_data.iter().enumerate() {
            assert_eq!(bytes[i], expected);
        }
    }

    #[test]
    fn test_file_seek_and_position() {
        let (_temp_dir, file_path) = create_temp_file("0123456789");
        
        let mut file = File::open(&file_path).expect("Failed to open file");
        
        // Test seeking to different positions
        let pos = file.seek(std::io::SeekFrom::Start(5)).expect("Failed to seek");
        assert_eq!(pos, 5);
        
        let current_pos = file.position().expect("Failed to get position");
        assert_eq!(current_pos, 5);
        
        // Seek to end
        let end_pos = file.seek(std::io::SeekFrom::End(0)).expect("Failed to seek to end");
        assert_eq!(end_pos, 10);
    }

    #[test]
    fn test_file_size() {
        let content = "Test content for size check";
        let (_temp_dir, file_path) = create_temp_file(content);
        
        let file = File::open(&file_path).expect("Failed to open file");
        let size = file.size().expect("Failed to get size");
        
        assert_eq!(size, content.len() as u64);
    }

    #[test]
    fn test_open_options() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("options_test.txt");
        
        // Create and write initial content
        let options = OpenOptions::new()
            .write(true)
            .create(true);
        
        let mut file = File::open_with_options(&file_path, &options).expect("Failed to open with options");
        file.write_string(&String::from("Initial")).expect("Failed to write");
        file.flush().expect("Failed to flush");
        drop(file);
        
        // Append more content
        let append_options = OpenOptions::new()
            .write(true)
            .append(true);
        
        let mut file = File::open_with_options(&file_path, &append_options).expect("Failed to open for append");
        file.write_string(&String::from(" Appended")).expect("Failed to append");
        file.flush().expect("Failed to flush");
        drop(file);
        
        // Verify final content
        let content = fs::read_to_string(&file_path).expect("Failed to read final content");
        assert_eq!(content, "Initial Appended");
    }

    #[test]
    fn test_convenience_functions() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("convenience_test.txt");
        
        // Test write_file
        let content = String::from("Convenience test content");
        write_file(&file_path, &content).expect("Failed to write file");
        
        // Test read_file
        let read_content = read_file(&file_path).expect("Failed to read file");
        assert_eq!(read_content.as_str(), content.as_str());
        
        // Test file_exists
        assert!(file_exists(&file_path));
        assert!(!file_exists(temp_dir.path().join("nonexistent.txt")));
        
        // Test file_size
        let size = file_size(&file_path).expect("Failed to get file size");
        assert_eq!(size, content.len() as u64);
        
        // Test append_file
        let additional = String::from(" Additional");
        append_file(&file_path, &additional).expect("Failed to append");
        
        let final_content = read_file(&file_path).expect("Failed to read after append");
        assert_eq!(final_content.as_str(), "Convenience test content Additional");
    }

    #[test]
    fn test_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let source_path = temp_dir.path().join("source.txt");
        let dest_path = temp_dir.path().join("dest.txt");
        
        // Create source file
        let content = "Content to copy";
        fs::write(&source_path, content).unwrap();
        
        // Copy file
        let bytes_copied = copy_file(&source_path, &dest_path).expect("Failed to copy file");
        assert_eq!(bytes_copied, content.len() as u64);
        
        // Verify destination content
        let dest_content = fs::read_to_string(&dest_path).expect("Failed to read destination");
        assert_eq!(dest_content, content);
    }

    #[test]
    fn test_directory_operations() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().join("test_dir");
        let nested_path = temp_dir.path().join("nested").join("deep").join("path");
        
        // Test create_dir
        create_dir(&dir_path).expect("Failed to create directory");
        assert!(dir_path.exists());
        assert!(dir_path.is_dir());
        
        // Test create_dir_all
        create_dir_all(&nested_path).expect("Failed to create nested directories");
        assert!(nested_path.exists());
        assert!(nested_path.is_dir());
        
        // Test remove_dir (empty directory)
        remove_dir(&dir_path).expect("Failed to remove directory");
        assert!(!dir_path.exists());
    }

    #[test]
    fn test_remove_dir_all() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("remove_test");
        
        // Create directory structure with files
        create_dir_all(&test_dir.join("sub1").join("sub2")).expect("Failed to create structure");
        fs::write(test_dir.join("file1.txt"), "content1").unwrap();
        fs::write(test_dir.join("sub1").join("file2.txt"), "content2").unwrap();
        fs::write(test_dir.join("sub1").join("sub2").join("file3.txt"), "content3").unwrap();
        
        // Remove entire structure
        remove_dir_all(&test_dir).expect("Failed to remove directory tree");
        assert!(!test_dir.exists());
    }

    #[test]
    fn test_remove_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("to_remove.txt");
        
        // Create file
        fs::write(&file_path, "temporary content").unwrap();
        assert!(file_path.exists());
        
        // Remove file
        remove_file(&file_path).expect("Failed to remove file");
        assert!(!file_path.exists());
    }

    #[test]
    fn test_buffered_reader() {
        use std::io::Cursor;
        
        let data = b"Line 1\nLine 2\nLine 3\n";
        let cursor = Cursor::new(data);
        let _buf_reader = BufReader::new(cursor);
        
        // Test with capacity
        let cursor2 = Cursor::new(data);
        let _buf_reader2 = BufReader::with_capacity(1024, cursor2);
    }

    #[test]
    fn test_buffered_writer() {
        let buffer = std::vec::Vec::new();
        let _buf_writer = BufWriter::new(buffer);
        
        // Test with capacity
        let buffer2 = std::vec::Vec::new();
        let _buf_writer2 = BufWriter::with_capacity(1024, buffer2);
    }

    #[test]
    fn test_stdin_creation() {
        let _stdin = Stdin::new();
        // Note: Cannot easily test actual reading from stdin in unit tests
        // as it would block waiting for user input
    }

    #[test]
    fn test_stdout_creation() {
        let mut stdout = Stdout::new();
        let test_string = String::from("Test output");
        
        // Note: This will actually write to stdout during tests
        // In a real implementation, we might want to redirect stdout for testing
        stdout.write(&test_string).expect("Failed to write to stdout");
        stdout.write_line(&String::from("Test line")).expect("Failed to write line");
    }

    #[test]
    fn test_stderr_creation() {
        let mut stderr = Stderr::new();
        let test_string = String::from("Test error");
        
        // Note: This will actually write to stderr during tests
        stderr.write(&test_string).expect("Failed to write to stderr");
        stderr.write_line(&String::from("Test error line")).expect("Failed to write error line");
    }

    #[test]
    fn test_utf8_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("utf8_test.txt");
        
        let utf8_content = String::from("Hello 世界! 🦀 Здравствуй мир");
        
        // Write UTF-8 content
        write_file(&file_path, &utf8_content).expect("Failed to write UTF-8 file");
        
        // Read back UTF-8 content
        let read_content = read_file(&file_path).expect("Failed to read UTF-8 file");
        assert_eq!(read_content.as_str(), utf8_content.as_str());
    }

    #[test]
    fn test_large_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_file.txt");
        
        // Create large content (1MB)
        let chunk = "0123456789".repeat(100); // 1KB
        let large_content = String::from(&chunk.repeat(1000)); // 1MB
        
        // Write large file
        write_file(&file_path, &large_content).expect("Failed to write large file");
        
        // Read back and verify
        let read_content = read_file(&file_path).expect("Failed to read large file");
        assert_eq!(read_content.len(), large_content.len());
        assert_eq!(read_content.as_str(), large_content.as_str());
    }

    #[test]
    fn test_error_handling() {
        // Test opening non-existent file
        let result = File::open("/non/existent/path/file.txt");
        assert!(result.is_err());
        
        // Test creating file in non-existent directory
        let result = File::create("/non/existent/directory/file.txt");
        assert!(result.is_err());
        
        // Test reading non-existent file
        let result = read_file("/non/existent/file.txt");
        assert!(result.is_err());
        
        // Test file size for non-existent file
        let result = file_size("/non/existent/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_file_operations_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test empty file
        let empty_path = temp_dir.path().join("empty.txt");
        write_file(&empty_path, &String::new()).expect("Failed to write empty file");
        let empty_content = read_file(&empty_path).expect("Failed to read empty file");
        assert_eq!(empty_content.len(), 0);
        
        // Test file with only newlines
        let newlines_path = temp_dir.path().join("newlines.txt");
        let newlines_content = String::from("\n\n\n");
        write_file(&newlines_path, &newlines_content).expect("Failed to write newlines file");
        let read_newlines = read_file(&newlines_path).expect("Failed to read newlines file");
        assert_eq!(read_newlines.as_str(), "\n\n\n");
        
        // Test file with null bytes
        let null_path = temp_dir.path().join("null_bytes.bin");
        let mut file = File::create(&null_path).expect("Failed to create null bytes file");
        let null_data = b"before\x00middle\x00after";
        file.write_bytes(null_data).expect("Failed to write null bytes");
        file.flush().expect("Failed to flush");
        drop(file);
        
        let mut file = File::open(&null_path).expect("Failed to open null bytes file");
        let read_bytes = file.read_to_bytes().expect("Failed to read null bytes");
        assert_eq!(read_bytes.len(), null_data.len());
        for (i, &expected) in null_data.iter().enumerate() {
            assert_eq!(read_bytes[i], expected);
        }
    }
}