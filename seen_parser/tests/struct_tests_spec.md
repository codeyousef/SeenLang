# Struct Feature Test Specification

## Overview
This specification defines the test requirements for struct support in the Seen language, enabling the creation of custom data types with named fields.

## Grammar Requirements

### Struct Declaration Syntax
```
StructDeclaration ::= ('struct' | 'هيكل') Identifier '{' FieldList? '}'
FieldList ::= Field (',' Field)*
Field ::= Identifier ':' Type
```

### Bilingual Support
- English keyword: `struct`
- Arabic keyword: `هيكل` (haykal - structure)

## Test Categories

### 1. Basic Struct Parsing Tests
- **Simple struct**: `struct Point { x: int, y: int }`
- **Empty struct**: `struct Empty {}`
- **Single field**: `struct Container { value: string }`
- **Mixed types**: `struct Mixed { name: string, age: int, active: bool }`

### 2. Complex Type Field Tests
- **Array fields**: `struct Data { items: [int], names: [string] }`
- **Nested structs**: `struct Address { street: string } struct Person { addr: Address }`
- **Optional fields**: `struct Config { debug: bool, timeout: int? }`

### 3. Bilingual Keyword Tests
- **Arabic struct**: `هيكل نقطة { x: int, y: int }`
- **Mixed syntax**: Arabic keywords with English field names
- **Full Arabic**: Arabic keywords and identifiers where possible

### 4. Struct Instantiation Tests
- **Field access**: `person.name`, `point.x`
- **Struct literals**: `Point { x: 10, y: 20 }`
- **Nested access**: `person.address.street`

### 5. Type System Integration
- **Type checking**: Validate field types match declarations
- **Type inference**: Infer struct types from literals
- **Error reporting**: Clear errors for undefined fields, type mismatches

### 6. Error Handling Tests
- **Syntax errors**: Missing braces, semicolons in field list
- **Duplicate fields**: `struct Bad { x: int, x: string }`
- **Invalid types**: Non-existent type references
- **Circular references**: Struct containing itself directly

## Test Implementation Requirements

### Unit Tests Location
- `seen_parser/src/tests/struct_parsing_test.rs`
- `seen_typechecker/src/tests/struct_type_test.rs`
- `seen_interpreter/src/tests/struct_runtime_test.rs`

### Integration Tests Location
- `seen_parser/tests/struct_integration_test.rs`
- `tests/e2e/struct_e2e_test.rs`

### Property-Based Tests
- Generate random struct definitions and validate parsing
- Test struct field access with various nesting levels
- Validate type checking with randomized field types

## Success Criteria

### Parsing Success
- All valid struct syntax parses correctly
- Invalid syntax produces appropriate error messages
- Bilingual keywords work consistently

### Type Checking Success
- Struct types are properly validated
- Field access type checks correctly
- Error messages are clear and helpful

### Runtime Success
- Struct instantiation works correctly
- Field access returns correct values
- Memory safety is maintained

## Performance Requirements
- Struct parsing should not add more than 10% overhead to parser performance
- Type checking overhead should be minimal
- Runtime struct access should be constant time O(1)

## Compatibility Requirements
- Must work with existing language features (functions, variables, expressions)
- Should integrate cleanly with array types
- Must maintain bilingual keyword support consistently