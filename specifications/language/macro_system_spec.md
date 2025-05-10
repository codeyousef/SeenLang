# Seen Macro System Specification

**Version:** 0.1 (Initial Draft)

## 1. Introduction and Goals

This document specifies the design for Seen's metaprogramming capabilities, primarily focusing on a hygienic macro system. Macros allow developers to write code that writes code, enabling powerful abstractions, reducing boilerplate, and extending the language's syntax.

**Goals:**

*   **Hygienic by Default:** Prevent accidental variable capture and ensure macros are robust and predictable.
*   **Expressiveness:** Support common use cases such as:
    *   Custom `derive` for traits.
    *   Attribute-like macros for annotating declarations.
    *   Function-like macros for generating code snippets or DSLs.
*   **Integration with Seen's Features:** Seamless interaction with Seen's type system, module system, and bilingual keyword/identifier support.
*   **Developer Experience:** Provide clear syntax, understandable error messages for macro-related issues, and good tooling support (e.g., for LSP).
*   **Safety:** Ensure macros do not compromise Seen's core safety guarantees outside of explicitly `unsafe` operations within macro-generated code.

**Non-Goals:**

*   **Arbitrary Compile-Time Code Execution (Initially):** While procedural macros effectively are compile-time functions, the initial focus is on structured AST transformations rather than general-purpose computation at compile time (e.g., no file I/O or network access within macro expansion by default).
*   **Non-Hygienic Macros (Initially):** While some languages offer escape hatches for non-hygienic behavior, Seen will prioritize hygiene. Non-hygienic features might be considered later if strong use cases emerge.

## 2. Core Concepts

### 2.1. Hygiene

Hygiene is a cornerstone of Seen's macro system. It ensures that identifiers introduced by a macro (macro-internal variables) do not clash with identifiers in the code where the macro is invoked (invocation-site variables), and vice-versa.

*   **Alpha-Renaming:** Conceptually, all identifiers are given a unique 'color' or 'context' based on where they are defined (macro definition site vs. macro invocation site). This prevents name collisions.
*   **Implications:**
    *   A macro cannot accidentally shadow a variable from its invocation site.
    *   A variable from the invocation site cannot accidentally capture a variable used internally by the macro.
    *   This makes macros more modular and easier to reason about.

### 2.2. Macro Types

Seen will support several kinds of macros, likely leaning towards procedural macros due to their power and flexibility:

*   **Function-like Macros:** These macros look like function calls (e.g., `my_macro!(...)` or `my_macro(...)`). They take a stream of tokens as input and produce a new stream of tokens (which must parse into valid Seen code items) as output.
    *   *Example Use Cases:* Creating DSLs (e.g., `html!(...)`, `sql!(...)`), complex initializers, logging utilities.
*   **Derive Macros:** These are special attribute-like macros used with `#[derive(...)]` on `struct` and `enum` definitions. They generate implementations for traits.
    *   *Example Use Cases:* Automatically generating `toString()`, `equals()`, `hashCode()`, `Serializable`, or custom trait implementations.
*   **Attribute-like Macros:** These macros can be applied to various items (functions, structs, enums, modules, etc.) using the `#[my_attribute(...)]` syntax. They can transform the annotated item or generate auxiliary code.
    *   *Example Use Cases:* Conditional compilation, code transformation (e.g., adding logging, memoization), generating FFI bindings.

## 3. Syntax (Preliminary)

*(This section is highly preliminary and subject to change based on implementation feasibility and further design refinement. It aims to illustrate the concepts.)*

### 3.1. Defining Macros

Defining procedural macros in Seen will involve writing special Seen functions that operate on Abstract Syntax Tree (AST) representations or token streams. These macro-defining functions would themselves be annotated to register them as macros.

```seen
// Hypothetical syntax for defining a function-like macro
#[function_macro(name = "my_custom_log")]
// The macro function would take a TokenStream and return a TokenStream
// or operate on AST nodes provided by the compiler.
func define_my_custom_log_macro(input: compiler::TokenStream) -> compiler::TokenStream {
    // 1. Parse 'input' token stream (e.g., into arguments for the macro)
    // 2. Construct new Seen code as a TokenStream or AST nodes
    //    Example: Create a call to a logging function with the input expression
    // 3. Return the generated TokenStream
    let log_message = ... ; // process input
    return quote! { // 'quote!' is a hypothetical quasiquoting macro
        println!("LOG: " + ${log_message});
    };
}

// Hypothetical syntax for defining a derive macro
#[derive_macro(trait_name = "MyDebug")]
func define_my_debug_derive_macro(item_ast: compiler::ast::StructDefinition) -> compiler::TokenStream {
    // 1. Inspect 'item_ast' (fields, name, etc.)
    // 2. Construct an 'impl MyDebug for ${item_ast.name} { ... }' block
    // 3. Return the generated TokenStream
    let struct_name = item_ast.name;
    let field_names = item_ast.fields.map(|f| f.name);
    return quote! {
        impl MyDebug for ${struct_name} {
            func debug_format() -> String {
                // ... construct string from field_names and their values ...
            }
        }
    };
}
```

