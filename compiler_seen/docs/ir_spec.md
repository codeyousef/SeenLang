# Seen Intermediate Representation (IR) Specification

**Version:** 0.1 (Initial Draft)

## 1. Introduction

This document specifies the design for the Seen Intermediate Representation (IR). The IR is a crucial component of the Seen compiler, serving as the primary data structure upon which various analyses and optimizations are performed after parsing and semantic analysis (type checking) and before code generation.

**Goals of Seen IR:**

*   **Facilitate Optimization:** The IR should be designed to make it easy to implement a wide range of compiler optimizations (e.g., constant folding, dead code elimination, inlining, loop optimizations, register allocation).
*   **Language and Target Agnostic (to a degree):** While reflecting Seen's semantics, the IR should be abstract enough to potentially support different target architectures in the future and not be overly tied to the source language syntax.
*   **Explicit Control Flow:** Control flow should be represented explicitly, typically using a Control Flow Graph (CFG).
*   **Static Single Assignment (SSA) Form (Optional but Recommended):** Using SSA form can simplify many data-flow analyses and optimizations.
*   **Type Information:** The IR must carry type information for variables and operations to ensure type safety and enable type-based optimizations.
*   **Debug Information:** The IR should be able to carry information necessary for generating debug symbols, mapping IR instructions back to source code locations.
*   **Extensibility:** The design should allow for future extensions and additions of new instruction types or metadata.

## 2. Overall Structure

The Seen IR will be structured hierarchically:

*   **Module:** The top-level container, representing a single compilation unit (e.g., a Seen file or a collection of files being compiled together).
    *   Contains global variable declarations.
    *   Contains function definitions.
    *   Metadata (e.g., target architecture, source file names).
*   **Function:** Represents a single function or procedure.
    *   Signature (name, parameters with types, return type).
    *   Contains a list of Basic Blocks.
    *   Local variable allocations (e.g., stack slots for variables that are not in SSA registers).
*   **Basic Block:** A sequence of instructions with a single entry point (the first instruction) and a single exit point (the last instruction, which must be a terminator instruction).
    *   Instructions within a basic block are executed sequentially.
    *   Identified by a unique label.
    *   Links to predecessor and successor basic blocks, forming the CFG.
*   **Instruction:** The fundamental unit of computation.
    *   Each instruction has an opcode, operands (which can be constants, registers, or memory locations), and a result (typically assigned to a register).
    *   Instructions carry type information for their operands and results.
    *   Associated with source code location for debugging.

```mermaid
graph TD
    IRModule[IR Module] -->|Contains| GlobalVars[Global Variables]
    IRModule -->|Contains| Function1[Function A]
    IRModule -->|Contains| Function2[Function B]

    Function1 -->|Entry Block| BB1_A[Basic Block 1 (Entry)]
    Function1 --> BB2_A[Basic Block 2]
    Function1 --> BB3_A[Basic Block 3 (Exit)]

    BB1_A -->|Instruction List| Inst1_A1[Instruction 1.1]
    BB1_A --> Inst2_A1[Instruction 1.2]
    BB1_A --> Terminator1_A[Terminator (e.g., Branch)]

    BB2_A -->|Instruction List| Inst1_A2[Instruction 2.1]
    BB2_A --> Terminator2_A[Terminator (e.g., Conditional Branch)]
    
    Terminator1_A --> BB2_A
    Terminator2_A -- True --> BB1_A
    Terminator2_A -- False --> BB3_A
```

## 3. Static Single Assignment (SSA) Form

Seen IR will likely use Static Single Assignment (SSA) form. In SSA form:

*   Each variable (virtual register in IR terms) is assigned exactly once.
*   Variables that are assigned multiple times in the source code are split into different versions (e.g., `x_1`, `x_2`), each with a single assignment.
*   At points where different control flow paths merge, special `phi` (Φ) functions are used to select the correct version of a variable based on the path taken.

**Benefits of SSA:**

*   Simplifies many data-flow analyses and optimizations like constant propagation, dead code elimination, and register allocation.
*   Makes use-def chains explicit.

**Conversion to SSA:** This will be a distinct pass that transforms a non-SSA form of IR (perhaps generated directly from the AST) into SSA form.

## 4. Instruction Set (Illustrative)

The IR will have a defined set of instructions. This is a preliminary, illustrative list and will be expanded:

*   **Arithmetic Operations:**
    *   `add <type> <res>, <op1>, <op2>`
    *   `sub <type> <res>, <op1>, <op2>`
    *   `mul <type> <res>, <op1>, <op2>`
    *   `div <type> <res>, <op1>, <op2>` (signed/unsigned variants)
    *   `rem <type> <res>, <op1>, <op2>` (signed/unsigned variants)
    *   `neg <type> <res>, <op1>` (unary negation)
