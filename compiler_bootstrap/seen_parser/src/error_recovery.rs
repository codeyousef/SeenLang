//! Parser error recovery

use seen_common::{Diagnostics, SeenError};

/// Parser error recovery strategy
pub struct ErrorRecovery {
    pub diagnostics: Diagnostics,
}

impl ErrorRecovery {
    pub fn new() -> Self {
        Self {
            diagnostics: Diagnostics::new(),
        }
    }
}

impl Default for ErrorRecovery {
    fn default() -> Self {
        Self::new()
    }
}