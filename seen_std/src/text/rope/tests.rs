//! Tests for rope data structure
//!
//! Comprehensive tests for text manipulation operations and performance

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_creation() {
        let empty_rope = Rope::new();
        assert!(empty_rope.is_empty());
        assert_eq!(empty_rope.len(), 0);
        assert_eq!(empty_rope.byte_len(), 0);

        let rope = Rope::from_str("Hello, World!");
        assert!(!rope.is_empty());
        assert_eq!(rope.len(), 13);
        assert_eq!(rope.to_string().as_str(), "Hello, World!");

        let string_rope = Rope::from_string(String::from("Test string"));
        assert_eq!(string_rope.len(), 11);
        assert_eq!(string_rope.to_string().as_str(), "Test string");
    }

    #[test]
    fn test_rope_insertion() {
        let mut rope = Rope::new();
        
        // Insert into empty rope
        rope.insert(0, "Hello");
        assert_eq!(rope.to_string().as_str(), "Hello");
        assert_eq!(rope.len(), 5);

        // Insert at beginning
        rope.insert(0, "Hi, ");
        assert_eq!(rope.to_string().as_str(), "Hi, Hello");

        // Insert at end
        rope.insert(rope.len(), "!");
        assert_eq!(rope.to_string().as_str(), "Hi, Hello!");

        // Insert in middle
        rope.insert(4, "there ");
        assert_eq!(rope.to_string().as_str(), "Hi, there Hello!");
    }

    #[test]
    fn test_rope_removal() {
        let mut rope = Rope::from_str("Hello, World! How are you?");
        
        // Remove from middle
        rope.remove(7, 13); // Remove "World!"
        assert_eq!(rope.to_string().as_str(), "Hello,  How are you?");

        // Remove from beginning
        rope.remove(0, 7); // Remove "Hello, "
        assert_eq!(rope.to_string().as_str(), " How are you?");

        // Remove from end
        rope.remove(9, rope.len()); // Remove " you?"
        assert_eq!(rope.to_string().as_str(), " How are");

        // Remove everything
        rope.remove(0, rope.len());
        assert!(rope.is_empty());
    }

    #[test]
    fn test_rope_splitting() {
        let rope = Rope::from_str("Hello, World!");
        
        let (left, right) = rope.split(7);
        assert_eq!(left.to_string().as_str(), "Hello, ");
        assert_eq!(right.to_string().as_str(), "World!");

        // Split at beginning
        let (left, right) = rope.split(0);
        assert!(left.is_empty());
        assert_eq!(right.to_string().as_str(), "Hello, World!");

        // Split at end
        let (left, right) = rope.split(rope.len());
        assert_eq!(left.to_string().as_str(), "Hello, World!");
        assert!(right.is_empty());

        // Split beyond end
        let (left, right) = rope.split(rope.len() + 10);
        assert_eq!(left.to_string().as_str(), "Hello, World!");
        assert!(right.is_empty());
    }

    #[test]
    fn test_rope_append() {
        let rope1 = Rope::from_str("Hello, ");
        let rope2 = Rope::from_str("World!");
        
        let combined = rope1.append(&rope2);
        assert_eq!(combined.to_string().as_str(), "Hello, World!");
        
        // Append to empty rope
        let empty = Rope::new();
        let result = empty.append(&rope1);
        assert_eq!(result.to_string().as_str(), "Hello, ");
        
        // Append empty rope
        let result = rope1.append(&empty);
        assert_eq!(result.to_string().as_str(), "Hello, ");
    }

    #[test]
    fn test_rope_slicing() {
        let rope = Rope::from_str("Hello, World! How are you?");
        
        let slice = rope.slice(7, 13);
        assert_eq!(slice.to_string().as_str(), "World!");

        let slice = rope.slice(0, 5);
        assert_eq!(slice.to_string().as_str(), "Hello");

        let slice = rope.slice(rope.len() - 4, rope.len());
        assert_eq!(slice.to_string().as_str(), "you?");

        // Slice beyond bounds
        let slice = rope.slice(rope.len(), rope.len() + 10);
        assert!(slice.is_empty());

        // Invalid slice
        let slice = rope.slice(10, 5);
        assert!(slice.is_empty());
    }

    #[test]
    fn test_rope_char_access() {
        let rope = Rope::from_str("Hello, 世界!");
        
        assert_eq!(rope.char_at(0), Some('H'));
        assert_eq!(rope.char_at(7), Some('世'));
        assert_eq!(rope.char_at(8), Some('界'));
        assert_eq!(rope.char_at(9), Some('!'));
        assert_eq!(rope.char_at(10), None);
        assert_eq!(rope.char_at(100), None);
    }

    #[test]
    fn test_rope_utf8_support() {
        let rope = Rope::from_str("Hello, 世界! 🦀 Здравствуй мир");
        assert_eq!(rope.len(), 30); // Character count, not byte count
        assert!(rope.byte_len() > rope.len()); // UTF-8 uses more bytes
        
        // Test UTF-8 character insertion
        let mut rope = Rope::from_str("Hello");
        rope.insert(5, " 世界");
        assert_eq!(rope.to_string().as_str(), "Hello 世界");
        
        // Test UTF-8 slicing
        let slice = rope.slice(6, 8);
        assert_eq!(slice.to_string().as_str(), "世界");
    }

    #[test]
    fn test_rope_iterators() {
        let rope = Rope::from_str("Hello\nWorld\nTest");
        
        // Test character iterator
        let chars: Vec<char> = rope.chars().collect();
        assert_eq!(chars.len(), 16);
        assert_eq!(chars[0], 'H');
        assert_eq!(chars[5], '\n');
        
        // Test line iterator
        let lines: Vec<Rope> = rope.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].to_string().as_str(), "Hello");
        assert_eq!(lines[1].to_string().as_str(), "World");
        assert_eq!(lines[2].to_string().as_str(), "Test");
    }

    #[test]
    fn test_rope_search() {
        let rope = Rope::from_str("Hello, World! Hello, Universe!");
        
        assert_eq!(rope.find("Hello"), Some(0));
        assert_eq!(rope.find("World"), Some(7));
        assert_eq!(rope.find("Universe"), Some(21));
        assert_eq!(rope.find("NotFound"), None);
        
        // Test case sensitivity
        assert_eq!(rope.find("hello"), None);
    }

    #[test]
    fn test_rope_replace() {
        let rope = Rope::from_str("Hello, World! Hello, World!");
        
        let replaced = rope.replace_all("World", "Universe");
        assert_eq!(replaced.to_string().as_str(), "Hello, Universe! Hello, Universe!");
        
        let replaced = rope.replace_all("Hello", "Hi");
        assert_eq!(replaced.to_string().as_str(), "Hi, World! Hi, World!");
        
        // Replace with empty string (deletion)
        let replaced = rope.replace_all(", World", "");
        assert_eq!(replaced.to_string().as_str(), "Hello! Hello!");
    }

    #[test]
    fn test_rope_large_text() {
        // Create a large rope to test tree structure
        let chunk = "The quick brown fox jumps over the lazy dog. ";
        let large_text = chunk.repeat(1000); // ~45KB
        let rope = Rope::from_str(&large_text);
        
        assert_eq!(rope.len(), large_text.chars().count());
        assert_eq!(rope.to_string(), large_text);
        
        // Test operations on large rope
        let mut rope = rope;
        rope.insert(1000, "INSERTED ");
        assert!(rope.to_string().contains("INSERTED"));
        
        rope.remove(900, 1100);
        assert!(rope.len() < large_text.chars().count());
    }

    #[test]
    fn test_rope_rebalancing() {
        let mut rope = Rope::new();
        
        // Create an unbalanced rope by repeated insertions
        for i in 0..100 {
            rope.insert(rope.len(), &format!("Item {} ", i));
        }
        
        // Rope should still be functional
        assert!(rope.to_string().contains("Item 0"));
        assert!(rope.to_string().contains("Item 99"));
        
        // Test that operations still work on unbalanced rope
        rope.remove(0, 10);
        rope.insert(50, "MIDDLE");
        assert!(rope.to_string().contains("MIDDLE"));
    }

    #[test]
    fn test_rope_memory_stats() {
        let rope = Rope::from_str("Hello, World!");
        let stats = rope.memory_usage();
        
        assert!(stats.leaf_count > 0);
        assert!(stats.total_bytes > 0);
        assert_eq!(stats.leaf_bytes, rope.byte_len());
        
        // Test with larger rope that will have branches
        let large_rope = Rope::from_str(&"x".repeat(10000));
        let large_stats = large_rope.memory_usage();
        assert!(large_stats.branch_count > 0 || large_stats.leaf_count == 1);
    }

    #[test]
    fn test_rope_edge_cases() {
        // Empty string operations
        let mut rope = Rope::new();
        rope.insert(0, "");
        assert!(rope.is_empty());
        
        rope.remove(0, 0);
        assert!(rope.is_empty());
        
        // Single character
        let rope = Rope::from_str("a");
        assert_eq!(rope.len(), 1);
        assert_eq!(rope.char_at(0), Some('a'));
        
        let (left, right) = rope.split(1);
        assert_eq!(left.to_string().as_str(), "a");
        assert!(right.is_empty());
        
        // Only whitespace
        let rope = Rope::from_str("   \n\t  ");
        assert_eq!(rope.len(), 7);
        assert!(!rope.is_empty());
    }

    #[test]
    fn test_rope_performance_insertions() {
        let mut rope = Rope::new();
        let start = std::time::Instant::now();
        
        // Perform many insertions - should be efficient
        for i in 0..1000 {
            rope.insert(rope.len(), &format!("Line {}\n", i));
        }
        
        let duration = start.elapsed();
        assert!(duration.as_millis() < 100); // Should complete in <100ms
        assert_eq!(rope.lines().count(), 1000);
    }

    #[test]
    fn test_rope_performance_splits() {
        let large_text = "x".repeat(100000);
        let rope = Rope::from_str(&large_text);
        
        let start = std::time::Instant::now();
        
        // Perform many splits - should be efficient
        for i in (0..1000).step_by(100) {
            let (_, _) = rope.split(i);
        }
        
        let duration = start.elapsed();
        assert!(duration.as_millis() < 50); // Should complete in <50ms
    }

    #[test]
    fn test_rope_incremental_editing() {
        // Simulate typical text editor operations
        let mut rope = Rope::from_str("function hello() {\n    return \"Hello, World!\";\n}");
        
        // Insert at beginning (add comment)
        rope.insert(0, "// This is a function\n");
        assert!(rope.to_string().starts_with("// This is a function"));
        
        // Insert in middle (add parameter)
        let paren_pos = rope.find("()").unwrap();
        rope.remove(paren_pos, paren_pos + 2);
        rope.insert(paren_pos, "(name)");
        assert!(rope.to_string().contains("hello(name)"));
        
        // Replace string content
        let start_quote = rope.find("\"Hello").unwrap();
        let end_quote = rope.find("World!\"").unwrap() + 6;
        rope.remove(start_quote, end_quote + 1);
        rope.insert(start_quote, "\"Hello, \" + name");
        
        assert!(rope.to_string().contains("\"Hello, \" + name"));
    }

    #[test]
    fn test_rope_equality() {
        let rope1 = Rope::from_str("Hello, World!");
        let rope2 = Rope::from_str("Hello, World!");
        let rope3 = Rope::from_str("Hello, Universe!");
        
        assert_eq!(rope1, rope2);
        assert_ne!(rope1, rope3);
        
        // Test with different internal structures
        let mut rope4 = Rope::new();
        rope4.insert(0, "Hello, ");
        rope4.insert(rope4.len(), "World!");
        assert_eq!(rope1, rope4);
    }

    #[test]
    fn test_rope_display_debug() {
        let rope = Rope::from_str("Hello, World!");
        
        let display_str = format!("{}", rope);
        assert_eq!(display_str, "Hello, World!");
        
        let debug_str = format!("{:?}", rope);
        assert!(debug_str.contains("Rope"));
        assert!(debug_str.contains("len=13"));
    }
}