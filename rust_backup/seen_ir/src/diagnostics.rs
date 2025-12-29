//! Structured error reporting and source location tracking for the Seen compiler.
//!
//! This module provides:
//! - `SourceSpan`: Source location tracking (file, line, column)
//! - `CompilerError`: Structured errors with context and suggestions
//! - `DiagnosticEmitter`: Formatters for terminal and JSON output

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

/// Source location in the original Seen code.
/// Attached to IR instructions to enable precise error reporting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct SourceSpan {
    /// File path (relative or absolute)
    pub file: Option<PathBuf>,
    /// 1-based line number
    pub line: u32,
    /// 1-based column number
    pub column: u32,
    /// Length of the span in characters (0 = point location)
    pub length: u32,
}

impl SourceSpan {
    pub fn new(file: impl Into<PathBuf>, line: u32, column: u32) -> Self {
        Self {
            file: Some(file.into()),
            line,
            column,
            length: 0,
        }
    }

    pub fn with_length(mut self, length: u32) -> Self {
        self.length = length;
        self
    }

    pub fn unknown() -> Self {
        Self::default()
    }

    pub fn is_known(&self) -> bool {
        self.line > 0
    }
    
    /// Create a SourceSpan from a parser Position
    pub fn from_position(pos: &seen_parser::Position) -> Self {
        Self {
            file: None,
            line: pos.line as u32,
            column: pos.column as u32,
            length: 0,
        }
    }
    
    /// Create a SourceSpan from a parser Position with file path
    pub fn from_position_with_file(pos: &seen_parser::Position, file: impl Into<PathBuf>) -> Self {
        Self {
            file: Some(file.into()),
            line: pos.line as u32,
            column: pos.column as u32,
            length: 0,
        }
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}:{}:{}", file.display(), self.line, self.column)
        } else if self.line > 0 {
            write!(f, "<unknown>:{}:{}", self.line, self.column)
        } else {
            write!(f, "<unknown location>")
        }
    }
}

/// Compiler phase where an error occurred
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompilerPhase {
    Lexer,
    Parser,
    TypeChecker,
    Monomorphization,
    IRGeneration,
    IRValidation,
    IROptimization,
    LLVMCodegen,
    Linking,
}

impl fmt::Display for CompilerPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompilerPhase::Lexer => write!(f, "lexer"),
            CompilerPhase::Parser => write!(f, "parser"),
            CompilerPhase::TypeChecker => write!(f, "type checker"),
            CompilerPhase::Monomorphization => write!(f, "monomorphization"),
            CompilerPhase::IRGeneration => write!(f, "IR generation"),
            CompilerPhase::IRValidation => write!(f, "IR validation"),
            CompilerPhase::IROptimization => write!(f, "IR optimization"),
            CompilerPhase::LLVMCodegen => write!(f, "LLVM codegen"),
            CompilerPhase::Linking => write!(f, "linking"),
        }
    }
}

/// Severity level of a diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
    Help,
}

impl fmt::Display for DiagnosticSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticSeverity::Error => write!(f, "error"),
            DiagnosticSeverity::Warning => write!(f, "warning"),
            DiagnosticSeverity::Note => write!(f, "note"),
            DiagnosticSeverity::Help => write!(f, "help"),
        }
    }
}

/// A structured compiler error with full context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilerError {
    /// Severity (error, warning, note)
    pub severity: DiagnosticSeverity,
    /// Which compiler phase produced this error
    pub phase: CompilerPhase,
    /// Primary error message
    pub message: String,
    /// Source location if known
    pub location: Option<SourceSpan>,
    /// Expected type/value (for type mismatches)
    pub expected: Option<String>,
    /// Actual type/value found
    pub actual: Option<String>,
    /// Function/method where error occurred
    pub in_function: Option<String>,
    /// Instruction index within function (for IR errors)
    pub instruction_index: Option<usize>,
    /// Suggested fix
    pub suggestion: Option<String>,
    /// Related notes (e.g., "defined here")
    pub notes: Vec<(String, Option<SourceSpan>)>,
    /// Error code for documentation lookup (e.g., "E0001")
    pub code: Option<String>,
}

