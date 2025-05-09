# Code Generation Test Specifications (MVP)

This document specifies the test cases for the code generation phase of the Seen compiler. The tests verify that the compiler correctly translates Seen AST to LLVM IR and produces working executables.

## Test Strategy

The code generation tests are structured in three levels:

1. **Unit Tests**: Test individual code generation functions in isolation
2. **Integration Tests**: Test the complete code generation process for small programs
3. **End-to-End Tests**: Test the entire compilation pipeline from source to executable

## Unit Test Specifications

### 1. Type Conversion Tests

**Objective**: Verify that Seen types are correctly mapped to LLVM types.

**Test Cases**:
- Test conversion of all primitive types (int, float, bool, string)
- Test conversion of array types
- Test handling of function types
- Test error cases (undefined types)

**Example**:
```rust
#[test]
fn test_primitive_type_conversion() {
    let context = Context::create();
    let type_system = TypeSystem::new(&context);
    
    // Test int type
    let int_type = type_system.convert_type(&ast::Type::Simple("int".to_string())).unwrap();
    assert!(int_type.is_int_type());
    
    // Test float type
    let float_type = type_system.convert_type(&ast::Type::Simple("float".to_string())).unwrap();
    assert!(float_type.is_float_type());
    
    // Test bool type
    let bool_type = type_system.convert_type(&ast::Type::Simple("bool".to_string())).unwrap();
    assert!(bool_type.is_int_type());
    assert_eq!(bool_type.into_int_type().get_bit_width(), 1);
    
    // Test string type
    let string_type = type_system.convert_type(&ast::Type::Simple("string".to_string())).unwrap();
    assert!(string_type.is_pointer_type());
}
```

### 2. Literal Expression Tests

**Objective**: Verify that literal expressions generate the correct LLVM constants.

**Test Cases**:
- Test integer literals
- Test floating-point literals
- Test string literals
- Test boolean literals
- Test null literals

**Example**:
```rust
#[test]
fn test_integer_literal() {
    let context = Context::create();
    let module = context.create_module("test");
    let builder = context.create_builder();
    
    let mut codegen = CodeGenerator::new(&context, &module, &builder).unwrap();
    
    let literal = ast::LiteralExpression::Number(ast::NumberLiteral {
        value: "42".to_string(),
        is_float: false,
        location: dummy_location(),
    });
    
    let value = codegen.generate_literal_expression(&literal).unwrap();
    
    assert!(value.is_int_value());
    assert_eq!(value.into_int_value().get_zero_extended_constant().unwrap(), 42);
}
```

### 3. Binary Operation Tests

**Objective**: Verify that binary operations generate the correct LLVM instructions.

**Test Cases**:
- Test arithmetic operations (add, subtract, multiply, divide, modulo)
- Test comparison operations (equal, not equal, less than, greater than, etc.)
- Test logical operations (and, or)
- Test operations with different types
- Test error cases (incompatible types)

**Example**:
```rust
#[test]
fn test_binary_add() {
    // Setup
    let context = Context::create();
    let module = context.create_module("test");
    let builder = context.create_builder();
    
    let mut codegen = CodeGenerator::new(&context, &module, &builder).unwrap();
    
    // Create two integer literals: 5 and 10
    let left = create_int_literal(5);
    let right = create_int_literal(10);
    
    // Create a binary expression: 5 + 10
    let expr = ast::Expression::Binary(ast::BinaryExpression {
        left: Box::new(left),
        operator: ast::BinaryOperator::Add,
        right: Box::new(right),
        location: dummy_location(),
    });
    
    // Generate code for the expression
    let value = codegen.generate_expression(&expr).unwrap();
    
    // Verify the result is an integer and equals 15
    assert!(value.is_int_value());
    assert_eq!(value.into_int_value().get_zero_extended_constant().unwrap(), 15);
}
```

### 4. Variable Declaration Tests

**Objective**: Verify that variable declarations generate the correct LLVM alloca instructions and stores.

