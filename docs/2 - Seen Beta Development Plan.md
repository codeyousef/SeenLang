# Seen Language — **Beta** Plan (Multi‑Platform Updated)

Beta emphasizes **performance, parity, and robustness** across Linux, Windows, macOS, Android, iOS, and Web (JS/WASM).

---

## 1) Objectives
- Optimize compilers/backends (x86_64, RV64, Metal, Vulkan, WebGPU) and stdlib.
- Harden determinism and long‑run stability across platforms.
- Expand `seen-std`, official plugins, and engine scale.

---

## 2) Tracks

### A) Compiler & Runtime Optimization
- LTO/PGO pipelines; sanitizer integrations; cross‑module inlining.
- SIMD passes for x86 (SSE/AVX) and RVV; WebAssembly SIMD/threads tuning.
- Memory/layout audits for mobile/web constraints (allocator arenas, small‑footprint modes).

### B) Ecosystem Maturity
- **`seen-std`**: collections (stable iteration), math, io, serde, concurrency utils.  
- Certified plugin catalog: graphics (Vulkan/Metal/WebGPU), physics, audio, networking.

### C) Platform & Engine Scaling
- Backend parity: D3D12 (Windows) or DXC path for DXIL; Metal/MSL features (argument buffers); WebGPU stability.
- ECS parallelism across archetypes; 10k+ jobs/frame in job system.  
- Mobile: thermal budgets, memory caps, lifecycle resiliency.

### D) DX & Tooling
- `seen trace` GUI (timeline/spans/GPU markers); gdb/lldb pretty‑printers.  
- Advanced LSP: refactors, code actions, semantic tokens; project‑wide formatting policies.

### E) Determinism & CI
- Multi‑arch determinism suite; WASM identical outputs under `--deterministic`.  
- Long‑run soak tests for ECS/Render‑graph; perf regression alerts.

---

## 3) Beta DoD (Updated)
| Area | Requirement |
|------|-------------|
| Compiler | LTO/PGO; SIMD (x86/RVV/WASM); sanitizers; stable profiles. |
| Runtime | `seen-std` stable; low‑footprint/mobile modes. |
| Engine | Backend parity (Vulkan/Metal/WebGPU/DX12); 10k+ jobs/frame; ECS parallel safety. |
| Tooling | Trace GUI; IDE/LSP full features; debugger bridges. |
| Determinism | Cross‑platform determinism suite green. |
| CI | Matrix covers 6 targets with scale tests. |

---

## 4) Transition to Release
- Freeze IR/ABI candidates; complete reproducible build guarantees per target.  
- Audit plugins/packages; finalize documentation; prepare governance & LTS.

