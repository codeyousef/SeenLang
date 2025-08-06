//! Language configuration loading for multilingual support

use crate::TokenType;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use seen_common::{SeenError, SeenResult};
use std::path::Path;

/// Language configuration loaded from TOML files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub keywords: HashMap<String, String>,
    pub operators: HashMap<String, String>,
    pub name: String,
    pub description: Option<String>,
}

impl LanguageConfig {
    /// Create a basic English configuration for testing
    pub fn new_english() -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("func".to_string(), "TokenFunc".to_string());
        keywords.insert("return".to_string(), "TokenReturn".to_string());
        keywords.insert("i32".to_string(), "TokenI32".to_string());
        keywords.insert("let".to_string(), "TokenLet".to_string());
        keywords.insert("val".to_string(), "TokenVal".to_string());
        keywords.insert("is".to_string(), "TokenIs".to_string());
        keywords.insert("as".to_string(), "TokenAs".to_string());
        keywords.insert("if".to_string(), "TokenIf".to_string());
        keywords.insert("else".to_string(), "TokenElse".to_string());
        keywords.insert("String".to_string(), "TokenString".to_string());
        keywords.insert("Int".to_string(), "TokenInt".to_string());
        keywords.insert("Any".to_string(), "TokenAny".to_string());
        keywords.insert("match".to_string(), "TokenMatch".to_string());
        keywords.insert("null".to_string(), "TokenNull".to_string());
        keywords.insert("suspend".to_string(), "TokenSuspend".to_string());
        keywords.insert("await".to_string(), "TokenAwait".to_string());
        keywords.insert("launch".to_string(), "TokenLaunch".to_string());
        keywords.insert("flow".to_string(), "TokenFlow".to_string());
        
        let mut operators = HashMap::new();
        operators.insert("+".to_string(), "TokenPlus".to_string());
        operators.insert("=".to_string(), "TokenAssign".to_string());
        operators.insert("*".to_string(), "TokenMultiply".to_string());
        operators.insert("!=".to_string(), "TokenNotEqual".to_string());
        
