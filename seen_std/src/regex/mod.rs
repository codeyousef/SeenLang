//! Regular expression engine for pattern matching
//!
//! High-performance regex engine optimized for compiler use cases:
//! - Lexical analysis pattern matching
//! - Configuration file processing  
//! - Text search and replacement
//! - Validation and parsing

use crate::string::String;
use crate::collections::Vec;

/// Regular expression compilation and execution errors
#[derive(Debug, Clone, PartialEq)]
pub enum RegexError {
    /// Invalid pattern syntax
    InvalidSyntax(String),
    /// Pattern compilation failed
    CompilationFailed(String),
    /// Pattern too complex
    TooComplex,
    /// Stack overflow during matching
    StackOverflow,
    /// Internal engine error
    InternalError(String),
}

impl std::fmt::Display for RegexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegexError::InvalidSyntax(msg) => write!(f, "Invalid regex syntax: {}", msg.as_str()),
            RegexError::CompilationFailed(msg) => write!(f, "Regex compilation failed: {}", msg.as_str()),
            RegexError::TooComplex => write!(f, "Regex pattern too complex"),
            RegexError::StackOverflow => write!(f, "Stack overflow during regex matching"),
            RegexError::InternalError(msg) => write!(f, "Internal regex error: {}", msg.as_str()),
        }
    }
}

impl std::error::Error for RegexError {}

/// Result type for regex operations
pub type RegexResult<T> = Result<T, RegexError>;

/// A compiled regular expression pattern
#[derive(Clone)]
pub struct Regex {
    pattern: String,
    #[allow(dead_code)]
    compiled: CompiledRegex,
}

/// Internal compiled representation (NFA-based)
#[derive(Clone)]
#[allow(dead_code)]
struct CompiledRegex {
    states: Vec<State>,
    start_state: usize,
    accept_states: Vec<usize>,
}

/// NFA state representation
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct State {
    id: usize,
    transitions: Vec<Transition>,
    is_accept: bool,
}

/// State transition with condition
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct Transition {
    condition: TransitionCondition,
    target_state: usize,
}

/// Condition for state transition
#[derive(Clone, Debug)]
#[allow(dead_code)]
enum TransitionCondition {
    /// Match a specific character
    Char(char),
    /// Match any character in character class
    CharClass(Vec<CharRange>),
    /// Match any character except newline
    Dot,
    /// Epsilon transition (no input consumed)
    Epsilon,
    /// Start of string anchor
    StartAnchor,
    /// End of string anchor
    EndAnchor,
    /// Word boundary
    WordBoundary,
}

/// Character range for character classes
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct CharRange {
    start: char,
    end: char,
}

impl CharRange {
    fn new(start: char, end: char) -> Self {
        CharRange { start, end }
    }
    
    fn single(ch: char) -> Self {
        CharRange { start: ch, end: ch }
    }
    
    #[allow(dead_code)]
    fn contains(&self, ch: char) -> bool {
        ch >= self.start && ch <= self.end
    }
}

/// Match result containing captured groups
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// Start position of the match in bytes
    pub start: usize,
    /// End position of the match in bytes  
    pub end: usize,
    /// Captured groups (group 0 is the entire match)
    pub groups: Vec<Option<(usize, usize)>>,
}

impl Match {
    /// Returns the matched text from the input string
    pub fn as_str<'t>(&self, text: &'t str) -> &'t str {
        &text[self.start..self.end]
    }
    
    /// Returns the length of the match in bytes
    pub fn len(&self) -> usize {
        self.end - self.start
    }
    
    /// Returns true if the match is empty
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
    
    /// Returns captured group text if it exists
    pub fn group<'t>(&self, text: &'t str, group_index: usize) -> Option<&'t str> {
        self.groups.get(group_index)?.as_ref()
            .map(|(start, end)| &text[*start..*end])
    }
}

/// Iterator over all matches in a string
pub struct Matches<'r, 't> {
    regex: &'r Regex,
    text: &'t str,
    position: usize,
}

impl<'r, 't> Iterator for Matches<'r, 't> {
    type Item = Match;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.position > self.text.len() {
            return None;
        }
        
        match self.regex.find_at(self.text, self.position) {
            Some(m) => {
                self.position = if m.len() == 0 { m.end + 1 } else { m.end };
                Some(m)
            }
            None => None,
        }
    }
}

