# Seen Language Production Requirements Document (PRD)

**Version**: 1.5 - Complete Specification  
**Syntax**: Kotlin-style decorators (`@`) throughout  
**Core Principle**: Safety by default, nondeterminism explicitly opt-in via annotation  
**Scope**: Deterministic systems language for sovereign infrastructure, native applications, and game engines

---

## 1. Target Platform Requirements

### 1.1 Linux (x86_64-unknown-linux-gnu)

**Primary Use Cases**: Native game client, dedicated server, development environment, CI/CD

**ABI and Linking**:
- Must support `extern "C"` bindings to Vulkan, SDL3, PipeWire, evdev, X11, and Wayland
- Must enable static linking of `libSDL3.a`, `libvulkan.a`, `libpipewire-0.3.a`, `libevdev.a`, `libxkbcommon.a`, and all other required C libraries
- Must support dynamic linking for runtime dependencies (e.g., `steam_api64.so`)
- Must provide full cross-compilation support from any host to Linux target
- Must be compatible with `#![no_std]` plus `alloc` for WASM portability

**Determinism**:
- Must produce identical ELF binaries across different Linux distributions when using `--profile deterministic`
- All object code must be generated with deterministic linker flags (no timestamps, no embedded paths)
- Must not rely on nondeterministic `/proc` filesystem in deterministic profile
- Must enforce deterministic symbol mangling and sorting

**Graphics and Windowing**:
- Must provide native Wayland support (no XWayland dependency) with X11 fallback
- Must support Vulkan 1.3+ with SDL3 or GLFW backend
- Must support multiple GPU vendors (Intel Mesa, AMD Mesa, NVIDIA proprietary)

**Audio**:
- Must support PipeWire (primary) with ALSA fallback
- Must provide real-time safe audio callbacks without allocations

**Input**:
- Must support evdev/libinput for keyboard, mouse, and gamepad
- Must integrate with Steam Input API

**Performance**:
- Must achieve compile times of less than 1 second for 10k LOC projects with incremental compilation
- Must support 100+ concurrent players on dedicated server with zero memory leaks in 24-hour soak tests
- Must achieve client frame rates of 60 FPS on GTX 1060 / RX 580 hardware, matching Windows performance within 10%

**Distribution**:
- Must support AppImage packaging for broad distribution
- Must integrate with Steam for Linux and achieve Deck Verified status
- Must support Flatpak packaging (Year 2)

**Testing**:
- Must pass deterministic build verification on Pop!_OS 22.04+, Arch Linux, Ubuntu 22.04+, Fedora 38+, and SteamOS 3.5+
- Must pass Valgrind/ASAN clean in CI gates
- Must pass 24-hour dedicated server stress test without crashes
- Must achieve 95%+ positive Steam reviews from Linux players

---

### 1.2 Windows (x86_64-pc-windows-msvc)

**Primary Use Case**: Primary game client (Hearthshire Steam release)

**ABI and Linking**:
- Must support `extern "C"` bindings to Win32, Vulkan, SDL3, and Steamworks SDK
- Must enable static linking of `SDL3-static.lib`, `vulkan-1.lib`
- Must support dynamic linking to `steam_api64.dll` and other runtime dependencies
- Must provide cross-compilation from Linux host to Windows target

**Determinism**:
- Must produce identical PE32+ binaries across different Windows versions when using `--profile deterministic`
- Must use deterministic linker flags: `/TIMESTAMP:0`, `/BREPRO`, `/NOLOGO`
- Must strip embedded PDB paths and other non-deterministic metadata in release builds
- Must disable ASLR in deterministic builds (`/DYNAMICBASE:NO`)

**Graphics**:
- Must support Vulkan 1.3+ with SDL3 backend
- Must support multiple GPU vendors (NVIDIA, AMD, Intel)

**Distribution**:
- Must support WiX v4 MSI installer generation with digital signature
- Must integrate with Steamworks SDK for achievements, overlay, and multiplayer
- Must provide seamless integration with Steam client
- Must bundle Visual C++ Redistributable without manual user installation

**Testing**:
- Must pass Windows Application Verifier with zero warnings
- Must pass deterministic build verification on Windows 10 21H2+ and Windows 11
- Must pass 24-hour client stress test without memory leaks
- Must achieve <0.1% crash rate attributable to compiler/runtime

---

### 1.3 RISC-V (riscv64gc-unknown-linux-gnu)

**Primary Use Cases**: Sovereignty infrastructure, academic grants, UWW edge nodes

**ABI and Linking**:
- Must support `extern "C"` bindings to RISC-V system libraries
- Must enable static linking with `riscv64-linux-gnu-gcc` and `ld`
- Must support cross-compilation from x86_64 Linux host
- Must target `rv64gc` with `m`, `a`, `f`, `d`, `c` extensions enabled