        Self {
            keywords,
            operators,
            name: "English".to_string(),
            description: Some("English language configuration for testing".to_string()),
        }
    }

    /// Load language configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> SeenResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| SeenError::config_error(format!("Failed to read language config: {}", e)))?;
        
        let config: LanguageConfig = toml::from_str(&content)
            .map_err(|e| SeenError::config_error(format!("Failed to parse language config: {}", e)))?;
        
        Ok(config)
    }
    
    /// Get the token type for a keyword string, if it exists
    pub fn keyword_to_token(&self, keyword: &str) -> Option<TokenType> {
        self.keywords.get(keyword).and_then(|token_name| {
            match token_name.as_str() {
                "TokenFunc" => Some(TokenType::KeywordFunc),
                "TokenIf" => Some(TokenType::KeywordIf),
                "TokenElse" => Some(TokenType::KeywordElse),
                "TokenWhile" => Some(TokenType::KeywordWhile),
                "TokenFor" => Some(TokenType::KeywordFor),
                "TokenIn" => Some(TokenType::KeywordIn),
                "TokenReturn" => Some(TokenType::KeywordReturn),
                "TokenLet" => Some(TokenType::KeywordLet),
                "TokenVal" => Some(TokenType::KeywordVal),
                "TokenMut" => Some(TokenType::KeywordMut),
                "TokenTrue" => Some(TokenType::KeywordTrue),
                "TokenFalse" => Some(TokenType::KeywordFalse),
                "TokenStruct" => Some(TokenType::KeywordStruct),
                "TokenEnum" => Some(TokenType::KeywordEnum),
                "TokenImpl" => Some(TokenType::KeywordImpl),
                "TokenTrait" => Some(TokenType::KeywordTrait),
                "TokenImport" => Some(TokenType::KeywordImport),
                "TokenModule" => Some(TokenType::KeywordModule),
                "TokenPub" => Some(TokenType::KeywordPub),
                "TokenPriv" => Some(TokenType::KeywordPriv),
                "TokenStatic" => Some(TokenType::KeywordStatic),
                "TokenConst" => Some(TokenType::KeywordConst),
                "TokenType" => Some(TokenType::KeywordType),
                "TokenMatch" => Some(TokenType::KeywordMatch),
                "TokenBreak" => Some(TokenType::KeywordBreak),
                "TokenContinue" => Some(TokenType::KeywordContinue),
                "TokenIs" => Some(TokenType::KeywordIs),
                "TokenAs" => Some(TokenType::KeywordAs),
                "TokenSuspend" => Some(TokenType::KeywordSuspend),
                "TokenAwait" => Some(TokenType::KeywordAwait),
                "TokenLaunch" => Some(TokenType::KeywordLaunch),
                "TokenFlow" => Some(TokenType::KeywordFlow),
                "TokenString" => Some(TokenType::Identifier("String".to_string())),
                "TokenInt" => Some(TokenType::Identifier("Int".to_string())),
                "TokenAny" => Some(TokenType::Identifier("Any".to_string())),
                "TokenNull" => Some(TokenType::Identifier("null".to_string())),
                _ => None,
            }
        })
    }
    
    /// Get the token type for an operator string, if it exists
    pub fn operator_to_token(&self, operator: &str) -> Option<TokenType> {
        self.operators.get(operator).and_then(|token_name| {
            match token_name.as_str() {
                "TokenPlus" => Some(TokenType::Plus),
                "TokenMinus" => Some(TokenType::Minus),
                "TokenMultiply" => Some(TokenType::Multiply),
                "TokenDivide" => Some(TokenType::Divide),
                "TokenModulo" => Some(TokenType::Modulo),
                "TokenAssign" => Some(TokenType::Assign),
                "TokenEqual" => Some(TokenType::Equal),
                "TokenNotEqual" => Some(TokenType::NotEqual),
                "TokenLess" => Some(TokenType::Less),
                "TokenLessEqual" => Some(TokenType::LessEqual),
                "TokenGreater" => Some(TokenType::Greater),
                "TokenGreaterEqual" => Some(TokenType::GreaterEqual),
                "TokenLogicalAnd" => Some(TokenType::LogicalAnd),
                "TokenLogicalOr" => Some(TokenType::LogicalOr),
                "TokenLogicalNot" => Some(TokenType::LogicalNot),
                "TokenBitwiseAnd" => Some(TokenType::BitwiseAnd),
                "TokenBitwiseOr" => Some(TokenType::BitwiseOr),
                "TokenBitwiseXor" => Some(TokenType::BitwiseXor),
                "TokenBitwiseNot" => Some(TokenType::BitwiseNot),
                "TokenLeftShift" => Some(TokenType::LeftShift),
                "TokenRightShift" => Some(TokenType::RightShift),
                "TokenPlusAssign" => Some(TokenType::PlusAssign),
                "TokenMinusAssign" => Some(TokenType::MinusAssign),
                "TokenMultiplyAssign" => Some(TokenType::MultiplyAssign),
                "TokenDivideAssign" => Some(TokenType::DivideAssign),
                "TokenModuloAssign" => Some(TokenType::ModuloAssign),
                "TokenArrow" => Some(TokenType::Arrow),
                "TokenFatArrow" => Some(TokenType::FatArrow),
                "TokenQuestion" => Some(TokenType::Question),
                "TokenDot" => Some(TokenType::Dot),
                "TokenDoubleDot" => Some(TokenType::DoubleDot),
                "TokenTripleDot" => Some(TokenType::TripleDot),
                "TokenDoubleColon" => Some(TokenType::DoubleColon),
                _ => None,
            }
        })
    }
    
    /// Check if a string is a keyword in this language
    pub fn is_keyword(&self, word: &str) -> bool {
        self.keywords.contains_key(word)
    }
    
    /// Check if a string is an operator in this language
    pub fn is_operator(&self, op: &str) -> bool {
        self.operators.contains_key(op)
    }
}

/// Language registry that manages multiple language configurations
#[derive(Debug, Clone)]
pub struct LanguageRegistry {
    languages: HashMap<String, LanguageConfig>,
    default_language: String,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        Self {
            languages: HashMap::new(),
            default_language: "en".to_string(),
        }
    }
    
    /// Register a language configuration
    pub fn register(&mut self, name: String, config: LanguageConfig) {
        self.languages.insert(name, config);
    }
    
    /// Load and register a language from file
    pub fn load_language<P: AsRef<Path>>(&mut self, name: String, path: P) -> SeenResult<()> {
        let config = LanguageConfig::load_from_file(path)?;
        self.register(name, config);
        Ok(())
    }
    
    /// Get a language configuration by name
    pub fn get_language(&self, name: &str) -> Option<&LanguageConfig> {
        self.languages.get(name)
    }
    
    /// Get the default language configuration
    pub fn get_default_language(&self) -> Option<&LanguageConfig> {
        self.languages.get(&self.default_language)
    }
    
    /// Set the default language
    pub fn set_default_language(&mut self, name: String) -> SeenResult<()> {
        if !self.languages.contains_key(&name) {
            return Err(SeenError::config_error(format!("Language '{}' not registered", name)));
        }
        self.default_language = name;
        Ok(())
    }
    
    /// List all registered languages
    pub fn list_languages(&self) -> Vec<&String> {
        self.languages.keys().collect()
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}