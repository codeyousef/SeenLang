use std::fmt;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

/// Represents a position in the source code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
}

impl Position {
    /// Create a new position
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Represents a range in the source code
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// File where the source code is located
    pub file: PathBuf,
    /// Start position (inclusive)
    pub start: Position,
    /// End position (inclusive)
    pub end: Position,
}

impl Location {
    /// Create a new location
    pub fn new(file: PathBuf, start: Position, end: Position) -> Self {
        Self { file, start, end }
    }
}

/// Represents a language context (English, Arabic, or mixed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageContext {
    /// English language context
    English,
    /// Arabic language context
    Arabic,
    /// Mixed language context
    Mixed,
}

/// Represents a severity level for diagnostic messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Error severity (compilation cannot continue)
    Error,
    /// Warning severity (compilation can continue but there may be issues)
    Warning,
    /// Informational message
    Info,
    /// Hint message
    Hint,
}

/// Represents a message in different languages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedMessage {
    /// English message
    pub en: String,
    /// Arabic message (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ar: Option<String>,
}

impl LocalizedMessage {
    /// Create a new English-only message
    pub fn english(en: impl Into<String>) -> Self {
        Self {
            en: en.into(),
            ar: None,
        }
    }
    
    /// Create a new bilingual message
    pub fn bilingual(en: impl Into<String>, ar: impl Into<String>) -> Self {
        Self {
            en: en.into(),
            ar: Some(ar.into()),
        }
    }
    
    /// Get the message in the preferred language
    pub fn get(&self, lang_pref: LanguagePreference) -> &str {
        match (lang_pref, &self.ar) {
            (LanguagePreference::Arabic, Some(ar)) => ar,
            _ => &self.en,
        }
    }
}

/// Represents a suggestion for fixing an error
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixSuggestion {
    /// Suggestion message in different languages
    pub message: LocalizedMessage,
    /// The text to replace the source_text with
    pub replacement: String,
    /// Custom range for the replacement (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement_range: Option<Location>,
}

/// Represents related information for a diagnostic
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedInformation {
    /// Related information message in different languages
    pub message: LocalizedMessage,
    /// Location of the related information
    pub location: Location,
}

/// Represents a diagnostic message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Unique identifier for the diagnostic
    pub id: String,
    /// Severity level
    pub severity: Severity,
    /// Message in different languages
    pub message: LocalizedMessage,
    /// Location in the source code
    pub location: Location,
    /// The source text that triggered the diagnostic
    pub source_text: String,
    /// What was expected instead (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_text: Option<String>,
    /// Suggestions for fixing the issue (optional)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fix_suggestions: Vec<FixSuggestion>,
    /// Related diagnostic information (optional)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub related_information: Vec<RelatedInformation>,
    /// URL to documentation explaining the error in more detail (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation_url: Option<String>,
    /// The language context in which the error occurred
    pub language_context: LanguageContext,
}

/// Represents the user's language preference
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguagePreference {
    /// English language preference
    English,
    /// Arabic language preference
    Arabic,
}

/// Formatter for diagnostics
pub struct DiagnosticFormatter {
    /// Language preference
    language_pref: LanguagePreference,
    /// Whether to use colored output
    use_colors: bool,
}

impl DiagnosticFormatter {
    /// Create a new diagnostic formatter
    pub fn new(language_pref: LanguagePreference, use_colors: bool) -> Self {
        Self {
            language_pref,
            use_colors,
        }
    }
    
