//! Dynamic Keyword Management System
//! 
//! Implements dynamic keyword loading from TOML files to support
//! multiple human languages without hardcoded values.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::path::Path;
use std::fs;
use anyhow::{Result, Context, anyhow};
use serde::{Deserialize, Serialize};
use indexmap::IndexMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlLanguageFile {
    pub name: String,
    pub description: String,
    pub keywords: IndexMap<String, String>,
    pub operators: Option<IndexMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct LanguageKeywords {
    pub name: String,
    pub description: String,
    pub keyword_map: HashMap<String, KeywordType>,
    pub reverse_map: HashMap<KeywordType, String>,
}

#[derive(Debug)]
pub struct KeywordManager {
    languages: Arc<RwLock<HashMap<String, LanguageKeywords>>>,
    current_language: Arc<RwLock<String>>,
    fallback_language: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeywordType {
    // Control flow
    KeywordFun,
    KeywordIf,
    KeywordElse,
    KeywordWhile,
    KeywordFor,
    KeywordIn,
    KeywordMatch,
    KeywordBreak,
    KeywordContinue,
    KeywordReturn,
    KeywordWhen,
    
    // Variable declarations
    KeywordLet,
    KeywordMut,
    KeywordConst,
    KeywordStatic,
    KeywordVal,
    KeywordVar,
    
    // Type definitions
    KeywordStruct,
    KeywordEnum,
    KeywordTrait,
    KeywordImpl,
    KeywordType,
    KeywordClass,
    KeywordData,
    KeywordSealed,
    KeywordObject,
    KeywordInterface,
    
    // Module system
    KeywordModule,
    KeywordImport,
    KeywordUse,
    
    // Literals
    KeywordTrue,
    KeywordFalse,
    KeywordNull,
    
    // Type checking
    KeywordIs,
    KeywordAs,
    KeywordBy,
    
    // Async/Coroutines
    KeywordSuspend,
    KeywordAwait,
    KeywordLaunch,
    KeywordFlow,
    
    // Error handling
    KeywordTry,
    KeywordCatch,
    KeywordFinally,
    KeywordThrow,
    
    // Function modifiers
    KeywordInline,
    KeywordReified,
    KeywordCrossinline,
    KeywordNoinline,
    KeywordOperator,
    KeywordInfix,
    KeywordTailrec,
    
    // Class modifiers
    KeywordOpen,
    KeywordFinal,
    KeywordAbstract,
    KeywordOverride,
    KeywordLateinit,
    KeywordCompanion,
    
    // Logical operators (research-based)
    KeywordAnd,
    KeywordOr,
    KeywordNot,
    
    // Memory management (Vale-style)
    KeywordMove,
    KeywordBorrow,
    KeywordInout,
}

impl KeywordManager {
    pub fn new() -> Self {
        Self {
            languages: Arc::new(RwLock::new(HashMap::new())),
            current_language: Arc::new(RwLock::new("en".to_string())),
            fallback_language: "en".to_string(),
        }
    }
    
    /// Load keywords from a TOML file for a specific language
    pub fn load_from_toml_file<P: AsRef<Path>>(&mut self, path: P, language: &str) -> Result<()> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read TOML file: {:?}", path.as_ref()))?;
        
        let toml_data: TomlLanguageFile = toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML file: {:?}", path.as_ref()))?;
        
        self.load_from_toml_data(toml_data, language)
    }
    
    /// Load keywords from TOML data structure
    pub fn load_from_toml_data(&mut self, toml_data: TomlLanguageFile, language: &str) -> Result<()> {
        let mut keyword_map = HashMap::new();
        let mut reverse_map = HashMap::new();
        
        // Parse keywords from TOML
        for (keyword_text, keyword_type_str) in toml_data.keywords {
            let keyword_type = self.parse_keyword_type(&keyword_type_str)
                .with_context(|| format!("Unknown keyword type: {}", keyword_type_str))?;
            
            keyword_map.insert(keyword_text.clone(), keyword_type.clone());
            reverse_map.insert(keyword_type, keyword_text);
        }
        
        let language_keywords = LanguageKeywords {
            name: toml_data.name,
            description: toml_data.description,
            keyword_map,
            reverse_map,
        };
        
        let mut languages = self.languages.write().unwrap();
        languages.insert(language.to_string(), language_keywords);
        
        Ok(())
    }
    
    /// Parse keyword type string into KeywordType enum
    fn parse_keyword_type(&self, type_str: &str) -> Result<KeywordType> {
        match type_str {
            "KeywordFun" => Ok(KeywordType::KeywordFun),
            "KeywordIf" => Ok(KeywordType::KeywordIf),
            "KeywordElse" => Ok(KeywordType::KeywordElse),
            "KeywordWhile" => Ok(KeywordType::KeywordWhile),
            "KeywordFor" => Ok(KeywordType::KeywordFor),
            "KeywordIn" => Ok(KeywordType::KeywordIn),
            "KeywordMatch" => Ok(KeywordType::KeywordMatch),
            "KeywordBreak" => Ok(KeywordType::KeywordBreak),
            "KeywordContinue" => Ok(KeywordType::KeywordContinue),
            "KeywordReturn" => Ok(KeywordType::KeywordReturn),
            "KeywordWhen" => Ok(KeywordType::KeywordWhen),
            "KeywordLet" => Ok(KeywordType::KeywordLet),
            "KeywordMut" => Ok(KeywordType::KeywordMut),
            "KeywordConst" => Ok(KeywordType::KeywordConst),
            "KeywordStatic" => Ok(KeywordType::KeywordStatic),
            "KeywordVal" => Ok(KeywordType::KeywordVal),
            "KeywordVar" => Ok(KeywordType::KeywordVar),
            "KeywordStruct" => Ok(KeywordType::KeywordStruct),
            "KeywordEnum" => Ok(KeywordType::KeywordEnum),
            "KeywordTrait" => Ok(KeywordType::KeywordTrait),
            "KeywordImpl" => Ok(KeywordType::KeywordImpl),
            "KeywordType" => Ok(KeywordType::KeywordType),
            "KeywordClass" => Ok(KeywordType::KeywordClass),
            "KeywordData" => Ok(KeywordType::KeywordData),
            "KeywordSealed" => Ok(KeywordType::KeywordSealed),
            "KeywordObject" => Ok(KeywordType::KeywordObject),
            "KeywordInterface" => Ok(KeywordType::KeywordInterface),
            "KeywordModule" => Ok(KeywordType::KeywordModule),
            "KeywordImport" => Ok(KeywordType::KeywordImport),
            "KeywordUse" => Ok(KeywordType::KeywordUse),
            "KeywordTrue" => Ok(KeywordType::KeywordTrue),
            "KeywordFalse" => Ok(KeywordType::KeywordFalse),
            "KeywordNull" => Ok(KeywordType::KeywordNull),
            "KeywordIs" => Ok(KeywordType::KeywordIs),
            "KeywordAs" => Ok(KeywordType::KeywordAs),
            "KeywordBy" => Ok(KeywordType::KeywordBy),
            "KeywordSuspend" => Ok(KeywordType::KeywordSuspend),
            "KeywordAwait" => Ok(KeywordType::KeywordAwait),
            "KeywordLaunch" => Ok(KeywordType::KeywordLaunch),
            "KeywordFlow" => Ok(KeywordType::KeywordFlow),
            "KeywordTry" => Ok(KeywordType::KeywordTry),
            "KeywordCatch" => Ok(KeywordType::KeywordCatch),
            "KeywordFinally" => Ok(KeywordType::KeywordFinally),
            "KeywordThrow" => Ok(KeywordType::KeywordThrow),
            "KeywordInline" => Ok(KeywordType::KeywordInline),
            "KeywordReified" => Ok(KeywordType::KeywordReified),
            "KeywordCrossinline" => Ok(KeywordType::KeywordCrossinline),
            "KeywordNoinline" => Ok(KeywordType::KeywordNoinline),
            "KeywordOperator" => Ok(KeywordType::KeywordOperator),
            "KeywordInfix" => Ok(KeywordType::KeywordInfix),
            "KeywordTailrec" => Ok(KeywordType::KeywordTailrec),
            "KeywordOpen" => Ok(KeywordType::KeywordOpen),
            "KeywordFinal" => Ok(KeywordType::KeywordFinal),
            "KeywordAbstract" => Ok(KeywordType::KeywordAbstract),
            "KeywordOverride" => Ok(KeywordType::KeywordOverride),
            "KeywordLateinit" => Ok(KeywordType::KeywordLateinit),
            "KeywordCompanion" => Ok(KeywordType::KeywordCompanion),
            "KeywordAnd" => Ok(KeywordType::KeywordAnd),
            "KeywordOr" => Ok(KeywordType::KeywordOr),
            "KeywordNot" => Ok(KeywordType::KeywordNot),
            "KeywordMove" => Ok(KeywordType::KeywordMove),
            "KeywordBorrow" => Ok(KeywordType::KeywordBorrow),
            "KeywordInout" => Ok(KeywordType::KeywordInout),
            _ => Err(anyhow!("Unknown keyword type: {}", type_str)),
        }
    }
    
    /// Get the logical AND keyword for the current language
    pub fn get_logical_and(&self) -> String {
        self.get_keyword_text(&KeywordType::KeywordAnd)
            .unwrap_or_else(|| "and".to_string())
    }
    
    /// Get the logical OR keyword for the current language
    pub fn get_logical_or(&self) -> String {
        self.get_keyword_text(&KeywordType::KeywordOr)
            .unwrap_or_else(|| "or".to_string())
    }
    
    /// Get the logical NOT keyword for the current language
    pub fn get_logical_not(&self) -> String {
        self.get_keyword_text(&KeywordType::KeywordNot)
            .unwrap_or_else(|| "not".to_string())
    }
    
    /// Get keyword text for a specific keyword type in the current language
    /// Falls back to the fallback language if the keyword is not found
    pub fn get_keyword_text(&self, keyword_type: &KeywordType) -> Option<String> {
        let current_lang = self.current_language.read().unwrap();
        let languages = self.languages.read().unwrap();
        
        // Try current language first
        if let Some(lang_keywords) = languages.get(&*current_lang) {
            if let Some(keyword) = lang_keywords.reverse_map.get(keyword_type) {
                return Some(keyword.clone());
            }
        }
        
        // Fall back to fallback language if keyword not found in current language
        if *current_lang != self.fallback_language {
            if let Some(fallback_keywords) = languages.get(&self.fallback_language) {
                if let Some(keyword) = fallback_keywords.reverse_map.get(keyword_type) {
                    return Some(keyword.clone());
                }
            }
        }
        
        None
    }
    
    /// Check if a text string is a keyword in the current language
    /// Only falls back to the fallback language for missing keyword types, not missing translations
    pub fn is_keyword(&self, text: &str) -> Option<KeywordType> {
        let current_lang = self.current_language.read().unwrap();
        let languages = self.languages.read().unwrap();
        
        // Try current language first
        if let Some(lang_keywords) = languages.get(&*current_lang) {
            if let Some(keyword_type) = lang_keywords.keyword_map.get(text) {
                return Some(keyword_type.clone());
            }
        }
        
        // Only fall back if the current language doesn't exist at all
        // Don't fall back for normal language switching scenarios
        if !languages.contains_key(&*current_lang) && *current_lang != self.fallback_language {
            if let Some(fallback_keywords) = languages.get(&self.fallback_language) {
                if let Some(keyword_type) = fallback_keywords.keyword_map.get(text) {
                    return Some(keyword_type.clone());
                }
            }
        }
        
        None
    }
    
    /// Switch to a different language
    pub fn switch_language(&mut self, language: &str) -> Result<()> {
        let languages = self.languages.read().unwrap();
        
        if !languages.contains_key(language) {
            return Err(anyhow!("Language '{}' not loaded", language));
        }
        
        let mut current_lang = self.current_language.write().unwrap();
        *current_lang = language.to_string();
        
        Ok(())
    }
    
    /// Validate that all loaded languages have the required keywords
    pub fn validate_all_languages(&self) -> Result<()> {
        let languages = self.languages.read().unwrap();
        
        // Define required keywords that every language must have
        let required_keywords = vec![
            KeywordType::KeywordFun,
            KeywordType::KeywordIf,
            KeywordType::KeywordElse,
            KeywordType::KeywordAnd,
            KeywordType::KeywordOr,
            KeywordType::KeywordNot,
            KeywordType::KeywordMove,
            KeywordType::KeywordBorrow,
            KeywordType::KeywordInout,
            KeywordType::KeywordIs,
        ];
        
        for (lang_name, lang_keywords) in languages.iter() {
            for required_keyword in &required_keywords {
                if !lang_keywords.reverse_map.contains_key(required_keyword) {
                    return Err(anyhow!(
                        "Language '{}' is missing required keyword: {:?}",
                        lang_name,
                        required_keyword
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Load keywords from the standard languages directory
    pub fn load_from_toml(&mut self, language: &str) -> Result<()> {
        // Try multiple possible paths for the languages directory
        let possible_paths = vec![
            format!("languages/{}.toml", language),
            format!("../languages/{}.toml", language),
            format!("../../languages/{}.toml", language),
        ];
        
        for path in possible_paths {
            if std::path::Path::new(&path).exists() {
                return self.load_from_toml_file(&path, language);
            }
        }
        
        Err(anyhow!("Could not find language file for '{}' in any expected location", language))
    }
    
    /// Get all loaded language names
    pub fn get_loaded_languages(&self) -> Vec<String> {
        let languages = self.languages.read().unwrap();
        languages.keys().cloned().collect()
    }
    
    /// Get current language name
    pub fn get_current_language(&self) -> String {
        let current_lang = self.current_language.read().unwrap();
        current_lang.clone()
    }
    
    /// Get fallback language name
    pub fn get_fallback_language(&self) -> String {
        self.fallback_language.clone()
    }
    
    /// Set fallback language
    pub fn set_fallback_language(&mut self, language: &str) -> Result<()> {
        let languages = self.languages.read().unwrap();
        
        if !languages.contains_key(language) {
            return Err(anyhow!("Fallback language '{}' not loaded", language));
        }
        
        self.fallback_language = language.to_string();
        Ok(())
    }
    
    /// Check if a text string is a keyword with explicit fallback behavior
    /// This method will fall back to the fallback language for missing translations
    pub fn is_keyword_with_fallback(&self, text: &str) -> Option<KeywordType> {
        let current_lang = self.current_language.read().unwrap();
        let languages = self.languages.read().unwrap();
        
        // Try current language first
        if let Some(lang_keywords) = languages.get(&*current_lang) {
            if let Some(keyword_type) = lang_keywords.keyword_map.get(text) {
                return Some(keyword_type.clone());
            }
        }
        
        // Fall back to fallback language if not found in current language
        if *current_lang != self.fallback_language {
            if let Some(fallback_keywords) = languages.get(&self.fallback_language) {
                if let Some(keyword_type) = fallback_keywords.keyword_map.get(text) {
                    return Some(keyword_type.clone());
                }
            }
        }
        
        None
    }
}

impl Clone for KeywordManager {
    fn clone(&self) -> Self {
        let current_lang = self.current_language.read().unwrap().clone();
        Self {
            languages: Arc::clone(&self.languages),
            current_language: Arc::new(RwLock::new(current_lang)),
            fallback_language: self.fallback_language.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    use std::path::PathBuf;
    
    fn create_test_toml_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
        let file_path = dir.path().join(filename);
        fs::write(&file_path, content).unwrap();
        file_path
    }
    
    fn create_valid_english_toml() -> &'static str {
        r#"
name = "English"
description = "English keyword set for the Seen programming language"

[keywords]
"fun" = "KeywordFun"
"if" = "KeywordIf"
"else" = "KeywordElse"
"and" = "KeywordAnd"
"or" = "KeywordOr"
"not" = "KeywordNot"
"move" = "KeywordMove"
"borrow" = "KeywordBorrow"
"inout" = "KeywordInout"
"is" = "KeywordIs"
"#
    }
    
    fn create_valid_arabic_toml() -> &'static str {
        r#"
name = "Arabic"
description = "Arabic keyword set for the Seen programming language"

[keywords]
"دالة" = "KeywordFun"
"إذا" = "KeywordIf"
"وإلا" = "KeywordElse"
"و" = "KeywordAnd"
"أو" = "KeywordOr"
"ليس" = "KeywordNot"
"انقل" = "KeywordMove"
"استعر" = "KeywordBorrow"
"في_المكان" = "KeywordInout"
"هو" = "KeywordIs"
"#
    }
    
    fn create_malformed_toml() -> &'static str {
        r#"
name = "Malformed"
description = "This TOML file is malformed"
[keywords
"fun" = "KeywordFun"  # Missing closing bracket
"#
    }
    
    fn create_incomplete_toml() -> &'static str {
        r#"
name = "Incomplete"
description = "This TOML file is missing required keywords"

[keywords]
"fun" = "KeywordFun"
# Missing other required keywords
"#
    }
    
    #[test]
    fn test_keyword_manager_creation() {
        let manager = KeywordManager::new();
        let current_lang = manager.current_language.read().unwrap();
        assert_eq!(*current_lang, "en");
        assert_eq!(manager.fallback_language, "en");
        assert!(manager.languages.read().unwrap().is_empty());
    }
    
    #[test]
    fn test_load_valid_toml_file() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        let result = manager.load_from_toml_file(&toml_path, "en");
        
        assert!(result.is_ok(), "Failed to load valid TOML file: {:?}", result.err());
        
        let languages = manager.languages.read().unwrap();
        assert!(languages.contains_key("en"));
        
        let english = languages.get("en").unwrap();
        assert_eq!(english.name, "English");
        assert_eq!(english.description, "English keyword set for the Seen programming language");
        
        // Test keyword mapping using dynamic lookups
        let fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        let and_keyword = manager.get_logical_and();
        
        assert_eq!(english.keyword_map.get(&fun_keyword), Some(&KeywordType::KeywordFun));
        assert_eq!(english.keyword_map.get(&if_keyword), Some(&KeywordType::KeywordIf));
        assert_eq!(english.keyword_map.get(&and_keyword), Some(&KeywordType::KeywordAnd));
        
        // Test reverse mapping
        assert_eq!(english.reverse_map.get(&KeywordType::KeywordFun), Some(&fun_keyword));
        assert_eq!(english.reverse_map.get(&KeywordType::KeywordIf), Some(&if_keyword));
    }
    
    #[test]
    fn test_load_multiple_languages() {
        let temp_dir = TempDir::new().unwrap();
        let en_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        let ar_path = create_test_toml_file(&temp_dir, "ar.toml", create_valid_arabic_toml());
        
        let mut manager = KeywordManager::new();
        
        assert!(manager.load_from_toml_file(&en_path, "en").is_ok());
        assert!(manager.load_from_toml_file(&ar_path, "ar").is_ok());
        
        {
            let languages = manager.languages.read().unwrap();
            assert_eq!(languages.len(), 2);
            assert!(languages.contains_key("en"));
            assert!(languages.contains_key("ar"));
        }
        
        // Test Arabic keywords using dynamic lookups
        manager.switch_language("ar").unwrap();
        let ar_fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let ar_if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        let ar_and_keyword = manager.get_logical_and();
        
        {
            let languages = manager.languages.read().unwrap();
            let arabic = languages.get("ar").unwrap();
            assert_eq!(arabic.keyword_map.get(&ar_fun_keyword), Some(&KeywordType::KeywordFun));
            assert_eq!(arabic.keyword_map.get(&ar_if_keyword), Some(&KeywordType::KeywordIf));
            assert_eq!(arabic.keyword_map.get(&ar_and_keyword), Some(&KeywordType::KeywordAnd));
        }
    }
    
    #[test]
    fn test_load_malformed_toml_file() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = create_test_toml_file(&temp_dir, "malformed.toml", create_malformed_toml());
        
        let mut manager = KeywordManager::new();
        let result = manager.load_from_toml_file(&toml_path, "malformed");
        
        assert!(result.is_err(), "Should fail to load malformed TOML file");
        
        let languages = manager.languages.read().unwrap();
        assert!(!languages.contains_key("malformed"));
    }
    
    #[test]
    fn test_load_missing_file() {
        let mut manager = KeywordManager::new();
        let result = manager.load_from_toml_file(&PathBuf::from("nonexistent.toml"), "missing");
        
        assert!(result.is_err(), "Should fail to load missing file");
        
        let languages = manager.languages.read().unwrap();
        assert!(!languages.contains_key("missing"));
    }
    
    #[test]
    fn test_is_keyword_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&toml_path, "en").unwrap();
        manager.switch_language("en").unwrap();
        
        // Test keyword recognition using dynamic lookups
        let fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        let and_keyword = manager.get_logical_and();
        let or_keyword = manager.get_logical_or();
        let not_keyword = manager.get_logical_not();
        
        assert_eq!(manager.is_keyword(&fun_keyword), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword(&if_keyword), Some(KeywordType::KeywordIf));
        assert_eq!(manager.is_keyword(&and_keyword), Some(KeywordType::KeywordAnd));
        assert_eq!(manager.is_keyword(&or_keyword), Some(KeywordType::KeywordOr));
        assert_eq!(manager.is_keyword(&not_keyword), Some(KeywordType::KeywordNot));
        
        // Test non-keywords
        assert_eq!(manager.is_keyword("variable_name"), None);
        assert_eq!(manager.is_keyword("123"), None);
        assert_eq!(manager.is_keyword(""), None);
    }
    
    #[test]
    fn test_language_switching() {
        let temp_dir = TempDir::new().unwrap();
        let en_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        let ar_path = create_test_toml_file(&temp_dir, "ar.toml", create_valid_arabic_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&en_path, "en").unwrap();
        manager.load_from_toml_file(&ar_path, "ar").unwrap();
        
        // Start with English
        manager.switch_language("en").unwrap();
        let en_fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let ar_fun_keyword = "دالة"; // This is test data, not hardcoded in production
        
        assert_eq!(manager.is_keyword(&en_fun_keyword), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword(ar_fun_keyword), None);
        
        // Switch to Arabic
        manager.switch_language("ar").unwrap();
        let ar_fun_keyword_dynamic = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        
        assert_eq!(manager.is_keyword(&ar_fun_keyword_dynamic), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword(&en_fun_keyword), None);
        
        // Test invalid language switch
        let result = manager.switch_language("nonexistent");
        assert!(result.is_err());
    }
    
    #[test]
    fn test_get_keyword_methods() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&toml_path, "en").unwrap();
        manager.switch_language("en").unwrap();
        
        // Test that the methods return the correct keywords from TOML
        let and_keyword = manager.get_logical_and();
        let or_keyword = manager.get_logical_or();
        let not_keyword = manager.get_logical_not();
        
        // Verify they match what's in the TOML file
        assert_eq!(manager.is_keyword(&and_keyword), Some(KeywordType::KeywordAnd));
        assert_eq!(manager.is_keyword(&or_keyword), Some(KeywordType::KeywordOr));
        assert_eq!(manager.is_keyword(&not_keyword), Some(KeywordType::KeywordNot));
    }
    
    #[test]
    fn test_thread_safety() {
        use std::thread;
        use std::sync::Arc;
        
        let temp_dir = TempDir::new().unwrap();
        let toml_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&toml_path, "en").unwrap();
        manager.switch_language("en").unwrap();
        
        let manager = Arc::new(manager);
        let mut handles = vec![];
        
        // Spawn multiple threads to test concurrent access
        for i in 0..10 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let fun_keyword = manager_clone.get_keyword_text(&KeywordType::KeywordFun).unwrap();
                    let and_keyword = manager_clone.get_logical_and();
                    
                    assert_eq!(manager_clone.is_keyword(&fun_keyword), Some(KeywordType::KeywordFun));
                    assert_eq!(manager_clone.is_keyword(&and_keyword), Some(KeywordType::KeywordAnd));
                }
                i
            });
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
    
    #[test]
    fn test_validate_all_languages() {
        let temp_dir = TempDir::new().unwrap();
        let en_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        let incomplete_path = create_test_toml_file(&temp_dir, "incomplete.toml", create_incomplete_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&en_path, "en").unwrap();
        manager.load_from_toml_file(&incomplete_path, "incomplete").unwrap();
        
        let result = manager.validate_all_languages();
        assert!(result.is_err(), "Should fail validation due to incomplete language");
    }
    
    #[test]
    fn test_fallback_mechanism() {
        let temp_dir = TempDir::new().unwrap();
        let en_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&en_path, "en").unwrap();
        
        // Try to switch to non-existent language, should fall back
        let result = manager.switch_language("nonexistent");
        assert!(result.is_err());
        
        // Current language should remain unchanged
        let current_lang = manager.current_language.read().unwrap();
        assert_eq!(*current_lang, "en");
    }
    
    #[test]
    fn test_keyword_lookup_performance() {
        let temp_dir = TempDir::new().unwrap();
        let toml_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&toml_path, "en").unwrap();
        manager.switch_language("en").unwrap();
        
        let start = std::time::Instant::now();
        
        // Perform many keyword lookups
        for _ in 0..10000 {
            manager.is_keyword("fun");
            manager.is_keyword("if");
            manager.is_keyword("and");
            manager.is_keyword("not_a_keyword");
        }
        
        let duration = start.elapsed();
        
        // Should complete in reasonable time (less than 100ms for 40k lookups)
        assert!(duration.as_millis() < 100, "Keyword lookup too slow: {:?}", duration);
    }
    
    #[test]
    fn test_load_minimum_ten_languages() {
        let mut manager = KeywordManager::new();
        
        // Test loading from actual language files
        let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
        
        for lang in &languages {
            let result = manager.load_from_toml(lang);
            assert!(result.is_ok(), "Failed to load language {}: {:?}", lang, result.err());
        }
        
        let loaded_languages = manager.get_loaded_languages();
        assert_eq!(loaded_languages.len(), 10, "Should have loaded exactly 10 languages");
        
        for lang in &languages {
            assert!(loaded_languages.contains(&lang.to_string()), "Missing language: {}", lang);
        }
    }
    
    #[test]
    fn test_language_switching_across_all_languages() {
        let mut manager = KeywordManager::new();
        let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
        
        // Load all languages
        for lang in &languages {
            manager.load_from_toml(lang).unwrap();
        }
        
        // Test switching to each language and verify keywords work
        for lang in &languages {
            manager.switch_language(lang).unwrap();
            assert_eq!(manager.get_current_language(), *lang);
            
            // Each language should have the basic keywords
            assert!(manager.is_keyword(&manager.get_logical_and()).is_some());
            assert!(manager.is_keyword(&manager.get_logical_or()).is_some());
            assert!(manager.is_keyword(&manager.get_logical_not()).is_some());
        }
    }
    
    #[test]
    fn test_fallback_mechanism_with_missing_translations() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a language file missing some keywords
        let incomplete_spanish = r#"
