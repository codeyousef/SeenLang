# Seen Enhanced Error Handling Specification

**Version:** 0.2 (Extends initial design document concepts)

## 1. Introduction

This document specifies the enhanced error handling mechanisms for the Seen language, building upon the foundational `Result<T, E>` type and `panic` mechanism. The goal is to provide a robust, ergonomic, and informative error handling system that supports developers in building reliable software.

Key enhancements include a standard error trait, support for custom error types, error chaining, and detailed specification for panic and diagnostic localization.

## 2. Core Error Handling Primitives Recap

*   **`Result<T, E>` Enum:** For recoverable errors. `Ok(T)` for success, `Err(E)` for failure.
    *   The `?` operator will be supported for propagating `Err` values.
*   **`panic` / `هلع` Keyword:** For unrecoverable errors (programming bugs, broken invariants). Leads to stack unwinding and program/thread termination.

## 3. Standard Error Trait

Seen will define a standard `Error` trait in its standard library (e.g., `std::error::Error`). All types intended to be used as error values in the `E` position of `Result<T, E>` should implement this trait.

### 3.1. `std::error::Error` Trait Definition (Preliminary)

```seen
// In std::error module
public trait Error {
    // Provides a user-facing description of the error.
    // This should be localizable.
    func description() -> String;

    // Optionally, provides the underlying cause of this error, if any.
    // Used for error chaining.
    func source() -> Option<Ref<dyn Error>> { // Ref<dyn Error> refers to a reference to a trait object
        return Option.None;
    }

    // Optionally, provides a unique error code or identifier.
    // Useful for programmatic error handling or external tooling.
    // func error_code() -> Option<String> { // Or an enum type for codes
    //     return Option.None;
    // }
}
```
*(Note: `Ref<dyn Error>` syntax for trait object reference is illustrative. `error_code()` is a potential addition.)*

### 3.2. Implementing `Error`

Developers can implement `Error` for their custom `struct` or `enum` types.

```seen
struct MyCustomError {
    message: String,
    internal_code: Int,
    cause: Option<Box<dyn Error>> // Example of storing a boxed error cause
}

impl std::error::Error for MyCustomError {
    func description() -> String {
        return this.message;
    }

    func source() -> Option<Ref<dyn Error>> {
        // 'as_ref_dyn_error' is a hypothetical method to get Ref<dyn Error> from Box<dyn Error>
        return this.cause.as_ref().map(|c| c.as_ref_dyn_error()); 
    }
}
```

## 4. Custom Error Types

*   Developers are encouraged to define specific `enum` or `struct` types for different error conditions in their libraries and applications.
*   These types should implement `std::error::Error`.
*   This allows for rich, structured error information beyond simple strings.

**Example:**
```seen
public enum NetworkError {
    ConnectionFailed(host: String, port: UInt16),
    Timeout(duration_ms: UInt32),
    DnsResolutionFailed(hostname: String),
    Generic(message: String, cause: Option<Box<dyn Error>>)
}

impl std::error::Error for NetworkError {
    func description() -> String {
        when this {
            is ConnectionFailed(host, port) -> return "Connection failed to ${host}:${port}";
            is Timeout(duration) -> return "Operation timed out after ${duration}ms";
            // ... other cases ...
            is Generic(message, _) -> return message;
        }
    }

    func source() -> Option<Ref<dyn Error>> {
        when this {
            is Generic(_, cause_opt) -> return cause_opt.as_ref().map(|c| c.as_ref_dyn_error());
            else -> return Option.None;
        }
    }
}
```

## 5. Error Chaining (Wrapping Errors)

*   The `source()` method in the `std::error::Error` trait enables error chaining.
*   When an error is caused by another error, the higher-level error can store and expose the original error.
*   This allows for a full causal chain of errors to be inspected, which is invaluable for debugging.
*   Error reporting utilities (e.g., for logging or displaying unhandled errors) should iterate through the `source()` chain to display all relevant information.