    /// Format a diagnostic message
    pub fn format(&self, diagnostic: &Diagnostic) -> String {
        let mut output = String::new();
        
        // Format the header with severity and location
        let severity_str = match diagnostic.severity {
            Severity::Error => {
                if self.use_colors {
                    "\x1b[1;31merror\x1b[0m" // Bold red
                } else {
                    "error"
                }
            }
            Severity::Warning => {
                if self.use_colors {
                    "\x1b[1;33mwarning\x1b[0m" // Bold yellow
                } else {
                    "warning"
                }
            }
            Severity::Info => {
                if self.use_colors {
                    "\x1b[1;34minfo\x1b[0m" // Bold blue
                } else {
                    "info"
                }
            }
            Severity::Hint => {
                if self.use_colors {
                    "\x1b[1;32mhint\x1b[0m" // Bold green
                } else {
                    "hint"
                }
            }
        };
        
        output.push_str(&format!(
            "{}[{}]: {} at {}:{}:{}\n",
            severity_str,
            diagnostic.id,
            diagnostic.message.get(self.language_pref),
            diagnostic.location.file.display(),
            diagnostic.location.start.line,
            diagnostic.location.start.column
        ));
        
        // Add the source line
        // Note: In a real implementation, we would read the source file and display the specific line
        output.push_str(&format!("    {}\n", diagnostic.source_text));
        
        // Add the caret line
        let caret_padding = " ".repeat(diagnostic.location.start.column + 3);
        let caret_length = std::cmp::max(1, diagnostic.location.end.column - diagnostic.location.start.column + 1);
        let caret = if self.use_colors {
            format!("\x1b[31m{}\x1b[0m", "^".repeat(caret_length))
        } else {
            "^".repeat(caret_length)
        };
        output.push_str(&format!("{}{}\n", caret_padding, caret));
        
        // Add the expected text if available
        if let Some(expected) = &diagnostic.expected_text {
            let expected_msg = match self.language_pref {
                LanguagePreference::English => format!("expected: {}", expected),
                LanguagePreference::Arabic => format!("المتوقع: {}", expected),
            };
            output.push_str(&format!("    {}\n", expected_msg));
        }
        
        // Add fix suggestions if available
        if !diagnostic.fix_suggestions.is_empty() {
            let suggestion_header = match self.language_pref {
                LanguagePreference::English => "suggestion:",
                LanguagePreference::Arabic => "اقتراح:",
            };
            
            output.push_str(&format!("\n{}\n", suggestion_header));
            
            for (i, suggestion) in diagnostic.fix_suggestions.iter().enumerate() {
                output.push_str(&format!(
                    "    {}: {}\n",
                    i + 1,
                    suggestion.message.get(self.language_pref)
                ));
            }
        }
        
        // Add related information if available
        if !diagnostic.related_information.is_empty() {
            let related_header = match self.language_pref {
                LanguagePreference::English => "related:",
                LanguagePreference::Arabic => "ذو صلة:",
            };
            
            output.push_str(&format!("\n{}\n", related_header));
            
            for related in &diagnostic.related_information {
                output.push_str(&format!(
                    "    {} at {}:{}:{}\n",
                    related.message.get(self.language_pref),
                    related.location.file.display(),
                    related.location.start.line,
                    related.location.start.column
                ));
            }
        }
        
        // Add documentation URL if available
        if let Some(url) = &diagnostic.documentation_url {
            let doc_msg = match self.language_pref {
                LanguagePreference::English => format!("For more information, see: {}", url),
                LanguagePreference::Arabic => format!("لمزيد من المعلومات، راجع: {}", url),
            };
            output.push_str(&format!("\n{}\n", doc_msg));
        }
        
        output
    }
}

/// Error type for the lexer
#[derive(Debug)]
pub enum LexicalError {
    /// Unexpected character
    UnexpectedCharacter {
        /// Location of the error
        location: Location,
        /// The unexpected character
        character: char,
    },
    /// Unterminated string
    UnterminatedString {
        /// Location of the error
        location: Location,
    },
    /// Invalid number
    InvalidNumber {
        /// Location of the error
        location: Location,
        /// The invalid number
        text: String,
    },
    /// Invalid identifier
    InvalidIdentifier {
        /// Location of the error
        location: Location,
        /// The invalid identifier
        text: String,
    },
}

impl LexicalError {
    /// Convert the lexical error to a diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            LexicalError::UnexpectedCharacter { location, character } => Diagnostic {
                id: "LEX0001".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Unexpected character: '{}'", character),
                    format!("حرف غير متوقع: '{}'", character),
                ),
                location: location.clone(),
                source_text: character.to_string(),
                expected_text: None,
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            LexicalError::UnterminatedString { location } => Diagnostic {
                id: "LEX0002".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    "Unterminated string literal",
                    "سلسلة نصية غير مغلقة",
                ),
                location: location.clone(),
                source_text: "\"...".to_string(), // Placeholder, actual value determined at runtime
                expected_text: Some("\"".to_string()),
                fix_suggestions: vec![
                    FixSuggestion {
                        message: LocalizedMessage::bilingual(
                            "Add a closing double quote",
                            "أضف علامة اقتباس مزدوجة للإغلاق",
                        ),
                        replacement: "\"...\"".to_string(), // Placeholder, actual value determined at runtime
                        replacement_range: None,
                    },
                ],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            LexicalError::InvalidNumber { location, text } => Diagnostic {
                id: "LEX0003".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Invalid number: '{}'", text),
                    format!("رقم غير صالح: '{}'", text),
                ),
                location: location.clone(),
                source_text: text.clone(),
                expected_text: Some("valid number".to_string()),
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            LexicalError::InvalidIdentifier { location, text } => Diagnostic {
                id: "LEX0004".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Invalid identifier: '{}'", text),
                    format!("معرّف غير صالح: '{}'", text),
                ),
                location: location.clone(),
                source_text: text.clone(),
                expected_text: Some("valid identifier".to_string()),
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
        }
    }
}

