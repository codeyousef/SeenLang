# Remaining Work

## Current state

- Raw Vulkan bootstrap stubs are gated in `seen_runtime/seen_runtime.c`, so the default runtime object no longer exports raw `vk*` symbols by default.
- The runtime regression for that change is in `tests/misc_root_tests/seen_runtime_vulkan_symbols.sh`.
- `compiler_seen/src/tools/c_import_gen.seen` now parses enum constants and emits `let NAME: Int = value` bindings.
- `compiler_seen/src/tools/c_import_gen.seen` now also handles redeclaration-style real-header AST lines for both functions and enum constants by choosing the last identifier before the quoted type and deduping by name.
- `compiler_seen/src/tools/c_import_gen.seen` now parses public `TypedefDecl` entries, emits `type NAME = Int|Ptr` aliases, and reuses those aliases in generated function signatures.
- `compiler_seen/src/tools/c_import_gen.seen` now parses named `RecordDecl` definitions into `@repr(C)` classes, parses named C unions into `@union` classes, skips same-name struct/union typedef aliases, and emits typed record pointers in generated function signatures.
- `compiler_seen/src/tools/c_import_gen.seen` now lowers function-pointer typedefs into typed Seen `fn(...) -> ...` aliases and preserves those callback aliases in generated records and function signatures.
- Seen now parses fixed-size type syntax like `T[N]`, lowers it through the type/codegen pipeline to LLVM array layouts, and sizes imported `@repr(C)` / `@union` fields correctly from those lowered layouts.
- `compiler_seen/src/tools/c_import_gen.seen` now lowers imported inline C arrays to Seen fixed-array field/typedef layouts instead of warning and falling back to opaque placeholders, while still preserving higher-level API typedefs and normalizing standard scalar typedefs like `int32_t` / `uint32_t` to concrete field element types.
- `compiler_seen/src/tools/c_import_gen.seen` now synthesizes named Seen carrier types for anonymous C structs/unions and preserves them as nested parent fields instead of truncating the parent layout when clang emits nested `RecordDecl` definitions. Promoted anonymous members are currently accessed through those synthesized nested fields rather than flattened aliases.
- `compiler_seen/src/tools/c_import_gen.seen` now collapses imported C bitfields into ABI-correct backing storage fields and emits generated getter/setter helpers for validated layouts, including <=32-bit storage-unit cases plus widened 64-bit storage-unit groups handled through a dedicated UInt64 bitwise helper path.
- `compiler_seen/src/tools/c_import_gen.seen` also uses bootstrap-safe `class` carriers for imported functions/constants/results instead of the broken `data` lowering path.
- `compiler_seen/src/parser/real_parser.seen` no longer rebuilds the lexer token array in `RealParser.new()`, which keeps bootstrap-built parsers from silently dropping all tokens before parse.
- `compiler_seen/src/main_compiler.seen` now makes `--no-fork` serialize Pass 2b opt/object work as well as IR generation, which lowers rebuild memory pressure.
- `scripts/fix_ir.py` now inserts synthesized declarations at a stable top-level point instead of splitting multiline globals like `@llvm.global_ctors`.
- `scripts/safe_rebuild.sh` now hardens the low-memory rebuild path for this host: bootstrap smoke fallback, explicit compiler/opt caps, opt wrapper caps for `fix_ir.py`, and clean temp-state resets between retries.
- `tests/misc_root_tests/seen_import_c_typedefs.sh` now covers synthetic typedef aliases plus generated `@repr(C)` record layouts and typed callback aliases, `tests/misc_root_tests/seen_import_c_unions.sh` covers synthetic C union lowering, `tests/misc_root_tests/seen_import_c_array_fallbacks.sh` now covers real inline-array struct/union lowering, `tests/misc_root_tests/seen_import_c_anonymous_records.sh` covers synthesized anonymous struct/union lowering, and `tests/misc_root_tests/seen_import_c_real_headers.sh` covers real `string.h`, `stdio.h`, and `vulkan.h` imports to reject bogus `prev` / `referenced` bindings while preserving Vulkan typedefs, callbacks, records, unions, and inline arrays like `VkClearColorValue`.

## Bootstrap status

The bootstrap-safe production `import-c` blocker is **resolved**.

- `./scripts/safe_rebuild.sh` completes successfully on this host and updates `compiler_seen/target/seen`.
- Recovery smoke now passes with the rebuilt stage3 compiler.
- The new focused bootstrap regressions are:
  - `tests/misc_root_tests/seen_fix_ir_global_ctors.sh`
  - `tests/misc_root_tests/seen_import_c_bootstrap_module.sh`

## Validated

- `./scripts/safe_rebuild.sh`
- `bash tests/misc_root_tests/seen_fix_ir_global_ctors.sh`
- `bash tests/misc_root_tests/seen_import_c_bootstrap_module.sh`
- `bash tests/misc_root_tests/seen_import_c_enums.sh`
- `bash tests/misc_root_tests/seen_import_c_typedefs.sh`
- `bash tests/misc_root_tests/seen_import_c_unions.sh`
- `bash tests/misc_root_tests/seen_import_c_array_fallbacks.sh`
- `bash tests/misc_root_tests/seen_import_c_anonymous_records.sh`
- `bash tests/misc_root_tests/seen_import_c_bitfields.sh`
- `bash tests/misc_root_tests/seen_import_c_bitfields_64.sh`
- `bash tests/misc_root_tests/seen_import_c_real_headers.sh`
- `bash tests/misc_root_tests/seen_runtime_vulkan_symbols.sh`
- `compiler_seen/target/seen compile tests/e2e_multilang/en/test_stdlib_io_en.seen /tmp/seen_io_runtime_test --fast && /tmp/seen_io_runtime_test`

## Remaining follow-up

1. Decide whether `import-c` should flatten promoted anonymous members via `IndirectFieldDecl` aliases or keep the current explicit nested-field representation.
2. If first-party package code needs to actively manipulate imported fixed-array fields beyond passing them through FFI, validate whether any broader static-array indexing/assignment work is still missing outside the current layout-focused support.
3. If package bindings need storage units wider than 64-bit (or multi-unit/packed edge cases), extend helper coverage beyond the currently validated <=64-bit paths.
4. Decide whether any tracked public summary should complement the ignored `docs/private/hearton-no-project-c-split.md`.
