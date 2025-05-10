# Seen Compiler Intermediate Representation (IR)

This directory contains the source code for the Seen Intermediate Representation (IR).

The IR is a data structure used by the compiler after parsing and semantic analysis, and before code generation. Optimizations are performed on this representation.

## Modules

*   `types.rs`: Defines the type system used within the IR.
*   `instruction.rs`: Defines IR instructions and their operands.
*   `basic_block.rs`: Defines Basic Blocks, which group instructions.
*   `function.rs`: Defines Functions, which group Basic Blocks.
*   `module.rs`: Defines a Module, the top-level IR container for a compilation unit.
*   `mod.rs`: Main module file for the IR, re-exporting key components.

Refer to `compiler_seen/docs/ir_spec.md` for the detailed design and specification of the IR.
