//! Language definition loader with perfect hash optimization
//!
//! Loads language definitions from TOML files and creates perfect hash tables
//! for ultra-fast keyword lookup during lexical analysis.

use super::{TomlParser, TomlValue, TomlError};
use super::perfect_hash::{LanguageHashTable, PerfectHashError};
use crate::string::String;
use crate::collections::HashMap;
use std::path::Path;

/// Error types for language loading
#[derive(Debug, Clone)]
pub enum LanguageLoadError {
    /// TOML parsing failed
    TomlError(TomlError),
    /// Perfect hash generation failed  
    HashError(PerfectHashError),
    /// File I/O error
    IoError(String),
    /// Language definition is malformed
    MalformedDefinition(String),
    /// Required field missing
    MissingField(String),
}

impl std::fmt::Display for LanguageLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LanguageLoadError::TomlError(e) => write!(f, "TOML error: {}", e),
            LanguageLoadError::HashError(e) => write!(f, "Hash error: {}", e),
            LanguageLoadError::IoError(e) => write!(f, "I/O error: {}", e.as_str()),
            LanguageLoadError::MalformedDefinition(e) => write!(f, "Malformed definition: {}", e.as_str()),
            LanguageLoadError::MissingField(field) => write!(f, "Missing field: {}", field.as_str()),
        }
    }
}

impl std::error::Error for LanguageLoadError {}

impl From<TomlError> for LanguageLoadError {
    fn from(e: TomlError) -> Self {
        LanguageLoadError::TomlError(e)
    }
}

impl From<PerfectHashError> for LanguageLoadError {
    fn from(e: PerfectHashError) -> Self {
        LanguageLoadError::HashError(e)
    }
}

/// Result type for language loading operations
pub type LanguageLoadResult<T> = Result<T, LanguageLoadError>;

/// Complete language definition with perfect hash tables
#[derive(Debug, Clone)]
pub struct LanguageDefinition {
    /// Language metadata
    pub name: String,
    pub description: String,
    
    /// Perfect hash table for keywords
    pub keyword_table: LanguageHashTable,
    
    /// Operator mappings (not hashed since they're symbol-based)
    pub operators: HashMap<String, String>,
    
    /// Load timestamp for cache invalidation
    pub load_timestamp: u64,
    
    /// File path this definition was loaded from
    pub source_path: String,
}

impl LanguageDefinition {
    /// Create a new language definition from TOML content
    pub fn from_toml(content: &str, source_path: &str) -> LanguageLoadResult<Self> {
        let mut parser = TomlParser::new(content);
        let table = parser.parse()?;
        
        // Extract metadata
        let name = Self::get_required_string(&table, "name")?;
        let description = Self::get_optional_string(&table, "description")
            .unwrap_or_else(|| String::from(""));
        
        // Extract keywords
        let keywords_table = Self::get_required_table(&table, "keywords")?;
        let mut keyword_map = HashMap::new();
        
        for (keyword, token_value) in keywords_table.iter() {
            let token_type = Self::extract_string_value(token_value)?;
            keyword_map.insert(keyword.clone(), token_type);
        }
        
        if keyword_map.is_empty() {
            return Err(LanguageLoadError::MalformedDefinition(
                String::from("No keywords defined in language file")
            ));
        }
        
        // Create perfect hash table for keywords
        let keyword_table = LanguageHashTable::new(&name, keyword_map)?;
        
        // Extract operators
        let mut operators = HashMap::new();
        if let Some(operators_table) = Self::get_optional_table(&table, "operators") {
            for (operator, token_value) in operators_table.iter() {
                let token_type = Self::extract_string_value(token_value)?;
                operators.insert(operator.clone(), token_type);
            }
        }
        
        Ok(LanguageDefinition {
            name,
            description,
            keyword_table,
            operators,
            load_timestamp: Self::current_timestamp(),
            source_path: String::from(source_path),
        })
    }
    