/// Error type for the parser
#[derive(Debug)]
pub enum SyntaxError {
    /// Unexpected token
    UnexpectedToken {
        /// Location of the error
        location: Location,
        /// The unexpected token
        found: String,
        /// The expected token(s)
        expected: Vec<String>,
    },
    /// Unexpected end of file
    UnexpectedEOF {
        /// Location of the error
        location: Location,
        /// The expected token(s)
        expected: Vec<String>,
    },
    /// Missing semicolon
    MissingSemicolon {
        /// Location of the error
        location: Location,
    },
    /// Invalid declaration
    InvalidDeclaration {
        /// Location of the error
        location: Location,
        /// Error message
        message: String,
    },
}

impl SyntaxError {
    /// Convert the syntax error to a diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            SyntaxError::UnexpectedToken { location, found, expected } => {
                let expected_str = if expected.len() == 1 {
                    expected[0].clone()
                } else {
                    let mut s = expected[..expected.len() - 1].join(", ");
                    s.push_str(&format!(" or {}", expected[expected.len() - 1]));
                    s
                };
                
                Diagnostic {
                    id: "PAR0001".to_string(),
                    severity: Severity::Error,
                    message: LocalizedMessage::bilingual(
                        format!("Unexpected token: '{}', expected: {}", found, expected_str),
                        format!("رمز غير متوقع: '{}', المتوقع: {}", found, expected_str),
                    ),
                    location: location.clone(),
                    source_text: found.clone(),
                    expected_text: Some(expected_str),
                    fix_suggestions: vec![],
                    related_information: vec![],
                    documentation_url: None,
                    language_context: LanguageContext::Mixed, // Determined at runtime
                }
            },
            SyntaxError::UnexpectedEOF { location, expected } => {
                let expected_str = if expected.len() == 1 {
                    expected[0].clone()
                } else {
                    let mut s = expected[..expected.len() - 1].join(", ");
                    s.push_str(&format!(" or {}", expected[expected.len() - 1]));
                    s
                };
                
                Diagnostic {
                    id: "PAR0002".to_string(),
                    severity: Severity::Error,
                    message: LocalizedMessage::bilingual(
                        format!("Unexpected end of file, expected: {}", expected_str),
                        format!("نهاية غير متوقعة للملف، المتوقع: {}", expected_str),
                    ),
                    location: location.clone(),
                    source_text: "EOF".to_string(),
                    expected_text: Some(expected_str),
                    fix_suggestions: vec![],
                    related_information: vec![],
                    documentation_url: None,
                    language_context: LanguageContext::Mixed, // Determined at runtime
                }
            },
            SyntaxError::MissingSemicolon { location } => Diagnostic {
                id: "PAR0003".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    "Missing semicolon at the end of statement",
                    "فاصلة منقوطة مفقودة في نهاية العبارة",
                ),
                location: location.clone(),
                source_text: "".to_string(), // Placeholder, actual value determined at runtime
                expected_text: Some(";".to_string()),
                fix_suggestions: vec![
                    FixSuggestion {
                        message: LocalizedMessage::bilingual(
                            "Add a semicolon",
                            "أضف فاصلة منقوطة",
                        ),
                        replacement: ";".to_string(),
                        replacement_range: None,
                    },
                ],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            SyntaxError::InvalidDeclaration { location, message } => Diagnostic {
                id: "PAR0004".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Invalid declaration: {}", message),
                    format!("تصريح غير صالح: {}", message),
                ),
                location: location.clone(),
                source_text: "".to_string(), // Placeholder, actual value determined at runtime
                expected_text: None,
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
        }
    }
}

/// Error type for the semantic analyzer
#[derive(Debug)]
pub enum SemanticError {
    /// Undefined variable
    UndefinedVariable {
        /// Location of the error
        location: Location,
        /// The undefined variable
        name: String,
    },
    /// Type mismatch
    TypeMismatch {
        /// Location of the error
        location: Location,
        /// The expected type
        expected: String,
        /// The actual type
        actual: String,
    },
    /// Immutable assignment
    ImmutableAssignment {
        /// Location of the error
        location: Location,
        /// The immutable variable
        name: String,
        /// Where the variable was declared
        declaration_location: Location,
    },
}

