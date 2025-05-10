# Compiler Optimization: Function Inlining

**Version:** 0.1 (Initial Draft)

## 1. Introduction

Function inlining is a compiler optimization that replaces a call to a function with the actual body of that function at the call site. This can eliminate the overhead associated with function calls (such as stack frame setup/teardown, parameter passing, and jumps) and can expose further optimization opportunities by bringing the context of the caller and callee together.

This document outlines the design for implementing function inlining in the self-hosted Seen compiler.

## 2. Goals

*   Reduce function call overhead.
*   Enable other inter-procedural optimizations by making the callee's code visible within the caller's context (e.g., more effective constant propagation, dead code elimination).
*   Improve overall program performance, particularly for small, frequently called functions.

## 3. Scope of Application

Inlining is typically considered for:

*   **Small Functions:** Functions with a small number of IR instructions are good candidates.
*   **Frequently Called Functions:** Inlining functions called within hot loops can yield significant performance gains.
*   **Functions Called Only Once:** These can often be inlined without code size penalty.
*   **Functions Marked with `#[inline(always)]` (or similar Seen attribute):** Allowing the developer to hint or force inlining.
*   **Functions Where Inlining Enables Significant Constant Propagation:** If arguments passed to a function are constants, inlining might allow large parts of the function body to be constant-folded away.

## 4. Benefits

*   **Reduced Call Overhead:** Eliminates time spent on call/return sequences, stack manipulation, and parameter passing.
*   **Improved Instruction Cache Locality:** Linearizing code can sometimes improve cache performance.
*   **Enables Further Optimizations:** This is often the most significant benefit. After inlining, optimizations like constant folding, dead code elimination, and register allocation can be more effective on the larger, combined block of code.

## 5. Costs and Heuristics

While beneficial, indiscriminate inlining can lead to problems:

*   **Code Bloat:** Replacing many calls to a large function with its body can significantly increase the overall code size. This can negatively impact instruction cache performance and binary size.
*   **Increased Compile Time:** Inlining can increase the size of the IR that subsequent optimization passes need to process.

Therefore, compilers use **heuristics** to decide when inlining is beneficial. Common factors in these heuristics include:

*   **Callee Size:** A primary factor. A threshold is set for the number of IR instructions in the callee.
*   **Caller Size:** Sometimes considered; inlining a small function into a very large one might be less beneficial.
*   **Call Site Context:** Is the call within a loop? Is it a performance-critical section?
*   **Recursion:** Direct recursion is usually not inlined, or only for a very small depth. Mutual recursion also complicates inlining.
*   **Developer Hints:** Attributes like `#[inline(always)]`, `#[inline(never)]`, or `#[inline(hint)]` (names TBD for Seen) can guide the compiler.
*   **Optimization Level:** More aggressive inlining might be performed at higher optimization levels.
*   **Profile-Guided Optimization (PGO):** If PGO data is available, it can provide accurate call frequencies to guide inlining decisions.

## 6. Interaction with Seen Intermediate Representation (IR)

Inlining is performed on the IR. The process typically involves:

1.  **Identification:** Identify candidate call sites and the functions they call based on heuristics.
2.  **Substitution:**
    *   Create a copy of the callee's IR.
    *   Map the callee's formal parameters to the actual arguments at the call site. This might involve substituting argument expressions directly or creating temporary variables in the caller's scope.
    *   Map the callee's return statements to assignments to a temporary variable (if the result is used) and jumps to the instruction following the original call site in the caller.
    *   Carefully handle naming conflicts between variables in the caller and the inlined callee body (e.g., by renaming variables from the callee).
    *   Insert the modified copy of the callee's IR at the call site, replacing the original call instruction.
3.  **Cleanup:** After inlining, other optimization passes (like constant propagation and dead code elimination) are often run to simplify the combined code.

**Considerations for Seen's IR:**

*   The IR must support copying and modification of function bodies.
*   A clear way to map parameters to arguments is needed.
*   Handling of control flow from `return` statements within the inlined body is crucial.
*   Variable scoping and renaming mechanisms are important to avoid conflicts.

## 7. Algorithm Sketch (Simplified)

```
func inline_pass(module_ir) {
    for function_ir in module_ir.functions {
        for block in function_ir.blocks {
            for instruction in block.instructions {
                if instruction is a function_call_site {
                    callee = instruction.get_callee();
                    actual_arguments = instruction.get_arguments();

                    if should_inline(function_ir, callee, instruction_site_context) { // Heuristic check
                        // 1. Clone callee's IR body.
                        inlined_body_ir = callee.clone_ir_body();

                        // 2. Create a mapping for formal parameters to actual arguments.
                        param_arg_map = create_param_arg_map(callee.formal_params, actual_arguments);

                        // 3. Substitute/replace formal parameter uses in inlined_body_ir with corresponding actual arguments.
                        //    This may involve creating temp variables in the caller for complex arguments.
                        substitute_parameters(inlined_body_ir, param_arg_map);

                        // 4. Rename local variables in inlined_body_ir to avoid clashes with caller's variables.
                        rename_locals_for_inlining(inlined_body_ir, function_ir.get_scope());

                        // 5. Handle return statements in inlined_body_ir:
                        //    - If call has a return value, assign it to a temporary variable at original call site.
                        //    - Convert 'return' to 'jump' to point after original call site.
                        transform_returns_for_inlining(inlined_body_ir, instruction_site_context);
                        
                        // 6. Replace the call instruction with the transformed inlined_body_ir.
                        block.replace_instruction_with_block_sequence(instruction, inlined_body_ir);
                        
                        // Mark that IR has changed, may need to re-run analyses or other optimizations.
                    }
                }
            }
        }
    }
}

func should_inline(caller, callee, context) {
    // Implement heuristics:
    // - Check callee size (e.g., IR instruction count).
    // - Check for #[inline(always)] or #[inline(never)] attributes.
    // - Check for recursion.
    // - Consider call site frequency (if PGO data available or estimated).
    // - (More advanced) Estimate benefit vs. cost.
    return decision;
}
```

## 8. Potential Limitations/Challenges

*   **Heuristic Tuning:** Developing effective inlining heuristics is challenging and often requires empirical tuning. Poor heuristics can lead to performance regressions or excessive code bloat.
*   **Recursive Functions:** Simple inlining doesn't work for direct recursion. Limited unrolling or specialized techniques might be applied, but general recursive inlining is complex.
*   **Large Functions/Virtual Calls:** Inlining very large functions is usually detrimental. Inlining virtual calls (dynamic dispatch) requires a preceding devirtualization pass or speculative inlining with guards.
*   **Debugging:** Inlined code can make debugging harder as the source code structure no longer directly maps to the executed code. Debug information (e.g., DWARF) needs to correctly represent inlined frames.
*   **Managing IR Complexity:** The process of substituting and integrating IR can be intricate, especially with complex control flow or variable scoping rules.

## 9. Open Questions

*   What will be Seen's specific set of inlining attributes (`#[inline(always)]`, etc.)?
*   What initial heuristics will Seen employ? How will these be tuned?
*   How will inlining interact with Seen's module system and visibility rules (e.g., inlining functions from other modules/crates)? This usually requires the IR of the callee to be available at compile time.
*   What is the strategy for handling debug information for inlined functions?

Function inlining is a powerful optimization that requires careful implementation and heuristic design. It is often a key enabler for other compiler passes.
