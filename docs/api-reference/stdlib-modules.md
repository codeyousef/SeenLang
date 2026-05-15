# Standard Library Module Index

This index lists every module currently present under `seen_std/src`. Detailed
pages cover the stable/user-facing families; lower-level platform and runtime
modules are listed here so missing API-reference coverage is visible.

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
| `fs` | Path utilities |
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

### `async`

- `async/future`
- `async/runtime`

### `audio`

- `audio/audio`
- `audio/openal`

### `collections`

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
- `core/intrinsics`
- `core/json_derive`
- `core/option`
- `core/ord`
- `core/packet`
- `core/prelude`
- `core/reflect`
- `core/result`
- `core/to_string`
- `core/unit`

### `crypto`

- `crypto/md5`

### `env`

- `env/env`

### `ffi`

- `ffi/c_types`
- `ffi/cinterop`

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

- `fs/path`

### `graphics`

- `graphics/gpu`
- `graphics/renderer`
- `graphics/shader`

### `hash`

- `hash/mod`

### `input`

- `input/gamepad`
- `input/gamepad_unified`

### `io`

- `io/buffered`
- `io/file`
- `io/stdio`

### `json`

- `json/builder`
- `json/mod`
- `json/parser`
- `json/value`

### `math`

- `math/math`

### `memory`

- `memory/allocation`
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

### `scripting`

- `scripting/lua`

### `security`

- `security/enclave`

### `simd`

- `simd/simd_math`
- `simd/simd_types`

### `str`

- `str/char`
- `str/escape`
- `str/string`

### `sync`

- `sync/atomic`
- `sync/atomic_queue`
- `sync/atomic_stack`
- `sync/barrier`
- `sync/channel`
- `sync/mpsc_queue`
- `sync/mutex`
- `sync/ordering`
- `sync/rwlock`
- `sync/spsc_queue`
- `sync/thread_local`

### `thread`

- `thread/affinity`
- `thread/mod`
- `thread/pool`

### `time`

- `time/mod`
- `time/time`

### `uww`

- `uww/fixed`
- `uww/mod`
