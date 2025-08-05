//! Diagnostic message handling for the Seen compiler

use crate::{Span, SeenError};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity level for diagnostic messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
            Severity::Hint => write!(f, "hint"),
        }
    }
}

/// A diagnostic message with location and severity
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub code: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            span,
            code: None,
            help: None,
        }
    }
    
    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            span,
            code: None,
            help: None,
        }
    }
    
    pub fn info(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Info,
            message: message.into(),
            span,
            code: None,
            help: None,
        }
    }
    
    pub fn hint(message: impl Into<String>, span: Span) -> Self {
        Self {
            severity: Severity::Hint,
            message: message.into(),
            span,
            code: None,
            help: None,
        }
    }
    
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }
    
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
    
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} at {}", self.severity, self.message, self.span)?;
        
        if let Some(code) = &self.code {
            write!(f, " [{}]", code)?;
        }
        
        if let Some(help) = &self.help {
            write!(f, "\n  help: {}", help)?;
        }
        
        Ok(())
    }
}

/// Collection of diagnostic messages
#[derive(Debug, Clone, Default)]
pub struct Diagnostics {
    pub messages: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.messages.push(diagnostic);
    }
    
    pub fn error(&mut self, message: impl Into<String>, span: Span) {
        self.add(Diagnostic::error(message, span));
    }
    
    pub fn warning(&mut self, message: impl Into<String>, span: Span) {
        self.add(Diagnostic::warning(message, span));
    }
    
    pub fn info(&mut self, message: impl Into<String>, span: Span) {
        self.add(Diagnostic::info(message, span));
    }
    
    pub fn hint(&mut self, message: impl Into<String>, span: Span) {
        self.add(Diagnostic::hint(message, span));
    }
    
    pub fn has_errors(&self) -> bool {
        self.messages.iter().any(|d| d.severity == Severity::Error)
    }
    
    pub fn error_count(&self) -> usize {
        self.messages.iter().filter(|d| d.severity == Severity::Error).count()
    }
    
    pub fn warning_count(&self) -> usize {
        self.messages.iter().filter(|d| d.severity == Severity::Warning).count()
    }
    
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    
    pub fn clear(&mut self) {
        self.messages.clear();
    }
    
    pub fn extend(&mut self, other: Diagnostics) {
        self.messages.extend(other.messages);
    }
    
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.messages.iter().filter(|d| d.severity == Severity::Error)
    }
    
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.messages.iter().filter(|d| d.severity == Severity::Warning)
    }
}

impl From<SeenError> for Diagnostic {
    fn from(error: SeenError) -> Self {
        let span = Span::single(crate::Position::start(), 0);
        Diagnostic::error(error.to_string(), span)
    }
}