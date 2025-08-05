//! Character stream abstraction for efficient lexing with lookahead and backtracking

use seen_common::{Position, SeenResult, SeenError};

/// A buffered character stream with lookahead and backtracking support
pub struct CharStream<'a> {
    /// The underlying input string
    input: &'a str,
    
    /// Current byte position in the input
    byte_position: usize,
    
    /// Current character position (for Position tracking)
    char_position: Position,
    
    /// Lookahead buffer for peeking ahead
    lookahead_buffer: Vec<(char, usize)>, // (character, byte_length)
    
    /// Saved positions for backtracking
    saved_positions: Vec<StreamState>,
    
    /// Pre-computed line starts for efficient line/column calculation
    line_starts: Vec<usize>,
}

/// Saved state for backtracking
#[derive(Clone, Debug)]
struct StreamState {
    byte_position: usize,
    char_position: Position,
    lookahead_buffer: Vec<(char, usize)>,
}

impl<'a> CharStream<'a> {
    /// Create a new character stream
    pub fn new(input: &'a str) -> Self {
        let line_starts = Self::compute_line_starts(input);
        
        Self {
            input,
            byte_position: 0,
            char_position: Position::start(),
            lookahead_buffer: Vec::with_capacity(8),
            saved_positions: Vec::new(),
            line_starts,
        }
    }
    
    /// Compute line start positions for efficient line/column calculation
    fn compute_line_starts(input: &str) -> Vec<usize> {
        let mut starts = vec![0];
        
        for (i, ch) in input.char_indices() {
            if ch == '\n' {
                starts.push(i + ch.len_utf8());
            }
        }
        
        starts
    }
    
    /// Get the current character without advancing
    pub fn current(&mut self) -> Option<char> {
        self.peek(0)
    }
    
    /// Peek ahead n characters without advancing
    pub fn peek(&mut self, n: usize) -> Option<char> {
        // Ensure we have enough characters in the lookahead buffer
        while self.lookahead_buffer.len() <= n {
            if !self.fill_lookahead() {
                return None;
            }
        }
        
        self.lookahead_buffer.get(n).map(|(ch, _)| *ch)
    }
    
    /// Fill the lookahead buffer with one more character
    fn fill_lookahead(&mut self) -> bool {
        let current_pos = self.byte_position + self.lookahead_buffer
            .iter()
            .map(|(_, len)| len)
            .sum::<usize>();
        
        if current_pos >= self.input.len() {
            return false;
        }
        
        // Get the next character
        let remaining = &self.input[current_pos..];
        if let Some(ch) = remaining.chars().next() {
            self.lookahead_buffer.push((ch, ch.len_utf8()));
            true
        } else {
            false
        }
    }
    
    /// Advance the stream by one character
    pub fn advance(&mut self) -> Option<char> {
        if self.lookahead_buffer.is_empty() && !self.fill_lookahead() {
            return None;
        }
        
        if let Some((ch, byte_len)) = self.lookahead_buffer.drain(..1).next() {
            self.byte_position += byte_len;
            
            // Update position
            if ch == '\n' {
                self.char_position.line += 1;
                self.char_position.column = 1;
            } else {
                self.char_position.column += 1;
            }
            self.char_position.offset = self.byte_position as u32;
            
            Some(ch)
        } else {
            None
        }
    }
    
    /// Advance while a predicate is true
    pub fn advance_while<F>(&mut self, mut predicate: F) -> String
    where
        F: FnMut(char) -> bool,
    {
        let mut result = String::new();
        
        while let Some(ch) = self.current() {
            if !predicate(ch) {
                break;
            }
            result.push(ch);
            self.advance();
        }
        
        result
    }
    
    /// Skip characters while a predicate is true
    pub fn skip_while<F>(&mut self, mut predicate: F) -> usize
    where
        F: FnMut(char) -> bool,
    {
        let mut count = 0;
        
        while let Some(ch) = self.current() {
            if !predicate(ch) {
                break;
            }
            self.advance();
            count += 1;
        }
        
        count
    }
    
    /// Get the current position
    pub fn position(&self) -> Position {
        self.char_position
    }
    
    /// Get the current byte position
    pub fn byte_position(&self) -> usize {
        self.byte_position
    }
    
    /// Save the current position for potential backtracking
    pub fn save_position(&mut self) {
        self.saved_positions.push(StreamState {
            byte_position: self.byte_position,
            char_position: self.char_position,
            lookahead_buffer: self.lookahead_buffer.clone(),
        });
    }
    
    /// Restore to the last saved position
    pub fn restore_position(&mut self) -> SeenResult<()> {
        if let Some(state) = self.saved_positions.pop() {
            self.byte_position = state.byte_position;
            self.char_position = state.char_position;
            self.lookahead_buffer = state.lookahead_buffer;
            Ok(())
        } else {
            Err(SeenError::lex_error("No saved position to restore"))
        }
    }
    
