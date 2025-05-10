# Compiler Optimization: Constant Folding

**Version:** 0.1 (Initial Draft)

## 1. Introduction

Constant folding is a compiler optimization technique that evaluates constant expressions at compile time rather than computing them at runtime. If an expression's operands are all known constants, the expression can be replaced by its computed value, leading to smaller and faster code.

This document outlines the design for implementing constant folding in the self-hosted Seen compiler.

## 2. Goals

*   Reduce runtime computation by performing calculations at compile time.
*   Decrease code size by replacing constant expressions with their results.
*   Improve overall program performance.

## 3. Scope of Application

Constant folding can be applied to a variety of expressions and operations, including:

*   **Arithmetic Operations:**
    *   `2 + 3` -> `5`
    *   `10 * 5` -> `50`
    *   `7 / 2` -> `3` (for integer division)
    *   `7.0 / 2.0` -> `3.5`
    *   `10 % 3` -> `1`
*   **Bitwise Operations:**
    *   `0b1010 & 0b1100` -> `0b1000`
    *   `0b1010 | 0b0101` -> `0b1111`
    *   `0b1010 ^ 0b1100` -> `0b0110`
    *   `~0b1010` (assuming a fixed bit width) -> `...11110101`
    *   `0b101 << 2` -> `0b10100`
*   **Logical Operations (Boolean):**
    *   `true and false` -> `false`
    *   `true or false` -> `true`
    *   `not true` -> `false`
*   **Comparisons:**
    *   `5 > 3` -> `true`
    *   `10 == 20` -> `false`
*   **String Concatenation (if strings are immutable and values known):**
    *   `"hello" + " " + "world"` -> `"hello world"`
*   **Array/Tuple Indexing (if array/tuple and index are constant):**
    *   `val arr = [10, 20, 30]; val x = arr[1];` -> `val x = 20;` (if `arr` is a known constant array)
*   **Type Casts between Numeric Constants:**
    *   `(Float)5` -> `5.0`

Constant folding can also propagate constants through variable assignments if the variable is not reassigned and its value is used in subsequent constant expressions.

## 4. Interaction with Seen Intermediate Representation (IR)

Constant folding will operate on an Intermediate Representation (IR) of the Seen code, after semantic analysis and type checking have completed. The IR should clearly represent expressions, their operators, and operands, along with type information.

**Assumptions about the IR:**

*   Expressions are represented in a tree-like structure (e.g., an expression tree within a control flow graph).
*   Nodes in the IR have type information associated with them.
*   Literal constant values are clearly identifiable.

**Process:**

The optimization pass will traverse the IR. When an expression node is encountered:

1.  Check if all operands of the expression are compile-time constants.
2.  If yes, evaluate the expression using the constant operands and their types.
3.  Replace the expression node in the IR with a new node representing the computed constant value.
4.  The type of the resulting constant must be consistent with the original expression's type.

## 5. Algorithm Sketch

```
func fold_constants(ir_node) {
    // Post-order traversal (fold children first)
    for child in ir_node.children {
        fold_constants(child);
    }

    if ir_node is an Expression {
        if ir_node.operator is one_of(+, -, *, /, %, &, |, ^, <<, >>, ==, !=, <, >, <=, >=, and, or, not, string_concat, numeric_cast) {
            are_all_operands_constant = true;
            constant_operands = [];
            for operand in ir_node.operands {
                if operand is_not_a_compile_time_constant {
                    are_all_operands_constant = false;
                    break;
                }
                add operand.value to constant_operands;
            }

            if are_all_operands_constant {
                // Ensure type safety and handle potential errors (e.g., division by zero)
                // Perform the calculation based on ir_node.operator and constant_operands values and types
                try {
                    computed_value = evaluate(ir_node.operator, constant_operands, ir_node.type);
                    // Replace ir_node with a new IR node representing literal 'computed_value'
                    ir_node.replace_with_literal(computed_value, ir_node.type);
                } catch (EvaluationError e.g., DivisionByZero) {
                    // Do not fold if evaluation causes an error that should be runtime
                    // Or, if the language spec allows certain compile-time errors, report them.
                    // For now, assume we don't fold in case of error.
                }
            }
        }
    }
}

// The main optimization pass would iterate over all functions/modules in the IR.
```

**Considerations:**

*   **Floating-Point Precision:** Be mindful of differences in floating-point arithmetic between the compile-time environment and the target runtime environment. Generally, for standard IEEE 754, this should be consistent, but it's a known caveat.
*   **Division by Zero:** The compiler must handle division by zero. It could either refuse to fold (leaving it as a runtime error) or, if the language specification mandates, report a compile-time error.
*   **Overflow/Underflow:** Similar to division by zero, integer overflows should be handled according to the language specification (e.g., wrap, trap, or be undefined -- Seen should define this clearly. If defined as wrapping, folding should replicate that.).
*   **Side Effects:** Constant folding must not alter the program's semantics. It should only be applied to pure expressions without side effects.
*   **Order of Operations:** Evaluation must respect Seen's defined order of operations.

## 6. Benefits

*   **Improved Runtime Performance:** Fewer computations at runtime.
*   **Reduced Code Size:** Constant expressions are replaced by single values.
*   **Enables Further Optimizations:** Constant folding can expose further optimization opportunities, such as dead code elimination (if a conditional branch depends on a constant expression that folds to `true` or `false`).

## 7. Potential Limitations/Challenges

*   **Complexity:** While basic arithmetic folding is simple, handling all types and potential edge cases (like floating-point issues or string operations) can add complexity.
*   **Interaction with IR:** The effectiveness and ease of implementation depend heavily on the design of the IR.
*   **Compile Time:** Aggressive constant folding on very large expressions could potentially increase compile time, though typically the benefits outweigh this.

## 8. Open Questions

*   How should errors like division by zero or overflow during compile-time evaluation be handled? (Defer to main language spec for error semantics, but compiler must implement it).
*   What is the precise set of operations and types that Seen's constant folding will support in its initial implementation?
*   How will constant folding interact with more complex language features like macros or user-defined constant functions (if any)?

This document serves as an initial design. Details will be refined during the implementation of the optimization pass in the Seen compiler.
