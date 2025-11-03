# Seen Language — **Alpha** Plan (Multi‑Platform Updated)

Alpha focuses on stabilization and ecosystem readiness **across Linux, Windows, macOS, Android, iOS, and Web (JS/WASM)**.

---

## 1) Objectives
- Freeze syntax/semantics; polish diagnostics and IDE experience.
- Validate cross‑platform builds with a runnable **mini‑engine**.
- Roll out package registry + plugin ABI; harden CI/CD.

---

## 2) Tracks (Delta‑aware)

### A) Language Stability & Ergonomics
- Macro hygiene (attribute + item/proc); better error spans across expansions.
- Region/borrow visualizations in diagnostics; `seen doctor`.
- `seen fmt` team‑wide config; CI `--check` gate.

### B) Engine Integration (All Targets)
- **Mini‑engine** repo `seen-engine-min` using: window/input, audio, file IO, job system.
- **Backends:** Vulkan (Linux/Win/Android), Metal (macOS/iOS), WebGPU (Web). MoltenVK optional.
- **Shaders:** `seen build shaders` cross‑compiles SPIR‑V/WGSL to MSL/DXIL/WGSL.

**Definition of Done (Engine Alpha):**
- Deterministic fixed‑step loop; input/gamepad; audio tone; hot‑reload (VFS watcher).  
- **Zero validation errors** on canonical scenes on all targets.

### C) Ecosystem Infrastructure
- Online **package registry** (login/publish/search); lockfile checksums; vendor mode.  
- **Plugin ABI** versioning + loader; capability flags (fs/net/time).  
- Signed artifacts (Win .exe/.dll, macOS notarized app; Android AAB; iOS IPA).

### D) Tooling & CI Matrix
- Incremental/cached builds; `seen trace` → `seen replay` CLI.  
- **CI Matrix**: Linux (x64/arm64), Windows (x64/arm64), macOS (U2), Android (arm64), iOS (device+sim), Web (Emscripten).  
- Store headers for Web demos (COOP/COEP), Android/iOS manifest templates.

### E) Documentation & Education
- **Seen Book (Alpha)** with platform chapters: Linux/Win/macOS/Android/iOS/Web.  
- Vulkan/Metal/WebGPU guides; mini‑engine and plugin tutorials.

---

## 3) Alpha DoD (Updated)
| Area | Requirement |
|------|-------------|
| Language | Syntax & core semantics frozen; macro hygiene stable. |
| Engine | Mini‑engine runs on all 6 targets; zero validation errors; deterministic replay verified. |
| Ecosystem | Registry online; plugin ABI stable; signed/notarized packages. |
| Tooling | Incremental builds; trace/replay; CI matrix green. |
| Docs | Seen Book (Alpha) live; platform bring‑up guides complete. |

---

## 4) Transition to Beta
- Gather multi‑platform feedback; prioritize perf issues (mobile/web).  
- Promote `seen-std` foundation crate; broaden official plugins.  
- Begin Beta with performance and scale as primary focus.

