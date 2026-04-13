# Remaining Work

## Current state

- Raw Vulkan bootstrap stubs are gated in `seen_runtime/seen_runtime.c`, so the default runtime object no longer exports raw `vk*` symbols by default.
- The runtime regression for that change is in `tests/misc_root_tests/seen_runtime_vulkan_symbols.sh`.
- `compiler_seen/src/tools/c_import_gen.seen` now parses enum constants and emits `let NAME: Int = value` bindings.
- `compiler_seen/src/tools/c_import_gen.seen` also uses bootstrap-safe `class` carriers for imported functions/constants/results instead of the broken `data` lowering path.
- `compiler_seen/src/parser/real_parser.seen` no longer rebuilds the lexer token array in `RealParser.new()`, which keeps bootstrap-built parsers from silently dropping all tokens before parse.
- `compiler_seen/src/main_compiler.seen` now makes `--no-fork` serialize Pass 2b opt/object work as well as IR generation, which lowers rebuild memory pressure.
- `scripts/fix_ir.py` now inserts synthesized declarations at a stable top-level point instead of splitting multiline globals like `@llvm.global_ctors`.
- `scripts/safe_rebuild.sh` now hardens the low-memory rebuild path for this host: bootstrap smoke fallback, explicit compiler/opt caps, opt wrapper caps for `fix_ir.py`, and clean temp-state resets between retries.

## Bootstrap status

The bootstrap-safe production `import-c` blocker is **resolved**.

- `SEEN_LOW_MEMORY=1 ./scripts/safe_rebuild.sh` completes successfully on this host and updates `compiler_seen/target/seen`.
- Recovery smoke now passes with the rebuilt stage3 compiler.
- The new focused bootstrap regressions are:
  - `tests/misc_root_tests/seen_fix_ir_global_ctors.sh`
  - `tests/misc_root_tests/seen_import_c_bootstrap_module.sh`

## Validated

- `./scripts/safe_rebuild.sh`
- `bash tests/misc_root_tests/seen_fix_ir_global_ctors.sh`
- `bash tests/misc_root_tests/seen_import_c_bootstrap_module.sh`
- `bash tests/misc_root_tests/seen_import_c_enums.sh`
- `bash tests/misc_root_tests/seen_runtime_vulkan_symbols.sh`
- `compiler_seen/target/seen compile tests/e2e_multilang/en/test_stdlib_io_en.seen /tmp/seen_io_runtime_test --fast && /tmp/seen_io_runtime_test`

## Remaining follow-up

1. Confirm `compiler_seen/target/seen import-c <header.h>` works end to end on real headers beyond the enum/bootstrap smoke coverage.
2. Extend `import-c` beyond functions + enum constants so first-party SDL/Vulkan packages can hide more native glue.
3. Decide whether any tracked public summary should complement the ignored `docs/private/hearton-no-project-c-split.md`.