## 6. `panic` Mechanism Details

### 6.1. Behavior

*   Invoking `panic("message");` or `هلع("رسالة");` will initiate a panic.
*   The panic payload is typically a string, but could potentially be extended to other types that implement a specific `PanicPayload` trait (TBD).
*   Panics result in stack unwinding.

### 6.2. Stack Unwinding

*   During unwinding, the runtime will iterate up the call stack, calling destructors (or equivalent cleanup logic defined by Seen's memory model) for all stack-allocated objects that go out of scope.
*   This ensures resource cleanup (memory, file handles, locks, etc.) even during a panic, preventing leaks where possible.

### 6.3. Panic Propagation and Boundaries

*   By default, panics propagate across function calls.
*   **FFI Boundary:** Special consideration is needed for Foreign Function Interface (FFI) boundaries (e.g., when Seen code calls C code, or C code calls Seen code).
    *   **Seen calling C:** A panic originating in Seen code should NOT be allowed to unwind through a C stack frame. This would be undefined behavior. The Seen runtime must catch the panic at the boundary, print diagnostic information, and terminate the program (or the thread, if threads have isolated panic domains).
    *   **C calling Seen:** If a C function calls into Seen code that then panics, the panic must be caught at the boundary before returning to C. The Seen FFI layer should translate this into an error code or special return value that the C caller can understand. Unwinding a Seen panic into C code is forbidden.
*   **Thread Boundaries:** Panics are generally thread-local. A panic in one thread does not directly cause other threads to panic, but it will likely lead to the termination of the panicking thread. The main thread panicking will terminate the program.

### 6.4. Catching Panics (`catch_unwind` - TBD)

*   Seen might provide a mechanism similar to Rust's `std::panic::catch_unwind` to allow specific parts of code to catch a panic originating from a called function.
*   This is an advanced feature and should be used sparingly, typically for:
    *   FFI boundaries (as part of the implementation detail).
    *   Frameworks or libraries that need to isolate plugins or user-submitted code.
    *   Preventing a single task failure from bringing down an entire server (though structured concurrency might be a better approach here).
*   If implemented, `catch_unwind` would return a `Result` indicating whether the code panicked or completed successfully.

## 7. Diagnostics and Localization

### 7.1. Diagnostic Message Structure

Compiler and runtime error messages (including panic messages) should be structured for clarity and localization. As per the Design Document (`diagnostic_message_format.schema.json`), this would involve:

*   **Error Code:** A unique, stable identifier for the error type.
*   **Message Template:** A template string with placeholders for dynamic values.
*   **Source Spans:** Precise location(s) in the source code related to the error.
*   **Severity:** (Error, Warning, Hint).
*   **Suggestions:** Potential fixes or next steps.

### 7.2. Localization

*   Message templates will be translated into supported languages (e.g., English, Arabic).
*   The `description()` method of the `std::error::Error` trait should return localized strings based on the active locale settings.
*   Panic messages, if generated from string literals, will be in the language of that literal. If generated by the runtime for specific conditions, they should be localizable.
*   The system for selecting the active language for diagnostics will be defined by the project configuration (`seen.toml`) and environment settings.

### 7.3. Stack Trace Localization

*   Function names and file paths in stack traces that involve user-defined bilingual identifiers must be correctly represented.
*   Any diagnostic messages accompanying stack trace frames should also be localized.

## 8. Open Questions

*   Final design of the `std::error::Error` trait, including any helper methods or associated types.
*   Specifics of the `PanicPayload` concept if panics are to carry more than strings.
*   The exact API and semantics of a potential `catch_unwind` facility.
*   Detailed strategy for runtime selection and management of localization for error messages.
*   Integration of structured error types with the `?` operator (e.g., automatic conversion via a `From` trait).

This specification provides the groundwork for a comprehensive error handling system in Seen. Implementation will involve updates to the compiler, runtime, and standard library.
