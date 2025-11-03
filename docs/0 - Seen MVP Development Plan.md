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

## 2) Phase PB — Pre‑Bootstrap (In Progress)
Pre‑bootstrap should make the Rust toolchain a stable foundation before we attempt Stage‑1. These items were previously marked complete but are still missing. Break them down and check them off as we implement them:

- [x] **Unicode NFC + visibility policy**
  - Normalize identifiers/literals to NFC during lexing.
  - Support `Seen.toml` switches for `caps`/`explicit` visibility and error when source disagrees.
- [x] **Result/Abort error model**
  - Wire a consistent `Result<T, E>` type across compiler crates.
  - Add an `abort` intrinsic for unrecoverable failures and ensure diagnostics surface it.
- [x] **Operator precedence & formatter lock**
  - Freeze word/operator precedence tables in the parser.
  - Extend formatter/pretty-printer so it enforces the frozen precedence (no drift across runs).
- [x] **RAII `defer` + generational refs runtime**
  - ✅ Interpreter defer stack + scope unwinding complete (`seen_interpreter` + tests).
  - ✅ Task runtime now uses generational handles with stale-handle tests in `seen_concurrency`.
  - ✅ Channel handles: generational IDs + validation in runtime/interpreter.
  - ✅ Actor handles: generational IDs and stale-handle detection integrated with actor system.
  - ✅ LLVM backend/runtime parity so compiled stages enforce the same invariants.
- [ ] **Deterministic IR emission**
  - Ensure IR generator/optimizer emit sorted structures (no HashMap iteration).
  - Add regression test that hashes IR for the same input twice and matches.
- [ ] **Runtime split**
  - Carve shared code into `seen_core` (compiler) vs `seen_std` (future runtime) crates.
  - Update CLI to depend only on `seen_core`.
- [ ] **CLI determinism profile**
  - Introduce `--profile deterministic` (or similar) that locks randomness, timestamps, temp paths.
  - Document usage in quickstart/plan.

## 3) Phase PSH — Pre‑Self‑Host (Pending)
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

7)
S1) Scope and Non‑Goals for SIMD (MVP)

S1.1 Scope
• Introduce a **portable SIMD baseline** that automatically vectorizes straightforward numeric loops and common math operations across eligible targets (desktop, mobile, web) with safe scalar fallbacks.
• Provide **portable vector types**, **numerics intrinsics**, and **compiler flags** to control vectorization policy and reporting.
• Ensure **determinism compatibility** by allowing builds to force scalar lowering.

S1.2 Non‑Goals (MVP)
• No hand‑written architecture‑specific pipelines required.
• No mandatory user annotations to get vectorization.
• No changes to the existing memory model, error model, or FFI/ABI.

Acceptance (S1): This section is appended without modifying any prior MVP sections or checklists.

────────────────────────────────────────────────────────────────────────
S2) Targets and Capability Model

Inputs: current target matrix from MVP (Linux/Windows/macOS/Android/iOS/Web).
Outputs: per‑target SIMD capability record.

S2.1 Target Support Baseline
• x86‑64: enable SSE2+ and allow AVX/AVX2/AVX‑512 when available.
• ARM64: enable NEON by default.
• RISC‑V: enable RVV 1.0 when toolchain advertises support.
• Web: enable **WASM SIMD** when toolchain and headers permit; otherwise use scalar fallback.

S2.2 Capability Detection
• Build‑time records: which vector ISAs are compiled into artifacts.
• Run‑time records (optional in MVP): a single capability bitset per process to select paths if multi‑versioning is present.

Constraints: if a target lacks SIMD, the scalar path must be selected automatically.
Acceptance (S2): A capabilities table exists in the build log for each target indicating whether SIMD was enabled and which families were considered.

────────────────────────────────────────────────────────────────────────
S3) Language and Library Surface (Portable Types and Intrinsics)

Inputs: type system and attribute system as already defined in MVP.
Outputs: a **documented list** of portable vector types and numerics intrinsics.

S3.1 Portable Vector Types (MVP set)
• Provide fixed‑width vector types with documented element type and lane count (examples of categories only; no code):
– 32‑bit float lanes (4‑wide)
– 64‑bit float lanes (2‑wide)
– 8/16/32‑bit integer lanes (16/8/4‑wide respectively)
• Each type has a **scalar fallback** implementation and a **16‑byte alignment requirement**.

