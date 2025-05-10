# Seen Compiler-as-a-Library API Specification

**Version:** 0.1 (Initial Draft)

## 1. Introduction

This document specifies the Application Programming Interface (API) for using the Seen compiler's components as a library. This API is crucial for building tools like the Seen Language Server (`seen-lsp`), linters, formatters, and potentially IDE plugins or REPLs that require access to the compiler's internal stages and data structures.

**Goals:**

*   Provide stable, well-documented interfaces to the compiler's core functionalities.
*   Enable programmatic access to lexical analysis, parsing, Abstract Syntax Tree (AST) construction, semantic analysis, and diagnostic information.
*   Support Seen's bilingual features transparently through the API.
*   Ensure the API is suitable for use in potentially concurrent environments (like an LSP server).

## 2. Overall Architecture

The Seen compiler will be structured to expose its main phases as distinct modules or services that can be invoked independently or as part of a pipeline.

A central `CompilerService` or `ProjectContext` object will likely serve as the main entry point. This service would manage project-level configurations (like active language from `seen.toml`), source file management, and caching of intermediate results (e.g., parsed ASTs).

```mermaid
graph TD
    Tooling[Tooling (e.g., seen-lsp, seen-fmt)] -->|Requests| CompilerLibAPI[Compiler-as-a-Library API (Facade/Service)]
    CompilerLibAPI --> LexerModule[Lexer Module]
    CompilerLibAPI --> ParserModule[Parser Module]
    CompilerLibAPI --> SemanticAnalyzerModule[Semantic Analyzer Module]
    CompilerLibAPI --> IRGeneratorModule[IR Generator Module (Optional)]
    CompilerLibAPI --> ProjectConfig[Project Configuration Access]

    LexerModule -->|Token Stream| ParserModule
    ParserModule -->|AST| SemanticAnalyzerModule
    SemanticAnalyzerModule -->|Typed AST, Diagnostics| Tooling
    SemanticAnalyzerModule -->|Typed AST| IRGeneratorModule
```

## 3. Core API Components

### 3.1. Project Context / Compiler Service

*   **Purpose:** Manages project-wide settings and orchestrates compilation stages for one or more source files.
*   **Key Operations:**
    *   `new(project_root_path: Path) -> Result<Self, ConfigError>`: Initializes the service for a given project, loading `seen.toml`.
    *   `set_active_language(language: String)`: Override or set the active language (English/Arabic) for subsequent operations.
    *   `get_source_file(file_path: Path) -> Result<SourceFileHandle, FileError>`: Retrieves a handle to a source file, potentially loading it from disk or an in-memory cache.
    *   `get_ast(file_path: Path) -> Result<AstHandle, CompilationError>`: Parses a source file and returns a handle to its AST. This would internally trigger lexing and parsing.
    *   `get_typed_ast(file_path: Path) -> Result<TypedAstHandle, CompilationError>`: Performs semantic analysis on a parsed AST and returns a handle to the type-checked AST, along with diagnostics.
    *   `get_diagnostics(file_path: Path) -> Vec<Diagnostic>`: Returns all diagnostics (errors, warnings) for a given file after relevant stages have been run.
    *   `on_document_change(file_path: Path, new_content: String)`: Informs the compiler service about in-memory changes to a file (e.g., from an editor buffer), allowing it to update its internal state.

### 3.2. Source File Representation (`SourceFileHandle`)

*   **Purpose:** Represents a single source file.
*   **Key Information (Accessible via methods):**
    *   `path() -> Path`
    *   `get_content() -> String` (or `Rope` for efficient updates)
    *   `get_line_map() -> LineMap` (for converting byte offsets to line/column numbers)

### 3.3. Lexer API

*   **Purpose:** Provides access to the token stream for a given source file.
*   **Key Operations (Likely used internally by `get_ast` but could be exposed):**
    *   `tokenize(source_content: String, language_config: LanguageConfig) -> Result<Vec<Token>, LexerError>`
*   **`Token` Structure:**
    ```seen
    // Illustrative
    struct Token {
        token_type: TokenType, // Language-neutral token type (e.g., Identifier, KeywordVal, LiteralInteger)
        lexeme: String,      // The actual text segment (e.g., "myVar", "val", "قيمة", "123")
        span: TextSpan,      // Start and end position in the source file
        language: Language,  // Which language this lexeme belongs to (English/Arabic), if applicable
    }
    ```

### 3.4. Parser API (`AstHandle`)