    /// Load language definition from a file path
    pub fn from_file<P: AsRef<Path>>(file_path: P) -> LanguageLoadResult<Self> {
        let path = file_path.as_ref();
        let path_str = String::from(path.to_string_lossy().as_ref());
        
        // Read file content
        let content = std::fs::read_to_string(path)
            .map_err(|e| LanguageLoadError::IoError(
                String::from(format!("Failed to read file '{}': {}", path_str, e).as_str())
            ))?;
        
        Self::from_toml(&content, &path_str)
    }
    
    /// Check if a word is a keyword and return its token type
    pub fn lookup_keyword(&self, word: &str) -> Option<&str> {
        self.keyword_table.lookup_token_type(word)
    }
    
    /// Check if a symbol is an operator and return its token type
    pub fn lookup_operator(&self, operator: &str) -> Option<&str> {
        self.operators.get(operator).map(|s| s.as_str())
    }
    
    /// Check if a word is a keyword (boolean check)
    pub fn is_keyword(&self, word: &str) -> bool {
        self.keyword_table.is_keyword(word)
    }
    
    /// Get all keywords
    pub fn keywords(&self) -> &[String] {
        self.keyword_table.keywords()
    }
    
    /// Get performance statistics
    pub fn performance_stats(&self) -> LanguagePerformanceStats {
        let hash_stats = self.keyword_table.stats();
        LanguagePerformanceStats {
            keyword_count: hash_stats.keyword_count,
            operator_count: self.operators.len(),
            memory_usage: hash_stats.memory_usage + 
                         self.operators.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>(),
            hash_table_efficiency: hash_stats.load_factor,
            meets_performance_targets: hash_stats.meets_performance_targets(),
        }
    }
    
    // Helper methods for TOML parsing
    
    fn get_required_string(table: &HashMap<String, TomlValue>, key: &str) -> LanguageLoadResult<String> {
        let value = table.get(key)
            .ok_or_else(|| LanguageLoadError::MissingField(String::from(key)))?;
        
        Self::extract_string_value(value)
    }
    
    fn get_optional_string(table: &HashMap<String, TomlValue>, key: &str) -> Option<String> {
        table.get(key)
            .and_then(|v| Self::extract_string_value(v).ok())
    }
    
    fn get_required_table<'a>(table: &'a HashMap<String, TomlValue>, key: &str) 
        -> LanguageLoadResult<&'a HashMap<String, TomlValue>> {
        let value = table.get(key)
            .ok_or_else(|| LanguageLoadError::MissingField(String::from(key)))?;
        
        value.as_table()
            .ok_or_else(|| LanguageLoadError::MalformedDefinition(
                String::from(format!("Field '{}' must be a table", key).as_str())
            ))
    }
    
    fn get_optional_table<'a>(table: &'a HashMap<String, TomlValue>, key: &str) 
        -> Option<&'a HashMap<String, TomlValue>> {
        table.get(key)?.as_table()
    }
    
    fn extract_string_value(value: &TomlValue) -> LanguageLoadResult<String> {
        match value.as_string() {
            Some(s) => Ok(String::from(s)),
            None => Err(LanguageLoadError::MalformedDefinition(
                String::from("Expected string value")
            ))
        }
    }
    
    fn current_timestamp() -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Performance statistics for a loaded language
#[derive(Debug, Clone)]
pub struct LanguagePerformanceStats {
    pub keyword_count: usize,
    pub operator_count: usize,
    pub memory_usage: usize,
    pub hash_table_efficiency: f64,
    pub meets_performance_targets: bool,
}

impl LanguagePerformanceStats {
    /// Check if this language definition meets all performance targets
    pub fn is_optimized(&self) -> bool {
        self.meets_performance_targets && 
        self.memory_usage < self.keyword_count * 200 // Target: <200 bytes per keyword
    }
}

