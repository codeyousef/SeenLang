# Seen Language — Design Document (Multi‑Platform, No Benchmarks)

> This document replaces earlier design drafts. It aligns with the updated MVP/Alpha/Beta/Release plans and removes all benchmark claims. It defines **what** Seen is, **why** it exists, and **how** it is built to target Linux, Windows, macOS, Android, iOS, and Web (JS/WASM).

---

## 1. Mission & Scope
**Mission:** A deterministic, self‑hosted systems language that makes **graphics and engine development safe by design** without sacrificing performance or ergonomics.

**Primary domains:**
- Graphics/compute (Vulkan, Metal, WebGPU) and engine runtime
- Real‑time simulation (ECS, job systems, audio)
- Cross‑platform application/game distribution

**Non‑goals (v1):** GC, dynamic reflection everywhere, runtime exceptions/unwinding, JIT by default, implicit allocations.

---

## 2. Core Principles
1. **Determinism first** — stable containers, explicit randomness, reproducible builds.
2. **Memory safety without borrow pain** — regions + RAII + generational handles.
3. **Zero‑cost by construction** — monomorphization + inlining; opt‑in features only.
4. **FFI parity** — exact C ABI, clear layout/align/pack controls.
5. **Ergonomics via types** — typestates/phantoms, sealed traits, pattern matching.
6. **Multi‑platform from day one** — Linux/Windows/macOS/Android/iOS/Web with first‑class tools.

---

## 3. Language Architecture
### 3.1 Type System
- Hindley–Milner style inference, **monomorphized generics**.
- **Traits** with associated types, orphan rule; **sealed traits** to restrict unsafe impls.
- **Sum types / enums** (closed), pattern matching with exhaustiveness checks.
- **Typestates** via phantom parameters to model object lifecycles (e.g., Vulkan command buffers).

### 3.2 Memory & Lifetimes
- **Regions**: stack/arena ownership scopes with bulk destruction.
- **RAII**: deterministic `Drop` at scope end; `defer` is LIFO.
- **Generational references**: UAF guard for opaque handles.
- **Async rule**: no suspend across active borrows; `move` into tasks; structured `scope {}` join.

### 3.3 Errors & Control Flow
- Recoverable: `Result<T,E>` + `match` (or `?` sugar).
- Unrecoverable: `panic` → **abort** (no unwinding).
- Attributes: `#[cold]`, `#[hot]`, `#[inline(always|never)]`.

### 3.4 Concurrency
- Atomics (all orders), fences, TLS.
- Fibers/coroutines with **structured concurrency** (spawn/scope/cancel/deadline).
- Bounded **MPMC channels** and work‑stealing job system.

### 3.5 Numerics
- Float environment attributes (FTZ/DAZ, rounding modes).
- SIMD‑friendly value types (`vec2/3/4`, `mat3/4`, `quat`) with 16‑byte alignment.
- Optional fixed‑point `Qm.n` type.

---

## 4. Interop & Layout
- **FFI:** `extern "C"`, function pointers, varargs (where available), callbacks.
- **Layout:** `#[repr(C|transparent)]`, `#[align(N)]`, `#[packed(N)]`, unions, bitfields (carefully constrained).
- **Symbols:** `#[no_mangle]`, `#[export(name=...)]`, deterministic mangling.
- **Visibility policy:** `Seen.toml` → `visibility = "caps" | "explicit"`, `export_alias = "ascii"` for non‑cased scripts.

---

## 5. Modules, Build & Tooling
- `seen build` with `--target <triple>`, `--shared`, `--static`, `--deterministic`, LTO/PGO hooks.
- `#[embed(path=...)]` for shader/data injection.
- `seen fmt` formatting rules (word‑operator spacing, visibility policy), `seen test`, `seen trace`, `seen pkg` (registry/lockfiles).
- LSP: hover, goto‑def, diagnostics, rename, semantic tokens, code actions.

---

## 6. Graphics & Shaders
- Backends: **Vulkan** (Linux/Windows/Android), **Metal** (macOS/iOS), **WebGPU** (Web); optional MoltenVK.
- Shader flow: author SPIR‑V/WGSL → emit MSL/DXIL/WGSL; integrate SPIRV‑Tools + SPIRV‑Cross/Tint.
- Render‑graph DSL compiles passes/resources/edges → barriers + lifetime checks (hazard‑free).

---

## 7. Engine Runtime
- **ECS**: archetype design with borrow‑safe queries (read/write sets), deferred structural changes, fixed‑step scheduler.
- **Jobs**: parallel_for, spawn_many, scoped joining; TLS arenas.
- **Audio**: RT‑safe callbacks (no alloc/locks); platform backends via FFI.
- **VFS & assets**: layered mounts, hot‑reload, versioned serde; stable content hashes.

---

## 8. Platforms & Distribution
**Triples & artifacts**
- Linux: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` → `.so` + ELF.
- Windows: `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` → `.dll` + `.exe`.
- macOS: `x86_64-apple-darwin`, `aarch64-apple-darwin` (Universal2) → `.dylib` + app bundle (codesigned/notarized).
- Android: `aarch64-linux-android` (+ `armeabi-v7a` optional) → `.so` + AAB; JNI bridge.
- iOS: `aarch64-apple-ios`, `x86_64-apple-ios` (sim) → `.framework`/`.a` + IPA (Metal).
- Web: `wasm32-unknown-emscripten` (threads/SIMD) or `wasm32-unknown-unknown` → `.wasm` + loader; COOP/COEP headers.

**Platform shims**
- Window/input/gamepad: SDL3 or native (Win32, Cocoa/AppKit, UIKit, NativeActivity, Web APIs).
- Audio: PipeWire/ALSA, WASAPI, CoreAudio/AVAudio, AAudio/Oboe, WebAudio.
- Filesystem: native + Emscripten MEMFS/IDBFS for Web persistence.

---

## 9. Security, Packaging, Governance
- Signed/notarized releases per platform; registry with checksums/signing; SBOM.
- Plugin sandbox with capability tokens (fs/net/time), deny‑by‑default.
- Governance: RFCs, maintainers council, LTS policy.

---

## 10. Roadmap Pointers (no perf claims)
- MVP → cross‑platform runnable samples; deterministic self‑host.
- Alpha → macro hygiene, mini‑engine, registry, plugin ABI.
- Beta → optimization passes, `seen-std`, backend parity, trace GUI.
- Release → v1.0 spec freeze, signing, LTS, governance.

*All performance assertions are intentionally omitted here; only architecture and requirements are specified.*