*   **Bitwise Operations:**
    *   `and <type> <res>, <op1>, <op2>`
    *   `or  <type> <res>, <op1>, <op2>`
    *   `xor <type> <res>, <op1>, <op2>`
    *   `shl <type> <res>, <op1>, <op2>` (shift left)
    *   `shr <type> <res>, <op1>, <op2>` (shift right, logical/arithmetic variants)
    *   `not <type> <res>, <op1>` (bitwise not)
*   **Memory Operations:**
    *   `alloc <type> <res_ptr>, <size_bytes>` (stack allocation for locals)
    *   `load <type> <res>, <ptr_op>` (load from memory)
    *   `store <type> <val_op>, <ptr_op>` (store to memory)
    *   `getElementPtr <type> <res_ptr>, <base_ptr>, <index1>, [index2...]` (address calculation for arrays/structs)
*   **Control Flow (Terminator Instructions):**
    *   `br <label_target_block>` (unconditional branch)
    *   `br_cond <cond_op>, <label_true_block>, <label_false_block>` (conditional branch)
    *   `ret <type> [val_op]` (return from function, optionally with a value)
    *   `unreachable` (indicates a point that should not be reached)
*   **Conversion Operations:**
    *   `cast_int_truncate <res_type> <res>, <op>`
    *   `cast_int_sign_extend <res_type> <res>, <op>`
    *   `cast_int_zero_extend <res_type> <res>, <op>`
    *   `cast_int_to_float <res_type> <res>, <op>`
    *   `cast_float_to_int <res_type> <res>, <op>`
    *   `cast_ptr_to_int <res_type> <res>, <op>`
    *   `cast_int_to_ptr <res_type> <res>, <op>`
*   **Function Calls:**
    *   `call <type> <res_opt>, <func_name_or_ptr>, (<type1> <arg1>, <type2> <arg2>, ...)`
*   **SSA Specific:**
    *   `phi <type> <res>, [<val1>, <label_pred1>], [<val2>, <label_pred2>], ...`
*   **Other:**
    *   `cmp <cond_type> <type> <res>, <op1>, <op2>` (comparison, e.g., eq, ne, lt, gt)

**Operands:**

*   **Constants:** e.g., `i32 10`, `f64 3.14`, `bool true`.
*   **Virtual Registers:** e.g., `%val1`, `%tmp_result`. In SSA form, each register is defined once.
*   **Memory Addresses:** Often held in registers that are pointers.
*   **Labels:** For basic blocks (e.g., `BB1`, `loop_header`).

## 5. Type System in IR

The IR must have its own type system, which mirrors or can represent all types from the Seen source language.

*   **Primitive Types:** `i8`, `i32`, `f64`, `bool`, `ptr` (generic pointer).
*   **Aggregate Types:**
    *   `struct { <type1>, <type2>, ... }`
    *   `array <size> x <element_type>`
*   **Function Types:** `(<param_type1>, <param_type2>, ...) -> <return_type>`

Type information is associated with every value (register, constant) and operation.

## 6. Generation of IR

The IR will be generated from the type-checked Abstract Syntax Tree (AST) produced by the semantic analysis phase.

*   Each AST node will be translated into one or more IR instructions.
*   Control flow structures from the AST (if/else, loops) will be lowered into basic blocks and branch instructions.
*   This initial IR might not be in SSA form and will require a subsequent SSA construction pass.

## 7. Debug Information

To support debugging, IR instructions should be associated with source location information (file, line, column) from the original Seen code. This allows debuggers to map executable code back to the source.

## 8. Representation

*   **In-Memory:** The compiler will use data structures (e.g., structs and enums in Rust or Seen) to represent modules, functions, basic blocks, and instructions in memory.
*   **Textual Form (Optional but useful for debugging):** A human-readable textual representation of the IR can be very helpful for debugging the compiler itself. LLVM IR's textual format is a good example.
    ```llvm
    ; Example of what a textual IR might look like (syntax TBD)
    define i32 @my_function(i32 %a, i32 %b) {
    entry:
      %sum = add i32 %a, %b
      %result = mul i32 %sum, i32 2
      ret i32 %result
    }
    ```

## 9. Open Questions & Future Considerations

*   Detailed design of `phi` node placement and SSA construction algorithm.
*   Handling of exceptions or panic/recover mechanisms in IR.
*   Memory model details (e.g., for garbage collection or ARC metadata if applicable).
*   Specific attributes for functions and instructions (e.g., `#[inline]`, `readonly`, `writeonly` for memory operations).
*   Support for SIMD operations or other target-specific intrinsics.
*   How will bilingualism in Seen source map to IR names (e.g., function names, variable debug names)? Typically, IR uses canonical, language-neutral identifiers.

This specification provides the foundational design for Seen's IR. It will be refined as the compiler implementation progresses.
