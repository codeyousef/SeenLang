# Comprehensive Parser Test Specification (Phase 1.2)

## Overview
This document specifies comprehensive test cases for the Seen language parser, covering all grammar constructs and error scenarios.

## Current Test Coverage Analysis

### Existing Tests:
- **Unit Tests (7)**: Basic parsing, if/while/return statements, blocks, expressions
- **Integration Tests (10)**: Hello World, control flow, expressions, error handling
- **Missing**: Struct literal parsing (known issue), pattern matching, comprehensive error recovery

## Test Categories

### 1. Declaration Tests

#### 1.1 Function Declarations
```rust
// Test cases to implement:
- Simple function: func main() {}
- Function with parameters: func add(a: int, b: int) -> int
- Function with complex return type: func getData() -> [string]
- Generic function: func identity<T>(x: T) -> T
- Nested functions (if supported)
- Function overloading (if supported)
```

#### 1.2 Variable Declarations
```rust
// Test cases:
- Immutable: val x = 10
- Mutable: var y = 20
- With type annotation: val z: string = "hello"
- Array declaration: val arr: [int] = [1, 2, 3]
- Struct type: val point: Point = Point { x: 10, y: 20 }
```

#### 1.3 Struct Declarations
```rust
// Test cases:
- Empty struct: struct Empty {}
- Simple struct: struct Point { x: int, y: int }
- Nested struct types: struct Line { start: Point, end: Point }
- Generic struct: struct Box<T> { value: T }
- Struct with methods (if supported)
```

#### 1.4 Enum Declarations
```rust
// Test cases:
- Simple enum: enum Color { Red, Green, Blue }
- Enum with data: enum Option<T> { Some(T), None }
- Complex enum: enum Result<T, E> { Ok(T), Err(E) }
```

### 2. Statement Tests

#### 2.1 Control Flow Statements
```rust
// If statements:
- if x > 0 { ... }
- if x > 0 { ... } else { ... }
- if x > 0 { ... } else if x < 0 { ... } else { ... }
- Nested if statements

// While loops:
- while x > 0 { ... }
- while true { break; }
- Nested while loops

// For loops:
- for x in 0..10 { ... }
- for item in array { ... }
- for (index, value) in array.enumerate() { ... }
```

#### 2.2 Jump Statements
```rust
// Test cases:
- return;
- return 42;
- break;
- continue;
- break with label (if supported)
```

#### 2.3 Pattern Matching
```rust
// Match/when expressions:
- match x { 1 => "one", 2 => "two", _ => "other" }
- when color { Red => 0xFF0000, Green => 0x00FF00, Blue => 0x0000FF }
- Pattern guards: match x { n if n > 0 => "positive" }
- Destructuring: match point { Point { x, y } => x + y }
```

### 3. Expression Tests

#### 3.1 Literal Expressions
```rust
// All literal types:
- Integer: 42, 0xFF, 0b1010, 0o755
- Float: 3.14, 1e10, .5, 5.
- String: "hello", "escape\n", r"raw"
- Boolean: true, false
- Null: null
- Array: [1, 2, 3]
- Struct: Point { x: 10, y: 20 }
```

#### 3.2 Binary Expressions
```rust
// Arithmetic: a + b, a - b, a * b, a / b, a % b
// Comparison: a < b, a > b, a <= b, a >= b
// Equality: a == b, a != b
// Logical: a && b, a || b
// Bitwise: a & b, a | b, a ^ b, a << b, a >> b
// Assignment: a = b, a += b, a -= b
```

#### 3.3 Unary Expressions
```rust
// Test cases:
- Negation: -x
- Logical not: !x
- Bitwise not: ~x
- Pre/post increment: ++x, x++ (if supported)
```

#### 3.4 Complex Expressions
```rust
// Call expressions: func(a, b, c)
// Method calls: object.method(args)
// Index access: array[index]
// Field access: struct.field
// Range: 0..10, 0..=10
// Type cast: x as int
// Ternary: condition ? true_val : false_val
```

### 4. Type Tests

#### 4.1 Type Annotations
```rust
// Simple types: int, float, string, bool
// Array types: [int], [[string]]
// Optional types: int?, string?
// Generic types: Vec<T>, Map<K, V>
// Function types: (int, int) -> int
// Struct types: Point, Line
// Union types: int | string (if supported)
```

### 5. Error Recovery Tests

#### 5.1 Syntax Error Recovery
```rust
// Missing semicolons
// Missing closing braces
// Invalid tokens
// Incomplete statements
// Multiple errors in one file
```

#### 5.2 Error Messages
```rust
// Clear error messages with:
- Exact error location
- Expected vs found tokens
- Helpful suggestions
- Error codes
```

### 6. Position Tracking Tests

#### 6.1 AST Location Information
```rust
// Every AST node should have accurate:
- Start position (line, column)
- End position (line, column)
- Span calculation
- Multi-line construct handling
```

### 7. Bilingual Parsing Tests

#### 7.1 Arabic Keywords
```rust
// Test Arabic equivalents:
- دالة main() { ... }
- متغير x = 10
- إذا (condition) { ... }
- بينما (condition) { ... }
```

#### 7.2 Mixed Scripts
```rust
// Identifiers in different scripts
// Comments in different languages
// String literals with mixed content
```

### 8. Performance Tests

#### 8.1 Large File Parsing
```rust
// 10,000 line files
// Deeply nested structures
// Very long expressions
// Memory usage tracking
```

### 9. Property-Based Tests

#### 9.1 Parser Properties
```rust
use proptest::prelude::*;

proptest! {
    // Valid programs should parse without errors
    // Parser should never panic
    // AST should be deterministic
    // Pretty-print -> parse roundtrip
}
```

### 10. Integration with Other Components

#### 10.1 Lexer Integration
```rust
// Token stream handling
// Error propagation
// Position preservation
```

#### 10.2 Type Checker Integration
```rust
// AST structure compatibility
// Type annotation preservation
```

## Implementation Plan

### Phase A: Core Grammar (Week 1)
1. Complete struct literal parsing fix
2. Implement enum parsing
3. Add pattern matching support
4. Comprehensive expression tests

### Phase B: Error Handling (Week 2)
1. Error recovery mechanisms
2. Detailed error messages
3. Position tracking accuracy
4. Multiple error reporting

### Phase C: Advanced Features (Week 3)
1. Generic type parsing
2. Bilingual keyword tests
3. Property-based tests
4. Performance benchmarks

## Success Criteria

- **Correctness**: All valid programs parse correctly
- **Error Handling**: Clear, actionable error messages
- **Recovery**: Parser recovers from errors gracefully
- **Performance**: Parse 100K lines/second
- **Coverage**: 95%+ code coverage
- **Robustness**: No panics on any input