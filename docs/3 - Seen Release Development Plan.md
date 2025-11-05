# Seen Language — Release Plan (Multi‑Platform + SIMD) — Agent‑Executable Spec

Purpose: Finalize version 1.0 with locked grammar, IR, and ABI, certified determinism, secure distribution, and long‑term support. Steps are written for execution by a coding agent. No sample code is included.

────────────────────────────────────────────────────────────────────────
A) Language and Compiler Freeze

A1. Versioned Specification
• Produce a document titled “spec‑v1.0” containing grammar, type rules, visibility policy, regions and destruction ordering, concurrency rules, attributes catalog, FFI/ABI layout guarantees, numerics and SIMD policies, and determinism constraints.
Acceptance: The document is published under the documentation directory and referenced by the compiler as its versioned specification.

A2. Compatibility and Deprecation Policy
• Define forward and backward compatibility rules and a deprecation schedule for language and library items.
Acceptance: The policy file exists and is linked from the specification and the project website.

A3. IR Stability
• Freeze the intermediate representation format and textual dump schema.
Acceptance: Tools relying on the IR are not broken by minor releases; changes are queued for the next major version.

────────────────────────────────────────────────────────────────────────
B) Determinism and Reproducibility Certification

B1. Reproducible Builds per Platform
• Ensure that two builds with identical inputs and flags yield bit‑identical outputs on supported platforms.
Acceptance: A certification script compares outputs and reports identity or a precise diff when not identical.

B2. Determinism Profiles for Libraries and Plugins
• Provide a description of how libraries and plugins should respect deterministic mode including stable container usage and the prohibition of nondeterministic APIs.
Acceptance: The documentation appears in a guide titled “determinism for integrators”.

────────────────────────────────────────────────────────────────────────
C) Security and Distribution

C1. Signing and Notarization
• Sign Windows binaries, notarize macOS app bundles, sign Android application bundles, sign iOS packages, and publish Web assets with subresource integrity metadata. Store signatures next to artifacts.
Acceptance: Verification commands succeed for each artifact type.

C2. Package Registry Security
• Enforce checksum verification and optional signing of packages. Provide a verification command in the build driver.
Acceptance: Installing or verifying a package with altered content fails and reports the mismatch.

C3. Plugin Sandbox
• Enforce capability declarations for filesystem, networking, and time. Deny undeclared operations by default and provide diagnostics.
Acceptance: A plugin without declared capabilities cannot access restricted resources.

────────────────────────────────────────────────────────────────────────
D) Ecosystem and LTS

D1. Certified Core Libraries and Backends
• Publish and maintain certified releases of the foundation library, Vulkan, Metal, WebGPU wrappers, ECS, audio, and networking libraries.
Acceptance: Certification involves passing the canonical scenes, determinism checks, and packaging policies.

D2. Long‑Term Support Channel
• Provide a channel that receives security fixes and critical patches for twenty‑four months.
Acceptance: The update command recognizes the LTS channel and switches the project to it.

────────────────────────────────────────────────────────────────────────
E) Documentation, Governance, and Processes

E1. Book v1.0 and Deployment Guides
• Publish the complete book with chapters for each platform’s packaging requirements and verification steps.
Acceptance: Each chapter includes prerequisites, a numbered procedure, and a final checklist of expected files and signatures.

E2. Governance
• Establish a foundation or equivalent governance structure, a maintainers council, and a public RFC process with a repository for proposals.
Acceptance: The governance documents are published and referenced from the main documentation index.

────────────────────────────────────────────────────────────────────────
F) SIMD Stability Contract

F1. Intrinsics Surface Freeze
• Freeze the list of intrinsics in the core SIMD module. Each intrinsic must document lane widths, supported targets, and a scalar fallback.
Acceptance: The documentation is published and versioned. Removing or altering intrinsics requires a major version.

F2. Feature Detection ABI
• Provide a stable runtime query mechanism for CPU features and target capabilities.
Acceptance: The mechanism is documented and its symbol names are reserved and versioned.

F3. Fat Libraries With Dispatch
• For select libraries, ship multi‑versioned slices that perform runtime dispatch to the best available variant.
Acceptance: The packaging places variant slices in a deterministic structure and the dispatch records the selected variant.

────────────────────────────────────────────────────────────────────────
G) Performance Certification & Observatory

G1. Benchmark Canonization
• Publish a v1.0 “performance canon” suite with automated runs across desktop, mobile, and web, comparing Seen outputs against Rust/C++ baselines for runtime, peak memory, binary size, and compile time (docs/research/10 & 13).
Acceptance: The public performance portal shows the suite, target metrics, and parity deltas; CI fails when regressions exceed agreed thresholds.

G2. Optimizer Provenance Lock
• Version and hash the equality-saturation rule sets, ML heuristic models, and LENS-derived superoptimizer recipes; record them in release metadata and expose via `seen --diagnose-optimizer`.
Acceptance: Rebuilding with the recorded artifacts reproduces identical optimizer decisions; provenance hashes appear in the release notes.

G3. Hardware & Topology Validation
• Certify APX/AVX10, SVE/SVE2, RVV, and WASM SIMD codegen as well as CXL-aware placement policies; document supported configurations and fallback behaviour.
Acceptance: Automated validation runs pass on reference hardware (or emulation) and produce signed reports archived with the release.

────────────────────────────────────────────────────────────────────────
Release Definition of Done

• “spec‑v1.0” and compatibility policy are published and referenced by the compiler.
• Reproducible build certification passes on supported platforms.
• Determinism integration guide exists for libraries and plugins.
• All distribution artifacts are signed and verifiable; notarization and integrity metadata are present where applicable.
• Package registry verifies checksums and signatures; the verification command is available and fails on tampering.
• Plugin sandbox denies undeclared operations and reports clear diagnostics.
• Certified libraries and backends are published and meet canonical scene and determinism criteria.
• LTS channel is active and switchable.
• Governance documents and the RFC process are public.
• SIMD intrinsics surface is frozen and documented; feature detection ABI is documented; fat libraries are packaged deterministically.
• Performance canon, optimizer provenance, and hardware/topology validation reports are published and passing.
