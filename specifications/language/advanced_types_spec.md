# Seen Advanced Type System Features Specification

**Version:** 0.1 (Initial Draft)

## 1. Introduction

This document specifies advanced features of the Seen type system, building upon the core types. The primary focus is on Generics (Parametric Polymorphism) and Traits (Interfaces), which are essential for writing reusable, abstract, and type-safe code. This specification will also touch upon potential future enhancements like Higher-Kinded Types (HKTs) and Associated Types.

**Goals:**

*   Provide robust mechanisms for code abstraction and reuse.
*   Ensure full type safety at compile time.
*   Enable polymorphism through traits.
*   Maintain performance characteristics through mechanisms like monomorphization for generics.
*   Seamlessly integrate with Seen's existing features, including its memory model and bilingual capabilities.

## 2. Generics (Parametric Polymorphism)

Generics allow functions, data structures (`struct`, `enum`), and traits to be defined over one or more abstract type parameters. These type parameters are then instantiated with concrete types when the generic entity is used.

### 2.1. Syntax

*   **Generic Functions:**
    ```seen
    func identity<T>(item: T) -> T {
        return item;
    }

    func print_array<T>(arr: Array<T>) { /* ... */ }
    ```
*   **Generic Structs:**
    ```seen
    struct Point<T> {
        x: T,
        y: T
    }
    ```
*   **Generic Enums:**
    ```seen
    enum Option<T> {
        Some(value: T),
        None
    }

    enum Result<T, E> {
        Ok(value: T),
        Err(error: E)
    }
    ```
*   **Generic Traits:** (See section 3)
    ```seen
    trait Processor<TInput, TOutput> {
        func process(input: TInput) -> TOutput;
    }
    ```

### 2.2. Type Parameter Constraints (Trait Bounds)

Type parameters can be constrained to only accept types that implement specific traits. This allows generic functions to call methods from those traits on instances of the type parameter.

*   **Syntax:**
    ```seen
    // Single trait bound
    func display_debug<T: DebugFormat>(item: T) {
        print(item.debug_string());
    }

    // Multiple trait bounds
    func process_serializable_clonable<T: Serializable + Cloneable>(item: T) -> T { /* ... */ }

    // 'where' clause for complex bounds (if needed, TBD)
    // func complex_generic<T>(item: T)
    // where T: DebugFormat,
    //       T: Cloneable {
    //     /* ... */
    // }
    ```

### 2.3. Type Inference

*   The compiler will infer type arguments for generic functions from the types of the actual arguments passed during a call, where possible.
    ```seen
    val num = identity(5); // T inferred as Int
    val p = Point(x: 1.0, y: 2.0); // T for Point inferred as Float (or relevant float type)
    ```
*   Type annotations can be used to disambiguate or explicitly specify type arguments if inference fails or a specific instantiation is desired.
    ```seen
    val float_num = identity<Float>(5); // Explicitly T as Float
    ```

### 2.4. Compilation Strategy (Monomorphization)

*   Seen will primarily use **monomorphization** for compiling generic code.
*   **Process:** For each unique set of concrete type arguments used with a generic definition, the compiler will generate a specialized version of that code with the type parameters replaced by the concrete types.
*   **Advantages:** High runtime performance (no overhead from dynamic dispatch or boxing for generics), enables further optimizations on specialized code.
*   **Disadvantages:** Can lead to larger binary sizes if a generic function is instantiated with many different types. The compiler may employ techniques to mitigate this where possible (e.g., sharing equivalent code sections).

## 3. Traits (Interfaces)

Traits define a set of methods that a type can implement, providing a way to achieve abstraction and polymorphism. They define a contract for behavior.

### 3.1. Defining Traits

*   **Syntax:**
    ```seen
    trait Reader {
        // Abstract method (must be implemented by conforming types)
        func read(buffer: MutRef<ByteArray>) -> Result<Int, IoError>;

        // Method with a default implementation
        func read_all_to_string() -> Result<String, IoError> {
            var result_string = "";
            var buffer = ByteArray(1024);
            loop {
                val bytes_read = this.read(mutRef buffer)?;
                if bytes_read == 0 {
                    break;
                }
                result_string += string_from_utf8_bytes(buffer.slice(0, bytes_read));
            }
            return Ok(result_string);
        }
    }

    // Generic Trait
    trait Equality<Rhs = Self> {
        func equals(other: Rhs) -> Bool;
    }
    ```
    *(Note: `Rhs = Self` indicates a default type for a generic parameter, a feature to be confirmed.)*