**Test Cases**:
- Test immutable variable declarations (val)
- Test mutable variable declarations (var)
- Test declarations with explicit types
- Test declarations with type inference
- Test global variables vs. local variables

**Example**:
```rust
#[test]
fn test_local_variable_declaration() {
    // Setup
    let context = Context::create();
    let module = context.create_module("test");
    let builder = context.create_builder();
    
    // Create a function in which to create variables
    let function_type = context.void_type().fn_type(&[], false);
    let function = module.add_function("test_function", function_type, None);
    let basic_block = context.append_basic_block(function, "entry");
    builder.position_at_end(basic_block);
    
    let mut codegen = CodeGenerator::new(&context, &module, &builder).unwrap();
    
    // Create a variable declaration: var x: int = 42;
    let var_decl = ast::VariableDeclaration {
        is_mutable: true,
        name: "x".to_string(),
        var_type: Some(ast::Type::Simple("int".to_string())),
        initializer: Box::new(create_int_literal(42)),
        location: dummy_location(),
    };
    
    // Generate code for the variable declaration
    codegen.generate_variable_declaration(&var_decl).unwrap();
    
    // Verify the variable exists in the environment
    assert!(codegen.environment.get("x").is_some());
    
    // Create an identifier expression to load the variable
    let ident = ast::Expression::Identifier(ast::IdentifierExpression {
        name: "x".to_string(),
        location: dummy_location(),
    });
    
    // Generate code to load the variable
    let value = codegen.generate_expression(&ident).unwrap();
    
    // Verify the loaded value is 42
    assert!(value.is_int_value());
    assert_eq!(value.into_int_value().get_zero_extended_constant().unwrap(), 42);
}
```

### 5. Function Declaration Tests

**Objective**: Verify that function declarations generate the correct LLVM functions.

**Test Cases**:
- Test functions with no parameters and void return type
- Test functions with parameters and return values
- Test recursive functions
- Test function calls

**Example**:
```rust
#[test]
fn test_function_declaration() {
    // Setup
    let context = Context::create();
    let module = context.create_module("test");
    let builder = context.create_builder();
    
    let mut codegen = CodeGenerator::new(&context, &module, &builder).unwrap();
    
    // Create a function declaration: func add(a: int, b: int) -> int { return a + b; }
    let func_decl = ast::FunctionDeclaration {
        name: "add".to_string(),
        parameters: vec![
            ast::Parameter {
                name: "a".to_string(),
                param_type: ast::Type::Simple("int".to_string()),
                location: dummy_location(),
            },
            ast::Parameter {
                name: "b".to_string(),
                param_type: ast::Type::Simple("int".to_string()),
                location: dummy_location(),
            },
        ],
        return_type: Some(ast::Type::Simple("int".to_string())),
        body: ast::Block {
            statements: vec![
                ast::Statement::Return(ast::ReturnStatement {
                    value: Some(Box::new(ast::Expression::Binary(ast::BinaryExpression {
                        left: Box::new(ast::Expression::Identifier(ast::IdentifierExpression {
                            name: "a".to_string(),
                            location: dummy_location(),
                        })),
                        operator: ast::BinaryOperator::Add,
                        right: Box::new(ast::Expression::Identifier(ast::IdentifierExpression {
                            name: "b".to_string(),
                            location: dummy_location(),
                        })),
                        location: dummy_location(),
                    }))),
                    location: dummy_location(),
                }),
            ],
            location: dummy_location(),
        },
        location: dummy_location(),
    };
    
    // Generate code for the function declaration
    let function = codegen.generate_function(&func_decl).unwrap();
    
    // Verify the function has been created
    assert!(module.get_function("add").is_some());
}
```

### 6. Control Flow Tests

**Objective**: Verify that control flow statements generate the correct LLVM basic blocks and branches.

**Test Cases**:
- Test if statements with then branch only
- Test if statements with both then and else branches
- Test while loops
- Test nested control flow statements