*   **Purpose:** Provides access to the Abstract Syntax Tree (AST) of a parsed file.
*   **`AstHandle` Key Operations/Properties:**
    *   `get_root_node() -> AstNode`
    *   `find_node_at_span(span: TextSpan) -> Option<AstNode>`
    *   `get_syntax_errors() -> Vec<SyntaxError>`
*   **`AstNode` Structure:**
    *   Will be an enum representing different kinds of syntax nodes (e.g., `FunctionDeclaration`, `VariableDeclaration`, `ExpressionStatement`, `BinaryExpression`).
    *   Each variant will contain relevant child nodes and associated data (e.g., names, types if available after semantic analysis).
    *   Nodes will have `TextSpan` information.
    *   The AST structure will be formally defined (e.g., in `ast.rs` or a dedicated spec).

### 3.5. Semantic Analyzer API (`TypedAstHandle`)

*   **Purpose:** Provides access to the AST after semantic checks (name resolution, type checking) and type inference.
*   **`TypedAstHandle` Key Operations/Properties (Extends `AstHandle` conceptually):**
    *   `get_type_of_node(node_id: NodeId) -> Option<Type>`
    *   `get_definition_of_symbol_at_span(span: TextSpan) -> Option<DefinitionLocation>`
    *   `get_references_to_symbol_at_span(span: TextSpan) -> Vec<ReferenceLocation>`
    *   `get_semantic_diagnostics() -> Vec<SemanticError>` (part of `CompilerService.get_diagnostics`)
*   **`Type` Structure:** Represents Seen types (e.g., `Int`, `Float`, `String`, `Array<T>`, `CustomStruct{...}`).

### 3.6. Diagnostics

*   **`Diagnostic` Structure:**
    ```seen
    // As per specifications/schemas/diagnostic_message.schema.json (conceptual)
    struct Diagnostic {
        severity: Severity, // Error, Warning, Info, Hint
        code: Option<String>, // Unique error code (e.g., "E001", "W010")
        message_template_id: String, // ID for a localizable message template
        message_args: Vec<String>, // Arguments for the template
        rendered_message: String, // Pre-rendered, localized message for convenience
        span: TextSpan, // Location of the diagnostic
        suggestions: Vec<CodeActionSuggestion> // Potential fixes
    }
    ```

## 4. Configuration

*   **Project Configuration (`seen.toml`):** The `CompilerService` will read project-specific settings from `seen.toml`, including the default active language.
*   **API-Level Configuration:** Some API calls might allow overriding certain configurations for that specific call (e.g., providing source content directly instead of a file path, overriding language for a specific tokenization request if needed for special tools).

## 5. Error Handling

*   API functions will return `Result<T, E>` types, where `E` is a specific error enum for that module/operation (e.g., `ConfigError`, `FileError`, `LexerError`, `ParserError`, `SemanticError`, `CompilationError` as a general wrapper).
*   Errors will contain detailed information, including spans where possible.

## 6. Threading and Concurrency

*   The `CompilerService` and its associated handles (`SourceFileHandle`, `AstHandle`, `TypedAstHandle`) must be designed with concurrency in mind, especially if used by an LSP server that handles multiple requests concurrently.
*   **Immutable Data Structures:** ASTs and type information, once computed, should be largely immutable to facilitate safe sharing across threads.
*   **Synchronization:** Internal mutable state within the `CompilerService` (e.g., caches, file content) must be properly synchronized (e.g., using Mutexes, RwLocks from `std::sync`).
*   Operations like `on_document_change` must correctly invalidate and trigger re-computation of relevant cached data in a thread-safe manner.

## 7. Bilingual Support

*   The API should abstract away the complexities of bilingualism where possible. For example, `TokenType` will be language-neutral.
*   However, `Token` structures will retain the original lexeme and an indicator of its source language (English/Arabic). This is important for tools that might need to display or work with the original source text.
*   AST nodes representing identifiers should store the identifier as it appeared in the source, potentially alongside a canonical representation if needed for name resolution.
*   Diagnostic messages must be localizable, and the `Diagnostic` structure will support this (e.g., via `message_template_id` and `rendered_message`).

## 8. Open Questions

*   Exact structure of `AstNode` enum and its variants.
*   Detailed API for querying symbol tables and scope information.
*   Strategy for incremental parsing and semantic analysis (essential for LSP performance).
*   API for accessing IR if tools need to go beyond semantic analysis (e.g., for advanced refactoring or inspection tools).
*   Versioning strategy for the compiler-as-a-library API to ensure stability for tool developers.

This specification provides a foundational design for the Seen compiler's library API. Details will be refined during the implementation of the compiler and the `seen-lsp`.
