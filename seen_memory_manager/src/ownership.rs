//! Automatic ownership analysis for Vale-style memory management
//!
//! This module implements automatic ownership inference without requiring manual
//! lifetime annotations. The system analyzes variable usage patterns to determine
//! optimal ownership strategies (move, borrow, copy).

use std::collections::{HashMap, HashSet};
use seen_parser::ast::*;
use seen_lexer::Position;

/// Represents different ownership modes for variables
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipMode {
    /// Variable owns the data and is responsible for cleanup
    Own,
    /// Variable holds a temporary reference to data owned elsewhere  
    Borrow,
    /// Variable contains a mutable reference
    BorrowMut,
    /// Variable receives ownership via move semantics
    Move,
    /// Variable creates a copy of the data (for primitive types)
    Copy,
}

/// Information about a variable's ownership throughout its lifetime
#[derive(Debug, Clone)]
pub struct VariableOwnership {
    /// Variable name
    pub name: String,
    /// Ownership mode
    pub mode: OwnershipMode,
    /// Position where ownership is established
    pub declared_at: Position,
    /// Positions where variable is accessed
    pub accessed_at: Vec<Position>,
    /// Position where ownership is transferred (if any)
    pub moved_at: Option<Position>,
    /// Whether variable is mutable
    pub is_mutable: bool,
}

/// Tracks ownership information for all variables in a scope
#[derive(Debug, Default, Clone)]
pub struct OwnershipInfo {
    /// Map of variable names to their ownership information
    pub variables: HashMap<String, VariableOwnership>,
    /// Variables that have been moved and are no longer accessible
    pub moved_variables: HashSet<String>,
    /// Borrowing relationships (borrower -> borrowed_from)
    pub borrow_graph: HashMap<String, String>,
}

impl OwnershipInfo {
    /// Create a new empty ownership info
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Record a variable declaration
    pub fn declare_variable(&mut self, name: String, mode: OwnershipMode, pos: Position, is_mutable: bool) {
        self.variables.insert(name.clone(), VariableOwnership {
            name: name.clone(),
            mode,
            declared_at: pos,
            accessed_at: Vec::new(),
            moved_at: None,
            is_mutable,
        });
    }
    
    /// Record variable access
    pub fn access_variable(&mut self, name: &str, pos: Position) -> Result<(), OwnershipError> {
        if self.moved_variables.contains(name) {
            return Err(OwnershipError::UseAfterMove {
                variable: name.to_string(),
                moved_at: self.variables[name].moved_at.unwrap(),
                used_at: pos,
            });
        }
        
        if let Some(var_info) = self.variables.get_mut(name) {
            var_info.accessed_at.push(pos);
        }
        
        Ok(())
    }
    
    /// Record variable move
    pub fn move_variable(&mut self, name: &str, pos: Position) -> Result<(), OwnershipError> {
        if self.moved_variables.contains(name) {
            return Err(OwnershipError::UseAfterMove {
                variable: name.to_string(),
                moved_at: self.variables[name].moved_at.unwrap(),
                used_at: pos,
            });
        }
        
        if let Some(var_info) = self.variables.get_mut(name) {
            var_info.moved_at = Some(pos);
            self.moved_variables.insert(name.to_string());
        }
        
        Ok(())
    }
    
    /// Check if a variable can be borrowed
    pub fn can_borrow(&self, name: &str) -> bool {
        !self.moved_variables.contains(name) && self.variables.contains_key(name)
    }
    
    /// Check if a variable can be mutably borrowed
    pub fn can_borrow_mut(&self, name: &str) -> bool {
        self.can_borrow(name) && 
        self.variables.get(name).map_or(false, |v| v.is_mutable) &&
        !self.has_existing_borrows(name)
    }
    
    /// Check if a variable already has existing borrows
    pub fn has_existing_borrows(&self, name: &str) -> bool {
        self.borrow_graph.values().any(|borrowed| borrowed == name)
    }
}

/// Errors that can occur during ownership analysis
#[derive(Debug, Clone)]
pub enum OwnershipError {
    /// Variable used after being moved
    UseAfterMove {
        variable: String,
        moved_at: Position,
        used_at: Position,
    },
    /// Multiple mutable borrows of the same variable
    MultipleMutableBorrows {
        variable: String,
        first_borrow: Position,
        second_borrow: Position,
    },
    /// Immutable and mutable borrow conflict
    BorrowConflict {
        variable: String,
        immutable_borrow: Position,
        mutable_borrow: Position,
    },
    /// Variable borrowed when owner is moved
    BorrowAfterMove {
        variable: String,
        moved_at: Position,
        borrowed_at: Position,
    },
}

impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OwnershipError::UseAfterMove { variable, moved_at, used_at } => {
                write!(f, "Variable '{}' used at {} after being moved at {}", 
                       variable, used_at, moved_at)
            }
            OwnershipError::MultipleMutableBorrows { variable, first_borrow, second_borrow } => {
                write!(f, "Variable '{}' borrowed mutably multiple times: first at {}, second at {}", 
                       variable, first_borrow, second_borrow)
            }
            OwnershipError::BorrowConflict { variable, immutable_borrow, mutable_borrow } => {
                write!(f, "Variable '{}' has borrow conflict: immutable at {}, mutable at {}", 
                       variable, immutable_borrow, mutable_borrow)
            }
            OwnershipError::BorrowAfterMove { variable, moved_at, borrowed_at } => {
                write!(f, "Variable '{}' borrowed at {} after being moved at {}", 
                       variable, borrowed_at, moved_at)
            }
        }
    }
}

impl std::error::Error for OwnershipError {}

/// Analyzer for automatic ownership inference
pub struct OwnershipAnalyzer {
    /// Current ownership information
    ownership_info: OwnershipInfo,
    /// Stack of scopes for nested blocks
    scope_stack: Vec<OwnershipInfo>,
}

impl OwnershipAnalyzer {
    /// Create a new ownership analyzer
    pub fn new() -> Self {
        Self {
            ownership_info: OwnershipInfo::new(),
            scope_stack: Vec::new(),
        }
    }
    
    /// Analyze ownership for a complete program
    pub fn analyze_program(&mut self, program: &Program) -> Result<OwnershipInfo, OwnershipError> {
        for expression in &program.expressions {
            self.analyze_expression(expression)?;
        }
        Ok(std::mem::take(&mut self.ownership_info))
    }
    
    /// Analyze ownership for an expression
    pub fn analyze_expression(&mut self, expr: &Expression) -> Result<OwnershipMode, OwnershipError> {
        match expr {
            // Literals create owned values
            Expression::IntegerLiteral { .. } |
            Expression::FloatLiteral { .. } |
            Expression::StringLiteral { .. } |
            Expression::BooleanLiteral { .. } |
            Expression::NullLiteral { .. } => Ok(OwnershipMode::Own),
            
            // Identifiers access existing variables
            Expression::Identifier { name, pos, .. } => {
                self.ownership_info.access_variable(name, *pos)?;
                
                // Return borrow by default unless explicitly moved
                Ok(OwnershipMode::Borrow)
            }
            
            // Binary operations typically borrow operands
            Expression::BinaryOp { left, right, .. } => {
                self.analyze_expression(left)?;
                self.analyze_expression(right)?;
                
                // Result is owned (computed value)
                Ok(OwnershipMode::Own)
            }
            
            // Unary operations borrow operand
            Expression::UnaryOp { operand, .. } => {
                self.analyze_expression(operand)?;
                
                // Result is owned (computed value)
                Ok(OwnershipMode::Own)
            }
            
            // Function calls transfer ownership based on parameter semantics
            Expression::Call { callee, args, .. } => {
                self.analyze_expression(callee)?;
                
                for arg in args {
                    self.analyze_expression(arg)?;
                }
                
                // Function result is owned
                Ok(OwnershipMode::Own)
            }
            
            // Member access borrows the object
            Expression::MemberAccess { object, .. } => {
                self.analyze_expression(object)?;
                
                // Field access result is borrowed from object
                Ok(OwnershipMode::Borrow)
            }
            
            // Variable bindings establish ownership
            Expression::Let { name, value, is_mutable, pos, .. } => {
                let value_ownership = self.analyze_expression(value)?;
                
                // Determine ownership mode based on value
                let ownership_mode = match value_ownership {
                    OwnershipMode::Own => OwnershipMode::Own,
                    OwnershipMode::Move => OwnershipMode::Own,
                    _ => OwnershipMode::Borrow,
                };
                
                self.ownership_info.declare_variable(
                    name.clone(), 
                    ownership_mode, 
                    *pos, 
                    *is_mutable
                );
                
                Ok(OwnershipMode::Own)
            }
            
            // Blocks create new scopes
            Expression::Block { expressions, .. } => {
                self.enter_scope();
                
                let mut last_ownership = OwnershipMode::Own;
                for expr in expressions {
                    last_ownership = self.analyze_expression(expr)?;
                }
                
                self.exit_scope();
                Ok(last_ownership)
            }
            
            // Arrays own their elements
            Expression::ArrayLiteral { elements, .. } => {
                for element in elements {
                    self.analyze_expression(element)?;
                }
                Ok(OwnershipMode::Own)
            }
            
            // Struct literals own their field values
            Expression::StructLiteral { fields, .. } => {
                for (_, value) in fields {
                    self.analyze_expression(value)?;
                }
                Ok(OwnershipMode::Own)
            }
            
            // Index access borrows from array
            Expression::IndexAccess { object, index, .. } => {
                self.analyze_expression(object)?;
                self.analyze_expression(index)?;
                Ok(OwnershipMode::Borrow)
            }
            
            // Control flow expressions
            Expression::If { condition, then_branch, else_branch, .. } => {
                self.analyze_expression(condition)?;
                let then_ownership = self.analyze_expression(then_branch)?;
                
                if let Some(else_expr) = else_branch {
                    let else_ownership = self.analyze_expression(else_expr)?;
                    
                    // If both branches have same ownership, use that; otherwise own
                    if then_ownership == else_ownership {
                        Ok(then_ownership)
                    } else {
                        Ok(OwnershipMode::Own)
                    }
                } else {
                    Ok(then_ownership)
                }
            }
            
            // Nullable operators
            Expression::Elvis { nullable, default, .. } => {
                self.analyze_expression(nullable)?;
                self.analyze_expression(default)?;
                Ok(OwnershipMode::Own)
            }
            
            Expression::ForceUnwrap { nullable, .. } => {
                self.analyze_expression(nullable)
            }
            
            // Other expressions default to owned
            _ => Ok(OwnershipMode::Own),
        }
    }
    