S3.2 Numerics Intrinsics (MVP set)
• Provide the following operations with defined semantics and error behavior:
– Fused multiply‑add
– Reciprocal square root
– Minimum and maximum (exact and fast‑approx variants)
– Horizontal reductions for sum/min/max

Constraints: Operations must either be total on the operand domain or document the exact exceptional conditions and returned sentinel behavior.
Acceptance (S3): A reference page lists every type and intrinsic, alignment requirements, result domains, and fallback availability per target.

────────────────────────────────────────────────────────────────────────
S4) Compiler Pipeline and Control Flags

Inputs: existing build driver and optimization pipeline.
Outputs: vectorization policy switches, deterministic reporting, and stable lowering behavior.

S4.1 Vectorization Policy Flag
• Add a policy switch recognized by the build driver with three values:
– **off**: disables vectorization globally.
– **auto** (default): enables vectorization for straight‑line loops and reductions when profitable under the cost model.
– **max**: attempts vectorization aggressively where legal; never violates correctness.

S4.2 Capability and Target Flags
• Add a CPU targeting switch that allows “native” and named micro‑architectures; if unspecified, use a conservative default suitable for distribution.
• Ensure that Web builds enable or disable WASM SIMD based on toolchain settings and required headers.

S4.3 Reporting Flag
• Add a reporting flag that emits a **deterministic table** listing: function identifier, loop identifier, vectorization decision, and reason for non‑vectorization when applicable (examples of reasons: data dependence, misalignment, unknown trip count, guarded memory access).

Constraints: Reports must be text‑only and stable across identical inputs and flags.
Acceptance (S4): Invoking the build with the reporting flag produces a table file alongside artifacts for each compilation unit.

────────────────────────────────────────────────────────────────────────
S5) Data Layout Policies and Helpers

Inputs: current memory model and struct layout controls.
Outputs: documented policies and small helper APIs (no code here) that guide vector‑friendly layouts.

S5.1 Alignment Policy
• Default **16‑byte alignment** for portable vector types.
• Document cache‑line assumptions used by the cost model.

S5.2 Layout Helpers (MVP)
• Provide documented helpers that encourage **structure‑of‑arrays** or **array‑of‑structures** choices without automatic source transformation.
• Provide guidance for choosing 32‑bit indices where safe to improve bandwidth.

Acceptance (S5): The documentation set contains a section titled “SIMD Layout Guidance” describing alignment and layout choices, with a link from the numerics chapter.

────────────────────────────────────────────────────────────────────────
S6) Determinism Interaction

Inputs: deterministic build/profile from MVP.
Outputs: documented interaction rules.

S6.1 Deterministic Mode Behavior
• When deterministic mode is active, the build must be allowed to **force scalar lowering**.
• Otherwise, within any given target profile, vector and scalar results must be **result‑equivalent** under the documented numerics model.

Acceptance (S6): Enabling deterministic mode produces artifacts that match scalar outputs exactly for the same inputs and flags.

────────────────────────────────────────────────────────────────────────
S7) Minimal Acceptance for SIMD in MVP

S7.1 Type and Intrinsics Availability
• The portable vector types list and the numerics intrinsics list are present in the documentation index.
• The build driver accepts the vectorization policy and reporting flags; invalid combinations are rejected with a clear message.

S7.2 Cross‑Target Enablement
• Desktop targets indicate SIMD enabled in capability records.
• Mobile targets indicate NEON availability where applicable.
• Web target indicates WASM SIMD enabled when headers/toolchain permit; otherwise lists scalar fallback.

S7.3 Report Presence
• Building a project with reporting enabled produces a deterministic vectorization report next to each object or module artifact.

S7.4 No Behavioral Regressions
• Existing MVP samples for graphics and systems **must run unchanged**.
• The addendum **does not alter** the error model, RAII/regions semantics, FFI/ABI, visibility policy, or any already complete MVP tasks.

Acceptance (S7): All prior MVP acceptance checks still pass; SIMD reports exist; capability tables reflect per‑target status; deterministic mode forces scalar successfully.

────────────────────────────────────────────────────────────────────────
S8) Traceability and Ownership

S8.1 Trace to MVP Sections
• Link this addendum to the existing MVP sections: build driver flags, numerics chapter, platform matrix, and determinism profile.

S8.2 Ownership
• Assign a responsible owner for: type surfaces, compiler flags, reporting, documentation, and target enablement. Owner names belong in your planning tracker and are not part of this document.

Acceptance (S8): The links are present in the documentation index; owners are recorded in the planning tracker.