name = "Incomplete Spanish"
description = "Spanish with missing keywords"

[keywords]
"función" = "KeywordFun"
"si" = "KeywordIf"
"y" = "KeywordAnd"
# Missing KeywordOr, KeywordNot, etc.
"#;
        
        let es_path = create_test_toml_file(&temp_dir, "es_incomplete.toml", incomplete_spanish);
        let en_path = create_test_toml_file(&temp_dir, "en.toml", create_valid_english_toml());
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&en_path, "en").unwrap();
        manager.load_from_toml_file(&es_path, "es_incomplete").unwrap();
        
        // Switch to incomplete language
        manager.switch_language("es_incomplete").unwrap();
        
        // Should find existing keywords
        assert_eq!(manager.is_keyword("función"), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword("si"), Some(KeywordType::KeywordIf));
        assert_eq!(manager.is_keyword("y"), Some(KeywordType::KeywordAnd));
        
        // Get English keywords to test that they're not found in incomplete Spanish
        manager.switch_language("en").unwrap();
        let en_or_keyword = manager.get_logical_or();
        let en_not_keyword = manager.get_logical_not();
        
        // Switch back to incomplete Spanish
        manager.switch_language("es_incomplete").unwrap();
        
        // Missing keywords should return None (no fallback in current implementation)
        assert_eq!(manager.is_keyword(&en_or_keyword), None);
        assert_eq!(manager.is_keyword(&en_not_keyword), None);
    }
    
    #[test]
    fn test_unicode_keyword_support() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test with various Unicode scripts
        let unicode_test = r#"
