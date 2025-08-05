//! Step 2 TDD Tests: Character stream abstraction, lookahead, backtracking

use seen_lexer::char_stream::{CharStream, CharExt};
use seen_common::Position;

/// FAILING TEST: Character stream abstraction works
#[test]
fn test_character_stream_abstraction() {
    let input = "func main() {\n    println(\"Hello, Seen!\");\n}";
    let mut stream = CharStream::new(input);
    
    // Test basic operations
    assert_eq!(stream.current(), Some('f'));
    assert_eq!(stream.advance(), Some('f'));
    assert_eq!(stream.current(), Some('u'));
    
    // Test position tracking
    let pos = stream.position();
    assert_eq!(pos.line, 1);
    assert_eq!(pos.column, 2);
    
    // Test advance_while
    let word = stream.advance_while(|ch| ch.is_alphabetic());
    assert_eq!(word, "unc");
    
    // Test line tracking
    stream.advance_while(|ch| ch != '\n');
    stream.advance(); // consume newline
    assert_eq!(stream.position().line, 2);
    assert_eq!(stream.position().column, 1);
}

/// FAILING TEST: Lookahead functionality works
#[test]
fn test_lookahead_functionality() {
    let mut stream = CharStream::new("if (x > 10) { return true; }");
    
    // Test multi-character lookahead
    assert_eq!(stream.peek(0), Some('i'));
    assert_eq!(stream.peek(1), Some('f'));
    assert_eq!(stream.peek(2), Some(' '));
    assert_eq!(stream.peek(3), Some('('));
    
    // Verify position hasn't changed
    assert_eq!(stream.current(), Some('i'));
    assert_eq!(stream.position().offset, 0);
    
    // Test lookahead beyond buffer
    assert_eq!(stream.peek(10), Some('1'));
    assert_eq!(stream.peek(11), Some('0'));
    
    // Test lookahead at end
    let mut end_stream = CharStream::new("abc");
    end_stream.advance();
    end_stream.advance();
    end_stream.advance();
    assert_eq!(end_stream.peek(0), None);
    assert_eq!(end_stream.peek(1), None);
}

/// FAILING TEST: Backtracking works correctly
#[test]
fn test_backtracking() {
    let mut stream = CharStream::new("let x = 42; let y = 99;");
    
    // Advance to first assignment
    stream.advance_while(|ch| ch != '=');
    stream.advance(); // skip '='
    stream.skip_while(|ch| ch.is_whitespace());
    
    // Save position before number
    stream.save_position();
    let start_pos = stream.position();
    
    // Read number
    let num = stream.advance_while(|ch| ch.is_numeric());
    assert_eq!(num, "42");
    
    // Continue past semicolon
    stream.advance_while(|ch| ch != ';');
    stream.advance();
    
    // Now backtrack
    stream.restore_position().unwrap();
    assert_eq!(stream.position(), start_pos);
    assert_eq!(stream.current(), Some('4'));
    
    // Test multiple save/restore levels
    stream.save_position();
    stream.advance();
    stream.save_position();
    stream.advance();
    
    assert_eq!(stream.current(), Some(';'));
    
    stream.restore_position().unwrap();
    assert_eq!(stream.current(), Some('2'));
    
    stream.restore_position().unwrap();
    assert_eq!(stream.current(), Some('4'));
}

/// FAILING TEST: String matching works
#[test]
fn test_string_matching() {
    let mut stream = CharStream::new("func async await function");
    
    // Test exact string matching
    assert!(stream.match_string("func"));
    assert_eq!(stream.current(), Some(' '));
    
    stream.advance(); // skip space
    
    // Test failed match doesn't advance
    stream.save_position();
    assert!(!stream.match_string("await"));
    stream.restore_position().unwrap();
    assert!(stream.match_string("async"));
    
    // Test match_any
    stream.advance(); // skip space
    let keywords = &["async", "await", "function"];
    let matched = stream.match_any(keywords);
    assert_eq!(matched, Some(1)); // "await" at index 1
    
    stream.advance(); // skip space
    let matched = stream.match_any(keywords);
    assert_eq!(matched, Some(2)); // "function" at index 2
}

/// FAILING TEST: Unicode support works correctly
#[test]
fn test_unicode_support() {
    let arabic_code = "دالة رئيسية() {\n    اطبع(\"مرحبا، سين!\");\n}";
    let mut stream = CharStream::new(arabic_code);
    
    // Test Arabic character handling
    assert_eq!(stream.current(), Some('د'));
    let word = stream.advance_while(|ch| !ch.is_whitespace());
    assert_eq!(word, "دالة");
    
    // Test position tracking with multi-byte characters
    let pos = stream.position();
    assert_eq!(pos.line, 1);
    // Column should count characters, not bytes
    assert_eq!(pos.column, 5); // After 4 Arabic characters
    
    // Test line tracking
    stream.advance_while(|ch| ch != '\n');
    stream.advance(); // newline
    assert_eq!(stream.position().line, 2);
}

/// FAILING TEST: Incremental lexing support
#[test]
fn test_incremental_lexing() {
    let mut stream = CharStream::new("let x = 42; let y = x + 1;");
    
    // Lex first statement
    stream.save_position();
    let first_stmt_start = stream.byte_position();
    stream.advance_while(|ch| ch != ';');
    stream.advance(); // semicolon
    let first_stmt = stream.slice_from(first_stmt_start);
    assert_eq!(first_stmt, "let x = 42;");
    
    // Save checkpoint after first statement
    let checkpoint = stream.position();
    stream.save_position();
    
    // Lex second statement
    stream.skip_while(|ch| ch.is_whitespace());
    let second_stmt_start = stream.byte_position();
    stream.advance_while(|ch| ch != ';');
    stream.advance();
    let second_stmt = stream.slice_from(second_stmt_start);
    assert_eq!(second_stmt, "let y = x + 1;");
    
    // Simulate incremental update - go back to checkpoint
    stream.restore_position().unwrap();
    assert_eq!(stream.position(), checkpoint);
    
    // Re-lex from checkpoint with "modified" content
    stream.skip_while(|ch| ch.is_whitespace());
    assert_eq!(stream.current(), Some('l')); // Start of "let"
}

/// FAILING TEST: Performance characteristics
#[test]
fn test_stream_performance() {
    use std::time::Instant;
    
    // Generate large input
    let mut input = String::new();
    for i in 0..10000 {
        input.push_str(&format!("let var{} = {}; ", i, i));
    }
    
    let mut stream = CharStream::new(&input);
    
    // Measure advance performance
    let start = Instant::now();
    let mut count = 0;
    while !stream.is_at_end() {
        stream.advance();
        count += 1;
    }
    let elapsed = start.elapsed();
    
    // Should process >10M characters/second
    let chars_per_second = count as f64 / elapsed.as_secs_f64();
    println!("Character stream performance: {:.2}M chars/sec", chars_per_second / 1_000_000.0);
    
    // Relaxed check for CI environments
    assert!(chars_per_second > 1_000_000.0, 
        "Character stream too slow: {:.2}M chars/sec", chars_per_second / 1_000_000.0);
}