**Determinism**:
- Must produce identical ELF64 binaries when compiled on different hosts (x86_64 vs RISC-V)
- Must not rely on RISC-V vendor-specific extensions in deterministic profile
- Must enforce deterministic floating-point rounding modes (or use fixed-point)
- Must enforce deterministic symbol mangling across architectures

**Hardware Support**:
- Must run on VisionFive 2 and Milk-V Pioneer hardware
- Must include VisionFive 2 in CI pipeline for regression testing
- Must support 24/7 uptime without crashes on RISC-V hardware

**Research and Grants**:
- Must enable RISC-V grant application (EU Horizon, US NSF)
- Must produce demo video showing UWW node running on $60 SBC
- Must achieve academic credibility with reproducible results

---

### 1.4 UWW-Compatible WASM (wasm32-unknown-unknown)

**Primary Use Cases**: Frontend and backend frameworks for UWW protocol, deterministic consensus applications

**WASM Execution**:
- Must emit WASM that runs within Firefox fork with UWW syscall bindings
- Must generate deterministic `.wasm` files with identical SHA-256 across compile hosts
- Must inject `.uww.metadata` section containing hash and attestation requirements
- Must support WASM SIMD via `-C target-feature=+simd128` when available

**Determinism**:
- Must strip all non-determinism from WASM emission: no timestamps, no embedded paths
- Must enforce deterministic floating-point (use soft-float or fixed-point in deterministic profile)
- Must replace nondeterministic instructions with deterministic alternatives
- Must enforce deterministic memory growth patterns

**Syscall Model**:
- Must support `@syscall` decorator for importing UWW runtime functions
- Must forbid WASI imports (`fd_read`, `clock_time_get`, `random_get`)
- Must provide deterministic alternatives (`uww::timestamp`, `uww::deterministic_rand`, `uww::storage::*`)
- Must ensure all syscalls are constant-time and have no side effects

**Size and Performance**:
- Must produce WASM files under 1MB for typical applications
- Must support aggressive dead code elimination in deterministic profile
- Must achieve ±5% of Rust WASM performance on compute benchmarks

**Testing**:
- Must pass hash verification across 5 different compile hosts
- Must pass UWW registry verification (WASM hash matches registered value)
- Must run identically on 3+ UWW nodes with state hash matching
- Must pass 24-hour soak test without memory leaks or divergence

---

## 2. Core Language Features

### 2.1 Deterministic Collections (Default)

**BTreeMap<K, V> / BTreeSet<T>**:
- Iteration order sorted by key (deterministic)
- Performance O(log n) operations
- Default, no annotation required
- Use cases: UWW state machine, consensus-critical paths, save/load data, VNode attributes

**Vec<T> with Deterministic Growth**:
- Capacity growth uses hardcoded doubling algorithm
- Deterministic capacity management
- Use cases: Append-only logs, archetype storage, VNode children, deterministic buffers

**Fixed-Size Arrays [T; N]**:
- Stack-allocated, fully deterministic
- Use cases: Small buffers, GPU vertex data, inline caches, performance-critical small data

### 2.2 Nondeterministic Collections (Opt-In)

**HashMap<K, V> / HashSet<T>**:
- Iteration order random (depends on runtime hash seed)
- Performance O(1) average, crucial for performance-critical code
- Requires `@nondeterministic` annotation on containing function/module
- Use cases: Render caches, temporary lookup tables where order doesn't affect correctness

**Vec<T> with Optimized Growth**:
- Capacity growth uses platform allocator heuristics
- Performance may reduce reallocations in workload-specific scenarios
- Requires `@allow_allocator_optimizations` annotation
- Use cases: Temporary buffers where exact capacity doesn't affect determinism

### 2.3 Deterministic Type System (Default)

**Fixed-Point Arithmetic (Fixed64, Fixed128)**:
- Default types for financial and consensus calculations
- Deterministic across all platforms
- No annotation required

**Deterministic RNG**:
- Provided by `uww::deterministic_rand(seed)` in UWW context
- Same seed produces same sequence on all platforms
- Default for server-side and consensus code

**Sorted Collections**:
- BTreeMap, BTreeSet are default map/set types
- Deterministic iteration order
- No annotation required

**Deterministic Iteration**:
- All iterator types guarantee deterministic order
- No reliance on address-based ordering

### 2.4 Nondeterministic Type System (Opt-In)

**Floating-Point (f32, f64)**:
- Available but nondeterministic due to rounding modes, optimizations
- Requires `@nondeterministic` annotation on any function using floats
- Compile error in deterministic profile: "f32 usage requires @nondeterministic or use Fixed64"
- Use cases: Rendering, physics approximations where performance > reproducibility

