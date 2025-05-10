# Compiler Optimization: Loop Optimizations

**Version:** 0.1 (Initial Draft)

## 1. Introduction

Loops are frequently the most performance-critical parts of programs, as they execute their bodies multiple times. Optimizing loops can therefore yield significant improvements in overall program execution speed. Loop optimizations encompass a variety of techniques aimed at reducing loop overhead, increasing instruction-level parallelism, and improving memory access patterns.

This document outlines the design for several common loop optimization techniques in the self-hosted Seen compiler.

## 2. Goals

*   Reduce the number of instructions executed within or by loops.
*   Improve cache utilization and reduce memory latency effects.
*   Expose more opportunities for instruction-level parallelism.
*   Overall, decrease the total execution time of programs.

## 3. Prerequisite Analyses

Effective loop optimization relies on several program analyses:

*   **Loop Detection:** Identifying natural loops in the Control Flow Graph (CFG). A loop consists of a header basic block, a set of body blocks, and back edges to the header.
*   **Data-Flow Analysis:**
    *   **Reaching Definitions:** To determine which definitions of a variable reach a particular point (useful for LICM).
    *   **Use-Def Chains / Def-Use Chains:** Linking definitions of variables to their uses and vice-versa.
    *   **Liveness Analysis:** Determining if a variable's value will be used in the future.
*   **Alias Analysis:** Determining if different memory references (pointers/references) can point to the same memory location. Crucial for safety when reordering memory operations.
*   **Dependency Analysis:** Determining if there are dependencies between statements, particularly across loop iterations, which can restrict reordering or parallelization.

## 4. Common Loop Optimization Techniques

### 4.1. Loop-Invariant Code Motion (LICM)

*   **Concept:** Moves computations from inside a loop to outside the loop (typically to a preheader block) if their results do not change across loop iterations.
*   **Example:**
    ```seen
    // Before LICM
    var limit = 100;
    var y = 5;
    for i from 0 to n - 1 {
        x = y * limit; // y * limit is loop-invariant
        arr[i] = x + i;
    }

    // After LICM
    var limit = 100;
    var y = 5;
    val invariant_val = y * limit; // Hoisted
    for i from 0 to n - 1 {
        x = invariant_val;
        arr[i] = x + i;
    }
    ```