impl SemanticError {
    /// Convert the semantic error to a diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            SemanticError::UndefinedVariable { location, name } => Diagnostic {
                id: "SEM0001".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Undefined variable: '{}'", name),
                    format!("متغير غير معرّف: '{}'", name),
                ),
                location: location.clone(),
                source_text: name.clone(),
                expected_text: None,
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            SemanticError::TypeMismatch { location, expected, actual } => Diagnostic {
                id: "SEM0002".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Type mismatch: expected '{}', got '{}'", expected, actual),
                    format!("عدم تطابق النوع: المتوقع '{}', الفعلي '{}'", expected, actual),
                ),
                location: location.clone(),
                source_text: "".to_string(), // Placeholder, actual value determined at runtime
                expected_text: Some(expected.clone()),
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            SemanticError::ImmutableAssignment { location, name, declaration_location } => Diagnostic {
                id: "SEM0003".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Cannot assign to immutable variable: '{}'", name),
                    format!("لا يمكن التعيين إلى متغير غير قابل للتغيير: '{}'", name),
                ),
                location: location.clone(),
                source_text: name.clone(),
                expected_text: None,
                fix_suggestions: vec![
                    FixSuggestion {
                        message: LocalizedMessage::bilingual(
                            "Change declaration to 'var' instead of 'val'",
                            "غيّر التعريف إلى 'متغير' بدلاً من 'ثابت'",
                        ),
                        replacement: "var".to_string(), // Simplified, actual would depend on language context
                        replacement_range: Some(declaration_location.clone()),
                    },
                ],
                related_information: vec![
                    RelatedInformation {
                        message: LocalizedMessage::bilingual(
                            format!("Variable '{}' is declared here as immutable", name),
                            format!("تم تعريف المتغير '{}' هنا كمتغير غير قابل للتغيير", name),
                        ),
                        location: declaration_location.clone(),
                    },
                ],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
        }
    }
}

/// Error type for the code generator
#[derive(Debug)]
pub enum CodeGenError {
    /// LLVM error
    LLVMError {
        /// Location of the error
        location: Location,
        /// The error message
        message: String,
    },
    /// Type conversion error
    TypeConversionError {
        /// Location of the error
        location: Location,
        /// The Seen type
        seen_type: String,
    },
    /// Function call error
    FunctionCallError {
        /// Location of the error
        location: Location,
        /// The function name
        name: String,
        /// The error message
        message: String,
    },
}

impl CodeGenError {
    /// Convert the code generation error to a diagnostic
    pub fn to_diagnostic(&self) -> Diagnostic {
        match self {
            CodeGenError::LLVMError { location, message } => Diagnostic {
                id: "GEN0001".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("LLVM error: {}", message),
                    format!("خطأ LLVM: {}", message),
                ),
                location: location.clone(),
                source_text: "".to_string(), // Placeholder, actual value determined at runtime
                expected_text: None,
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            CodeGenError::TypeConversionError { location, seen_type } => Diagnostic {
                id: "GEN0002".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Cannot convert Seen type '{}' to LLVM type", seen_type),
                    format!("لا يمكن تحويل نوع Seen '{}' إلى نوع LLVM", seen_type),
                ),
                location: location.clone(),
                source_text: seen_type.clone(),
                expected_text: None,
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
            CodeGenError::FunctionCallError { location, name, message } => Diagnostic {
                id: "GEN0003".to_string(),
                severity: Severity::Error,
                message: LocalizedMessage::bilingual(
                    format!("Function call error for '{}': {}", name, message),
                    format!("خطأ في استدعاء الدالة '{}': {}", name, message),
                ),
                location: location.clone(),
                source_text: name.clone(),
                expected_text: None,
                fix_suggestions: vec![],
                related_information: vec![],
                documentation_url: None,
                language_context: LanguageContext::Mixed, // Determined at runtime
            },
        }
    }
}

/// Collection of diagnostics
#[derive(Debug, Default)]
pub struct Diagnostics {
    /// List of diagnostic messages
    pub messages: Vec<Diagnostic>,
}

impl Diagnostics {
    /// Create a new empty diagnostics collection
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }
    
    /// Add a diagnostic
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.messages.push(diagnostic);
    }
    
    /// Add a lexical error
    pub fn add_lexical_error(&mut self, error: &LexicalError) {
        self.add(error.to_diagnostic());
    }
    
    /// Add a syntax error
    pub fn add_syntax_error(&mut self, error: &SyntaxError) {
        self.add(error.to_diagnostic());
    }
    
    /// Add a semantic error
    pub fn add_semantic_error(&mut self, error: &SemanticError) {
        self.add(error.to_diagnostic());
    }
    
    /// Add a code generation error
    pub fn add_codegen_error(&mut self, error: &CodeGenError) {
        self.add(error.to_diagnostic());
    }
    
    /// Check if there are any error diagnostics
    pub fn has_errors(&self) -> bool {
        self.messages.iter().any(|d| d.severity == Severity::Error)
    }
    
    /// Format all diagnostics
    pub fn format(&self, language_pref: LanguagePreference, use_colors: bool) -> String {
        let formatter = DiagnosticFormatter::new(language_pref, use_colors);
        
        let mut output = String::new();
        for diagnostic in &self.messages {
            output.push_str(&formatter.format(diagnostic));
            output.push('\n');
        }
        
        output
    }
}