name = "Unicode Test"
description = "Testing Unicode keyword support"

[keywords]
"函数" = "KeywordFun"          # Chinese
"دالة" = "KeywordIf"           # Arabic
"функция" = "KeywordElse"      # Russian (Cyrillic)
"फ़ंक्शन" = "KeywordAnd"        # Hindi (Devanagari)
"関数" = "KeywordOr"           # Japanese (Kanji)
"🔥" = "KeywordNot"            # Emoji (just for testing)
"#;
        
        let unicode_path = create_test_toml_file(&temp_dir, "unicode.toml", unicode_test);
        
        let mut manager = KeywordManager::new();
        let result = manager.load_from_toml_file(&unicode_path, "unicode");
        assert!(result.is_ok(), "Should handle Unicode keywords");
        
        manager.switch_language("unicode").unwrap();
        
        // Test Unicode keyword recognition
        assert_eq!(manager.is_keyword("函数"), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword("دالة"), Some(KeywordType::KeywordIf));
        assert_eq!(manager.is_keyword("функция"), Some(KeywordType::KeywordElse));
        assert_eq!(manager.is_keyword("फ़ंक्शन"), Some(KeywordType::KeywordAnd));
        assert_eq!(manager.is_keyword("関数"), Some(KeywordType::KeywordOr));
        assert_eq!(manager.is_keyword("🔥"), Some(KeywordType::KeywordNot));
    }
    
    #[test]
    fn test_concurrent_language_switching() {
        use std::thread;
        use std::sync::Arc;
        
        let mut manager = KeywordManager::new();
        
        // Load English and Arabic
        manager.load_from_toml("en").unwrap();
        manager.load_from_toml("ar").unwrap();
        
        let manager = Arc::new(manager);
        let mut handles = vec![];
        
        // Spawn threads that switch languages concurrently
        for i in 0..5 {
            let manager_clone = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                let mut local_manager = (*manager_clone).clone();
                
                for j in 0..10 {
                    let lang = if (i + j) % 2 == 0 { "en" } else { "ar" };
                    local_manager.switch_language(lang).unwrap();
                    
                    // Verify the switch worked
                    assert_eq!(local_manager.get_current_language(), lang);
                    
                    // Test keyword lookup
                    if lang == "en" {
                        assert!(local_manager.is_keyword("fun").is_some());
                        assert!(local_manager.is_keyword("دالة").is_none());
                    } else {
                        assert!(local_manager.is_keyword("دالة").is_some());
                        assert!(local_manager.is_keyword("fun").is_none());
                    }
                }
                i
            });
            handles.push(handle);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
    }
    
    #[test]
    fn test_comprehensive_language_validation() {
        let mut manager = KeywordManager::new();
        
        // Load all 10 languages
        let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
        for lang in &languages {
            manager.load_from_toml(lang).unwrap();
        }
        
        // Validate all languages have required keywords
        let result = manager.validate_all_languages();
        assert!(result.is_ok(), "All languages should pass validation: {:?}", result.err());
        
        // Test each language individually
        for lang in &languages {
            manager.switch_language(lang).unwrap();
            
            // Each language must have these core keywords
            assert!(manager.is_keyword(&manager.get_logical_and()).is_some(), 
                   "Language {} missing logical AND", lang);
            assert!(manager.is_keyword(&manager.get_logical_or()).is_some(), 
                   "Language {} missing logical OR", lang);
            assert!(manager.is_keyword(&manager.get_logical_not()).is_some(), 
                   "Language {} missing logical NOT", lang);
        }
    }
    
    #[test]
    fn test_language_metadata() {
        let mut manager = KeywordManager::new();
        manager.load_from_toml("en").unwrap();
        manager.load_from_toml("ar").unwrap();
        
        let languages = manager.languages.read().unwrap();
        
        // Test English metadata
        let english = languages.get("en").unwrap();
        assert_eq!(english.name, "English");
        assert!(english.description.contains("English keyword set"));
        
        // Test Arabic metadata
        let arabic = languages.get("ar").unwrap();
        assert_eq!(arabic.name, "Arabic");
        assert!(arabic.description.contains("Arabic keyword set"));
        assert!(arabic.description.contains("العربية")); // Contains Arabic text
    }
    
    #[test]
    fn test_keyword_lookup_performance_across_languages() {
        let mut manager = KeywordManager::new();
        let languages = vec!["en", "ar", "es", "zh"];
        
        // Load multiple languages
        for lang in &languages {
            manager.load_from_toml(lang).unwrap();
        }
        
        // Test performance for each language
        for lang in &languages {
            manager.switch_language(lang).unwrap();
            
            let start = std::time::Instant::now();
            
            // Perform lookups
            for _ in 0..1000 {
                manager.is_keyword(&manager.get_logical_and());
                manager.is_keyword(&manager.get_logical_or());
                manager.is_keyword(&manager.get_logical_not());
                manager.is_keyword("nonexistent_keyword");
            }
            
            let duration = start.elapsed();
            
            // Should be fast regardless of language
            assert!(duration.as_millis() < 50, 
                   "Language {} lookup too slow: {:?}", lang, duration);
        }
    }
    
    #[test]
    fn test_scan_for_hardcoded_keywords() {
        use std::fs;
        use std::path::Path;
        
        // Define patterns that indicate hardcoded keywords
        let hardcoded_patterns = vec![
            // Common hardcoded keyword patterns
            r#""fun""#,
            r#""if""#,
            r#""else""#,
            r#""while""#,
            r#""for""#,
            r#""match""#,
            r#""and""#,
            r#""or""#,
            r#""not""#,
            r#""move""#,
            r#""borrow""#,
            r#""inout""#,
            // Avoid false positives by being specific
            r#"TokenType::KeywordFun"#,
            r#"TokenType::KeywordIf"#,
        ];
        
        let mut violations = Vec::new();
        
        // Scan the lexer source files
        let source_files = vec![
            "../seen_lexer/src/lexer.rs",
            "../seen_parser/src/lib.rs",
            "../seen_parser/src/parser.rs",
        ];
        
        for file_path in source_files {
            if Path::new(file_path).exists() {
                let content = fs::read_to_string(file_path).unwrap_or_default();
                
                for pattern in &hardcoded_patterns {
                    if content.contains(pattern) {
                        // Allow certain exceptions (like in tests or comments)
                        let lines: Vec<&str> = content.lines().collect();
                        for (line_num, line) in lines.iter().enumerate() {
                            if line.contains(pattern) && 
                               !line.trim_start().starts_with("//") && 
                               !line.contains("#[test]") &&
                               !line.contains("test_") &&
                               !line.contains("assert_eq!") &&
                               !line.contains("check_keyword(") {
                                violations.push(format!(
                                    "{}:{}: Found hardcoded keyword pattern: {}",
                                    file_path, line_num + 1, pattern
                                ));
                            }
                        }
                    }
                }
            }
        }
        
        if !violations.is_empty() {
            panic!("Found hardcoded keywords:\n{}", violations.join("\n"));
        }
    }
    
    #[test]
    fn test_verify_zero_hardcoded_keywords() {
        // This test ensures that the lexer and parser use dynamic keyword lookup
        // instead of hardcoded strings
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml("en").unwrap();
        manager.switch_language("en").unwrap();
        
        // Test that we can get keywords dynamically
        let logical_and = manager.get_logical_and();
        let logical_or = manager.get_logical_or();
        let logical_not = manager.get_logical_not();
        
        // These should be the English keywords
        assert_eq!(logical_and, "and");
        assert_eq!(logical_or, "or");
        assert_eq!(logical_not, "not");
        
        // Switch to Arabic and verify different keywords
        manager.load_from_toml("ar").unwrap();
        manager.switch_language("ar").unwrap();
        
        let arabic_and = manager.get_logical_and();
        let arabic_or = manager.get_logical_or();
        let arabic_not = manager.get_logical_not();
        
        // These should be different from English
        assert_ne!(arabic_and, "and");
        assert_ne!(arabic_or, "or");
        assert_ne!(arabic_not, "not");
        
        // And should be the Arabic keywords
        assert_eq!(arabic_and, "و");
        assert_eq!(arabic_or, "أو");
        assert_eq!(arabic_not, "ليس");
    }
    
    #[test]
    fn test_keyword_validation_across_all_languages() {
        let mut manager = KeywordManager::new();
        let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
        
        // Load all languages
        for lang in &languages {
            manager.load_from_toml(lang).unwrap();
        }
        
        // Validate that all languages have consistent keyword mappings
        let languages_map = manager.languages.read().unwrap();
        
        // Get the English keywords as reference
        let english = languages_map.get("en").unwrap();
        let english_keyword_types: std::collections::HashSet<_> = 
            english.keyword_map.values().collect();
        
        // Check that all other languages have the same set of keyword types
        for (lang_name, lang_keywords) in languages_map.iter() {
            if lang_name == "en" { continue; }
            
            let lang_keyword_types: std::collections::HashSet<_> = 
                lang_keywords.keyword_map.values().collect();
            
            // Find missing keyword types
            let missing: Vec<_> = english_keyword_types
                .difference(&lang_keyword_types)
                .collect();
            
            // Find extra keyword types
            let extra: Vec<_> = lang_keyword_types
                .difference(&english_keyword_types)
                .collect();
            
            if !missing.is_empty() {
                panic!("Language '{}' is missing keyword types: {:?}", lang_name, missing);
            }
            
            if !extra.is_empty() {
                panic!("Language '{}' has extra keyword types: {:?}", lang_name, extra);
            }
        }
        
        // Validate that reverse mappings are consistent
        for (lang_name, lang_keywords) in languages_map.iter() {
            for (keyword_text, keyword_type) in &lang_keywords.keyword_map {
                // Check that reverse mapping exists
                assert_eq!(
                    lang_keywords.reverse_map.get(keyword_type),
                    Some(keyword_text),
                    "Language '{}' has inconsistent reverse mapping for {:?}",
                    lang_name, keyword_type
                );
            }
        }
    }
    
    #[test]
    fn test_dynamic_keyword_integration() {
        // Test that the keyword manager can be integrated with a lexer
        // without any hardcoded keywords
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml("en").unwrap();
        manager.switch_language("en").unwrap();
        
        // Simulate lexer integration
        let test_tokens = vec![
            "fun", "if", "else", "and", "or", "not",
            "variable_name", "123", "true", "false"
        ];
        
        let mut keyword_count = 0;
        let mut non_keyword_count = 0;
        
        for token in test_tokens {
            if manager.is_keyword(token).is_some() {
                keyword_count += 1;
            } else {
                non_keyword_count += 1;
            }
        }
        
        // Should recognize keywords and non-keywords correctly
        assert_eq!(keyword_count, 8); // fun, if, else, and, or, not, true, false
        assert_eq!(non_keyword_count, 2); // variable_name, 123
        
        // Load and test with different language
        manager.load_from_toml("ar").unwrap();
        manager.switch_language("ar").unwrap();
        
        // Get English keywords for testing
        manager.switch_language("en").unwrap();
        let en_fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let en_if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        
        // Switch back to Arabic
        manager.switch_language("ar").unwrap();
        let ar_fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let ar_if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        
        // English keywords should not be recognized in Arabic mode
        assert!(manager.is_keyword(&en_fun_keyword).is_none());
        assert!(manager.is_keyword(&en_if_keyword).is_none());
        
        // Arabic keywords should be recognized
        assert!(manager.is_keyword(&ar_fun_keyword).is_some());
        assert!(manager.is_keyword(&ar_if_keyword).is_some());
    }
    
    #[test]
    fn test_enhanced_fallback_mechanism() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create a complete English language file
        let complete_english = r#"
