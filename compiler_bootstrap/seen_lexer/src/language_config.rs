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
    /// Load English configuration from the en.toml file
    pub fn new_english() -> Self {
        Self::load_from_file("languages/en.toml")
            .unwrap_or_else(|_| Self::minimal_english())
    }
    
    /// Minimal English configuration for when TOML is not available
    fn minimal_english() -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("fun".to_string(), "KeywordFun".to_string());
        keywords.insert("return".to_string(), "KeywordReturn".to_string());
        keywords.insert("let".to_string(), "KeywordLet".to_string());
        keywords.insert("val".to_string(), "KeywordVal".to_string());
        keywords.insert("var".to_string(), "KeywordVar".to_string());
        keywords.insert("if".to_string(), "KeywordIf".to_string());
        keywords.insert("suspend".to_string(), "KeywordSuspend".to_string());
        keywords.insert("await".to_string(), "KeywordAwait".to_string());
        keywords.insert("launch".to_string(), "KeywordLaunch".to_string());
        keywords.insert("data".to_string(), "KeywordData".to_string());
        keywords.insert("class".to_string(), "KeywordClass".to_string());
        keywords.insert("match".to_string(), "KeywordMatch".to_string());
        keywords.insert("when".to_string(), "KeywordWhen".to_string());
        keywords.insert("is".to_string(), "KeywordIs".to_string());
        keywords.insert("null".to_string(), "KeywordNull".to_string());
        keywords.insert("else".to_string(), "KeywordElse".to_string());
        keywords.insert("use".to_string(), "KeywordUse".to_string());
        keywords.insert("move".to_string(), "KeywordMove".to_string());
        keywords.insert("borrow".to_string(), "KeywordBorrow".to_string());
        keywords.insert("inout".to_string(), "KeywordInout".to_string());
        
        let mut operators = HashMap::new();
        operators.insert("+".to_string(), "Plus".to_string());
        operators.insert("=".to_string(), "Assign".to_string());
        
        Self {
            keywords,
            operators,
            name: "English".to_string(),
            description: Some("Minimal English configuration".to_string()),
        }
    }
    
    /// Load Arabic configuration from the ar.toml file
    pub fn new_arabic() -> Self {
        Self::load_from_file("languages/ar.toml")
            .unwrap_or_else(|_| Self::minimal_arabic())
    }
    
    /// Minimal Arabic configuration for when TOML is not available
    fn minimal_arabic() -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("دالة".to_string(), "KeywordFun".to_string());
        keywords.insert("رجع".to_string(), "KeywordReturn".to_string());
        
        let mut operators = HashMap::new();
        operators.insert("+".to_string(), "Plus".to_string());
        operators.insert("=".to_string(), "Assign".to_string());
        
        Self {
            keywords,
            operators,
            name: "Arabic".to_string(),
            description: Some("Minimal Arabic configuration".to_string()),
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
                "KeywordFun" | "TokenFun" => Some(TokenType::KeywordFun),
                "KeywordIf" | "TokenIf" => Some(TokenType::KeywordIf),
                "KeywordElse" | "TokenElse" => Some(TokenType::KeywordElse),
                "KeywordWhile" | "TokenWhile" => Some(TokenType::KeywordWhile),
                "KeywordFor" | "TokenFor" => Some(TokenType::KeywordFor),
                "KeywordIn" | "TokenIn" => Some(TokenType::KeywordIn),
                "KeywordReturn" | "TokenReturn" => Some(TokenType::KeywordReturn),
                "KeywordLet" | "TokenLet" => Some(TokenType::KeywordLet),
                "KeywordVal" | "TokenVal" => Some(TokenType::KeywordVal),
                "KeywordVar" | "TokenVar" => Some(TokenType::KeywordVar),
                "KeywordMut" | "TokenMut" => Some(TokenType::KeywordMut),
                "KeywordTrue" | "TokenTrue" => Some(TokenType::KeywordTrue),
                "KeywordFalse" | "TokenFalse" => Some(TokenType::KeywordFalse),
                "KeywordStruct" | "TokenStruct" => Some(TokenType::KeywordStruct),
                "KeywordEnum" | "TokenEnum" => Some(TokenType::KeywordEnum),
                "KeywordImpl" | "TokenImpl" => Some(TokenType::KeywordImpl),
                "KeywordTrait" | "TokenTrait" => Some(TokenType::KeywordTrait),
                "KeywordUse" | "TokenUse" => Some(TokenType::KeywordUse),
                "KeywordImport" | "TokenImport" => Some(TokenType::KeywordImport),
                "KeywordModule" | "TokenModule" => Some(TokenType::KeywordModule),
                "KeywordStatic" | "TokenStatic" => Some(TokenType::KeywordStatic),
                "KeywordConst" | "TokenConst" => Some(TokenType::KeywordConst),
                "KeywordType" | "TokenType" => Some(TokenType::KeywordType),
                "KeywordMatch" | "TokenMatch" => Some(TokenType::KeywordMatch),
                "KeywordBreak" | "TokenBreak" => Some(TokenType::KeywordBreak),
                "KeywordContinue" | "TokenContinue" => Some(TokenType::KeywordContinue),
                "KeywordIs" | "TokenIs" => Some(TokenType::KeywordIs),
                "KeywordAs" | "TokenAs" => Some(TokenType::KeywordAs),
                "KeywordSuspend" | "TokenSuspend" => Some(TokenType::KeywordSuspend),
                "KeywordAwait" | "TokenAwait" => Some(TokenType::KeywordAwait),
                "KeywordLaunch" | "TokenLaunch" => Some(TokenType::KeywordLaunch),
                "KeywordFlow" | "TokenFlow" => Some(TokenType::KeywordFlow),
                "KeywordTry" | "TokenTry" => Some(TokenType::KeywordTry),
                "KeywordCatch" | "TokenCatch" => Some(TokenType::KeywordCatch),
                "KeywordFinally" | "TokenFinally" => Some(TokenType::KeywordFinally),
                "KeywordThrow" | "TokenThrow" => Some(TokenType::KeywordThrow),
                "KeywordClass" | "TokenClass" => Some(TokenType::KeywordClass),
                "KeywordInline" | "TokenInline" => Some(TokenType::KeywordInline),
                "KeywordReified" | "TokenReified" => Some(TokenType::KeywordReified),
                "KeywordCrossinline" | "TokenCrossinline" => Some(TokenType::KeywordCrossinline),
                "KeywordNoinline" | "TokenNoinline" => Some(TokenType::KeywordNoinline),
                "KeywordBy" | "TokenBy" => Some(TokenType::KeywordBy),
                "KeywordData" | "TokenData" => Some(TokenType::KeywordData),
                "KeywordSealed" | "TokenSealed" => Some(TokenType::KeywordSealed),
                "KeywordObject" | "TokenObject" => Some(TokenType::KeywordObject),
                "KeywordInterface" | "TokenInterface" => Some(TokenType::KeywordInterface),
                "KeywordOpen" | "TokenOpen" => Some(TokenType::KeywordOpen),
                "KeywordFinal" | "TokenFinal" => Some(TokenType::KeywordFinal),
                "KeywordAbstract" | "TokenAbstract" => Some(TokenType::KeywordAbstract),
                "KeywordOverride" | "TokenOverride" => Some(TokenType::KeywordOverride),
                "KeywordLateinit" | "TokenLateinit" => Some(TokenType::KeywordLateinit),
                "KeywordCompanion" | "TokenCompanion" => Some(TokenType::KeywordCompanion),
                "KeywordOperator" | "TokenOperator" => Some(TokenType::KeywordOperator),
                "KeywordInfix" | "TokenInfix" => Some(TokenType::KeywordInfix),
                "KeywordTailrec" | "TokenTailrec" => Some(TokenType::KeywordTailrec),
                "KeywordAnd" | "TokenAnd" => Some(TokenType::KeywordAnd),
                "KeywordOr" | "TokenOr" => Some(TokenType::KeywordOr),
                "KeywordNot" | "TokenNot" => Some(TokenType::KeywordNot),
                "KeywordMove" | "TokenMove" => Some(TokenType::KeywordMove),
                "KeywordBorrow" | "TokenBorrow" => Some(TokenType::KeywordBorrow),
                "KeywordInout" | "TokenInout" => Some(TokenType::KeywordInout),
                "TokenString" => Some(TokenType::Identifier("String".to_string())),
                "TokenInt" => Some(TokenType::Identifier("Int".to_string())),
                "TokenAny" => Some(TokenType::Identifier("Any".to_string())),
                "KeywordNull" | "TokenNull" => Some(TokenType::Identifier("null".to_string())),
                "KeywordWhen" | "TokenWhen" => Some(TokenType::KeywordWhen),
                _ => None,
            }
        })
    }
    
    /// Get the token type for an operator string, if it exists
    pub fn operator_to_token(&self, operator: &str) -> Option<TokenType> {
        self.operators.get(operator).and_then(|token_name| {
            match token_name.as_str() {
                "Plus" | "TokenPlus" => Some(TokenType::Plus),
                "Minus" | "TokenMinus" => Some(TokenType::Minus),
                "Multiply" | "TokenMultiply" => Some(TokenType::Multiply),
                "Divide" | "TokenDivide" => Some(TokenType::Divide),
                "Modulo" | "TokenModulo" => Some(TokenType::Modulo),
                "Assign" | "TokenAssign" => Some(TokenType::Assign),
                "Equal" | "TokenEqual" => Some(TokenType::Equal),
                "NotEqual" | "TokenNotEqual" => Some(TokenType::NotEqual),
                "Less" | "TokenLess" => Some(TokenType::Less),
                "LessEqual" | "TokenLessEqual" => Some(TokenType::LessEqual),
                "Greater" | "TokenGreater" => Some(TokenType::Greater),
                "GreaterEqual" | "TokenGreaterEqual" => Some(TokenType::GreaterEqual),
                "BitwiseAnd" | "TokenBitwiseAnd" => Some(TokenType::BitwiseAnd),
                "BitwiseOr" | "TokenBitwiseOr" => Some(TokenType::BitwiseOr),
                "BitwiseXor" | "TokenBitwiseXor" => Some(TokenType::BitwiseXor),
                "BitwiseNot" | "TokenBitwiseNot" => Some(TokenType::BitwiseNot),
                "LeftShift" | "TokenLeftShift" => Some(TokenType::LeftShift),
                "RightShift" | "TokenRightShift" => Some(TokenType::RightShift),
                "PlusAssign" | "TokenPlusAssign" => Some(TokenType::PlusAssign),
                "MinusAssign" | "TokenMinusAssign" => Some(TokenType::MinusAssign),
                "MultiplyAssign" | "TokenMultiplyAssign" => Some(TokenType::MultiplyAssign),
                "DivideAssign" | "TokenDivideAssign" => Some(TokenType::DivideAssign),
                "ModuloAssign" | "TokenModuloAssign" => Some(TokenType::ModuloAssign),
                "Arrow" | "TokenArrow" => Some(TokenType::Arrow),
                "FatArrow" | "TokenFatArrow" => Some(TokenType::FatArrow),
                "Question" | "TokenQuestion" => Some(TokenType::Question),
                "Dot" | "TokenDot" => Some(TokenType::Dot),
                "DoubleDot" | "TokenDoubleDot" => Some(TokenType::DoubleDot),
                "TripleDot" | "TokenTripleDot" => Some(TokenType::TripleDot),
                "DoubleColon" | "TokenDoubleColon" => Some(TokenType::DoubleColon),
                "QuestionDot" | "TokenQuestionDot" => Some(TokenType::QuestionDot),
                "Elvis" | "TokenElvis" => Some(TokenType::Elvis),
                "BangBang" | "TokenBangBang" => Some(TokenType::BangBang),
                "DotDot" | "TokenDotDot" => Some(TokenType::DotDot),
                "DotDotLess" | "TokenDotDotLess" => Some(TokenType::DotDotLess),
                "Underscore" | "TokenUnderscore" => Some(TokenType::Underscore),
                "LeftAngle" | "TokenLeftAngle" => Some(TokenType::LeftAngle),
                "RightAngle" | "TokenRightAngle" => Some(TokenType::RightAngle),
                "At" | "TokenAt" => Some(TokenType::At),
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
    
    /// Get the keyword string for a given token type
    pub fn token_to_keyword(&self, token_type: &TokenType) -> Option<String> {
        let token_name = match token_type {
            TokenType::KeywordFun => "KeywordFun",
            TokenType::KeywordIf => "KeywordIf",
            TokenType::KeywordElse => "KeywordElse",
            TokenType::KeywordWhile => "KeywordWhile",
            TokenType::KeywordFor => "KeywordFor",
            TokenType::KeywordIn => "KeywordIn",
            TokenType::KeywordReturn => "KeywordReturn",
            TokenType::KeywordLet => "KeywordLet",
            TokenType::KeywordMut => "KeywordMut",
            TokenType::KeywordVal => "KeywordVal",
            TokenType::KeywordVar => "KeywordVar",
            TokenType::KeywordTrue => "KeywordTrue",
            TokenType::KeywordFalse => "KeywordFalse",
            TokenType::KeywordStruct => "KeywordStruct",
            TokenType::KeywordEnum => "KeywordEnum",
            TokenType::KeywordImpl => "KeywordImpl",
            TokenType::KeywordTrait => "KeywordTrait",
            TokenType::KeywordUse => "KeywordUse",
            TokenType::KeywordImport => "KeywordImport",
            TokenType::KeywordModule => "KeywordModule",
            TokenType::KeywordStatic => "KeywordStatic",
            TokenType::KeywordConst => "KeywordConst",
            TokenType::KeywordType => "KeywordType",
            TokenType::KeywordMatch => "KeywordMatch",
            TokenType::KeywordBreak => "KeywordBreak",
            TokenType::KeywordContinue => "KeywordContinue",
            TokenType::KeywordIs => "KeywordIs",
            TokenType::KeywordAs => "KeywordAs",
            TokenType::KeywordSuspend => "KeywordSuspend",
            TokenType::KeywordAwait => "KeywordAwait",
            TokenType::KeywordLaunch => "KeywordLaunch",
            TokenType::KeywordFlow => "KeywordFlow",
            TokenType::KeywordTry => "KeywordTry",
            TokenType::KeywordCatch => "KeywordCatch",
            TokenType::KeywordFinally => "KeywordFinally",
            TokenType::KeywordThrow => "KeywordThrow",
            TokenType::KeywordClass => "KeywordClass",
            TokenType::KeywordInline => "KeywordInline",
            TokenType::KeywordReified => "KeywordReified",
            TokenType::KeywordCrossinline => "KeywordCrossinline",
            TokenType::KeywordNoinline => "KeywordNoinline",
            TokenType::KeywordBy => "KeywordBy",
            TokenType::KeywordData => "KeywordData",
            TokenType::KeywordSealed => "KeywordSealed",
            TokenType::KeywordObject => "KeywordObject",
            TokenType::KeywordInterface => "KeywordInterface",
            TokenType::KeywordOpen => "KeywordOpen",
            TokenType::KeywordFinal => "KeywordFinal",
            TokenType::KeywordAbstract => "KeywordAbstract",
            TokenType::KeywordOverride => "KeywordOverride",
            TokenType::KeywordLateinit => "KeywordLateinit",
            TokenType::KeywordCompanion => "KeywordCompanion",
            TokenType::KeywordOperator => "KeywordOperator",
            TokenType::KeywordInfix => "KeywordInfix",
            TokenType::KeywordTailrec => "KeywordTailrec",
            TokenType::KeywordWhen => "KeywordWhen",
            TokenType::KeywordNull => "KeywordNull",
            _ => return None,
        };
        
        // Find the keyword that maps to this token name
        for (keyword, token) in &self.keywords {
            if token == token_name {
                return Some(keyword.clone());
            }
        }
        None
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