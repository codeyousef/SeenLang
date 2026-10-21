# Standard Library Module Index

This index lists every module currently present under `seen_std/src`. Detailed
pages cover the stable/user-facing families; lower-level platform and runtime
modules are listed here so missing API-reference coverage is visible.

The separately versioned `seen/seen-tokenizers` package lives under
`packages/seen_tokenizers`. It provides bounded strict tokenizer JSON,
reproducibly generated Unicode 16.0.0 classification tables, deterministic GPT
byte-level BPE, validated decoding, and explicit special-token policies. Its
byte-offset and Unicode-codepoint APIs are intentionally distinct.

## Module Families

| Family | Purpose |
|--------|---------|
| `async` | Future and async runtime helpers |
| `audio` | Audio backend/device wrappers |
| `collections` | Collection types, buffers, maps, sets, pools |
| `core` | Core types, conversion, reflection, binary helpers |
| `crypto` | Hashing utilities |
| `env` | Environment helpers |
| `ffi` | C type and interop helpers |
| `framework` | Component, routing, middleware, store, VDOM, hot reload |
| `fs` | Raw paths, move-only files, directories, links, durability, and traversal |
| `graphics` | GPU, renderer, shader helpers |
| `hash` | Hash helpers |
| `input` | Gamepad/input abstractions |
| `io` | File, buffered, and stdio helpers |
| `json` | JSON values, parser, builder |
| `math` | Math helpers |
| `memory` | Allocation budget, statistics, and region helpers |
| `net` | Polling and TCP wrappers |
| `platform` | OS-specific windows/audio/input/GPU bindings |
| `process` | Process execution helpers |
| `random` | Random number generators |
| `scripting` | Lua integration |
| `security` | TEE/enclave helpers |
| `simd` | SIMD vector/math helpers |
| `str` | Character, escape, and string utilities |
| `sync` | Mutexes, atomics, channels, queues, barriers |
| `thread` | Threads, affinity, worker pools |
| `time` | Time and duration helpers |
| `uww` | Deterministic UWW/fixed-point helpers |

## Modules

### `accelerator`

- `accelerator/cuda/error`
- `accelerator/cuda/mod`

### `async`

- `async/executor_core`
- `async/executor_native`
- `async/mod`
- `async/multithread`
- `async/structured`

### `audio`

- `audio/audio`
- `audio/openal`

### `collections`

- `collections/algorithms`
- `collections/bit_set`
- `collections/btree_map`
- `collections/btree_set`
- `collections/byte_buffer`
- `collections/hash/mod`
- `collections/hash_map`
- `collections/hashset`
- `collections/linked_list`
- `collections/list_utils`
- `collections/map`
- `collections/pool`
- `collections/std_string`
- `collections/string_buffer`
- `collections/string_hash_map`
- `collections/vec`
- `collections/vecdeque`

### `core`

- `core/binary`
- `core/bitfield`
- `core/compress`
- `core/convert`
- `core/error`
- `core/intrinsics`
- `core/json_derive`
- `core/operation_context`
- `core/option`
- `core/ord`
- `core/ownership`
- `core/packet`
- `core/prelude`
- `core/reflect`
- `core/result`
- `core/secret`
- `core/to_string`
- `core/unit`

### `crypto`

- `crypto/md5`
- `crypto/sha256`

### `determinism`

- `determinism/context`
- `determinism/policy`

### `env`

- `env/env`

### `ffi`

- `ffi/c_types`
- `ffi/cinterop`

### `formats`

- `formats/safetensors`

### `framework`

- `framework/component`
- `framework/executor`
- `framework/hotreload`
- `framework/middleware`
- `framework/routing`
- `framework/store`
- `framework/vdom`
- `framework/vdom_lite`

### `fs`

- `fs/file`
- `fs/mod`
- `fs/operations`
- `fs/path`
- `fs/types`

### `graphics`

- `graphics/gpu`
- `graphics/renderer`
- `graphics/shader`

### `hash`

- `hash/mod`

### `inference`

- `inference/scalars`
- `inference/span`

### `input`

- `input/gamepad`
- `input/gamepad_unified`

### `io`

- `io/buffered`
- `io/bytes`
- `io/direct`
- `io/file`
- `io/stdio`

### `json`

- `json/builder`
- `json/canonical`
- `json/mod`
- `json/parser`
- `json/strict`
- `json/value`

### `math`

- `math/math`

### `memory`

- `memory/aligned_buffer`
- `memory/allocation`
- `memory/mapped_file`
- `memory/mapped_region`
- `memory/pool_region`
- `memory/stack_region`

### `net`

- `net/poll`
- `net/tcp`

### `platform`

- `platform/darwin/cocoa`
- `platform/darwin/coreaudio`
- `platform/darwin/gamecontroller`
- `platform/darwin/metal`
- `platform/darwin/sdl3`
- `platform/darwin/window`
- `platform/linux/alsa`
- `platform/linux/evdev`
- `platform/linux/libinput`
- `platform/linux/pipewire`
- `platform/linux/sdl3`
- `platform/linux/steam`
- `platform/linux/steam_wrapper`
- `platform/linux/vulkan`
- `platform/linux/wayland`
- `platform/linux/window`
- `platform/linux/x11`
- `platform/web/webgpu`
- `platform/windows/win32`
- `platform/windows/xinput`

### `process`

- `process/process`

### `random`

- `random/rng`

### `reactor`

- `reactor/mod`
- `reactor/reactor`

### `root`

- `error`
- `operation_context`
- `ownership`
- `secret`

### `scripting`

- `scripting/lua`

### `security`

- `security/enclave`

### `simd`

- `simd/dispatch_plan`
- `simd/ir_types`
- `simd/simd_math`
- `simd/simd_types`

### `str`

- `str/char`
- `str/escape`
- `str/string`

### `sync`

- `sync/arc`
- `sync/atomic`
- `sync/channel`
- `sync/contracts`
- `sync/lock_order`
- `sync/mod`
- `sync/mutex`

### `testing`

- `testing/categories`

### `thread`

- `thread/affinity`
- `thread/mod`

### `time`

- `time/backoff`
- `time/format`
- `time/mod`
- `time/time`
- `time/timer`

### `uww`

- `uww/fixed`
- `uww/mod`
