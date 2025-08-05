//! Error recovery mechanisms for the lexer

use crate::{Token, TokenType, TokenUtils};
use seen_common::{Span, Diagnostics};

/// Error recovery strategy for the lexer
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    /// Skip the current character and continue
    SkipCharacter,
    /// Skip to the next delimiter
    SkipToDelimiter,
    /// Skip to the end of the current line
    SkipToEndOfLine,
    /// Insert a missing token
    InsertToken(TokenType),
}

/// Error recovery context
pub struct ErrorRecovery {
    pub diagnostics: Diagnostics,
    pub recovery_tokens: Vec<Token>,
}

impl ErrorRecovery {
    pub fn new() -> Self {
        Self {
            diagnostics: Diagnostics::new(),
            recovery_tokens: Vec::new(),
        }
    }
    
    /// Apply recovery strategy for a lexical error
    pub fn recover_from_error(
        &mut self,
        error_message: &str,
        error_span: Span,
        strategy: RecoveryStrategy,
    ) {
        self.diagnostics.error(error_message, error_span);
        
        match strategy {
            RecoveryStrategy::SkipCharacter => {
                // Just record the error, character will be skipped
            }
            RecoveryStrategy::SkipToDelimiter => {
                // Add a marker for where we should skip to
                let marker = TokenUtils::new(
                    TokenType::Error("Skipping to delimiter".to_string()),
                    error_span
                );
                self.recovery_tokens.push(marker);
            }
            RecoveryStrategy::SkipToEndOfLine => {
                // Add a marker for end-of-line skip
                let marker = TokenUtils::new(
                    TokenType::Error("Skipping to end of line".to_string()),
                    error_span
                );
                self.recovery_tokens.push(marker);
            }
            RecoveryStrategy::InsertToken(token_type) => {
                // Insert the expected token
                let inserted_token: Token = TokenUtils::new(token_type, error_span);
                self.diagnostics.hint(
                    &format!("Inserted missing token: {}", inserted_token.value),
                    error_span
                );
                
                self.recovery_tokens.push(inserted_token);
            }
        }
    }
    
    /// Check if we should attempt error recovery at the current position
    pub fn should_recover_at_position(&self, current_char: char, next_char: Option<char>) -> Option<RecoveryStrategy> {
        match current_char {
            // Unexpected characters that we can skip
            '\x00'..='\x08' | '\x0B'..='\x0C' | '\x0E'..='\x1F' => {
                Some(RecoveryStrategy::SkipCharacter)
            }
            // For most other errors, skip to a delimiter
            _ if self.is_delimiter_ahead(next_char) => {
                Some(RecoveryStrategy::SkipToDelimiter)
            }
            // Default to skipping the character
            _ => Some(RecoveryStrategy::SkipCharacter),
        }
    }
    
    /// Check if there's a delimiter coming up that we can sync to
    fn is_delimiter_ahead(&self, next_char: Option<char>) -> bool {
        matches!(next_char, Some('(' | ')' | '{' | '}' | '[' | ']' | ';' | ',' | '\n'))
    }
    
    /// Suggest corrections for common lexical errors
    pub fn suggest_correction(&self, error_char: char, context: &str) -> Option<String> {
        match error_char {
            '`' => Some("Did you mean to use single quotes '?' for a character literal?".to_string()),
            '"' if context.contains('\'') => Some("Mismatched quote type - use matching quotes".to_string()),
            '\'' if context.contains('"') => Some("Mismatched quote type - use matching quotes".to_string()),
            '–' | '—' => Some("Use regular hyphen '-' instead of em-dash".to_string()),
            '\u{2018}' | '\u{2019}' => Some("Use regular single quote ' instead of curly quote".to_string()),
            '\u{201C}' | '\u{201D}' => Some("Use regular double quote \" instead of curly quote".to_string()),
            _ => None,
        }
    }
}

impl Default for ErrorRecovery {
    fn default() -> Self {
        Self::new()
    }
}