    /// Enter a new scope (for blocks, functions, etc.)
    fn enter_scope(&mut self) {
        let current_scope = std::mem::take(&mut self.ownership_info);
        self.scope_stack.push(current_scope);
        self.ownership_info = OwnershipInfo::new();
    }
    
    /// Exit current scope and merge ownership information
    fn exit_scope(&mut self) {
        if let Some(parent_scope) = self.scope_stack.pop() {
            // Merge relevant ownership information back to parent
            let current_scope = std::mem::take(&mut self.ownership_info);
            self.ownership_info = parent_scope;
            
            // Variables declared in inner scope don't affect outer scope
            // But moved variables from outer scope should be recorded
            for moved_var in current_scope.moved_variables {
                if self.ownership_info.variables.contains_key(&moved_var) {
                    self.ownership_info.moved_variables.insert(moved_var);
                }
            }
        }
    }
}

impl Default for OwnershipAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ownership_info_creation() {
        let info = OwnershipInfo::new();
        assert!(info.variables.is_empty());
        assert!(info.moved_variables.is_empty());
    }
    
    #[test]
    fn test_variable_declaration() {
        let mut info = OwnershipInfo::new();
        let pos = Position::new(1, 1, 0);
        
        info.declare_variable("x".to_string(), OwnershipMode::Own, pos, false);
        
        assert!(info.variables.contains_key("x"));
        assert_eq!(info.variables["x"].mode, OwnershipMode::Own);
        assert!(!info.variables["x"].is_mutable);
    }
    
    #[test]
    fn test_variable_access() {
        let mut info = OwnershipInfo::new();
        let pos = Position::new(1, 1, 0);
        
        info.declare_variable("x".to_string(), OwnershipMode::Own, pos, false);
        
        let access_result = info.access_variable("x", Position::new(2, 1, 0));
        assert!(access_result.is_ok());
        assert_eq!(info.variables["x"].accessed_at.len(), 1);
    }
    
    #[test]
    fn test_use_after_move() {
        let mut info = OwnershipInfo::new();
        let pos = Position::new(1, 1, 0);
        
        info.declare_variable("x".to_string(), OwnershipMode::Own, pos, false);
        info.move_variable("x", Position::new(2, 1, 0)).unwrap();
        
        let access_result = info.access_variable("x", Position::new(3, 1, 0));
        assert!(access_result.is_err());
        
        if let Err(OwnershipError::UseAfterMove { variable, .. }) = access_result {
            assert_eq!(variable, "x");
        } else {
            panic!("Expected UseAfterMove error");
        }
    }
    
    #[test]
    fn test_ownership_analyzer() {
        let mut analyzer = OwnershipAnalyzer::new();
        
        // Create a simple program: let x = 42
        let program = Program {
            expressions: vec![
                Expression::Let {
                    name: "x".to_string(),
                    type_annotation: None,
                    value: Box::new(Expression::IntegerLiteral {
                        value: 42,
                        pos: Position::new(1, 9, 8),
                    }),
                    is_mutable: false,
                    pos: Position::new(1, 1, 0),
                }
            ],
        };
        
        let result = analyzer.analyze_program(&program);
        assert!(result.is_ok());
        
        let ownership_info = result.unwrap();
        assert!(ownership_info.variables.contains_key("x"));
        assert_eq!(ownership_info.variables["x"].mode, OwnershipMode::Own);
    }
}