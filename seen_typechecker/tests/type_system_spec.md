# Type System Test Specification

This document specifies the comprehensive test suite for the Seen type checker. All tests should be written following TDD principles before implementing the corresponding functionality.

## Test Categories

### 1. Basic Type Checking

#### 1.1 Primitive Types
- **Test:** Type check integer literals
  - Input: `val x = 42`
  - Expected: Variable `x` has type `Int`
  
- **Test:** Type check float literals
  - Input: `val x = 3.14`
  - Expected: Variable `x` has type `Float`
  
- **Test:** Type check boolean literals
  - Input: `val x = true`
  - Expected: Variable `x` has type `Bool`
  
- **Test:** Type check string literals
  - Input: `val x = "hello"`
  - Expected: Variable `x` has type `String`

#### 1.2 Type Annotations
- **Test:** Explicit type annotations
  - Input: `val x: Int = 42`
  - Expected: Variable `x` has type `Int`, no type errors
  
- **Test:** Type annotation mismatch
  - Input: `val x: Int = "hello"`
  - Expected: Type error - cannot assign `String` to `Int`

### 2. Type Inference

#### 2.1 Variable Type Inference
- **Test:** Infer type from literal
  - Input: `val x = 42`
  - Expected: `x` inferred as `Int`
  
- **Test:** Infer type from expression
  - Input: `val x = 1 + 2`
  - Expected: `x` inferred as `Int`
  
- **Test:** Infer type from function call
  - Input: `val x = someFunction()`
  - Expected: `x` has return type of `someFunction`

#### 2.2 Complex Type Inference
- **Test:** Array type inference
  - Input: `val arr = [1, 2, 3]`
  - Expected: `arr` inferred as `Array<Int>`
  
- **Test:** Mixed array type error
  - Input: `val arr = [1, "hello", 3]`
  - Expected: Type error - inconsistent array element types

### 3. Function Type Checking

#### 3.1 Function Declarations
- **Test:** Function with explicit return type
  - Input: `func add(a: Int, b: Int) -> Int { return a + b }`
  - Expected: Function signature recorded, no errors
  
- **Test:** Function return type mismatch
  - Input: `func getValue() -> Int { return "hello" }`
  - Expected: Type error - returning `String` but expected `Int`

#### 3.2 Function Calls
- **Test:** Correct function call
  - Input: `add(1, 2)`
  - Expected: No type errors, result type is `Int`
  
- **Test:** Argument type mismatch
  - Input: `add("hello", 2)`
  - Expected: Type error - expected `Int` but got `String` for parameter `a`
  
- **Test:** Wrong number of arguments
  - Input: `add(1)`
  - Expected: Type error - function expects 2 arguments but got 1

### 4. Struct Type Checking

#### 4.1 Struct Declarations
- **Test:** Basic struct declaration
  - Input: `struct Point { x: Int, y: Int }`
  - Expected: Struct type `Point` registered with fields

#### 4.2 Struct Instantiation
- **Test:** Valid struct literal
  - Input: `val p = Point { x: 10, y: 20 }`
  - Expected: `p` has type `Point`, no errors
  
- **Test:** Missing struct field
  - Input: `val p = Point { x: 10 }`
  - Expected: Type error - missing field `y`
  
- **Test:** Extra struct field
  - Input: `val p = Point { x: 10, y: 20, z: 30 }`
  - Expected: Type error - unknown field `z`
  
- **Test:** Wrong field type
  - Input: `val p = Point { x: "hello", y: 20 }`
  - Expected: Type error - field `x` expects `Int` but got `String`

#### 4.3 Field Access
- **Test:** Valid field access
  - Input: `p.x`
  - Expected: Expression has type `Int`
  
- **Test:** Unknown field access
  - Input: `p.z`
  - Expected: Type error - struct `Point` has no field `z`

### 5. Array Type Checking

#### 5.1 Array Operations
- **Test:** Array indexing
  - Input: `arr[0]` where `arr: Array<Int>`
  - Expected: Expression has type `Int`
  
- **Test:** Array index type error
  - Input: `arr["hello"]`
  - Expected: Type error - array index must be `Int`

### 6. Control Flow Type Checking

#### 6.1 If Statements
- **Test:** If condition type check
  - Input: `if 42 { ... }`
  - Expected: Type error - condition must be `Bool`
  
- **Test:** If branches type consistency
  - Input: `val x = if cond { 1 } else { "hello" }`
  - Expected: Type error - branches have incompatible types

#### 6.2 While Loops
- **Test:** While condition type check
  - Input: `while "hello" { ... }`
  - Expected: Type error - condition must be `Bool`

#### 6.3 For Loops
- **Test:** For-in loop type check
  - Input: `for x in [1, 2, 3] { ... }`
  - Expected: `x` has type `Int` in loop body

### 7. Operator Type Checking

#### 7.1 Binary Operators
- **Test:** Arithmetic operators
  - Input: `1 + 2`
  - Expected: Valid, result type `Int`
  
- **Test:** Type mismatch in arithmetic
  - Input: `1 + "hello"`
  - Expected: Type error - cannot add `Int` and `String`
  
- **Test:** Comparison operators
  - Input: `1 < 2`
  - Expected: Valid, result type `Bool`

#### 7.2 Unary Operators
- **Test:** Negation operator
  - Input: `-42`
  - Expected: Valid, result type `Int`
  
- **Test:** Logical not
  - Input: `!true`
  - Expected: Valid, result type `Bool`

### 8. Type Compatibility

#### 8.1 Assignment Compatibility
- **Test:** Int to Float promotion
  - Input: `val x: Float = 42`
  - Expected: Valid, implicit conversion
  
- **Test:** Incompatible assignment
  - Input: `val x: Int = 3.14`
  - Expected: Type error - cannot assign `Float` to `Int`

#### 8.2 Optional Types
- **Test:** Assign to optional
  - Input: `val x: Int? = 42`
  - Expected: Valid, `Int` assignable to `Int?`
  
- **Test:** Null assignment
  - Input: `val x: Int? = null`
  - Expected: Valid

### 9. Scope and Variable Resolution

#### 9.1 Variable Scoping
- **Test:** Variable in scope
  - Input: `{ val x = 1; x + 1 }`
  - Expected: Valid
  
- **Test:** Variable out of scope
  - Input: `{ val x = 1; } x + 1`
  - Expected: Type error - undefined variable `x`

#### 9.2 Variable Shadowing
- **Test:** Inner scope shadowing
  - Input: `val x = 1; { val x = "hello"; }`
  - Expected: Valid, inner `x` has type `String`

### 10. Error Recovery

#### 10.1 Continue After Error
- **Test:** Multiple independent errors
  - Input: Program with multiple type errors
  - Expected: All errors reported, not just first one

#### 10.2 Error Cascading Prevention
- **Test:** Avoid cascading errors
  - Input: Code with initial type error
  - Expected: Related errors suppressed to avoid noise

## Performance Requirements

- Type checking should complete in < 100ms for programs up to 1000 lines
- Memory usage should be proportional to program size
- Error reporting should include precise location information

## Integration Test Scenarios

### Complete Program Type Checking
Test type checking of complete, realistic programs including:
- Multiple function definitions
- Struct definitions and usage
- Array operations
- Control flow
- Mixed expressions

### Bilingual Type Checking
Test that type checking works correctly with both English and Arabic keywords.

## Property-Based Tests

Use property-based testing for:
- Type inference consistency
- Type compatibility rules
- Error message generation
- Scope resolution