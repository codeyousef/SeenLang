# Compiler Optimization: Dead Code Elimination

**Version:** 0.1 (Initial Draft)

## 1. Introduction

Dead Code Elimination (DCE) is a compiler optimization that removes code that has no effect on the program's output. This includes code that is unreachable or code whose results are never used.

Removing dead code makes programs smaller and faster, and can also improve the effectiveness of other optimization passes.

This document outlines the design for implementing Dead Code Elimination in the self-hosted Seen compiler.

## 2. Goals

*   Identify and remove unreachable code segments.
*   Identify and remove computations whose results are unused (dead stores).
*   Reduce program size and improve runtime performance.
*   Improve clarity of the Intermediate Representation (IR) for subsequent optimization passes.

## 3. Types of Dead Code

### 3.1. Unreachable Code

Code that can never be executed, regardless of the program's input. This often occurs due to:

*   Conditional branches whose conditions are compile-time constants (e.g., after constant folding):
    ```seen
    if false {
        // This block is unreachable
        print("This will never print");
    }
    ```
*   Jumps that bypass sections of code unconditionally.
*   Functions that are never called (though this might also be handled by linker-level DCE or specific whole-program optimization passes).
*   Code after a `return` statement or a `panic` call within the same block.

### 3.2. Dead Stores / Useless Code

Code that computes values that are never used by any other part of the program. This includes:

*   Assignments to variables whose values are never read:
    ```seen
    func example() {
        val x = 10; // Dead store if x is not used later
        val y = 20;
        print(y); // x is not used
    }
    ```
*   Computations whose results are assigned to variables that are themselves dead.

## 4. Interaction with Seen Intermediate Representation (IR)

DCE operates on the IR, typically after other analyses and transformations like Control Flow Graph (CFG) construction and potentially liveness analysis.

**Required Analyses/Prerequisites:**

*   **Control Flow Graph (CFG):** Essential for identifying unreachable basic blocks. A basic block is unreachable if there is no path from the entry block of the function to it.
*   **Liveness Analysis (for Dead Stores):** To determine if a variable is live (its current value might be used in the future) at a particular point in the program. An assignment to a variable is a dead store if the variable is not live immediately after the assignment.
*   **Constant Folding:** Can create opportunities for DCE by simplifying conditional branches to constants (e.g., `if (true)` or `if (false)`).

**Process:**

1.  **Unreachable Code Elimination:**
    *   Build the CFG for each function.
    *   Perform a traversal (e.g., Depth First Search) starting from the function's entry block to identify all reachable basic blocks.
    *   Any basic block not visited during this traversal is unreachable and can be removed from the IR.
2.  **Dead Store Elimination (Iterative Approach):**
    *   Perform liveness analysis to determine live variables at each program point.
    *   Iterate through the IR instructions:
        *   If an instruction assigns a value to a variable `v` (e.g., `v = expression`), and `v` is not live immediately after this instruction, then the assignment is a dead store.
        *   The instruction can be removed if it has no other side effects (e.g., the `expression` part doesn't call a function that modifies global state or performs I/O).
    *   Removing a dead store might cause other variables (used in its `expression`) to become dead. Thus, DCE is often performed iteratively with liveness analysis until no more dead code can be found.

## 5. Algorithm Sketch

### 5.1. Unreachable Code Elimination

```
func eliminate_unreachable_code(function_ir) {
    cfg = function_ir.build_cfg();
    reachable_blocks = new Set();
    work_list = new Queue();

    work_list.enqueue(cfg.entry_block);
    reachable_blocks.add(cfg.entry_block);

    while work_list is not empty {
        current_block = work_list.dequeue();
        for successor_block in current_block.successors {
            if successor_block not in reachable_blocks {
                reachable_blocks.add(successor_block);
                work_list.enqueue(successor_block);
            }
        }
    }

    all_blocks = cfg.get_all_blocks();
    for block in all_blocks {
        if block not in reachable_blocks {
            function_ir.remove_block(block);
        }
    }
}
```

### 5.2. Dead Store Elimination (Simplified)

```
func eliminate_dead_stores(function_ir) {
    changed = true;
    while changed {
        changed = false;
        liveness_info = function_ir.analyze_liveness(); // Recompute liveness

        for block in function_ir.get_blocks_in_reverse_post_order() { // Process blocks in an order suitable for liveness
            for instruction in block.get_instructions_in_reverse_order() {
                if instruction is an assignment (e.g., v = expression) {
                    defined_variable = instruction.get_defined_variable(); // v
                    if not liveness_info.is_live_after(defined_variable, instruction) {
                        if not instruction.has_side_effects() { // Crucial check!
                            block.remove_instruction(instruction);
                            changed = true;
                        }
                    }
                }
            }
        }
    }
}
```
**Important Note on Side Effects:** Identifying side effects accurately is critical. A function call, memory write through a pointer/reference, I/O operation, or modification of global state are examples of side effects. Pure computations on local variables generally do not have side effects relevant to DCE of the assignment itself.

## 6. Benefits

*   **Reduced Program Size:** Smaller executables and less memory footprint.
*   **Improved Runtime Performance:** Less code to execute means faster execution. Eliminating dead stores also reduces memory traffic.
*   **Enables Other Optimizations:** A cleaner IR can make subsequent optimization passes more effective (e.g., by reducing the complexity of data flow analysis).
*   **Faster Compile Times (Potentially):** Subsequent compiler passes operate on smaller IRs.

## 7. Potential Limitations/Challenges

*   **Accurate Side-Effect Analysis:** This is the most challenging aspect. Incorrectly identifying an instruction as side-effect-free can lead to incorrect code removal and change program semantics.
*   **Liveness Analysis Complexity:** Liveness analysis itself can be complex for languages with intricate control flow or pointer/reference semantics.
*   **Iterative Nature:** Dead store elimination often requires multiple passes to be fully effective, which can impact compile time.
*   **Debugging Optimized Code:** Code removed by DCE won't be present in the final executable, which can sometimes make debugging more challenging if source mapping isn't perfect.

## 8. Open Questions

*   What level of sophistication will Seen's side-effect analysis have initially?
*   How will DCE interact with Seen's memory model, particularly regarding references and potential aliasing?
*   Will Seen's initial DCE focus primarily on unreachable basic blocks and simple dead stores, or attempt more aggressive forms from the start?
*   How will DCE handle code that might only be dead under certain compilation configurations or feature flags?

Dead Code Elimination is a fundamental optimization. Its implementation will be iterative, likely starting with unreachable code and simple dead stores, and becoming more sophisticated over time as the compiler's analysis capabilities mature.
