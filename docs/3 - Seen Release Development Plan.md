# Seen Language — **Release** Plan (Multi‑Platform Updated)

Release finalizes **v1.0** with locked grammar/IR/ABI, certified determinism, and production distribution **across Linux, Windows, macOS, Android, iOS, and Web**.

---

## 1) Objectives
- Freeze language and compiler; certify reproducibility and determinism.  
- Sign and notarize artifacts; secure registry; plugin sandboxing.
- Establish governance, LTS policy, and enterprise onboarding.

---

## 2) Tracks

### A) Compiler & Language Freeze (v1.0)
- `spec-v1.0.md` with formal grammar/ABI; compatibility/rust‑style RFCs.  
- IR stability; deprecation policy; semantic versioning.

### B) Performance & Determinism Certification
- Reproducible builds per target; deterministic profiles validated (WASM included).  
- Performance parity checks (desktop/mobile/web) with published results.

### C) Security & Distribution
- Signed releases:  
  - **Windows:** Authenticode; optional MSIX.  
  - **macOS:** codesign + notarization (Universal2).  
  - **Android:** AAB signing v2+; Play integrity.  
  - **iOS:** App Store signing; entitlements.  
  - **Web:** Subresource Integrity (SRI) + COOP/COEP headers.
- Package registry audit; signed packages; SBOMs; `seen verify`.
- Plugin sandbox: capability tokens; deny‑by‑default for fs/net/time.

### D) Ecosystem & LTS
- Certified core libs: `seen-std`, `seen-vulkan`, `seen-metal`, `seen-webgpu`, `seen-ecs`, `seen-audio`, `seen-net`.  
- LTS channel (24‑month support); security‑only patching; `seen update --lts`.

### E) Docs & Outreach
- **Seen Book v1.0**; platform deployment guides (AAB/IPA/notarization/MSIX/Web).  
- Enterprise integration guide; migration and stability guarantees.

### F) Governance
- Seen Foundation; maintainers council; RFC repo and process; public roadmap cadence.

---

## 3) Release DoD (Updated)
| Area | Requirement |
|------|-------------|
| Language | v1.0 spec frozen (grammar/IR/ABI). |
| Compiler | Reproducible, signed, notarized per platform; determinism certified. |
| Ecosystem | Certified core libs & backends; registry secure; plugin sandboxing. |
| Docs | Book v1.0; deployment guides; enterprise handbook. |
| Governance | Foundation + RFCs + LTS policy live. |

---

## 4) Post‑v1.0 Horizons
- GPU DSL 2.0 / shader subset growth.  
- Package trust chains & binary transparency.  
- Console platform enablement (as partnerships allow).