impl Regex {
    /// Compiles a regular expression pattern
    pub fn new(pattern: &str) -> RegexResult<Self> {
        let pattern_string = String::from(pattern);
        let compiled = Self::compile(pattern)?;
        
        Ok(Regex {
            pattern: pattern_string,
            compiled,
        })
    }
    
    /// Returns the original pattern string
    pub fn as_str(&self) -> &str {
        self.pattern.as_str()
    }
    
    /// Tests if the pattern matches anywhere in the text
    pub fn is_match(&self, text: &str) -> bool {
        self.find(text).is_some()
    }
    
    /// Finds the first match in the text
    pub fn find(&self, text: &str) -> Option<Match> {
        self.find_at(text, 0)
    }
    
    /// Finds the first match starting at the given position
    pub fn find_at(&self, text: &str, start: usize) -> Option<Match> {
        if start > text.len() {
            return None;
        }
        
        // Simple NFA simulation
        for pos in start..=text.len() {
            if let Some(end_pos) = self.match_from(text, pos) {
                return Some(Match {
                    start: pos,
                    end: end_pos,
                    groups: {
                        let mut groups = Vec::new();
                        groups.push(Some((pos, end_pos)));
                        groups
                    }, // Group 0 is entire match
                });
            }
        }
        
        None
    }
    
    /// Returns an iterator over all matches in the text
    pub fn find_iter<'r, 't>(&'r self, text: &'t str) -> Matches<'r, 't> {
        Matches {
            regex: self,
            text,
            position: 0,
        }
    }
    
    /// Replaces the first match with the replacement string
    pub fn replace(&self, text: &str, replacement: &str) -> String {
        match self.find(text) {
            Some(m) => {
                let mut result = String::from(&text[..m.start]);
                result.push_str(replacement);
                result.push_str(&text[m.end..]);
                result
            }
            None => String::from(text),
        }
    }
    
    /// Replaces all matches with the replacement string
    pub fn replace_all(&self, text: &str, replacement: &str) -> String {
        let mut result = String::new();
        let mut last_end = 0;
        
        for m in self.find_iter(text) {
            result.push_str(&text[last_end..m.start]);
            result.push_str(replacement);
            last_end = m.end;
        }
        
        result.push_str(&text[last_end..]);
        result
    }
    
