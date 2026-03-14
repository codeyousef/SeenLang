# Changelog

All notable changes to the Seen compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.2] - 2026-03-14

### Fixed

#### Codegen / Type Errors
- Bool return values compared with `==` no longer produce i1/i64 type mismatches in LLVM IR
- Float comparisons (`!=`, `==`) with mixed types now emit `fcmp` instead of `icmp`
- Implicit Int-to-Float promotion in arithmetic expressions now inserts `sitofp` conversion
- Cross-class method calls preserve the declared return type instead of inferring i1

#### Loop Compilation
- Nested while loops no longer clobber local variables across function calls. Removed incorrect `nuw`/`nsw` flags from general integer arithmetic and `nosync` from function attributes that were enabling unsafe LLVM optimizations. Induction variable state is now properly saved/restored across nested loop boundaries.
- `for i in 0..N` loops in nested contexts no longer hang or produce wrong values
- Nested loops with `floorFloat` calls no longer miscompile at specific coordinates

#### Linker / Module System
- `Array.pop()` now dispatches to runtime functions (`seen_arr_pop_i64`/`f64`/`str`) instead of emitting undefined `@Array_pop`
- `extern fun` declarations are now registered in the cross-module symbol table, eliminating the need to duplicate FFI declarations in every module that uses them
- `__Tan` is now auto-declared in multi-module builds alongside `__Sin` and `__Cos`
- `extern fun` return values used in conditionals are no longer eliminated by LTO; extern declares now carry `memory(inaccessiblemem: readwrite)` to preserve side-effect semantics

#### Build System / Module Resolver
- The compiler no longer requires CWD to be the seenlang root directory. Stdlib imports now resolve relative to the compiler binary's installation path.
- Import resolver no longer unconditionally strips the first path segment when it matches the source directory name. The segment is only skipped if the resulting file actually exists on disk.
- Stale `.o` files from previous builds with different module counts are now cleaned before linking

#### Runtime / Performance
- Method name `get()` with a single parameter no longer incorrectly resolves to `Array.get()` when called on non-Array receivers
- Added `Array.clear()` method that resets length to zero without freeing the backing buffer, enabling array reuse in hot loops instead of repeated allocation

### Added
- `seen_arr_clear` runtime function for zero-cost array reuse
- `getCompilerRoot()` helper for resolving compiler-relative paths from the binary location
- `fix_ir.py` fix for `allocsize(0)` attribute incorrectly rewritten to `allocsize(i64)` during IR fixup

## [0.3.1] - 2026-03-13

### Fixed
- Cross-module `let` constants no longer resolve as 0

### Added
- VSCode Tasks with problem matcher for build/run/check commands
- LSP diagnostics for missing `main`, clickable terminal links

## [0.3.0] - 2026-03-12

### Added
- Windows x86-64 cross-compilation support
- Windows installer (NSIS-based)
