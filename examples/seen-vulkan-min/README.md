# seen-vulkan-min

A deterministic simulation of the lifecycle phases in a Vulkan-style triangle
program. Despite its name, `src/main.seen` does not call Vulkan; it is useful for
exercising data types, frame-state transitions, and validation reporting without
a GPU dependency. `shaders/triangle.spv` is a bundled sample asset.

From the repository root:

```bash
mkdir -p .seen/agent-tools/examples
scripts/run_with_project_artifacts.sh vulkan-check -- \
  compiler_seen/target/seen check examples/seen-vulkan-min/src/main.seen
scripts/run_with_project_artifacts.sh vulkan-run -- \
  compiler_seen/target/seen run examples/seen-vulkan-min/src/main.seen
scripts/run_with_project_artifacts.sh vulkan-compile -- \
  compiler_seen/target/seen compile examples/seen-vulkan-min/src/main.seen \
  .seen/agent-tools/examples/seen-vulkan-min
.seen/agent-tools/examples/seen-vulkan-min
```

The program prints a frame summary including `validation_errors`. These are
simulation results, not messages from Vulkan validation layers.

For an Android target artifact, use the current target spelling and install the
required NDK/toolchain first:

```bash
scripts/run_with_project_artifacts.sh vulkan-android -- \
  compiler_seen/target/seen compile examples/seen-vulkan-min/src/main.seen \
  .seen/agent-tools/examples/seen-vulkan-min-android --target=android-arm64
```

This compiler command does not bundle the manifest, dex payload, resources, or
shader asset. See [`docs/targets.md`](../../docs/targets.md) and the repository's
Android packaging helpers for those separate steps. The shipped compiler is
LLVM-only and does not advertise a WebAssembly target.