impl CompilerError {
    /// Create a new error with minimal information
    pub fn new(phase: CompilerPhase, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            phase,
            message: message.into(),
            location: None,
            expected: None,
            actual: None,
            in_function: None,
            instruction_index: None,
            suggestion: None,
            notes: Vec::new(),
            code: None,
        }
    }

    /// Builder method: add source location
    pub fn at(mut self, location: SourceSpan) -> Self {
        self.location = Some(location);
        self
    }

    /// Builder method: add expected/actual for type mismatches
    pub fn expected_actual(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    /// Builder method: add function context
    pub fn in_function(mut self, name: impl Into<String>) -> Self {
        self.in_function = Some(name.into());
        self
    }

    /// Builder method: add instruction index
    pub fn at_instruction(mut self, index: usize) -> Self {
        self.instruction_index = Some(index);
        self
    }

    /// Builder method: add a suggestion
    pub fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Builder method: add a related note
    pub fn note(mut self, message: impl Into<String>, location: Option<SourceSpan>) -> Self {
        self.notes.push((message.into(), location));
        self
    }

    /// Builder method: set error code
    pub fn code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Builder method: change severity to warning
    pub fn as_warning(mut self) -> Self {
        self.severity = DiagnosticSeverity::Warning;
        self
    }

    /// Create a type mismatch error (common case)
    pub fn type_mismatch(
        phase: CompilerPhase,
        expected: impl Into<String>,
        actual: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        let expected = expected.into();
        let actual = actual.into();
        Self::new(phase, format!("type mismatch: expected `{}`, found `{}`", expected, actual))
            .expected_actual(&expected, &actual)
            .at(location.unwrap_or_default())
    }

    /// Create an "undefined" error (variable, function, type)
    pub fn undefined(
        phase: CompilerPhase,
        kind: &str,
        name: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        let name = name.into();
        Self::new(phase, format!("undefined {}: `{}`", kind, name))
            .at(location.unwrap_or_default())
    }

    /// Create an unresolved generic error
    pub fn unresolved_generic(
        generic_name: impl Into<String>,
        in_function: impl Into<String>,
        location: Option<SourceSpan>,
    ) -> Self {
        let generic_name = generic_name.into();
        Self::new(
            CompilerPhase::IRValidation,
            format!("unresolved generic type parameter `{}`", generic_name),
        )
        .in_function(in_function)
        .at(location.unwrap_or_default())
        .suggest("Generic parameters must be resolved during monomorphization before IR generation")
        .code("E0100")
    }
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Error code and severity
        if let Some(code) = &self.code {
            write!(f, "{}[{}]: ", self.severity, code)?;
        } else {
            write!(f, "{}: ", self.severity)?;
        }

        // Main message
        writeln!(f, "{}", self.message)?;

        // Location
        if let Some(loc) = &self.location {
            if loc.is_known() {
                writeln!(f, "  --> {}", loc)?;
            }
        }

        // Function context
        if let Some(func) = &self.in_function {
            write!(f, "  in function `{}`", func)?;
            if let Some(idx) = self.instruction_index {
                write!(f, " at instruction {}", idx)?;
            }
            writeln!(f)?;
        }

        // Expected/actual
        if let (Some(expected), Some(actual)) = (&self.expected, &self.actual) {
            writeln!(f, "  expected: {}", expected)?;
            writeln!(f, "     found: {}", actual)?;
        }

        // Notes
        for (note, loc) in &self.notes {
            write!(f, "  note: {}", note)?;
            if let Some(loc) = loc {
                if loc.is_known() {
                    write!(f, " (at {})", loc)?;
                }
            }
            writeln!(f)?;
        }

        // Suggestion
        if let Some(suggestion) = &self.suggestion {
            writeln!(f, "  help: {}", suggestion)?;
        }

        Ok(())
    }
}

impl std::error::Error for CompilerError {}

/// Result type for compiler operations
pub type CompilerResult<T> = Result<T, CompilerError>;

/// Collection of compiler diagnostics
#[derive(Debug, Default, Clone)]
pub struct Diagnostics {
    pub errors: Vec<CompilerError>,
    pub warnings: Vec<CompilerError>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn error(&mut self, error: CompilerError) {
        self.errors.push(error);
    }

    pub fn warning(&mut self, warning: CompilerError) {
        self.warnings.push(warning.as_warning());
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn merge(&mut self, other: Diagnostics) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }

    /// Convert to a single error if there are any errors
    pub fn into_result<T>(self, value: T) -> Result<T, Vec<CompilerError>> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(self.errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_span_display() {
        let span = SourceSpan::new("main.seen", 42, 5);
        assert_eq!(format!("{}", span), "main.seen:42:5");
    }

    #[test]
    fn test_compiler_error_builder() {
        let err = CompilerError::new(CompilerPhase::IRValidation, "test error")
            .at(SourceSpan::new("test.seen", 10, 1))
            .in_function("main")
            .expected_actual("Int", "String")
            .suggest("use toString()");

        assert!(format!("{}", err).contains("test error"));
        assert!(format!("{}", err).contains("test.seen:10:1"));
        assert!(format!("{}", err).contains("main"));
    }

    #[test]
    fn test_unresolved_generic_error() {
        let err = CompilerError::unresolved_generic("T", "Vec_push", None);
        assert!(format!("{}", err).contains("unresolved generic"));
        assert!(format!("{}", err).contains("monomorphization"));
    }
}