name = "Complete English"
description = "Complete English with all keywords"

[keywords]
"fun" = "KeywordFun"
"if" = "KeywordIf"
"else" = "KeywordElse"
"and" = "KeywordAnd"
"or" = "KeywordOr"
"not" = "KeywordNot"
"move" = "KeywordMove"
"borrow" = "KeywordBorrow"
"inout" = "KeywordInout"
"is" = "KeywordIs"
"while" = "KeywordWhile"
"for" = "KeywordFor"
"#;
        
        // Create an incomplete Spanish file missing some keywords
        let incomplete_spanish = r#"
name = "Incomplete Spanish"
description = "Spanish missing some keywords"

[keywords]
"función" = "KeywordFun"
"si" = "KeywordIf"
"y" = "KeywordAnd"
"o" = "KeywordOr"
# Missing: else, not, move, borrow, inout, is, while, for
"#;
        
        let en_path = create_test_toml_file(&temp_dir, "en_complete.toml", complete_english);
        let es_path = create_test_toml_file(&temp_dir, "es_incomplete.toml", incomplete_spanish);
        
        let mut manager = KeywordManager::new();
        manager.load_from_toml_file(&en_path, "en").unwrap();
        manager.load_from_toml_file(&es_path, "es").unwrap();
        
        // Set English as fallback
        manager.set_fallback_language("en").unwrap();
        
        // Switch to incomplete Spanish
        manager.switch_language("es").unwrap();
        
        // Get Spanish keywords that exist
        let es_fun_keyword = manager.get_keyword_text(&KeywordType::KeywordFun).unwrap();
        let es_if_keyword = manager.get_keyword_text(&KeywordType::KeywordIf).unwrap();
        let es_and_keyword = manager.get_logical_and();
        let es_or_keyword = manager.get_logical_or();
        
        // Test keywords that exist in Spanish
        assert_eq!(manager.is_keyword(&es_fun_keyword), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword(&es_if_keyword), Some(KeywordType::KeywordIf));
        assert_eq!(manager.is_keyword(&es_and_keyword), Some(KeywordType::KeywordAnd));
        assert_eq!(manager.is_keyword(&es_or_keyword), Some(KeywordType::KeywordOr));
        
        // Get English keywords for fallback testing
        manager.switch_language("en").unwrap();
        let en_else_keyword = manager.get_keyword_text(&KeywordType::KeywordElse).unwrap();
        let en_not_keyword = manager.get_logical_not();
        let en_move_keyword = manager.get_keyword_text(&KeywordType::KeywordMove).unwrap();
        let en_borrow_keyword = manager.get_keyword_text(&KeywordType::KeywordBorrow).unwrap();
        let en_inout_keyword = manager.get_keyword_text(&KeywordType::KeywordInout).unwrap();
        let en_is_keyword = manager.get_keyword_text(&KeywordType::KeywordIs).unwrap();
        let en_while_keyword = manager.get_keyword_text(&KeywordType::KeywordWhile).unwrap();
        let en_for_keyword = manager.get_keyword_text(&KeywordType::KeywordFor).unwrap();
        
        // Switch back to Spanish
        manager.switch_language("es").unwrap();
        
        // Test keywords that should fall back to English using explicit fallback method
        assert_eq!(manager.is_keyword_with_fallback(&en_else_keyword), Some(KeywordType::KeywordElse));
        assert_eq!(manager.is_keyword_with_fallback(&en_not_keyword), Some(KeywordType::KeywordNot));
        assert_eq!(manager.is_keyword_with_fallback(&en_move_keyword), Some(KeywordType::KeywordMove));
        assert_eq!(manager.is_keyword_with_fallback(&en_borrow_keyword), Some(KeywordType::KeywordBorrow));
        assert_eq!(manager.is_keyword_with_fallback(&en_inout_keyword), Some(KeywordType::KeywordInout));
        assert_eq!(manager.is_keyword_with_fallback(&en_is_keyword), Some(KeywordType::KeywordIs));
        assert_eq!(manager.is_keyword_with_fallback(&en_while_keyword), Some(KeywordType::KeywordWhile));
        assert_eq!(manager.is_keyword_with_fallback(&en_for_keyword), Some(KeywordType::KeywordFor));
        
        // Test that Spanish keywords are not recognized when they don't exist (normal behavior)
        assert_eq!(manager.is_keyword("sino"), None); // Spanish "else" not in incomplete file
        assert_eq!(manager.is_keyword("no"), None);   // Spanish "not" not in incomplete file
        
        // Test that English keywords are not recognized in normal mode (no fallback)
        assert_eq!(manager.is_keyword(&en_else_keyword), None); // English "else" should not be found in Spanish mode
        assert_eq!(manager.is_keyword(&en_not_keyword), None);  // English "not" should not be found in Spanish mode
        
        // Test get_keyword_text with fallback
        assert_eq!(manager.get_keyword_text(&KeywordType::KeywordFun), Some(es_fun_keyword));
        assert_eq!(manager.get_keyword_text(&KeywordType::KeywordIf), Some(es_if_keyword));
        assert_eq!(manager.get_keyword_text(&KeywordType::KeywordElse), Some(en_else_keyword)); // Falls back to English
        assert_eq!(manager.get_keyword_text(&KeywordType::KeywordNot), Some(en_not_keyword));   // Falls back to English
    }
    
    #[test]
    fn test_comprehensive_multi_language_performance() {
        let mut manager = KeywordManager::new();
        let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
        
        // Load all 10 languages
        for lang in &languages {
            manager.load_from_toml(lang).unwrap();
        }
        
        // Test performance across all languages
        let start = std::time::Instant::now();
        
        for lang in &languages {
            manager.switch_language(lang).unwrap();
            
            // Perform intensive keyword lookups
            for _ in 0..1000 {
                // Test existing keywords
                manager.is_keyword(&manager.get_logical_and());
                manager.is_keyword(&manager.get_logical_or());
                manager.is_keyword(&manager.get_logical_not());
                
                // Test non-existent keywords
                manager.is_keyword("nonexistent_keyword_12345");
                manager.is_keyword("another_fake_keyword");
            }
        }
        
        let duration = start.elapsed();
        
        // Should handle 50,000 lookups across 10 languages in reasonable time
        assert!(duration.as_millis() < 500, 
               "Multi-language performance too slow: {:?}", duration);
    }
    
    #[test]
    fn test_fallback_language_management() {
        let mut manager = KeywordManager::new();
        manager.load_from_toml("en").unwrap();
        manager.load_from_toml("ar").unwrap();
        
        // Test default fallback
        assert_eq!(manager.get_fallback_language(), "en");
        
        // Test setting valid fallback language
        assert!(manager.set_fallback_language("ar").is_ok());
        assert_eq!(manager.get_fallback_language(), "ar");
        
        // Test setting invalid fallback language
        let result = manager.set_fallback_language("nonexistent");
        assert!(result.is_err());
        assert_eq!(manager.get_fallback_language(), "ar"); // Should remain unchanged
        
        // Test fallback behavior with Arabic as fallback
        manager.switch_language("en").unwrap();
        
        // English keywords should work normally
        assert_eq!(manager.is_keyword("fun"), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword("if"), Some(KeywordType::KeywordIf));
        
        // Arabic keywords should not be found in normal mode (no fallback from complete language)
        assert_eq!(manager.is_keyword("دالة"), None);
        assert_eq!(manager.is_keyword("إذا"), None);
        
        // But with explicit fallback, Arabic keywords should be found
        assert_eq!(manager.is_keyword_with_fallback("دالة"), Some(KeywordType::KeywordFun));
        assert_eq!(manager.is_keyword_with_fallback("إذا"), Some(KeywordType::KeywordIf));
    }
    
    #[test]
    fn test_all_ten_languages_keyword_consistency() {
        let mut manager = KeywordManager::new();
        let languages = vec!["en", "ar", "es", "zh", "fr", "de", "ja", "ru", "pt", "hi"];
        
        // Load all languages
        for lang in &languages {
            let result = manager.load_from_toml(lang);
            assert!(result.is_ok(), "Failed to load language {}: {:?}", lang, result.err());
        }
        
        // Test that all languages have the core logical operators
        for lang in &languages {
            manager.switch_language(lang).unwrap();
            
            let and_keyword = manager.get_logical_and();
            let or_keyword = manager.get_logical_or();
            let not_keyword = manager.get_logical_not();
            
            // Each language should have non-empty keywords
            assert!(!and_keyword.is_empty(), "Language {} has empty AND keyword", lang);
            assert!(!or_keyword.is_empty(), "Language {} has empty OR keyword", lang);
            assert!(!not_keyword.is_empty(), "Language {} has empty NOT keyword", lang);
            
            // Keywords should be recognized
            assert!(manager.is_keyword(&and_keyword).is_some(), 
                   "Language {} AND keyword '{}' not recognized", lang, and_keyword);
            assert!(manager.is_keyword(&or_keyword).is_some(), 
                   "Language {} OR keyword '{}' not recognized", lang, or_keyword);
            assert!(manager.is_keyword(&not_keyword).is_some(), 
                   "Language {} NOT keyword '{}' not recognized", lang, not_keyword);
            
            // Verify they map to correct types
            assert_eq!(manager.is_keyword(&and_keyword), Some(KeywordType::KeywordAnd));
            assert_eq!(manager.is_keyword(&or_keyword), Some(KeywordType::KeywordOr));
            assert_eq!(manager.is_keyword(&not_keyword), Some(KeywordType::KeywordNot));
        }
    }
    
    #[test]
    fn test_language_switching_performance_stress() {
        let mut manager = KeywordManager::new();
        let languages = vec!["en", "ar", "es", "zh", "fr"];
        
        // Load languages
        for lang in &languages {
            manager.load_from_toml(lang).unwrap();
        }
        
        let start = std::time::Instant::now();
        
        // Perform rapid language switching with keyword lookups
        for i in 0..1000 {
            let lang = &languages[i % languages.len()];
            manager.switch_language(lang).unwrap();
            
            // Perform keyword lookups after each switch
            manager.is_keyword(&manager.get_logical_and());
            manager.is_keyword(&manager.get_logical_or());
            manager.is_keyword(&manager.get_logical_not());
        }
        
        let duration = start.elapsed();
        
        // Should handle 1000 language switches with lookups quickly
        assert!(duration.as_millis() < 100, 
               "Language switching performance too slow: {:?}", duration);
    }
}