//! Main interpreter implementation for the Seen programming language

use std::collections::HashMap;
use seen_parser::ast::*;
use seen_lexer::token::Location;
use crate::value::Value;
use crate::runtime::Runtime;
use crate::errors::InterpreterError;
use crate::InterpreterResult;

/// The main interpreter for Seen programs
pub struct Interpreter {
    /// Runtime environment
    runtime: Runtime,
}

impl Interpreter {
    /// Create a new interpreter
    pub fn new() -> Self {
        Self {
            runtime: Runtime::new(),
        }
    }

    /// Interpret a complete program
    pub fn interpret_program(&mut self, program: &Program) -> InterpreterResult {
        let mut result = InterpreterResult::new();

        for declaration in &program.declarations {
            match self.interpret_declaration(declaration) {
                Ok(value) => {
                    if let Some(v) = value {
                        result.set_value(v);
                    }
                }
                Err(error) => {
                    result.add_error(error);
                    break; // Stop on first error
                }
            }
        }

        result
    }

    /// Interpret a declaration
    fn interpret_declaration(&mut self, declaration: &Declaration) -> Result<Option<Value>, InterpreterError> {
        match declaration {
            Declaration::Function(func_decl) => {
                self.interpret_function_declaration(func_decl)
            }
            Declaration::Variable(var_decl) => {
                self.interpret_variable_declaration(var_decl)
            }
        }
    }

    /// Interpret a function declaration
    fn interpret_function_declaration(&mut self, func_decl: &FunctionDeclaration) -> Result<Option<Value>, InterpreterError> {
        // Create function value
        let parameters: Vec<String> = func_decl.parameters.iter()
            .map(|p| p.name.clone())
            .collect();

        let function_value = Value::Function {
            name: func_decl.name.clone(),
            parameters,
            body: Box::new(func_decl.body.clone()),
            closure: HashMap::new(), // TODO: Capture closure variables
        };

        // Define the function in the runtime
        self.runtime.define_variable(func_decl.name.clone(), function_value);
        Ok(None)
    }

    /// Interpret a variable declaration
    fn interpret_variable_declaration(&mut self, var_decl: &VariableDeclaration) -> Result<Option<Value>, InterpreterError> {
        // Evaluate the initializer
        let value = self.interpret_expression(&var_decl.initializer)?;
        
        // Define the variable in the runtime
        self.runtime.define_variable(var_decl.name.clone(), value);
        Ok(None)
    }

    /// Interpret a statement
    fn interpret_statement(&mut self, statement: &Statement) -> Result<Option<Value>, InterpreterError> {
        match statement {
            Statement::Expression(expr_stmt) => {
                let value = self.interpret_expression(&expr_stmt.expression)?;
                Ok(Some(value))
            }
            Statement::Block(block) => {
                self.interpret_block(block)
            }
            Statement::Return(return_stmt) => {
                self.interpret_return_statement(return_stmt)
            }
            Statement::If(if_stmt) => {
                self.interpret_if_statement(if_stmt)
            }
            Statement::While(while_stmt) => {
                self.interpret_while_statement(while_stmt)
            }
            Statement::Print(print_stmt) => {
                self.interpret_print_statement(print_stmt)
            }
            Statement::DeclarationStatement(decl) => {
                self.interpret_declaration(decl)
            }
        }
    }

    /// Interpret a block statement
    fn interpret_block(&mut self, block: &Block) -> Result<Option<Value>, InterpreterError> {
        // Push new environment
        self.runtime.push_environment(false);

        let mut last_value = None;
        for statement in &block.statements {
            match self.interpret_statement(statement) {
                Ok(value) => {
                    if let Some(v) = value {
                        last_value = Some(v);
                    }
                }
                Err(error) => {
                    // Pop environment before returning error
                    let _ = self.runtime.pop_environment();
                    return Err(error);
                }
            }
        }

        // Pop environment
        self.runtime.pop_environment()
            .map_err(|e| InterpreterError::runtime(e, Location::from_positions(0, 0, 0, 0)))?;

        Ok(last_value)
    }

    /// Interpret a return statement
    fn interpret_return_statement(&mut self, return_stmt: &ReturnStatement) -> Result<Option<Value>, InterpreterError> {
        let value = if let Some(expr) = &return_stmt.value {
            self.interpret_expression(expr)?
        } else {
            Value::Unit
        };

        self.runtime.set_return_value(value.clone())
            .map_err(|e| InterpreterError::runtime(e, return_stmt.location))?;

        Ok(Some(value))
    }