    /// Discard the last saved position without restoring
    pub fn discard_saved_position(&mut self) -> SeenResult<()> {
        if self.saved_positions.pop().is_some() {
            Ok(())
        } else {
            Err(SeenError::lex_error("No saved position to discard"))
        }
    }
    
    /// Check if we're at the end of the stream
    pub fn is_at_end(&mut self) -> bool {
        self.current().is_none()
    }
    
    /// Get a slice of the input from a byte position to current position
    pub fn slice_from(&self, start_byte_pos: usize) -> &'a str {
        &self.input[start_byte_pos..self.byte_position]
    }
    
    /// Match a string at the current position
    pub fn match_string(&mut self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        
        // Check if all characters match
        for (i, &expected) in chars.iter().enumerate() {
            if self.peek(i) != Some(expected) {
                return false;
            }
        }
        
        // Advance past the matched string
        for _ in 0..chars.len() {
            self.advance();
        }
        
        true
    }
    
    /// Try to match one of several strings, returning the index of the match
    pub fn match_any(&mut self, strings: &[&str]) -> Option<usize> {
        for (i, s) in strings.iter().enumerate() {
            self.save_position();
            if self.match_string(s) {
                self.discard_saved_position().ok()?;
                return Some(i);
            }
            self.restore_position().ok()?;
        }
        None
    }
    
    /// Get the line and column for a byte position
    pub fn line_column_for_byte_pos(&self, byte_pos: usize) -> (u32, u32) {
        // Binary search for the line
        let line_idx = self.line_starts
            .binary_search(&byte_pos)
            .unwrap_or_else(|i| i.saturating_sub(1));
        
        let line = line_idx as u32 + 1;
        let line_start = self.line_starts[line_idx];
        
        // Calculate column by counting characters from line start
        let column = self.input[line_start..byte_pos.min(self.input.len())]
            .chars()
            .count() as u32 + 1;
        
        (line, column)
    }
}

/// Extension trait for character classification
pub trait CharExt {
    fn is_identifier_start(&self) -> bool;
    fn is_identifier_continue(&self) -> bool;
    fn is_seen_whitespace(&self) -> bool;
}

impl CharExt for char {
    fn is_identifier_start(&self) -> bool {
        self.is_alphabetic() || *self == '_'
    }
    
    fn is_identifier_continue(&self) -> bool {
        self.is_alphanumeric() || *self == '_'
    }
    
    fn is_seen_whitespace(&self) -> bool {
        matches!(*self, ' ' | '\t' | '\r' | '\n')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_operations() {
        let mut stream = CharStream::new("hello world");
        
        assert_eq!(stream.current(), Some('h'));
        assert_eq!(stream.advance(), Some('h'));
        assert_eq!(stream.current(), Some('e'));
        
        let word = stream.advance_while(|ch| ch.is_alphabetic());
        assert_eq!(word, "ello");
        
        assert_eq!(stream.current(), Some(' '));
        stream.skip_while(|ch| ch.is_whitespace());
        assert_eq!(stream.current(), Some('w'));
    }
    
    #[test]
    fn test_lookahead() {
        let mut stream = CharStream::new("abc123");
        
        assert_eq!(stream.peek(0), Some('a'));
        assert_eq!(stream.peek(1), Some('b'));
        assert_eq!(stream.peek(2), Some('c'));
        assert_eq!(stream.peek(5), Some('3'));
        assert_eq!(stream.peek(6), None);
        
        // Verify position hasn't changed
        assert_eq!(stream.current(), Some('a'));
    }
    
    #[test]
    fn test_backtracking() {
        let mut stream = CharStream::new("hello world");
        
        stream.advance(); // 'h'
        stream.advance(); // 'e'
        
        stream.save_position();
        
        stream.advance(); // 'l'
        stream.advance(); // 'l'
        stream.advance(); // 'o'
        
        assert_eq!(stream.current(), Some(' '));
        
        stream.restore_position().unwrap();
        assert_eq!(stream.current(), Some('l'));
    }
    
    #[test]
    fn test_string_matching() {
        let mut stream = CharStream::new("func main() { }");
        
        assert!(stream.match_string("func"));
        assert_eq!(stream.current(), Some(' '));
        
        stream.advance(); // skip space
        assert!(!stream.match_string("function"));
        assert!(stream.match_string("main"));
    }
    
    #[test]
    fn test_unicode_support() {
        let mut stream = CharStream::new("مرحبا العالم");
        
        assert_eq!(stream.current(), Some('م'));
        stream.advance();
        assert_eq!(stream.current(), Some('ر'));
        
        let word = stream.advance_while(|ch| !ch.is_whitespace());
        assert_eq!(word, "رحبا");
    }
    
    #[test]
    fn test_line_column_tracking() {
        let mut stream = CharStream::new("line1\nline2\nline3");
        
        assert_eq!(stream.position().line, 1);
        assert_eq!(stream.position().column, 1);
        
        stream.advance_while(|ch| ch != '\n');
        stream.advance(); // consume '\n'
        
        assert_eq!(stream.position().line, 2);
        assert_eq!(stream.position().column, 1);
    }
}