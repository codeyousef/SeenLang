//! Tests for UTF-8 native string handling

use seen_std::string::{String, StringRef, StringBuilder};

#[test]
fn test_string_creation() {
    let s1 = String::new();
    assert!(s1.is_empty());
    assert_eq!(s1.len(), 0);
    
    let s2 = String::from("Hello, World!");
    assert_eq!(s2.as_str(), "Hello, World!");
    assert_eq!(s2.len(), 13);
    
    let s3 = String::with_capacity(100);
    assert!(s3.capacity() >= 100);
}

#[test]
fn test_string_utf8() {
    let s = String::from("Hello, 世界! 🦀");
    assert_eq!(s.len(), 19); // byte length (emoji is 4 bytes)
    assert_eq!(s.char_count(), 12); // character count
    
    // Test iteration over characters
    let chars: Vec<char> = s.chars().collect();
    assert_eq!(chars.len(), 12);
    assert_eq!(chars[7], '世');
    assert_eq!(chars[11], '🦀');
}

#[test]
fn test_string_push_append() {
    let mut s = String::new();
    s.push('H');
    s.push('i');
    assert_eq!(s.as_str(), "Hi");
    
    s.push_str(" there");
    assert_eq!(s.as_str(), "Hi there");
    
    let other = String::from("!");
    s.append(&other);
    assert_eq!(s.as_str(), "Hi there!");
}

#[test]
fn test_string_manipulation() {
    let mut s = String::from("Hello");
    
    s.insert(5, ',');
    assert_eq!(s.as_str(), "Hello,");
    
    s.insert_str(6, " World");
    assert_eq!(s.as_str(), "Hello, World");
    
    s.remove(5); // Remove comma
    assert_eq!(s.as_str(), "Hello World");
    
    s.truncate(5);
    assert_eq!(s.as_str(), "Hello");
}

#[test]
fn test_string_slicing() {
    let s = String::from("Hello, World!");
    
    let slice1 = s.slice(0, 5);
    assert_eq!(slice1.as_str(), "Hello");
    
    let slice2 = s.slice(7, 12);
    assert_eq!(slice2.as_str(), "World");
    
    // UTF-8 safe slicing
    let s2 = String::from("你好世界");
    let slice3 = s2.slice_chars(0, 2);
    assert_eq!(slice3.as_str(), "你好");
}

#[test]
fn test_string_comparison() {
    let s1 = String::from("abc");
    let s2 = String::from("abc");
    let s3 = String::from("def");
    
    assert_eq!(s1, s2);
    assert_ne!(s1, s3);
    assert!(s1 < s3);
    assert!(s3 > s1);
}

#[test]
fn test_string_search() {
    let s = String::from("Hello, World! Hello again!");
    
    assert!(s.contains("World"));
    assert!(!s.contains("world")); // case sensitive
    
    assert_eq!(s.find("Hello"), Some(0));
    assert_eq!(s.rfind("Hello"), Some(14));
    assert_eq!(s.find("Bye"), None);
    
    assert!(s.starts_with("Hello"));
    assert!(s.ends_with("again!"));
}

#[test]
fn test_string_case_conversion() {
    let s = String::from("Hello World!");
    
    assert_eq!(s.to_lowercase().as_str(), "hello world!");
    assert_eq!(s.to_uppercase().as_str(), "HELLO WORLD!");
    
    // Unicode case conversion
    let s2 = String::from("Привет Мир!");
    assert_eq!(s2.to_lowercase().as_str(), "привет мир!");
    assert_eq!(s2.to_uppercase().as_str(), "ПРИВЕТ МИР!");
}

#[test]
fn test_string_trim() {
    let s = String::from("  Hello World!  \n");
    
    assert_eq!(s.trim().as_str(), "Hello World!");
    assert_eq!(s.trim_start().as_str(), "Hello World!  \n");
    assert_eq!(s.trim_end().as_str(), "  Hello World!");
}

#[test]
fn test_string_split() {
    let s = String::from("one,two,three,four");
    
    let parts: Vec<StringRef> = s.split(',').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0].as_str(), "one");
    assert_eq!(parts[1].as_str(), "two");
    assert_eq!(parts[2].as_str(), "three");
    assert_eq!(parts[3].as_str(), "four");
    
    let s2 = String::from("Hello  World");
    let words: Vec<StringRef> = s2.split_whitespace().collect();
    assert_eq!(words.len(), 2);
    assert_eq!(words[0].as_str(), "Hello");
    assert_eq!(words[1].as_str(), "World");
}

#[test]
fn test_string_replace() {
    let s = String::from("Hello, World! Hello again!");
    
    let s2 = s.replace("Hello", "Hi");
    assert_eq!(s2.as_str(), "Hi, World! Hi again!");
    
    let s3 = s.replace_first("Hello", "Hi");
    assert_eq!(s3.as_str(), "Hi, World! Hello again!");
}

#[test]
fn test_string_builder() {
    let mut builder = StringBuilder::new();
    
    builder.append("Hello");
    builder.append(", ");
    builder.append("World");
    builder.append_char('!');
    
    assert_eq!(builder.len(), 13);
    
    let s = builder.build();
    assert_eq!(s.as_str(), "Hello, World!");
    
    // Builder with capacity
    let mut builder2 = StringBuilder::with_capacity(1000);
    for i in 0..100 {
        builder2.append_format("Item {}, ", i);
    }
    let s2 = builder2.build();
    assert!(s2.len() > 0);
}

#[test]
fn test_string_format() {
    let name = "World";
    let count = 42;
    
    let s = String::format("Hello, {}! The answer is {}.", &[name, &count.to_string()]);
    assert_eq!(s.as_str(), "Hello, World! The answer is 42.");
}

#[test]
fn test_string_parse() {
    let s1 = String::from("42");
    assert_eq!(s1.parse::<i32>(), Ok(42));
    
    let s2 = String::from("3.14");
    assert_eq!(s2.parse::<f64>(), Ok(3.14));
    
    let s3 = String::from("true");
    assert_eq!(s3.parse::<bool>(), Ok(true));
    
    let s4 = String::from("not_a_number");
    assert!(s4.parse::<i32>().is_err());
}

#[test]
fn test_string_ref() {
    let s = String::from("Hello, World!");
    let sref = StringRef::from(s.as_str());
    
    assert_eq!(sref.as_str(), "Hello, World!");
    assert_eq!(sref.len(), 13);
    
    // StringRef is a lightweight view
    assert_eq!(std::mem::size_of::<StringRef>(), 2 * std::mem::size_of::<usize>());
}

#[test]
fn test_string_clone() {
    let s1 = String::from("Hello");
    let s2 = s1.clone();
    
    assert_eq!(s1, s2);
    assert_ne!(s1.as_ptr(), s2.as_ptr()); // Different allocations
}

#[test]
fn test_string_memory_efficiency() {
    // Small string optimization (SSO)
    let small = String::from("Hi");
    assert_eq!(small.heap_allocated(), false); // Should use SSO
    
    let large = String::from("This is a much longer string that won't fit in SSO buffer");
    assert_eq!(large.heap_allocated(), true);
}