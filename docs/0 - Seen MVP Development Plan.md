# Seen Language — Unified **MVP** Plan (Multi‑Platform Updated)

This replaces previous MVP notes. It merges **Pre‑Bootstrap (PB)**, **Pre‑Self‑Host (PSH)**, and essential **Post‑Self‑Host (POST‑for‑MVP)** items so that the **engine + game** compile and run on **Linux, Windows, macOS, Android, iOS, and Web (JS/WASM)**.

---

## 1) Current Progress Snapshot
- Lexer/Parser ✅
- Type system (HM inference, traits, monomorphization, sealed classes) ✅
- Memory model (regions, RAII, generational refs, deterministic drop) ✅
- FFI/ABI (`extern "C"`, `repr(C)`, unions, align/pack, stable symbols) ✅
- Codegen (LLVM + deterministic IR emission) ✅
- LSP (hover, goto‑def, diagnostics, format, refs) ✅
- Tooling/CLI (`build/test/bench/fmt`, target triples, `--deterministic`) ✅
- Self‑hosting (Stage0→Stage1→Stage2 deterministic) ✅

> **Delta from earlier plans:** This MVP now **includes multi‑platform bring‑up** for minimal runnable samples on all targets.

---

## 2) Phase PB — Pre‑Bootstrap (**done**)
- Unicode NFC; visibility via `caps` or `explicit` (`Seen.toml`).
- Result/abort error model; word‑operator precedence frozen; formatter enforces style.
- RAII `defer`; region exit order; generational refs.
- Deterministic IR: sorted emission, no timestamps/paths.
- Runtime split: `seen_core` (compiler) vs `seen_std` (later).
- CLI determinism profile.

## 3) Phase PSH — Pre‑Self‑Host (**done**)
- Typestates + phantom types; sealed traits; monomorphized generics.
- Async boundary rules (no suspend across borrows; `move` into tasks; scoped joins).
- Atomics/fences/TLS; minimal channels/fibers/job stubs.
- `#[embed]` for shader/data blobs; `--shared`/`--static` outputs.

---

## 4) POST‑for‑MVP — Cross‑Platform Essentials (NEW)
These items were previously post‑MVP; they are now **required to conclude MVP**.

### 4.1 Target Triples & Artifacts
- **Linux:** `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` → `.so` + ELF exe.
- **Windows:** `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` → `.dll` + `.exe`.
- **macOS:** `x86_64-apple-darwin`, `aarch64-apple-darwin` (**Universal2**) → `.dylib` + app bundle (codesigned/notarized).
- **Android:** `aarch64-linux-android` (NDK r26+), optional `armeabi-v7a` → `.so` + **AAB**; JNI bridge.
- **iOS:** `aarch64-apple-ios` (device), `x86_64-apple-ios` (sim) → `.framework`/`.a` + **IPA**; Metal only.
- **Web:** `wasm32-unknown-emscripten` (threads+SIMD) and/or `wasm32-unknown-unknown` → `.wasm` + loader.

### 4.2 Graphics Backends & Shaders
- **Vulkan** (Linux/Windows/Android).
- **Metal** (macOS/iOS) native; **MoltenVK** optional for Vulkan‑API parity.
- **WebGPU/WebGL2** (Web) via Emscripten; WGSL preferred.
- **Shader toolchain:** author SPIR‑V/WGSL → emit **MSL** (Metal), **DXIL** (D3D12), **SPIR‑V** (Vulkan), **WGSL** (WebGPU). Integrate SPIRV‑Tools + SPIRV‑Cross/Tint into `seen build shaders`.

### 4.3 Platform Services
- **Windowing/Input/Gamepad:** SDL3 (bootstrap) or native shims (Win32, Cocoa/AppKit, UIKit, NativeActivity, Web events).
- **Audio:** PipeWire/ALSA (Linux), WASAPI/XAudio2 (Win), CoreAudio/AVAudio (Apple), AAudio/Oboe (Android), WebAudio (Web).
- **Filesystem/VFS:** native FS + layered VFS; Emscripten MEMFS/IDBFS for persistent Web storage.
- **Networking:** BSD/WinSock; Web fetch/websockets.

### 4.4 Toolchains & Packaging
- **Android:** Gradle + CMake + NDK r26+; AAB signing; `android:exported` configured.
- **iOS/macOS:** Xcodeproj gen; codesign; entitlements; **Universal2** build.
- **Windows:** MSVC v143; SDK 10.0.22000+; optional MinGW later.
- **Web:** Emscripten 3.1+; flags: `-sPTHREADS=1 -sUSE_WEBGPU=1 -sWASM_BIGINT`; set COOP/COEP for SharedArrayBuffer.

### 4.5 Determinism & Mobile/Web Limits
- Enforce `--deterministic` build profile; stable containers; fixed RNG seeds.
- WASM memory caps and async API returns `Result` (no panics).
- Mobile: foreground/background app lifecycle hooks; low‑memory callbacks.

---

## 5) MVP **Definition of Done** (Revised)
- **Self‑host:** Stage2 == Stage3 (deterministic) on Linux + macOS; Stage2 reproduces on Windows CI.
- **Samples (all six targets):**
   - *Graphics:* triangle + textured quad on Vulkan/Metal/WebGPU with **zero validation errors**.
   - *Systems:* input + audio beep + file read.
- **Artifacts:** AAB (Android), IPA (iOS), notarized app (macOS), signed exe/dll (Win), WASM demo with COOP/COEP (Web).
- **Tooling:** `seen build shaders`, `seen pkg` (local), `seen trace` (basic spans), `seen fmt` enforced.
- **Docs:** `/docs/spec` + platform bring‑up guides.

---

## 6) What Moves to Alpha (unchanged intent)
- Macro/DSL depth (render‑graph blocks), compile‑time reflection expansions, package registry online, plugin distro at scale, profiler GUI, ECS/gameplay templates.