    /// Interpret an if statement
    fn interpret_if_statement(&mut self, if_stmt: &IfStatement) -> Result<Option<Value>, InterpreterError> {
        let condition = self.interpret_expression(&if_stmt.condition)?;

        if condition.is_truthy() {
            self.interpret_statement(&if_stmt.then_branch)
        } else if let Some(else_branch) = &if_stmt.else_branch {
            self.interpret_statement(else_branch)
        } else {
            Ok(None)
        }
    }

    /// Interpret a while statement
    fn interpret_while_statement(&mut self, while_stmt: &WhileStatement) -> Result<Option<Value>, InterpreterError> {
        let mut last_value = None;

        loop {
            let condition = self.interpret_expression(&while_stmt.condition)?;
            if !condition.is_truthy() {
                break;
            }

            match self.interpret_statement(&while_stmt.body) {
                Ok(value) => {
                    if let Some(v) = value {
                        last_value = Some(v);
                    }
                }
                Err(error) => return Err(error),
            }
        }

        Ok(last_value)
    }

    /// Interpret a print statement
    fn interpret_print_statement(&mut self, print_stmt: &PrintStatement) -> Result<Option<Value>, InterpreterError> {
        for arg in &print_stmt.arguments {
            let value = self.interpret_expression(arg)?;
            self.runtime.println(&value)
                .map_err(|e| InterpreterError::runtime(e, print_stmt.location))?;
        }
        Ok(Some(Value::Unit))
    }

    /// Interpret an expression
    fn interpret_expression(&mut self, expression: &Expression) -> Result<Value, InterpreterError> {
        match expression {
            Expression::Literal(literal) => {
                Ok(self.interpret_literal_expression(literal))
            }
            Expression::Identifier(ident) => {
                self.runtime.get_variable(&ident.name)
                    .map_err(|e| InterpreterError::runtime(e, ident.location))
            }
            Expression::Binary(binary) => {
                self.interpret_binary_expression(binary)
            }
            Expression::Unary(unary) => {
                self.interpret_unary_expression(unary)
            }
            Expression::Call(call) => {
                self.interpret_call_expression(call)
            }
            Expression::Assignment(assignment) => {
                self.interpret_assignment_expression(assignment)
            }
            Expression::Parenthesized(paren) => {
                self.interpret_expression(&paren.expression)
            }
        }
    }

    /// Interpret a literal expression
    fn interpret_literal_expression(&self, literal: &LiteralExpression) -> Value {
        match literal {
            LiteralExpression::Number(num) => {
                if num.is_float {
                    if let Ok(f) = num.value.parse::<f64>() {
                        Value::Float(f)
                    } else {
                        Value::Float(0.0)
                    }
                } else {
                    if let Ok(i) = num.value.parse::<i64>() {
                        Value::Integer(i)
                    } else {
                        Value::Integer(0)
                    }
                }
            }
            LiteralExpression::String(s) => Value::String(s.value.clone()),
            LiteralExpression::Boolean(b) => Value::Boolean(b.value),
            LiteralExpression::Null(_) => Value::Null,
        }
    }

    /// Interpret a binary expression
    fn interpret_binary_expression(&mut self, binary: &BinaryExpression) -> Result<Value, InterpreterError> {
        let left = self.interpret_expression(&binary.left)?;
        let right = self.interpret_expression(&binary.right)?;

        let result = match binary.operator {
            BinaryOperator::Add => left.add(&right),
            BinaryOperator::Subtract => left.subtract(&right),
            BinaryOperator::Multiply => left.multiply(&right),
            BinaryOperator::Divide => left.divide(&right),
            BinaryOperator::Modulo => {
                // TODO: Implement modulo
                Err("Modulo not yet implemented".to_string())
            }
            BinaryOperator::Equal => Ok(Value::Boolean(left.equals(&right))),
            BinaryOperator::NotEqual => Ok(Value::Boolean(!left.equals(&right))),
            BinaryOperator::LessThan => left.less_than(&right),
            BinaryOperator::GreaterThan => right.less_than(&left),
            BinaryOperator::LessEqual => {
                match left.less_than(&right) {
                    Ok(Value::Boolean(lt)) => Ok(Value::Boolean(lt || left.equals(&right))),
                    Ok(_) => Err("Invalid comparison result".to_string()),
                    Err(e) => Err(e),
                }
            }
            BinaryOperator::GreaterEqual => {
                match right.less_than(&left) {
                    Ok(Value::Boolean(lt)) => Ok(Value::Boolean(lt || left.equals(&right))),
                    Ok(_) => Err("Invalid comparison result".to_string()),
                    Err(e) => Err(e),
                }
            }
            BinaryOperator::And => {
                Ok(Value::Boolean(left.is_truthy() && right.is_truthy()))
            }
            BinaryOperator::Or => {
                Ok(Value::Boolean(left.is_truthy() || right.is_truthy()))
            }
        };

        result.map_err(|e| InterpreterError::runtime(e, binary.location))
    }

