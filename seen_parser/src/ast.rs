//! Abstract Syntax Tree definitions

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub expressions: Vec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expression {
    // Placeholder - will be expanded following TDD methodology
    Placeholder,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ast_creation() {
        let program = Program {
            expressions: vec![Expression::Placeholder],
        };
        assert_eq!(program.expressions.len(), 1);
    }
}