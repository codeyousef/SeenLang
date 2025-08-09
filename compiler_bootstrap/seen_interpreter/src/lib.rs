//! AST Interpreter for Seen Language
//! 
//! This provides direct execution of Seen programs without LLVM compilation.
//! Used for `seen run` command and quick iteration during development.

use seen_common::{SeenResult, SeenError};
use seen_parser::{Program, ItemKind, Function, Block, Stmt, StmtKind, Expr, ExprKind, Literal};
use std::collections::HashMap;
use std::io::{self, Write};

#[derive(Debug, Clone)]
pub enum Value {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Unit,
}

impl Value {
    fn to_string(&self) -> String {
        match self {
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::String(s) => s.clone(),
            Value::Boolean(b) => b.to_string(),
            Value::Unit => "()".to_string(),
        }
    }
}

pub struct Interpreter {
    globals: HashMap<String, Value>,
    locals: Vec<HashMap<String, Value>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            globals: HashMap::new(),
            locals: vec![HashMap::new()],
        }
    }
    
    pub fn execute(&mut self, program: &Program) -> SeenResult<i32> {
        // Find main function
        let main_fn = program.items.iter()
            .find_map(|item| match &item.kind {
                ItemKind::Function(f) if f.name.value == "main" => Some(f),
                _ => None,
            })
            .ok_or_else(|| SeenError::runtime_error("No main function found".to_string()))?;
        
        // Execute main function
        self.execute_function(main_fn)?;
        
        Ok(0)
    }
    
    fn execute_function(&mut self, function: &Function) -> SeenResult<Value> {
        // Create new scope for function
        self.locals.push(HashMap::new());
        
        let result = self.execute_block(&function.body);
        
        // Pop function scope
        self.locals.pop();
        
        result
    }
    
    fn execute_block(&mut self, block: &Block) -> SeenResult<Value> {
        let mut last_value = Value::Unit;
        
        for stmt in &block.statements {
            last_value = self.execute_statement(stmt)?;
            
            // Check for early return
            if matches!(stmt.kind, StmtKind::Return(_)) {
                return Ok(last_value);
            }
        }
        
        Ok(last_value)
    }
    
    fn execute_statement(&mut self, stmt: &Stmt) -> SeenResult<Value> {
        match &stmt.kind {
            StmtKind::Expr(expr) => self.evaluate_expression(expr),
            StmtKind::Return(opt_expr) => {
                if let Some(expr) = opt_expr {
                    self.evaluate_expression(expr)
                } else {
                    Ok(Value::Unit)
                }
            }
            StmtKind::Let(let_stmt) => {
                let value = if let Some(init) = &let_stmt.initializer {
                    self.evaluate_expression(init)?
                } else {
                    Value::Unit
                };
                
                // Store in current scope
                if let Some(scope) = self.locals.last_mut() {
                    if let seen_parser::PatternKind::Identifier(name) = &let_stmt.pattern.kind {
                        scope.insert(name.value.to_string(), value);
                    }
                }
                
                Ok(Value::Unit)
            }
            _ => Ok(Value::Unit),
        }
    }
    
    fn evaluate_expression(&mut self, expr: &Expr) -> SeenResult<Value> {
        match &*expr.kind {
            ExprKind::Literal(lit) => self.evaluate_literal(lit),
            ExprKind::Identifier(name) => {
                // Look up variable in scopes (local first, then global)
                for scope in self.locals.iter().rev() {
                    if let Some(value) = scope.get(name.value) {
                        return Ok(value.clone());
                    }
                }
                
                if let Some(value) = self.globals.get(name.value) {
                    Ok(value.clone())
                } else {
                    Err(SeenError::runtime_error(format!("Undefined variable: {}", name.value)))
                }
            }
            ExprKind::Call { function, args, .. } => {
                // Handle built-in functions
                if let ExprKind::Identifier(name) = function.kind.as_ref() {
                    match name.value {
                        "print" => {
                            for arg in args {
                                let value = self.evaluate_expression(arg)?;
                                print!("{}", value.to_string());
                            }
                            io::stdout().flush().unwrap();
                            Ok(Value::Unit)
                        }
                        "println" => {
                            for arg in args {
                                let value = self.evaluate_expression(arg)?;
                                print!("{}", value.to_string());
                            }
                            println!();
                            Ok(Value::Unit)
                        }
                        _ => Err(SeenError::runtime_error(format!("Unknown function: {}", name.value)))
                    }
                } else {
                    Err(SeenError::runtime_error("Complex function calls not yet supported".to_string()))
                }
            }
            ExprKind::Binary { left, op, right } => {
                let left_val = self.evaluate_expression(left)?;
                let right_val = self.evaluate_expression(right)?;
                
                use seen_parser::BinaryOp;
                match (left_val, right_val, op) {
                    (Value::Integer(l), Value::Integer(r), BinaryOp::Add) => Ok(Value::Integer(l + r)),
                    (Value::Integer(l), Value::Integer(r), BinaryOp::Sub) => Ok(Value::Integer(l - r)),
                    (Value::Integer(l), Value::Integer(r), BinaryOp::Mul) => Ok(Value::Integer(l * r)),
                    (Value::Integer(l), Value::Integer(r), BinaryOp::Div) => {
                        if r == 0 {
                            Err(SeenError::runtime_error("Division by zero".to_string()))
                        } else {
                            Ok(Value::Integer(l / r))
                        }
                    }
                    (Value::Float(l), Value::Float(r), BinaryOp::Add) => Ok(Value::Float(l + r)),
                    (Value::Float(l), Value::Float(r), BinaryOp::Sub) => Ok(Value::Float(l - r)),
                    (Value::Float(l), Value::Float(r), BinaryOp::Mul) => Ok(Value::Float(l * r)),
                    (Value::Float(l), Value::Float(r), BinaryOp::Div) => {
                        if r == 0.0 {
                            Err(SeenError::runtime_error("Division by zero".to_string()))
                        } else {
                            Ok(Value::Float(l / r))
                        }
                    }
                    _ => Err(SeenError::runtime_error(format!("Unsupported binary operation")))
                }
            }
            _ => Err(SeenError::runtime_error("Expression type not yet supported".to_string()))
        }
    }
    
    fn evaluate_literal(&self, lit: &Literal) -> SeenResult<Value> {
        use seen_parser::LiteralKind;
        match &lit.kind {
            LiteralKind::Integer(i) => Ok(Value::Integer(*i)),
            LiteralKind::Float(f) => Ok(Value::Float(*f)),
            LiteralKind::String(s) => Ok(Value::String(s.to_string())),
            LiteralKind::Boolean(b) => Ok(Value::Boolean(*b)),
            _ => Err(SeenError::runtime_error("Literal type not yet supported".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_parser::Parser;
    
    #[test]
    fn test_simple_hello_world() {
        let code = r#"
            fun main() {
                println("Hello, World!")
            }
        "#;
        
        let config = seen_lexer::LanguageConfig::new_english();
        let tokens = seen_lexer::Lexer::new(code, 0, &config).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.execute(&program);
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
    
    #[test]
    fn test_arithmetic() {
        let code = r#"
            fun main() {
                let x = 5
                let y = 3
                println(x + y)
            }
        "#;
        
        let config = seen_lexer::LanguageConfig::new_english();
        let tokens = seen_lexer::Lexer::new(code, 0, &config).tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse_program().unwrap();
        
        let mut interpreter = Interpreter::new();
        let result = interpreter.execute(&program);
        
        assert!(result.is_ok());
    }
}