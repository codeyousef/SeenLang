# Seen (س) Programming Language

Seen (س) is a modern systems programming language designed with bilingual support (English and Arabic) and robust memory safety features.

## Project Overview

Seen aims to significantly simplify the development of safe, performant, and concurrent software by offering:

- Performance comparable to Rust and C++
- A more intuitive developer experience around memory safety
- First-class support for both English and Arabic programming keywords
- Memory safety without garbage collection
- Modern syntax inspired by Kotlin but adapted for systems programming

### Core Language Features

- **Native Bilingualism**: Full support for both English and Arabic keywords and identifiers
- **Automated Memory Safety**: Static analysis that prevents memory errors without requiring explicit lifetime annotations
- **Fearless Concurrency**: Thread-safety enforced by the compiler
- **Modern Syntax**: Expressive, readable code with zero-cost abstractions
- **Versatile Applications**: Suitable for systems programming, data science, machine learning, and backend services

## Repository Structure

The Seen compiler is structured as a set of modular components:

- `seen_lexer` - Lexical analyzer with bilingual keyword support
- `seen_parser` - Parser for generating abstract syntax trees
- `seen_compiler` - Main compiler component
- `seen_ir` - Intermediate representation
- `seen_cli` - Command-line tooling

Supporting directories:
- `specifications` - Formal language definitions, grammar, and schemas
- `docs` - Documentation and learning resources

## Current Implementation Status

The following components have been implemented:

- ✅ Core syntax definition (EBNF grammar)
- ✅ Bilingual keyword system with separate English and Arabic language files
- ✅ Lexical analyzer with full Unicode support
- ✅ Project configuration infrastructure
- ✅ Learning modules for Rust basics and compiler concepts

## Usage Example

Seen code can be written with either English or Arabic keywords:

**English:**
```
func main() {
    val greeting = "Hello, World!";
    println(greeting);
}
```

**Arabic:**
```
دالة رئيسية() {
    ثابت تحية = "مرحباً، يا عالم!";
    اطبع(تحية);
}
```

## Development Plan

The Seen language is being developed in phases:

1. **Phase 0**: Project setup and learning materials
2. **Phase 1**: Foundational "Hello World" with bilingual support
3. **Phase 2**: Achieving self-hosting (a Seen compiler written in Seen)

## Contributing

The Seen language project welcomes contributions, particularly in these areas:

- Compiler implementation
- Language design refinements
- Documentation and learning materials
- Test cases and examples

## License

[Insert license information here]

## Acknowledgements

Seen is inspired by modern programming languages like Rust, Kotlin, and Swift, while addressing the need for inclusive programming languages that support non-English speakers, particularly Arabic language users.