**Key Elements:**

*   **Macro Definition Functions:** Regular Seen functions with special annotations.
*   **Input/Output:** Macros typically take `TokenStream` or AST nodes as input and produce `TokenStream` as output.
*   **Quasiquoting:** A mechanism (e.g., a built-in `quote!(...)` macro) will be essential for constructing new Seen code easily within macro definitions.
*   **Compiler API:** A stable API (`compiler::TokenStream`, `compiler::ast::*`) will be provided for macros to interact with code representations.

### 3.2. Invoking Macros

*   **Function-like Macros:**
    ```seen
    my_custom_log!("An event occurred: " + event_details);
    // or potentially without the '!' if syntax allows unambiguous parsing:
    // my_custom_log("An event occurred: " + event_details);
    ```
*   **Derive Macros:**
    ```seen
    #[derive(MyDebug, PartialEq, SeenSerializable)]
    data struct User {
        id: Int,
        username: String
    }
    ```
*   **Attribute-like Macros:**
    ```seen
    #[route(GET, "/users/:id")]
    #[authorize(roles = ["admin"])]
    func get_user_details(id: Int) -> User { ... }
    ```

## 4. Macro Expansion Process

1.  **Discovery:** The compiler identifies macro definitions (e.g., through special annotations and potentially a distinct macro crate system).
2.  **Invocation Parsing:** When a macro invocation is encountered, the compiler parses the arguments/annotated item into a `TokenStream` or AST representation suitable for the macro.
3.  **Execution:** The corresponding macro definition function is executed by the compiler, passing it the input.
    *   This execution happens during compilation, not at runtime of the final program.
4.  **Output Substitution:** The `TokenStream` returned by the macro function is parsed by the compiler and replaces the original macro invocation site (or is added alongside the annotated item).
5.  **Recursive Expansion:** The output of a macro can itself contain further macro invocations, which are then expanded recursively until no macro invocations remain.
6.  **Semantic Analysis:** After all macros are expanded, the resulting code undergoes full semantic analysis (type checking, borrow checking, etc.) like any other Seen code.

## 5. Interaction with Seen's Features

### 5.1. Type System

*   Macros operate on syntax before full type checking. However, they can generate code that is then type-checked.
*   Macros can access limited type information if the compiler infrastructure supports it (e.g., resolving paths to types to generate correct type names in output).
*   Quasiquoting must correctly handle type paths and generic parameters.

### 5.2. Modules and Paths

*   Hygiene ensures that paths (to types, functions, modules) resolved within a macro definition refer to items visible *at the macro definition site* by default.
*   Paths passed *into* a macro as arguments are resolved at the *invocation site*.
*   Mechanisms might be needed for macros to generate code that correctly refers to items relative to the invocation site if necessary (e.g., `crate::foo` in generated code).

### 5.3. Bilingualism

*   Macro definitions themselves can be written using either English or Arabic Seen keywords, just like any Seen code.
*   Macros operate on a token stream or AST that is language-agnostic internally (keywords are canonicalized).
*   When a macro generates code (e.g., via quasiquoting), it generates canonical AST nodes or tokens. The final pretty-printing of this code by the compiler (e.g., for error messages or debug output) would respect the project's active language settings.
*   Macros should not need to be 'bilingual-aware' in their logic beyond standard identifier handling.

## 6. Error Reporting

Clear error reporting for macros is challenging but essential:

*   **Errors in Macro Definition:** Standard Seen compiler errors for the macro-defining code itself.
*   **Errors in Macro Invocation (Parsing Arguments):** Errors if the input to a macro doesn't match what it expects (e.g., wrong number/type of arguments).
*   **Errors in Macro-Generated Code:** When the code *produced* by a macro is invalid Seen code. The compiler should ideally attribute these errors to the original macro invocation site, or even to specific parts of the macro input that led to the erroneous output.
    *   Span information is crucial: mapping errors in expanded code back to the source code that invoked the macro.
*   **Panic in Macro Execution:** Macro definition functions can panic. The compiler must catch these panics and report them as errors tied to the macro invocation.

## 7. Limitations and Future Considerations

*   **Debugging Macros:** Debugging macro expansion can be complex. Tooling support (e.g., a macro expansion viewer) will be important.
*   **Compilation Time:** Heavy use of complex procedural macros can increase compilation times. The system should be efficient, and users should be aware of the trade-offs.
*   **Macro Crates:** A system for packaging and distributing macros as libraries (similar to Rust's `proc-macro` crates) will be necessary.
*   **AST Stability:** The stability of the `compiler::ast` API exposed to macros is a significant concern. Versioning or careful evolution of this API will be required.

## 8. Open Questions

*   The exact syntax for defining macros (annotations, function signatures).
*   The precise API provided to macro definitions (e.g., for manipulating `TokenStream`, AST nodes, querying limited type information).
*   Strategies for managing incremental compilation with macros.
*   Specifics of span tracking through multiple levels of macro expansion for diagnostics.
*   The design of a macro crate/packaging system.

This initial specification provides a foundation. Further details will be refined as implementation proceeds and more complex use cases are explored.
