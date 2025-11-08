# seen-vulkan-min

A tiny graphics workload that mirrors the steps of a Vulkan triangle sample without depending on platform APIs. It
runs entirely inside the core Seen runtime so the same source can be executed via the interpreter, LLVM, MLIR, or CLIF
backends on Linux, WebAssembly, or Android. The example focuses on the deterministic hand-off between lifecycle phases
(instance → device → swapchain → frame graph) and emits a validation summary so CI can assert that no simulated
validation layer errors occurred.

## Layout

- `src/main.seen` — state machine plus frame loop.
- `shaders/triangle.spv` — canonical SPIR-V blob reused by the shader CLI (`seen shaders ...`). It is copied from
  `examples/shaders/triangle.spv` so the sample can bundle and re-export a real shader asset when `seen build --bundle`
  is invoked.
- `Seen.toml` — project manifest describing the entry file and preferred targets.

## Running

All commands assume the workspace root and a built `seen_cli` binary under `target/release/seen_cli`.

### Linux interpreter fast path

```bash
target/release/seen_cli run examples/seen-vulkan-min/src/main.seen
```

### Linux LLVM build (shared library)

```bash
target/release/seen_cli build examples/seen-vulkan-min/src/main.seen \
  --backend llvm \
  --target x86_64-unknown-linux-gnu \
  --shared \
  --output build/linux/libseen_vulkan_min.so
```

### WebAssembly bundle (with loader)

```bash
target/release/seen_cli build examples/seen-vulkan-min/src/main.seen \
  --backend llvm \
  --target wasm32-unknown-unknown \
  --bundle wasm \
  --wasm-loader minimal \
  --output build/web/triangle.wasm
```

### Android `.aab`

```bash
export ANDROID_NDK_HOME=/path/to/ndk
export ANDROID_API_LEVEL=33

target/release/seen_cli build examples/seen-vulkan-min/src/main.seen \
  --backend llvm \
  --target aarch64-linux-android \
  --bundle android \
  --output build/android/seen-vulkan-min.aab
```

The CLI emits a per-frame validation report. CI can grep for `validation_errors=0` to ensure the simulated validation
layers stayed happy. Because the sample avoids host-specific syscalls it can be exercised on any platform that supports
the Seen interpreter.