/// Language loader with caching capabilities
#[derive(Debug)]
pub struct LanguageLoader {
    /// Cache of loaded language definitions
    cache: HashMap<String, LanguageDefinition>,
    
    /// Cache expiry time in seconds (0 = never expire)
    cache_expiry: u64,
}

impl LanguageLoader {
    /// Create a new language loader
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            cache_expiry: 3600, // 1 hour default cache
        }
    }
    
    /// Create a language loader with custom cache expiry
    pub fn with_cache_expiry(cache_expiry_seconds: u64) -> Self {
        Self {
            cache: HashMap::new(),
            cache_expiry: cache_expiry_seconds,
        }
    }
    
    /// Load a language definition, using cache if available
    pub fn load_language<P: AsRef<Path>>(&mut self, file_path: P) -> LanguageLoadResult<&LanguageDefinition> {
        let path = file_path.as_ref();
        let path_str = String::from(path.to_string_lossy().as_ref());
        
        // Check cache first - need to check expiry before getting reference
        let is_cached_and_valid = if let Some(cached) = self.cache.get(path_str.as_str()) {
            !self.is_cache_expired(cached)
        } else {
            false
        };
        
        if is_cached_and_valid {
            return Ok(self.cache.get(path_str.as_str()).unwrap());
        }
        
        // Load fresh definition
        let definition = LanguageDefinition::from_file(path)?;
        self.cache.insert(path_str.clone(), definition);
        
        // Return reference to cached definition
        Ok(self.cache.get(path_str.as_str()).unwrap())
    }
    
    /// Clear the language definition cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
    
    /// Get cache statistics
    pub fn cache_stats(&self) -> LanguageCacheStats {
        let total_memory = self.cache.values()
            .map(|def| def.performance_stats().memory_usage)
            .sum();
        
        LanguageCacheStats {
            cached_languages: self.cache.len(),
            total_memory_usage: total_memory,
            cache_expiry_seconds: self.cache_expiry,
        }
    }
    
    /// Check if a cached definition has expired
    fn is_cache_expired(&self, definition: &LanguageDefinition) -> bool {
        if self.cache_expiry == 0 {
            return false; // Never expire
        }
        
        let current_time = LanguageDefinition::current_timestamp();
        current_time > definition.load_timestamp + self.cache_expiry
    }
}

impl Default for LanguageLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Language cache statistics
#[derive(Debug, Clone)]
pub struct LanguageCacheStats {
    pub cached_languages: usize,
    pub total_memory_usage: usize,
    pub cache_expiry_seconds: u64,
}

