# seen-ecs-min

A deterministic entity-component-system (ECS) micro-simulation that ships with the Seen repository for POST-2. The
sample assembles a world with tagged entities, steps simple transform + velocity systems, and reports how many
survivors/casualties remain after a configurable number of frames. Everything executes inside Seen source so the same
program runs unchanged on Linux, WebAssembly, or Android.

## Layout

- `src/main.seen` — ECS definitions, systems, and simulation harness.
- `Seen.toml` — manifest that lists the preferred targets; the file is optional for `seen run` but helps downstream
  tooling package the sample.

## Running

### Linux interpreter (fast edit cycle)

```bash
target/release/seen_cli run examples/seen-ecs-min/src/main.seen
```

### Linux LLVM build

```bash
target/release/seen_cli build examples/seen-ecs-min/src/main.seen \
  --backend llvm \
  --target x86_64-unknown-linux-gnu \
  --output build/linux/seen-ecs-min
```

### WebAssembly artifact

```bash
target/release/seen_cli build examples/seen-ecs-min/src/main.seen \
  --backend llvm \
  --target wasm32-unknown-unknown \
  --output build/web/seen-ecs-min.wasm
```

### Android bundle

```bash
export ANDROID_NDK_HOME=/path/to/ndk
export ANDROID_API_LEVEL=33

target/release/seen_cli build examples/seen-ecs-min/src/main.seen \
  --backend llvm \
  --target aarch64-linux-android \
  --bundle android \
  --output build/android/seen-ecs-min.aab
```

The simulation prints a concise summary (frames, casualties, survivors, simulated distance). CI can run the interpreter
variant to prove the project builds without touching LLVM or the Android toolchain.