    /// Interpret a unary expression
    fn interpret_unary_expression(&mut self, unary: &UnaryExpression) -> Result<Value, InterpreterError> {
        let operand = self.interpret_expression(&unary.operand)?;

        let result = match unary.operator {
            UnaryOperator::Negate => operand.negate(),
            UnaryOperator::Not => Ok(operand.logical_not()),
            UnaryOperator::Plus => Ok(operand), // Unary plus is a no-op
        };

        result.map_err(|e| InterpreterError::runtime(e, unary.location))
    }

    /// Interpret a function call
    fn interpret_call_expression(&mut self, call: &CallExpression) -> Result<Value, InterpreterError> {
        // Handle built-in functions
        if call.callee == "println" {
            if call.arguments.len() != 1 {
                return Err(InterpreterError::argument_count_mismatch(
                    call.callee.clone(),
                    1,
                    call.arguments.len(),
                    call.location,
                ));
            }
            let arg = self.interpret_expression(&call.arguments[0])?;
            self.runtime.println(&arg)
                .map_err(|e| InterpreterError::runtime(e, call.location))?;
            return Ok(Value::Unit);
        }

        // Get function from runtime
        let function = self.runtime.get_variable(&call.callee)
            .map_err(|e| InterpreterError::runtime(e, call.location))?;

        if let Value::Function { name, parameters, body, closure: _ } = function {
            // Check argument count
            if call.arguments.len() != parameters.len() {
                return Err(InterpreterError::argument_count_mismatch(
                    name,
                    parameters.len(),
                    call.arguments.len(),
                    call.location,
                ));
            }

            // Evaluate arguments
            let mut arg_values = Vec::new();
            for arg in &call.arguments {
                arg_values.push(self.interpret_expression(arg)?);
            }

            // Push call frame
            self.runtime.push_call(name.clone(), call.location)
                .map_err(|e| InterpreterError::runtime(e, call.location))?;

            // Push function environment
            self.runtime.push_environment(true);

            // Bind parameters
            for (param, arg_value) in parameters.iter().zip(arg_values) {
                self.runtime.define_variable(param.clone(), arg_value);
            }

            // Execute function body
            let result = self.interpret_block(&body);

            // Get return value if any
            let return_value = self.runtime.get_return_value().cloned();

            // Pop function environment and call frame
            let _ = self.runtime.pop_environment();
            let _ = self.runtime.pop_call();

            match result {
                Ok(_) => Ok(return_value.unwrap_or(Value::Unit)),
                Err(error) => Err(error),
            }
        } else {
            Err(InterpreterError::type_error(
                format!("'{}' is not a function", call.callee),
                call.location,
            ))
        }
    }

    /// Interpret an assignment expression
    fn interpret_assignment_expression(&mut self, assignment: &AssignmentExpression) -> Result<Value, InterpreterError> {
        let value = self.interpret_expression(&assignment.value)?;
        
        self.runtime.set_variable(&assignment.name, value.clone())
            .map_err(|e| InterpreterError::runtime(e, assignment.location))?;

        Ok(value)
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use seen_parser::Parser;
    use seen_lexer::Lexer;

    fn create_test_keyword_manager() -> seen_lexer::KeywordManager {
        use seen_lexer::keyword_config::KeywordConfig;
        use std::path::PathBuf;
        
        // Get the specifications directory relative to the workspace root
        let specs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent() // Go up from seen_interpreter crate root to workspace root
            .unwrap()
            .join("specifications");
        
        let keyword_config = KeywordConfig::from_directory(&specs_dir)
            .expect("Failed to load keyword configuration for testing");
        
        seen_lexer::KeywordManager::new(keyword_config, "en".to_string())
            .expect("Failed to create KeywordManager for testing")
    }
    
    fn parse_and_interpret(source: &str) -> InterpreterResult {
        let keyword_manager = create_test_keyword_manager();
        let mut lexer = Lexer::new(source, &keyword_manager, "en".to_string());
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let program = parser.parse().unwrap();
        
        let mut interpreter = Interpreter::new();
        interpreter.interpret_program(&program)
    }

    #[test]
    fn test_simple_arithmetic() {
        let source = r#"val x = 5 + 3;
val y = x * 2;"#;
        
        let result = parse_and_interpret(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_variable_declaration() {
        let source = r#"val greeting = "Hello, World!";"#;
        
        let result = parse_and_interpret(source);
        assert!(result.is_ok());
    }
}