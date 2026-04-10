# Known Limitations

This page documents current compiler bugs, codegen limitations, and workarounds.

## Stage 1 (Frozen Compiler) Limitations

The frozen bootstrap compiler has several known issues:

| Issue | Workaround |
|-------|------------|
| No generic class methods with receiver type inference | Use free functions or explicit type parameters |
| Float function parameters broken | Inline the computation instead |
| `%` modulo operator broken | Use `a - (a / b) * b` |
| Boolean variables broken | Use `var x = 0; x = 1; if x == 1 {` |
| `if/else if` chains broken | Use nested `if/else` blocks |
| `if not X` broken | Use `if X { return }` + fall-through |
| `.getTokenType()` cross-module issue | Use `checkToken(SeenTokenType.X)` |

## Recently Covered Regression Patterns

The current compiler source now has regression coverage for several patterns that were previously listed here as active production codegen bugs:

- `static fun` in classes
- class constructor struct literals
- module-level `var x = func()`
- module-level `Array<T>` / `String` initialization
- functions returning `Array<T>` via `r:` syntax
- `let` bindings of string expressions inside `while` loops
- leading-brace string literals such as `"{identifier"`

## Cold Compile Hang

Compilers built from the refactored source hang on 12 specific modules (0, 3, 5, 9, 10, 11, 12, 14, 16, 23, 25, 35) when compiling from scratch without IR cache.

**Impact:** Cannot cold-compile the full compiler with the refactored-source compiler.

**Workaround:** Use the pre-refactoring compiler (stored as `stage1_frozen`) to populate the IR cache. The `safe_rebuild.sh` script handles this automatically.

**Note:** This is NOT fork-related. The hang occurs with `--no-fork` as well.

## Cross-Module GEP Bug

`getelementptr %ClassName` in module X fails if `%ClassName` is defined in module Y.

**Workaround:** Use `memset` zero-init instead of per-field GEP in isClassType() constructor path.

## `extern fun __foo()` Rule

Names starting with `__` with an empty body are skipped by codegen.

**Workaround:** Add explicit `declare` entry to `ir_declarations.seen`.

## SSA Register Ordering

SSA registers must be strictly ascending. Pre-allocate the register for the FIRST instruction.

## Array Invariant Loads

`!invariant.load` on array data pointers is incorrect -- data changes on push/resize. Do not mark array data as invariant.

## Stack-Allocated SeenArray Headers

Do not stack-allocate `SeenArray` headers. Escaping pointers will crash. Always heap-allocate.

## Runtime C Function Boolean Returns

Runtime C functions returning 0/1 `int64_t` for bool need `trunc i64 to i1` in codegen.

## Float `isNaN`/`isInfinite` Checks

Must NOT use `fast` flag (fast implies nnan/ninf).

## HashMap Non-Determinism

`HashMap` iteration order is non-deterministic. In `--deterministic` mode, the compiler rejects `HashMap` usage unless marked with `@nondeterministic`.

## Reporting Issues

If you encounter a new bug:

1. Create a minimal reproduction file (`repro_*.seen`)
2. Test with `--emit-llvm` to inspect generated IR
3. Use `SEEN_TRACE_LLVM=all` for detailed tracing
4. Report at [github.com/codeyousef/SeenLang/issues](https://github.com/codeyousef/SeenLang/issues)
