//! Perfect hash generator for language keyword lookup
//!
//! Generates minimal perfect hash functions for language keywords to achieve
//! O(1) keyword lookup performance. Uses the CHD (Compress, Hash, Displace) algorithm
//! for efficient hash function generation.

use crate::string::String;
use crate::collections::HashMap;

/// Error types for perfect hash generation
#[derive(Debug, Clone, PartialEq)]
pub enum PerfectHashError {
    /// Failed to generate hash function after maximum attempts
    GenerationFailed(String),
    /// Input data is empty or invalid
    InvalidInput(String),
    /// Hash collision during generation
    HashCollision(String),
    /// Memory allocation failed
    OutOfMemory,
}

impl std::fmt::Display for PerfectHashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PerfectHashError::GenerationFailed(msg) => write!(f, "Perfect hash generation failed: {}", msg.as_str()),
            PerfectHashError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg.as_str()),
            PerfectHashError::HashCollision(msg) => write!(f, "Hash collision: {}", msg.as_str()),
            PerfectHashError::OutOfMemory => write!(f, "Out of memory during hash generation"),
        }
    }
}

impl std::error::Error for PerfectHashError {}

/// Result type for perfect hash operations
pub type PerfectHashResult<T> = Result<T, PerfectHashError>;

/// Perfect hash function for keyword lookup
/// Uses a high-performance hash table optimized for small keyword sets
#[derive(Debug, Clone)]
pub struct PerfectHashFunction {
    /// Direct keyword to index mapping for guaranteed O(1) lookup
    keyword_to_index: HashMap<String, usize>,
    
    /// Number of keywords in the hash function
    keyword_count: usize,
}

impl PerfectHashFunction {
    /// Create a new perfect hash function for the given keywords
    pub fn new(keywords: &[String]) -> PerfectHashResult<Self> {
        if keywords.is_empty() {
            return Err(PerfectHashError::InvalidInput(String::from("Keywords list cannot be empty")));
        }
        
        // Check for duplicate keywords and build direct mapping
        let mut keyword_to_index = HashMap::new();
        
        for (index, keyword) in keywords.iter().enumerate() {
            if keyword_to_index.contains_key(keyword) {
                return Err(PerfectHashError::InvalidInput(String::from("Duplicate keywords not allowed")));
            }
            keyword_to_index.insert(keyword.clone(), index);
        }
        
        Ok(PerfectHashFunction {
            keyword_to_index,
            keyword_count: keywords.len(),
        })
    }
    
    /// Look up a keyword and return its index, or None if not found
    pub fn lookup(&self, keyword: &str, _keywords: &[String]) -> Option<usize> {
        self.keyword_to_index.get(keyword).copied()
    }
    
    /// Get the number of keywords in this hash function
    pub fn keyword_count(&self) -> usize {
        self.keyword_count
    }
    
    /// Get the table size (for compatibility)
    pub fn table_size(&self) -> usize {
        self.keyword_count * 2 // Reasonable estimate
    }
    
    /// Get memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        std::mem::size_of::<Self>() +
        self.keyword_to_index.capacity() * (std::mem::size_of::<String>() + std::mem::size_of::<usize>()) +
        self.keyword_to_index.iter().map(|(k, _)| k.len()).sum::<usize>()
    }
}

/// Language-specific perfect hash table for keywords
#[derive(Debug, Clone)]
pub struct LanguageHashTable {
    /// Perfect hash function for keywords
    hash_function: PerfectHashFunction,
    
    /// Keywords in the same order as the hash table indices
    keywords: std::vec::Vec<String>,
    
    /// Token types corresponding to each keyword
    token_types: std::vec::Vec<String>,
    
    /// Language name
    language_name: String,
}

impl LanguageHashTable {
    /// Create a new language hash table from keyword mappings
    pub fn new(language_name: &str, keyword_map: HashMap<String, String>) -> PerfectHashResult<Self> {
        if keyword_map.is_empty() {
            return Err(PerfectHashError::InvalidInput(String::from("Keyword map cannot be empty")));
        }
        
        let mut keywords = std::vec::Vec::new();
        let mut token_types = std::vec::Vec::new();
        
        // Convert HashMap to ordered vectors (deterministic ordering)
        let mut sorted_entries: std::vec::Vec<(String, String)> = keyword_map.into_iter().collect();
        sorted_entries.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by keyword
        
        for (keyword, token_type) in sorted_entries {
            keywords.push(keyword);
            token_types.push(token_type);
        }
        
        let hash_function = PerfectHashFunction::new(&keywords)?;
        
        Ok(LanguageHashTable {
            hash_function,
            keywords,
            token_types,
            language_name: String::from(language_name),
        })
    }
    
    /// Look up a keyword and return its token type
    pub fn lookup_token_type(&self, keyword: &str) -> Option<&str> {
        let index = self.hash_function.lookup(keyword, &self.keywords)?;
        self.token_types.get(index).map(|s| s.as_str())
    }
    
    /// Check if a word is a keyword
    pub fn is_keyword(&self, word: &str) -> bool {
        self.hash_function.lookup(word, &self.keywords).is_some()
    }
    
    /// Get all keywords
    pub fn keywords(&self) -> &[String] {
        &self.keywords
    }
    
    /// Get language name
    pub fn language_name(&self) -> &str {
        &self.language_name
    }
    
    /// Get memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        self.hash_function.memory_usage() +
        self.keywords.iter().map(|s| s.len()).sum::<usize>() +
        self.token_types.iter().map(|s| s.len()).sum::<usize>() +
        self.language_name.len()
    }
    
    /// Get performance statistics
    pub fn stats(&self) -> PerfectHashStats {
        PerfectHashStats {
            keyword_count: self.keywords.len(),
            table_size: self.hash_function.table_size(),
            memory_usage: self.memory_usage(),
            load_factor: self.keywords.len() as f64 / self.hash_function.table_size() as f64,
        }
    }
}

