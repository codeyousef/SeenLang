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
        keywords.insert("fun".to_string(), "KeywordFun".to_string());
        keywords.insert("return".to_string(), "KeywordReturn".to_string());
        keywords.insert("i32".to_string(), "TokenI32".to_string());
        keywords.insert("let".to_string(), "KeywordLet".to_string());
        keywords.insert("val".to_string(), "KeywordVal".to_string());
        keywords.insert("is".to_string(), "TokenIs".to_string());
        keywords.insert("as".to_string(), "TokenAs".to_string());
        keywords.insert("if".to_string(), "KeywordIf".to_string());
        keywords.insert("else".to_string(), "KeywordElse".to_string());
        keywords.insert("for".to_string(), "TokenFor".to_string());
        keywords.insert("in".to_string(), "TokenIn".to_string());
        keywords.insert("while".to_string(), "TokenWhile".to_string());
        keywords.insert("break".to_string(), "TokenBreak".to_string());
        keywords.insert("continue".to_string(), "TokenContinue".to_string());
        keywords.insert("try".to_string(), "TokenTry".to_string());
        keywords.insert("catch".to_string(), "TokenCatch".to_string());
        keywords.insert("finally".to_string(), "TokenFinally".to_string());
        keywords.insert("throw".to_string(), "TokenThrow".to_string());
        keywords.insert("class".to_string(), "KeywordClass".to_string());
        keywords.insert("private".to_string(), "TokenPriv".to_string());
        keywords.insert("public".to_string(), "TokenPub".to_string());
        keywords.insert("String".to_string(), "TokenString".to_string());
        keywords.insert("Int".to_string(), "TokenInt".to_string());
        keywords.insert("Any".to_string(), "TokenAny".to_string());
        keywords.insert("match".to_string(), "TokenMatch".to_string());
        keywords.insert("null".to_string(), "KeywordNull".to_string());
        keywords.insert("suspend".to_string(), "KeywordSuspend".to_string());
        keywords.insert("await".to_string(), "KeywordAwait".to_string());
        keywords.insert("launch".to_string(), "KeywordLaunch".to_string());
        keywords.insert("data".to_string(), "KeywordData".to_string());
        keywords.insert("var".to_string(), "KeywordVar".to_string());
        
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
    
    /// Create a basic Arabic configuration for testing
    pub fn new_arabic() -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("دالة".to_string(), "KeywordFun".to_string());
        keywords.insert("رجع".to_string(), "KeywordReturn".to_string());
        keywords.insert("صحيح32".to_string(), "TokenI32".to_string());
        keywords.insert("اجعل".to_string(), "KeywordLet".to_string());
        keywords.insert("ثابت".to_string(), "KeywordVal".to_string());
        keywords.insert("هو".to_string(), "TokenIs".to_string());
        keywords.insert("ك".to_string(), "TokenAs".to_string());
        keywords.insert("إذا".to_string(), "KeywordIf".to_string());
        keywords.insert("وإلا".to_string(), "KeywordElse".to_string());
        keywords.insert("لكل".to_string(), "TokenFor".to_string());
        keywords.insert("في".to_string(), "TokenIn".to_string());
        keywords.insert("بينما".to_string(), "TokenWhile".to_string());
        keywords.insert("اكسر".to_string(), "TokenBreak".to_string());
        keywords.insert("استمر".to_string(), "TokenContinue".to_string());
        keywords.insert("جرب".to_string(), "TokenTry".to_string());
        keywords.insert("امسك".to_string(), "TokenCatch".to_string());
        keywords.insert("أخيرا".to_string(), "TokenFinally".to_string());
        keywords.insert("ارم".to_string(), "TokenThrow".to_string());
        keywords.insert("فئة".to_string(), "KeywordClass".to_string());
        keywords.insert("خاص".to_string(), "TokenPriv".to_string());
        keywords.insert("عام".to_string(), "TokenPub".to_string());
        keywords.insert("نص".to_string(), "TokenString".to_string());
        keywords.insert("عدد".to_string(), "TokenInt".to_string());
        keywords.insert("أي".to_string(), "TokenAny".to_string());
        keywords.insert("طابق".to_string(), "TokenMatch".to_string());
        keywords.insert("عدم".to_string(), "KeywordNull".to_string());
        keywords.insert("علق".to_string(), "KeywordSuspend".to_string());
        keywords.insert("انتظر".to_string(), "KeywordAwait".to_string());
        keywords.insert("شغل".to_string(), "KeywordLaunch".to_string());
        keywords.insert("بيانات".to_string(), "KeywordData".to_string());
        keywords.insert("متغير".to_string(), "KeywordVar".to_string());
        
        let mut operators = HashMap::new();
        operators.insert("+".to_string(), "TokenPlus".to_string());
        operators.insert("-".to_string(), "TokenMinus".to_string());
        operators.insert("*".to_string(), "TokenMultiply".to_string());
        operators.insert("/".to_string(), "TokenDivide".to_string());
        operators.insert("=".to_string(), "TokenAssign".to_string());
        operators.insert("==".to_string(), "TokenEqual".to_string());
        operators.insert("!=".to_string(), "TokenNotEqual".to_string());
        operators.insert("<".to_string(), "TokenLess".to_string());
        operators.insert(">".to_string(), "TokenGreater".to_string());
        operators.insert("<=".to_string(), "TokenLessOrEqual".to_string());
        operators.insert(">=".to_string(), "TokenGreaterOrEqual".to_string());
        
        Self {
            keywords,
            operators,
            name: "Arabic".to_string(),
            description: Some("Arabic language configuration for testing".to_string()),
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
                "KeywordImport" | "TokenImport" => Some(TokenType::KeywordImport),
                "KeywordModule" | "TokenModule" => Some(TokenType::KeywordModule),
                "KeywordPub" | "TokenPub" => Some(TokenType::KeywordPub),
                "KeywordPriv" | "TokenPriv" => Some(TokenType::KeywordPriv),
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
                "LogicalAnd" | "TokenLogicalAnd" => Some(TokenType::LogicalAnd),
                "LogicalOr" | "TokenLogicalOr" => Some(TokenType::LogicalOr),
                "LogicalNot" | "TokenLogicalNot" => Some(TokenType::LogicalNot),
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