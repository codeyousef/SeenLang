# Production Lexer Specification

## Performance Requirements
- >10M tokens/second throughput
- <1MB memory overhead for 100K LOC files
- Error recovery without stopping
- Precise location tracking

## Token Types Supported
### Literals
- Integer: decimal, hexadecimal, binary, octal
- Float: decimal, scientific notation
- String: regular, raw, interpolated, multiline
- Character: single quotes with escape sequences
- Boolean: true/false

### Identifiers and Keywords
- Unicode identifiers (full Unicode support)
- Contextual keywords (soft keywords)
- Reserved keywords (hard keywords)

### Operators
- Arithmetic: +, -, *, /, %, **
- Comparison: ==, !=, <, >, <=, >=
- Logical: &&, ||, !
- Bitwise: &, |, ^, ~, <<, >>
- Assignment: =, +=, -=, *=, /=, %=, &=, |=, ^=, <<=, >>=
- Special: ?., ?:, ::, ->, =>, ..., ..<

### Delimiters
- Parentheses: ( )
- Brackets: [ ]
- Braces: { }
- Angle brackets: < > (for generics)

### Comments
- Line comments: //
- Block comments: /* */
- Doc comments: ///
- Module comments: //!

### Special Features
- Template literals with interpolation
- Raw strings for regex/paths
- Multiline strings
- Unicode escape sequences
- Hex/binary/octal literals

## Error Recovery Strategy
1. Synchronize on statement boundaries
2. Track unmatched delimiters
3. Continue after lexical errors
4. Report all errors with context

## Implementation Plan
1. Core tokenizer with state machine
2. Location tracking with spans
3. Error recovery mechanism
4. Unicode support
5. Performance optimizations