*   **Conditions for Hoisting `expr`:**
    1.  All operands of `expr` are either constants, defined outside the loop, or are themselves loop-invariant computations already hoisted.
    2.  The computation of `expr` must dominate all exit nodes of the loop where the defined variable is live-out, OR the expression has no side effects and the defined variable is only used inside the loop (or not at all if the expression itself is dead after hoisting).
    3.  The expression must yield the same result in every iteration.
    4.  The expression must have no side effects that cannot be safely moved before the loop (e.g., it cannot trap, like division by zero, unless it's guaranteed to execute at least once and dominate all loop exits).
*   **Benefits:** Reduces redundant computations within the loop.
*   **Challenges:** Requires accurate data-flow analysis. Determining safety regarding side effects and exceptions is crucial.

### 4.2. Loop Unrolling

*   **Concept:** Reduces loop control overhead (e.g., incrementing and testing the loop counter) and increases instruction-level parallelism by replicating the loop body multiple times within a single iteration of a new, shorter loop.
*   **Example (Unroll factor of 2):
    ```seen
    // Before Unrolling
    for i from 0 to 99 {
        arr[i] = i;
    }

    // After Unrolling by factor 2
    for i from 0 to 99 step 2 {
        arr[i] = i;
        arr[i+1] = i+1;
    }
    // May need a cleanup loop if n is not a multiple of unroll factor
    ```
*   **Benefits:**
    *   Reduces loop control overhead.
    *   Increases the number of independent instructions in the loop body, potentially allowing better scheduling and utilization of CPU functional units.
*   **Challenges:**
    *   Increases code size.
    *   Choosing an optimal unroll factor is heuristic-dependent (can depend on loop body size, target architecture).
    *   Requires handling remainder iterations if the total iteration count is not a multiple of the unroll factor.

### 4.3. Strength Reduction

*   **Concept:** Replaces computationally expensive operations within a loop with equivalent but cheaper operations. The classic example is replacing multiplication involving a loop induction variable with repeated additions.
*   **Example:**
    ```seen
    // Before Strength Reduction
    for i from 0 to n - 1 {
        val j = i * 4; // Multiplication
        arr[j] = ...;
    }

    // After Strength Reduction
    var temp_j = 0;
    for i from 0 to n - 1 {
        val j = temp_j; // Use temp_j
        arr[j] = ...;
        temp_j = temp_j + 4; // Replaced with addition
    }
    ```
*   **Applicability:** Typically for expressions involving the loop's induction variable(s) (variables that are systematically incremented or decremented in each iteration).
*   **Benefits:** Can significantly speed up loops if the "stronger" operation is much slower than the "weaker" one.
*   **Challenges:** Modern CPUs often have fast multipliers, so the benefit for simple arithmetic might be less pronounced than in the past. However, it can still be very effective for address calculations or more complex operations. Requires careful tracking of induction variables.

### 4.4. Loop Fusion (Combining Loops)

*   **Concept:** If two adjacent loops iterate over the same range and do not have dependencies that prevent merging, their bodies can be combined into a single loop.
*   **Example:**
    ```seen
    // Before Fusion
    for i from 0 to n - 1 { arr1[i] = i; }
    for i from 0 to n - 1 { arr2[i] = arr1[i] * 2; }

    // After Fusion
    for i from 0 to n - 1 {
        arr1[i] = i;
        arr2[i] = i * 2; // arr1[i] can be replaced by i if definition is clear
    }
    ```
*   **Benefits:** Reduces loop overhead, can improve data locality if the fused bodies access the same data.
*   **Challenges:** Requires careful dependency analysis to ensure fusion doesn't change program semantics.

### 4.5. Loop Fission (Distribution / Splitting Loops)

*   **Concept:** Splits a single loop into multiple loops, each containing a part of the original loop's body.
*   **Benefits:** Can improve data locality if different parts of the loop body access disjoint sets of data. Can also enable other optimizations (like vectorization) for parts of the loop.
*   **Challenges:** Increases loop overhead. Requires dependency analysis.

## 5. Interaction with Seen Intermediate Representation (IR)

Loop optimizations transform the IR structure:

*   **LICM:** Involves creating a preheader block (if one doesn't exist), moving instructions from loop body blocks to this preheader.
*   **Loop Unrolling:** Replicates sequences of IR instructions within the loop body, adjusts loop termination conditions and induction variable updates.
*   **Strength Reduction:** Modifies instructions within the loop body and adds new initialization instructions for the reduced-strength variables in the preheader.
*   **Loop Fusion/Fission:** Involves significant restructuring of CFG and instruction sequences.

The IR must be flexible enough to allow these transformations, including adding/removing basic blocks, moving instructions, and modifying control flow.

## 6. Implementation Strategy

1.  **Loop Identification:** Implement robust detection of natural loops in the CFG.
2.  **Analysis Passes:** Implement necessary data-flow analyses (reaching definitions, liveness, induction variable analysis).
3.  **Individual Optimizations:** Implement loop optimization passes one by one, starting with potentially LICM or simple unrolling.
    *   Each pass should clearly define its preconditions (required analyses) and post-conditions (IR transformations).
4.  **Ordering:** The order in which loop optimizations (and other optimizations) are applied matters. For instance, LICM might expose opportunities for other passes.
5.  **Heuristics:** Develop and tune heuristics for when to apply certain optimizations (e.g., unroll factor, fusion criteria).

## 7. Open Questions

*   What specific set of loop optimizations will Seen target for its initial self-hosted compiler releases?
*   How will Seen handle complex loop structures (e.g., nested loops, loops with multiple exits)?
*   What heuristics will be used for transformations like loop unrolling and fusion/fission?
*   How will these optimizations interact with Seen's memory model and potential auto-vectorization or auto-parallelization features in the future?
*   How will debug information be maintained accurately after significant loop transformations?

Loop optimizations are crucial for performance in many applications. A well-chosen set of loop transformations can significantly improve the efficiency of code generated by the Seen compiler.