**Platform RNG**:
- `rand::thread_rng()`, `rand::gen_range()` use platform entropy
- Requires `@nondeterministic` annotation
- Use cases: Particle effects, procedural generation in game client (not server)

**Threading (std::thread)**:
- Thread spawning is nondeterministic due to scheduler
- Requires `@nondeterministic` annotation
- Use cases: Background asset loading, parallel ECS iteration (only in nondeterministic profile)

**Platform Time (Instant, SystemTime)**:
- `Instant::now()`, `SystemTime::now()` are nondeterministic
- Requires `@nondeterministic` annotation
- Use cases: Frame timing, profiling (not consensus logic)

**Unsorted Collections (HashMap)**:
- Hash iteration order is nondeterministic
- Requires `@nondeterministic` annotation
- Use cases: Performance-critical caches where iteration order doesn't affect correctness

---

## 3. Decorator System

**Syntax**: All annotations use Kotlin-style decorators (`@`) throughout

### 3.1 Built-In Decorators

**`@deterministic`**: Marks module as deterministic-only, enforces deterministic child code  
**`@nondeterministic`**: Marks function/module as exempt from deterministic rules  
**`@component`**: Defines framework component with lifecycle hooks  
**`@store`**: Defines state management store with deterministic mutations  
**`@middleware_stack`**: Defines composable middleware chain  
**`@executor`**: Defines single-threaded async executor  
**`@test`**: Marks unit test (deterministic by default)  
**`@profile`**: Marks function for performance instrumentation  
**`@hot_reload`**: Marks function for runtime code reloading  
**`@derive(Reflect, Serialize, Deserialize)`**: Auto-generates reflection and serialization code  
**`@syscall("uww::...")`**: Imports UWW runtime function  
**`@_c_import("header.h")`**: Imports C library function  
**`@preallocate(size = N)`**: Pre-allocates region memory  
**`@allow_allocator_optimizations`**: Allows nondeterministic Vec growth  

### 3.2 User-Defined Decorators

**Procedural Macro System**: Allows custom decorators via compile-time code generation  
**Requirements**:  
- Macro expansion must be deterministic (no randomness, no timestamps)  
- Macros must respect `@deterministic`/`@nondeterministic` context  
- Macro-generated code must pass same determinism checks as hand-written code  

---

## 4. Determinism Enforcement

### 4.1 Compile-Time Enforcement

**Deterministic Profile (`--profile deterministic`)**:
- Allows: BTreeMap, Fixed64, async, deterministic_hash(), uww::*, Sorted collections
- Forbids: HashMap, f32, Instant, std::thread, Platform RNG, Unsorted collections
- Error message: "Use of nondeterministic type in deterministic profile. Mark with @nondeterministic or use deterministic alternative."

**Release Profile (`--profile release`)**:
- Allows all features
- Emits lint warnings for nondeterministic types in critical paths
- Performance prioritized over reproducibility

### 4.2 Function-Level Enforcement

**`@deterministic` module**: Cannot contain nondeterministic code unless explicitly annotated  
**`@nondeterministic` function**: Explicitly exempt from deterministic rules  
**Error on violation**: Compile error at call site, not just definition  

### 4.3 Module-Level Enforcement

**`@deterministic` crate**: Entire crate must be deterministic (forces clean architecture)  
**Sandboxing**: Framework can isolate nondeterministic user code within deterministic core  

---

## 5. Memory Safety Model

### 5.1 Regions

**Syntax**: `region name { ... }`  
**Semantics**: All allocations within region freed in O(1) at region exit  
**Use Cases**: ECS archetype storage, GPU resource cleanup, level unloading, WASM linear memory management, VDOM tree allocation  

**Guarantees**:
- No `Drop` calls for individual objects (bulk deallocation)
- Compile-time checked (no use-after-free)
- Zero-cost in release builds (region metadata elided)

### 5.2 Generational References

**Runtime Detection**: Debug builds detect use-after-free via generational handles  
**Compile-Time Elision**: Release builds elide checks when possible  
**Constraint**: No suspend across active borrows (async safety)

### 5.3 Allocation Model

**Deterministic Allocator**: Bump allocator for short-lived data, no fragmentation  
**Preallocation**: `@preallocate(size = N)` on regions for performance-critical paths  
**No Dynamic Growth**: `memory.grow` forbidden in deterministic profile (static WASM memory)

---

## 6. FFI and ABI Stability

### 6.1 FFI Syntax