## Integration Test Specifications

### 1. Basic Program Tests

**Objective**: Test complete small programs to verify the integration of various language features.

**Test Cases**:
- Hello World program
- Program with variable declarations and assignments
- Program with function definitions and calls
- Program with control flow statements

**Example**:
```rust
#[test]
fn test_hello_world_program() {
    // Create AST for: func main() { println("Hello, World!"); }
    let program = create_hello_world_ast();
    
    // Generate LLVM IR
    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "test_module").unwrap();
    let module = codegen.generate(&program).unwrap();
    
    // Verify the module contains the main function
    assert!(module.get_function("main").is_some());
    
    // Verify the module contains a call to printf
    assert!(module.get_function("printf").is_some());
}
```

### 2. Expression Evaluation Tests

**Objective**: Test the evaluation of complex expressions.

**Test Cases**:
- Arithmetic expressions with multiple operators and precedence
- Boolean expressions with logical operators
- Mixed-type expressions
- Expressions with function calls

### 3. Type Conversion Tests

**Objective**: Test implicit and explicit type conversions.

**Test Cases**:
- Integer to float conversion
- Float to integer conversion
- Boolean to integer conversion
- String operations

## End-to-End Test Specifications

### 1. Compilation Tests

**Objective**: Test the complete compilation pipeline from Seen source code to executable.

**Test Cases**:
- Compile and run Hello World
- Compile and run a program with arithmetic operations
- Compile and run a program with control flow
- Compile and run a program with functions

**Test Procedure**:
1. Parse Seen source code to AST
2. Generate LLVM IR from AST
3. Compile LLVM IR to object file
4. Link object file to create executable
5. Run executable and verify output

### 2. Error Handling Tests

**Objective**: Test that the compiler handles errors gracefully during code generation.

**Test Cases**:
- Type errors (e.g., trying to add a string and an integer)
- Undefined variable errors
- Undefined function errors
- Return type mismatch errors

### 3. Optimization Tests

**Objective**: Test that the compiler correctly applies optimizations.

**Test Cases**:
- Constant folding
- Dead code elimination
- Common subexpression elimination
- Loop optimizations

## Test Helpers

### Utility Functions

```rust
// Create a dummy location for testing
fn dummy_location() -> ast::Location {
    ast::Location::new(
        ast::Position::new(1, 1),
        ast::Position::new(1, 2),
    )
}

// Create an integer literal AST node
fn create_int_literal(value: i64) -> ast::Expression {
    ast::Expression::Literal(ast::LiteralExpression::Number(ast::NumberLiteral {
        value: value.to_string(),
        is_float: false,
        location: dummy_location(),
    }))
}

// Create a floating-point literal AST node
fn create_float_literal(value: f64) -> ast::Expression {
    ast::Expression::Literal(ast::LiteralExpression::Number(ast::NumberLiteral {
        value: value.to_string(),
        is_float: true,
        location: dummy_location(),
    }))
}

// Create a Hello World program AST
fn create_hello_world_ast() -> ast::Program {
    // Implementation details...
}
```

## Test Matrix

| Feature                | Unit Tests | Integration Tests | End-to-End Tests |
|------------------------|------------|-------------------|------------------|
| Type Conversion        | ✓          | ✓                 |                  |
| Literal Expressions    | ✓          | ✓                 |                  |
| Binary Operations      | ✓          | ✓                 |                  |
| Unary Operations       | ✓          | ✓                 |                  |
| Variable Declarations  | ✓          | ✓                 | ✓                |
| Function Declarations  | ✓          | ✓                 | ✓                |
| Control Flow           | ✓          | ✓                 | ✓                |
| Function Calls         | ✓          | ✓                 | ✓                |
| Print Function         | ✓          | ✓                 | ✓                |
| Error Handling         | ✓          |                   | ✓                |
| Optimizations          |            | ✓                 | ✓                |