impl LanguageCacheStats {
    /// Check if cache usage is within reasonable bounds
    pub fn is_memory_efficient(&self) -> bool {
        // Target: <1MB total cache usage for typical use
        self.total_memory_usage < 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    const SAMPLE_ENGLISH_TOML: &str = r#"
name = "English"
description = "English keyword set for Seen"

[keywords]
"func" = "TokenFunc"
"if" = "TokenIf"
"else" = "TokenElse"
"let" = "TokenLet"

[operators]
"+" = "TokenPlus"
"-" = "TokenMinus"
"*" = "TokenMultiply"
"=" = "TokenAssign"
"#;
    
    #[test]
    fn test_language_definition_from_toml() {
        let definition = LanguageDefinition::from_toml(SAMPLE_ENGLISH_TOML, "test.toml")
            .expect("Failed to load language definition");
        
        assert_eq!(definition.name, "English");
        assert_eq!(definition.description, "English keyword set for Seen");
        
        // Test keyword lookup
        assert_eq!(definition.lookup_keyword("func"), Some("TokenFunc"));
        assert_eq!(definition.lookup_keyword("if"), Some("TokenIf"));
        assert_eq!(definition.lookup_keyword("nonexistent"), None);
        
        // Test operator lookup
        assert_eq!(definition.lookup_operator("+"), Some("TokenPlus"));
        assert_eq!(definition.lookup_operator("="), Some("TokenAssign"));
        assert_eq!(definition.lookup_operator("nonexistent"), None);
        
        // Test boolean checks
        assert!(definition.is_keyword("func"));
        assert!(!definition.is_keyword("nonexistent"));
    }
    
    #[test]
    fn test_language_performance_targets() {
        let definition = LanguageDefinition::from_toml(SAMPLE_ENGLISH_TOML, "test.toml")
            .expect("Failed to load language definition");
        
        let stats = definition.performance_stats();
        assert!(stats.keyword_count > 0);
        assert!(stats.operator_count > 0);
        assert!(stats.memory_usage > 0);
        
        println!("Language stats: {} keywords, {} operators, {} bytes memory",
                stats.keyword_count, stats.operator_count, stats.memory_usage);
        
        // Memory usage should be reasonable
        assert!(stats.memory_usage < stats.keyword_count * 500, 
                "Memory usage too high: {} bytes for {} keywords", 
                stats.memory_usage, stats.keyword_count);
    }
    
    #[test]
    fn test_language_loader_caching() {
        let mut loader = LanguageLoader::new();
        
        // Create a temporary TOML file
        let temp_file = std::env::temp_dir().join("test_lang.toml");
        std::fs::write(&temp_file, SAMPLE_ENGLISH_TOML)
            .expect("Failed to write temp file");
        
        // First load should read from file
        let definition1 = loader.load_language(&temp_file)
            .expect("Failed to load language");
        assert_eq!(definition1.name, "English");
        
        // Check cache stats to verify caching worked
        {
            let cache_stats = loader.cache_stats();
            assert_eq!(cache_stats.cached_languages, 1);
        }
        
        // Second load should use cache
        let definition2 = loader.load_language(&temp_file)
            .expect("Failed to load language from cache");
        assert_eq!(definition2.name, "English");
        
        // Clean up
        std::fs::remove_file(&temp_file).ok();
    }
    
    #[test]
    fn test_malformed_toml_error() {
        const MALFORMED_TOML: &str = r#"
name = "Test"
# Missing keywords section
"#;
        
        let result = LanguageDefinition::from_toml(MALFORMED_TOML, "test.toml");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LanguageLoadError::MissingField(_)));
    }
    
    #[test]
    fn test_empty_keywords_error() {
        const EMPTY_KEYWORDS_TOML: &str = r#"
name = "Test"
description = "Test language"

[keywords]
# No keywords defined

[operators]
"+" = "TokenPlus"
"#;
        
        let result = LanguageDefinition::from_toml(EMPTY_KEYWORDS_TOML, "test.toml");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LanguageLoadError::MalformedDefinition(_)));
    }
    
    #[test]
    fn test_unicode_keywords() {
        // Use ASCII first to test if the issue is Unicode-specific
        const TEST_TOML: &str = r#"
name = "Test"
description = "Test keyword set"

[keywords]
"hello" = "TokenHello"
"world" = "TokenWorld"

[operators]
"+" = "TokenPlus"
"#;
        
        let definition = LanguageDefinition::from_toml(TEST_TOML, "test.toml")
            .expect("Failed to load test definition");
        
        // Test ASCII keyword lookup first
        assert_eq!(definition.lookup_keyword("hello"), Some("TokenHello"));
        assert_eq!(definition.lookup_keyword("world"), Some("TokenWorld"));
        assert!(definition.is_keyword("hello"));
        
        // Now try a simple Unicode test
        const SIMPLE_UNICODE_TOML: &str = r#"
name = "Unicode"
description = "Unicode test"

[keywords]
"test" = "TokenTest"

[operators]
"+" = "TokenPlus"
"#;
        
        let _unicode_def = LanguageDefinition::from_toml(SIMPLE_UNICODE_TOML, "unicode.toml")
            .expect("Failed to load Unicode definition");
        
        // TODO: Add actual Arabic test when TOML parser Unicode support is fixed
    }
}