### 3.2. Implementing Traits

*   **Syntax (`impl` keyword):**
    ```seen
    struct File {
        path: String,
        // ... other fields ...
    }

    impl Reader for File {
        func read(buffer: MutRef<ByteArray>) -> Result<Int, IoError> {
            // ... implementation for reading from a file ...
        }
    }

    // Implementing a generic trait
    struct MyInt { value: Int }
    impl Equality<MyInt> for MyInt {
        func equals(other: MyInt) -> Bool {
            return this.value == other.value;
        }
    }
    // Or with default generic parameter:
    // impl Equality for MyInt { ... }
    ```
*   All methods declared in the trait (without default implementations) must be implemented.
*   Default methods can be optionally overridden.

### 3.3. Using Traits

*   **As Type Bounds for Generics:** (Covered in section 2.2)
*   **Trait Objects (Dynamic Dispatch):** Traits can be used as types for variables, function parameters, or return types to enable dynamic dispatch.
    *   This typically involves a pointer to the data and a vtable (virtual method table) pointer.
    *   Syntax: `val reader_obj: Trait<Reader> = File(...);` (Exact syntax for trait objects TBD, could be `dyn Reader` or similar to Rust).
    *   **Object Safety:** Rules will define which traits can be made into trait objects (e.g., methods must be dispatchable; no `Self` in return types outside of specific patterns).

### 3.4. Method Resolution

*   Methods defined directly on a type take precedence over trait methods if names collide (details TBD).
*   The compiler must resolve which specific method implementation to call (static dispatch for generics, dynamic dispatch for trait objects).
*   Disambiguation syntax may be required if multiple traits implemented by a type define methods with the same name (`<MyType as MyTrait>::method()`).

### 3.5. Coherence and Orphan Rules

To maintain predictability and prevent conflicts, Seen will adopt coherence rules, similar to Rust's:

*   **Orphan Rule:** A trait implementation (`impl MyTrait for MyType`) is only allowed if either `MyTrait` or `MyType` is defined in the current crate (module/package).
    *   This prevents external crates from implementing traits for types they don't own in ways that could conflict globally.

### 3.6. Associated Types in Traits (Future Consideration)

*   Associated types allow a trait to declare placeholder types that implementing types must specify.
    ```seen
    // Hypothetical
    trait Iterator {
        type Item;
        func next() -> Option<Self.Item>;
    }
    ```
*   This is a powerful feature for creating generic abstractions where related types are determined by the implementer.

## 4. Higher-Kinded Types (HKTs) (Future Consideration)

*   HKTs allow abstraction over type constructors (e.g., abstracting over `Option<T>`, `Array<T>`, `Result<T, E>`).
*   Example: Defining a `Functor` trait that can be implemented for any `F<T>`.
    ```seen
    // Highly speculative syntax
    // trait Functor<F<_>> {
    //     func map<A, B>(fa: F<A>, func: (A) -> B) -> F<B>;
    // }
    ```
*   HKTs significantly increase the expressive power of the type system but also its complexity. They will be considered for later versions of Seen if strong use cases justify the complexity.

## 5. Interactions with Other Features

*   **Memory Management:** Monomorphized generic code will adhere to Seen's static memory safety rules. Trait objects might require specific handling by the memory model (e.g., regarding ownership of the pointed-to data).
*   **FFI:** Generics and traits are primarily Seen language features. Interaction with FFI (C interfaces) will likely require concrete types. Generic functions might not be directly callable via FFI without monomorphization and C-compatible signatures.
*   **Bilingualism:** Trait names, method names, and generic type parameter names can use Seen's bilingual identifier support. The underlying mechanisms are language-agnostic.

## 6. Open Questions

*   Exact syntax for trait objects (`dyn Trait`? `Trait<TraitName>`?).
*   Detailed rules for object safety.
*   Specifics of method resolution order and disambiguation syntax.
*   The extent of type inference capabilities with complex generic types and trait bounds.
*   Error reporting strategies for complex type errors involving generics and traits.
*   Viability and syntax for more advanced features like associated types and HKTs in future versions.

This specification will be refined as the implementation of generics and traits progresses in the Seen compiler.