**`@_c_import("header.h")`**: Imports C library functions  
**`@syscall("uww::...")`**: Imports UWW runtime functions  
**`extern "C"`**: Specifies C ABI for function definitions  
**`repr(C)`**: Specifies C layout for structs  

### 6.2 ABI Guarantees

**Stability**: `extern "C"` ABI locked at 1.0, no breaking changes  
**Testing**: Every release tests FFI compatibility with previous version  
**Cross-Platform**: Same FFI bindings work on all platforms (different implementations)

---

## 7. Testing Framework

### 7.1 Deterministic Unit Tests

**`@test`**: Runs test in deterministic profile by default  
**Guarantees**: Same input → same output every run  
**Use Cases**: UWW state machine tests, consensus logic, save/load validation  

### 7.2 Nondeterministic Property Tests

**`@test @nondeterministic`**: Allows randomness for property-based testing  
**Use Cases**: Statistical tests, RNG distribution tests, performance benchmarks  
**Constraint**: Cannot be used in deterministic code paths  

---

## 8. Framework-Building Features

### 8.1 Component Model

**`@component`**: Defines framework component with lifecycle hooks  
**Composition**: Components can nest within deterministic parent components  
**Enforcement**: Framework ensures deterministic child components by default  

### 8.2 Virtual DOM

**`@deterministic`**: Enforces VNode structs contain only deterministic field types  
**BTreeMap for Attributes**: Deterministic iteration order for VDOM diffing  
**Vec with Fixed Growth**: Deterministic capacity management for VDOM children  

### 8.3 State Management

**`@store`**: Defines state management store with deterministic mutations  
**Logging**: Auto-generates deterministic mutation log for replay  
**Snapshotting**: Provides time-travel debugging via snapshots  

### 8.4 Middleware System

**`@middleware_stack`**: Defines composable middleware chain  
**Deterministic Order**: Executes middleware in Vec iteration order  
**Sandboxing**: Framework isolates nondeterministic user middleware within deterministic core  

### 8.5 Routing

**`@deterministic`**: Enforces route registration and resolution is deterministic  
**BTreeMap Storage**: Routes stored in sorted order  
**Compile-Time Validation**: Route patterns validated at build time  

### 8.6 Async Executor

**`@executor`**: Defines single-threaded async executor  
**FIFO Task Order**: Deterministic task polling order (VecDeque)  
**Single-Threaded**: No `Send`/`Sync` requirements (WASM-safe)  

---

## 9. Non-Functional Requirements

### 9.1 Performance

**Compile Time**: <1 second for 10k LOC with incremental compilation  
**Full Rebuild**: <30 seconds for 10k LOC  
**Runtime**: Within ±5% of Rust on compute benchmarks (Linux/Windows)  
**WASM**: Within ±5% of Rust WASM performance  

### 9.2 Memory Safety

**Regions**: O(1) bulk deallocation, compile-time checked  
**Leak-Free**: Pass Valgrind/ASAN clean in CI  
**UAF Detection**: Debug builds only, elided in release  
**24-Hour Soak**: Zero leaks in dedicated server/client tests  

### 9.3 Reliability

**Crash Rate**: <0.1% attributable to compiler/runtime  
**Determinism**: Identical artifacts across 3+ compile hosts for all platforms  
**Error Messages**: Clear, actionable compile errors for determinism violations  

### 9.4 Testing

**Coverage**: Framework and language have >90% test coverage  
**Determinism Tests**: All deterministic code must pass hash verification  
**Cross-Platform**: Tests run on all supported platforms in CI  

---

## 10. Scope Exclusions

**Platforms**: iOS, Android, Web (WASM for browsers) explicitly excluded until Year 3  
**Language Features**: Garbage collection, runtime exception unwinding, JIT compilation, dynamic reflection, trait objects  
**Engine Features**: Physics solver, animation library, networking protocol (all part of HeartOn, not language)  

---

## 11. Definition of Done

**Language Completion Criteria**:
1. Self-hosting compiler passes complete bootstrap with zero type errors
2. All four platforms (Linux, Windows, RISC-V, WASM) produce deterministic artifacts across 3+ compile hosts
3. UWW "Sovereign HRMS" demo runs identically on 5+ nodes with matching state hashes
4. Hearthshire ships on Steam for Windows and Linux with 95%+ positive reviews
5. HeartOn ECS runs 10k+ entities at 60 FPS on Linux and Windows
6. Three third-party developers successfully build and ship applications using Seen frameworks
7. RISC-V demo runs for 1 week without crash on VisionFive 2
8. All nondeterministic features are explicitly annotated and opt-in only
9. Framework-building features (@component, @store, @middleware_stack, @executor) are production-ready
10. Complete test suite passes with >90% coverage on all platforms