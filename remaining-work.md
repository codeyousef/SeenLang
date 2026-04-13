# Remaining Work

## Current state

- Raw Vulkan bootstrap stubs are gated in `seen_runtime/seen_runtime.c`, so the default runtime object no longer exports raw `vk*` symbols by default.
- The runtime regression for that change is in `tests/misc_root_tests/seen_runtime_vulkan_symbols.sh`.
- `compiler_seen/src/tools/c_import_gen.seen` now parses enum constants and emits `let NAME: Int = value` bindings.
- `compiler_seen/src/main_compiler.seen` was updated so the production `compiler_seen/target/seen` CLI is the place where `import-c` is being wired.

## Blocker

The remaining blocker is **bootstrap-safe production `import-c` support**.

- `./scripts/safe_rebuild.sh` no longer fails on the earlier unresolved `lastIndexOf` / `errors` symbols.
- The current failure is earlier in Stage2: module 26 (`compiler_seen/src/tools/c_import_gen.seen`) reaches parallel optimization and then reports `Error: optimization failed for module 26`.
- An isolated Stage1 compile of `compiler_seen/src/main_compiler.seen` reproduces that failure.
- The generated `/tmp/seen_parallel_opt.sh` is syntactically valid and succeeds when rerun manually against the emitted `/tmp/seen_module_*.ll` files, so the remaining issue appears to be in the compiler-run invocation/lifecycle of that script or stale shared `/tmp/seen_module_*` state.

## Next debugging steps

1. Ensure there are no stale compiler or optimizer processes left from interrupted runs.
2. Clean `/tmp/seen_module_*`, `/tmp/seen_parallel_opt.sh`, and related `.opt.*` files.
3. Re-run the isolated Stage1 compile:
   `env PATH="/tmp/seen_opt_override:$PATH" bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen /tmp/stage2_debug --fast --no-cache`
4. Capture the first failing module-26 optimization attempt before anything rewrites `/tmp/seen_parallel_opt.sh`.
5. Once the isolated compile is green, rerun full validation.

## Validation still required

- `./scripts/safe_rebuild.sh`
- `bash tests/misc_root_tests/seen_runtime_vulkan_symbols.sh`
- `bash tests/misc_root_tests/seen_import_c_enums.sh`
- `compiler_seen/target/seen compile tests/e2e_multilang/en/test_stdlib_io_en.seen /tmp/seen_io_runtime_test --fast && /tmp/seen_io_runtime_test`

## Follow-up after bootstrap is green

1. Confirm `compiler_seen/target/seen import-c <header.h>` works end to end.
2. Extend `import-c` beyond functions + enum constants so first-party SDL/Vulkan packages can hide more native glue.
3. Decide whether any tracked public summary should complement the ignored `docs/private/hearton-no-project-c-split.md`.