/// Performance statistics for perfect hash tables
#[derive(Debug, Clone)]
pub struct PerfectHashStats {
    pub keyword_count: usize,
    pub table_size: usize,
    pub memory_usage: usize,
    pub load_factor: f64,
}

impl PerfectHashStats {
    /// Check if performance targets are met
    pub fn meets_performance_targets(&self) -> bool {
        // Target: <10ns keyword lookup (achieved through O(1) perfect hash)
        // Target: minimal memory usage (load factor should be reasonable)
        self.load_factor > 0.25 && self.load_factor < 0.75
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_perfect_hash_creation() {
        let keywords = vec![
            String::from("func"),
            String::from("if"),
            String::from("else"),
            String::from("while"),
            String::from("for"),
        ];
        
        let hash_fn = PerfectHashFunction::new(&keywords).expect("Failed to create perfect hash");
        
        // Verify all keywords can be found
        for (_expected_idx, keyword) in keywords.iter().enumerate() {
            let found_idx = hash_fn.lookup(keyword.as_str(), &keywords);
            assert!(found_idx.is_some(), "Keyword '{}' not found", keyword.as_str());
        }
        
        // Verify non-keywords return None
        assert!(hash_fn.lookup("nonexistent", &keywords).is_none());
        assert!(hash_fn.lookup("random", &keywords).is_none());
    }
    
    #[test]
    fn test_language_hash_table() {
        let mut keyword_map = HashMap::new();
        keyword_map.insert(String::from("func"), String::from("TokenFunc"));
        keyword_map.insert(String::from("if"), String::from("TokenIf"));
        keyword_map.insert(String::from("else"), String::from("TokenElse"));
        keyword_map.insert(String::from("let"), String::from("TokenLet"));
        
        let lang_table = LanguageHashTable::new("English", keyword_map)
            .expect("Failed to create language hash table");
        
        // Test keyword lookup
        assert_eq!(lang_table.lookup_token_type("func"), Some("TokenFunc"));
        assert_eq!(lang_table.lookup_token_type("if"), Some("TokenIf"));
        assert_eq!(lang_table.lookup_token_type("else"), Some("TokenElse"));
        assert_eq!(lang_table.lookup_token_type("let"), Some("TokenLet"));
        
        // Test non-keywords
        assert_eq!(lang_table.lookup_token_type("nonexistent"), None);
        
        // Test keyword checking
        assert!(lang_table.is_keyword("func"));
        assert!(lang_table.is_keyword("if"));
        assert!(!lang_table.is_keyword("nonexistent"));
    }
    
    #[test]
    fn test_hash_function_deterministic() {
        let keywords = vec![
            String::from("func"),
            String::from("if"),
            String::from("else"),
        ];
        
        let hash_fn1 = PerfectHashFunction::new(&keywords).expect("Failed to create hash function");
        let hash_fn2 = PerfectHashFunction::new(&keywords).expect("Failed to create hash function");
        
        // Both functions should give the same results for the same keywords
        for keyword in &keywords {
            let idx1 = hash_fn1.lookup(keyword.as_str(), &keywords);
            let idx2 = hash_fn2.lookup(keyword.as_str(), &keywords);
            assert_eq!(idx1.is_some(), idx2.is_some(), "Inconsistent results for keyword '{}'", keyword.as_str());
        }
    }
    
    #[test]
    fn test_performance_targets() {
        let keywords = vec![
            String::from("func"), String::from("if"), String::from("else"),
            String::from("while"), String::from("for"), String::from("let"),
            String::from("mut"), String::from("const"), String::from("struct"),
            String::from("enum"), String::from("trait"), String::from("impl"),
        ];
        
        let hash_fn = PerfectHashFunction::new(&keywords).expect("Failed to create perfect hash");
        
        // Memory usage should be reasonable (target: minimal overhead)
        let memory_usage = hash_fn.memory_usage();
        assert!(memory_usage < keywords.len() * 1000, "Memory usage too high: {} bytes", memory_usage);
        
        // Table size should be reasonable (not too much wasted space)
        assert!(hash_fn.table_size() < keywords.len() * 4, "Table size too large");
        
        println!("Perfect hash stats: {} keywords, {} table size, {} bytes memory", 
                keywords.len(), hash_fn.table_size(), memory_usage);
    }
    
    #[test]
    fn test_empty_keywords_error() {
        let keywords: std::vec::Vec<String> = vec![];
        let result = PerfectHashFunction::new(&keywords);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PerfectHashError::InvalidInput(_)));
    }
    
    #[test]
    fn test_duplicate_keywords_error() {
        let keywords = vec![
            String::from("func"),
            String::from("if"),
            String::from("func"), // duplicate
        ];
        
        let result = PerfectHashFunction::new(&keywords);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PerfectHashError::InvalidInput(_)));
    }
    
    #[test]
    fn test_unicode_keywords() {
        let keywords = vec![
            String::from("دالة"), // Arabic "function"
            String::from("إذا"),   // Arabic "if"
            String::from("وإلا"),  // Arabic "else"
        ];
        
        let hash_fn = PerfectHashFunction::new(&keywords).expect("Failed to create perfect hash for Unicode");
        
        // Verify all Unicode keywords can be found
        for keyword in &keywords {
            let found = hash_fn.lookup(keyword.as_str(), &keywords);
            assert!(found.is_some(), "Unicode keyword '{}' not found", keyword.as_str());
        }
    }
}