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