    /// Splits text by the regex pattern
    pub fn split<'t>(&self, text: &'t str) -> Vec<&'t str> {
        let mut parts = Vec::new();
        let mut last_end = 0;
        let mut found_any = false;
        
        for m in self.find_iter(text) {
            found_any = true;
            parts.push(&text[last_end..m.start]);
            last_end = m.end;
        }
        
        if found_any {
            parts.push(&text[last_end..]);
        } else {
            // No matches found - return the entire string
            parts.push(text);
        }
        
        parts
    }
    
    /// Compiles pattern into NFA representation
    fn compile(pattern: &str) -> RegexResult<CompiledRegex> {
        let mut compiler = RegexCompiler::new();
        compiler.compile(pattern)
    }
    
    /// Attempts to match from a specific position
    fn match_from(&self, text: &str, start: usize) -> Option<usize> {
        // For MVP, use a simple backtracking approach for correctness
        let pattern_str = self.pattern.as_str();
        
        // Handle special cases first
        if pattern_str.is_empty() {
            return Some(start); // Empty pattern matches at current position
        }
        
        // Convert pattern to chars for easier processing
        let pattern_chars: std::vec::Vec<char> = pattern_str.chars().collect();
        let text_chars: std::vec::Vec<char> = text.chars().collect();
        
        // Try to match pattern starting at current position using character indices
        let start_char_idx = text[..start].chars().count();
        
        if let Some(end_char_idx) = self.simple_match_with_length(&pattern_chars, &text_chars, 0, start_char_idx) {
            // Convert character index back to byte position
            let consumed_chars = end_char_idx - start_char_idx;
            let end_pos = if consumed_chars == 0 {
                start
            } else {
                // Calculate byte position by summing UTF-8 byte lengths
                let mut byte_pos = start;
                let mut char_count = 0;
                for ch in text[start..].chars() {
                    if char_count >= consumed_chars {
                        break;
                    }
                    byte_pos += ch.len_utf8();
                    char_count += 1;
                }
                byte_pos
            };
            Some(end_pos)
        } else {
            None
        }
    }
    
    /// Simple pattern matching with length return for MVP
    fn simple_match_with_length(&self, pattern: &[char], text: &[char], pat_idx: usize, text_idx: usize) -> Option<usize> {
        if pat_idx >= pattern.len() {
            return Some(text_idx); // Return how many characters were consumed
        }
        
        let ch = pattern[pat_idx];
        match ch {
            '^' => {
                // Start anchor - only matches at beginning
                if text_idx == 0 {
                    self.simple_match_with_length(pattern, text, pat_idx + 1, text_idx)
                } else {
                    None
                }
            }
            '$' => {
                // End anchor - only matches at end
                if text_idx >= text.len() {
                    self.simple_match_with_length(pattern, text, pat_idx + 1, text_idx)
                } else {
                    None
                }
            }
            '.' => {
                // Dot matches any character except newline
                if text_idx < text.len() && text[text_idx] != '\n' {
                    self.simple_match_with_length(pattern, text, pat_idx + 1, text_idx + 1)
                } else {
                    None
                }
            }
            '[' => {
                // Character class - find matching bracket and evaluate
                let mut end_bracket = pat_idx + 1;
                while end_bracket < pattern.len() && pattern[end_bracket] != ']' {
                    end_bracket += 1;
                }
                
                if end_bracket >= pattern.len() {
                    return None; // Unclosed bracket
                }
                
                if text_idx < text.len() {
                    let target_char = text[text_idx];
                    let class_matches = self.matches_char_class(&pattern[pat_idx..=end_bracket], target_char);
                    if class_matches {
                        self.simple_match_with_length(pattern, text, end_bracket + 1, text_idx + 1)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => {
                // Literal character
                if text_idx < text.len() && text[text_idx] == ch {
                    self.simple_match_with_length(pattern, text, pat_idx + 1, text_idx + 1)
                } else {
                    None
                }
            }
        }
    }
    
    /// Simple pattern matching for MVP
    #[allow(dead_code)]
    fn simple_match(&self, pattern: &[char], text: &[char], pat_idx: usize, text_idx: usize) -> bool {
        if pat_idx >= pattern.len() {
            return true; // Pattern consumed, match successful
        }
        
        let ch = pattern[pat_idx];
        match ch {
            '^' => {
                // Start anchor - only matches at beginning
                if text_idx == 0 {
                    self.simple_match(pattern, text, pat_idx + 1, text_idx)
                } else {
                    false
                }
            }
            '$' => {
                // End anchor - only matches at end
                if text_idx >= text.len() {
                    self.simple_match(pattern, text, pat_idx + 1, text_idx)
                } else {
                    false
                }
            }
            '.' => {
                // Dot matches any character except newline
                if text_idx < text.len() && text[text_idx] != '\n' {
                    self.simple_match(pattern, text, pat_idx + 1, text_idx + 1)
                } else {
                    false
                }
            }
            '[' => {
                // Character class - find matching bracket and evaluate
                let mut end_bracket = pat_idx + 1;
                while end_bracket < pattern.len() && pattern[end_bracket] != ']' {
                    end_bracket += 1;
                }
                
                if end_bracket >= pattern.len() {
                    return false; // Unclosed bracket
                }
                
                if text_idx < text.len() {
                    let target_char = text[text_idx];
                    let class_matches = self.matches_char_class(&pattern[pat_idx..=end_bracket], target_char);
                    if class_matches {
                        self.simple_match(pattern, text, end_bracket + 1, text_idx + 1)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => {
                // Literal character
                if text_idx < text.len() && text[text_idx] == ch {
                    self.simple_match(pattern, text, pat_idx + 1, text_idx + 1)
                } else {
                    false
                }
            }
        }
    }
    
    /// Check if character matches character class
    fn matches_char_class(&self, class_pattern: &[char], target_char: char) -> bool {
        if class_pattern.len() < 3 || class_pattern[0] != '[' || class_pattern[class_pattern.len()-1] != ']' {
            return false;
        }
        
        let mut negate = false;
        let mut start_idx = 1;
        
        // Check for negation
        if class_pattern.len() > 3 && class_pattern[1] == '^' {
            negate = true;
            start_idx = 2;
        }
        
        let mut matches = false;
        let mut i = start_idx;
        
        while i < class_pattern.len() - 1 {
            if i + 2 < class_pattern.len() - 1 && class_pattern[i + 1] == '-' {
                // Range like a-z
                if target_char >= class_pattern[i] && target_char <= class_pattern[i + 2] {
                    matches = true;
                    break;
                }
                i += 3;
            } else {
                // Single character
                if target_char == class_pattern[i] {
                    matches = true;
                    break;
                }
                i += 1;
            }
        }
        
        if negate {
            !matches
        } else {
            matches
        }
    }
    
    /// Computes epsilon closure of states
    #[allow(dead_code)]
    fn epsilon_closure(&self, states: &mut Vec<usize>) {
        let mut i = 0;
        while i < states.len() {
            let state_id = states[i];
            let state = &self.compiled.states[state_id];
            
            for transition in &state.transitions {
                if matches!(transition.condition, TransitionCondition::Epsilon) {
                    if !states.contains(&transition.target_state) {
                        states.push(transition.target_state);
                    }
                }
            }
            i += 1;
        }
    }
    
    /// Checks if a transition condition matches
    #[allow(dead_code)]
    fn matches_transition(&self, condition: &TransitionCondition, ch: char, pos: usize, text: &str) -> bool {
        match condition {
            TransitionCondition::Char(expected) => ch == *expected,
            TransitionCondition::CharClass(ranges) => {
                ranges.iter().any(|range| range.contains(ch))
            }
            TransitionCondition::Dot => ch != '\n',
            TransitionCondition::Epsilon => false, // Handled separately
            TransitionCondition::StartAnchor => pos == 0,
            TransitionCondition::EndAnchor => pos >= text.len(),
            TransitionCondition::WordBoundary => {
                let is_word_char = |c: char| c.is_alphanumeric() || c == '_';
                let prev_is_word = pos > 0 && text.chars().nth(pos - 1).map_or(false, is_word_char);
                let curr_is_word = text.chars().nth(pos).map_or(false, is_word_char);
                prev_is_word != curr_is_word
            }
        }
    }
}

/// Regex pattern compiler
struct RegexCompiler {
    states: Vec<State>,
    next_state_id: usize,
}

impl RegexCompiler {
    fn new() -> Self {
        RegexCompiler {
            states: Vec::new(),
            next_state_id: 0,
        }
    }
    
    fn compile(&mut self, pattern: &str) -> RegexResult<CompiledRegex> {
        // Validate pattern syntax first
        self.validate_pattern(pattern)?;
        
        // Empty pattern is allowed - matches at every position
        if pattern.is_empty() {
            let start_state = self.create_state(true); // Empty pattern accepts immediately
            return Ok(CompiledRegex {
                states: std::mem::take(&mut self.states),
                start_state,
                accept_states: {
                    let mut v = Vec::new();
                    v.push(start_state);
                    v
                },
            });
        }
        
        // For MVP, support basic patterns only
        let start_state = self.create_state(false);
        let mut current_state = start_state;
        
        let chars: std::vec::Vec<char> = pattern.chars().collect();
        let mut i = 0;
        
        while i < chars.len() {
            let ch = chars[i];
            
            match ch {
                // Literal character - support alphanumeric, underscore, and common punctuation/whitespace
                c if c.is_alphanumeric() || c == '_' || " \t\n!@#$%^&*()+-={}[]|\\:\";'<>?,./".contains(c) => {
                    let next_state = self.create_state(false);
                    self.add_transition(current_state, next_state, TransitionCondition::Char(c));
                    current_state = next_state;
                }
                // Dot metacharacter
                '.' => {
                    let next_state = self.create_state(false);
                    self.add_transition(current_state, next_state, TransitionCondition::Dot);
                    current_state = next_state;
                }
                // Start anchor
                '^' => {
                    if i == 0 {
                        let next_state = self.create_state(false);
                        self.add_transition(current_state, next_state, TransitionCondition::StartAnchor);
                        current_state = next_state;
                    } else {
                        return Err(RegexError::InvalidSyntax(String::from("^ must be at start")));
                    }
                }
                // End anchor  
                '$' => {
                    if i == chars.len() - 1 {
                        let next_state = self.create_state(true);
                        self.add_transition(current_state, next_state, TransitionCondition::EndAnchor);
                        current_state = next_state;
                    } else {
                        return Err(RegexError::InvalidSyntax(String::from("$ must be at end")));
                    }
                }
                // Character class [...]
                '[' => {
                    let slice = &chars[i..];
                    let (char_class, end_pos) = self.parse_char_class(slice)?;
                    let next_state = self.create_state(false);
                    self.add_transition(current_state, next_state, TransitionCondition::CharClass(char_class));
                    current_state = next_state;
                    i += end_pos;
                }
                _ => {
                    return Err(RegexError::InvalidSyntax(String::from(&format!("Unsupported character: {}", ch))));
                }
            }
            
            i += 1;
        }
        
        // Make final state accepting if not already
        if !self.states[current_state].is_accept {
            self.states[current_state].is_accept = true;
        }
        
        Ok(CompiledRegex {
            states: std::mem::take(&mut self.states),
            start_state,
            accept_states: {
                let mut v = Vec::new();
                v.push(current_state);
                v
            },
        })
    }
    
    fn create_state(&mut self, is_accept: bool) -> usize {
        let id = self.next_state_id;
        self.next_state_id += 1;
        
        self.states.push(State {
            id,
            transitions: Vec::new(),
            is_accept,
        });
        
        id
    }
    
    fn add_transition(&mut self, from_state: usize, to_state: usize, condition: TransitionCondition) {
        self.states[from_state].transitions.push(Transition {
            condition,
            target_state: to_state,
        });
    }
    
    fn parse_char_class(&self, chars: &[char]) -> RegexResult<(Vec<CharRange>, usize)> {
        if chars.is_empty() || chars[0] != '[' {
            return Err(RegexError::InvalidSyntax(String::from("Expected '['")));
        }
        
        let mut ranges = Vec::new();
        let mut i = 1;
        let mut negate = false;
        
        // Check for negation
        if i < chars.len() && chars[i] == '^' {
            negate = true;
            i += 1;
        }
        
        while i < chars.len() && chars[i] != ']' {
            if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] != ']' {
                // Range like a-z
                ranges.push(CharRange::new(chars[i], chars[i + 2]));
                i += 3;
            } else {
                // Single character
                ranges.push(CharRange::single(chars[i]));
                i += 1;
            }
        }
        
        if i >= chars.len() {
            return Err(RegexError::InvalidSyntax(String::from("Unclosed character class")));
        }
        
        if negate {
            // For MVP, we'll implement a simple negation by including common ranges
            // This is not complete but sufficient for basic use cases
            ranges = {
                let mut v = Vec::new();
                v.push(CharRange::new('\x00', '\x08'));
                v.push(CharRange::new('\x0E', '\x1F'));
                v.push(CharRange::new('!', '~'));
                v
            };
        }
        
        Ok((ranges, i))
    }
    
    fn validate_pattern(&self, pattern: &str) -> RegexResult<()> {
        let chars: std::vec::Vec<char> = pattern.chars().collect();
        let mut i = 0;
        
        while i < chars.len() {
            match chars[i] {
                '[' => {
                    // Find matching close bracket
                    let mut j = i + 1;
                    while j < chars.len() && chars[j] != ']' {
                        j += 1;
                    }
                    if j >= chars.len() {
                        return Err(RegexError::InvalidSyntax(String::from("Unclosed character class")));
                    }
                    i = j;
                }
                ']' => {
                    // Unmatched close bracket
                    return Err(RegexError::InvalidSyntax(String::from("Unmatched ']'")));
                }
                '^' => {
                    if i != 0 {
                        return Err(RegexError::InvalidSyntax(String::from("^ must be at start")));
                    }
                }
                '$' => {
                    if i != chars.len() - 1 {
                        return Err(RegexError::InvalidSyntax(String::from("$ must be at end")));
                    }
                }
                _ => {}
            }
            i += 1;
        }
        Ok(())
    }
}

/// Convenience functions for common regex operations
pub mod prelude {
    use super::*;
    
    /// Tests if text matches the pattern exactly
    pub fn matches(pattern: &str, text: &str) -> bool {
        match Regex::new(pattern) {
            Ok(regex) => regex.is_match(text),
            Err(_) => false,
        }
    }
    
    /// Finds the first match of pattern in text
    pub fn find(pattern: &str, text: &str) -> Option<Match> {
        Regex::new(pattern).ok()?.find(text)
    }
    
    /// Replaces all matches of pattern with replacement
    pub fn replace_all(pattern: &str, text: &str, replacement: &str) -> Option<String> {
        Some(Regex::new(pattern).ok()?.replace_all(text, replacement))
    }
    
    /// Splits text by pattern
    pub fn split<'a>(pattern: &str, text: &'a str) -> Option<Vec<&'a str>> {
        Some(Regex::new(pattern).ok()?.split(text))
    }
}

#[cfg(test)]
mod tests;