# Se enLang Project Documentation Index

**Generated:** 2025-11-17  
**Project:** SeenLang Compiler  
**Documentation Type:** Master Index for AI-Assisted Development

---

## 🎯 Quick Navigation

### For AI Agents & Developers

This index serves as the **primary entry point** for understanding the SeenLang compiler codebase. All paths are relative to the project root.

---

## 📊 Project Overview

- **Type:** Monorepo (Cargo workspace)
- **Primary Language:** Rust (2021 Edition)
- **Architecture:** Multi-stage Compiler Pipeline
- **Crates:** 24+ modules
- **Status:** Alpha (Self-hosting in progress)

### ⚠️ Critical Information

**LLVM Backend Missing:** The project currently lacks a complete LLVM backend for AOT compilation. This is blocking production benchmarks and optimized binary generation. The JIT path via Cranelift is functional.

---

## 📚 Generated Documentation (BMM)

### Core Architecture Documents

- **[Project Overview](./bmm-project-overview.md)** - Executive summary, tech stack, project goals
- **[Architecture](./bmm-architecture.md)** - Complete compiler architecture, pipeline stages, LLVM gap analysis

### Component Documentation

- **[Development Guide](./bmm-development-guide.md)** _(To be generated)_ - Build instructions, testing, workflows
- **[Source Tree Analysis](./bmm-source-tree.md)** _(To be generated)_ - Annotated directory structure
- **[Component Inventory](./bmm-component-inventory.md)** _(To be generated)_ - Module breakdown and responsibilities

---

## 📖 Existing Project Documentation

### Language Specification & Design

| Document | Purpose | Path |
|----------|---------|------|
| **Language Spec** | Complete language specification v0.9.1 | `./Seen Language Spec.md` |
| **Design Document** | Core principles, architecture, memory model | `./Seen Design Document.md` |
| **Syntax Design** | Syntax rules, keywords, grammar | `./Seen Syntax Design Document.md` |

### Development Roadmap

| Document | Phase | Path |
|----------|-------|------|
| **MVP Plan** | Phase 0 - Core functionality | `./0 - Seen MVP Development Plan.md` |
| **Alpha Plan** | Phase 1 - Self-hosting & tooling | `./1 - Seen Alpha Development Plan.md` |
| **Beta Plan** | Phase 2 - Optimization & ecosystem | `./2 - Seen Beta Development Plan.md` |
| **Release Plan** | Phase 3 - Production readiness | `./3 - Seen Release Development Plan.md` |

### Technical Plans

| Document | Purpose | Path |
|----------|---------|------|
| **Self-Hosting Plan** | Bootstrap strategy & stages | `./SELF_HOSTING_PLAN.md` |
| **Installer Plan** | Installation & distribution | `./Installer Plan.md` |
| **VSCode Extension** | Editor integration plan | `./VSCode Extension Plan.md` |

### Operational Guides

| Document | Purpose | Path |
|----------|---------|------|
| **Quick Start** | Getting started guide | `./quickstart.md` |
| **Operations** | Operational procedures | `./operations.md` |
| **Release Playbook** | Release process | `./release-playbook.md` |
| **Crash Triage** | Debugging crashes | `./crash-triage.md` |
| **Concurrency Patterns** | Concurrency best practices | `./concurrency-patterns.md` |
| **Performance Baseline** | Performance standards | `./performance-baseline.md` |
| **Performance Dashboard** | Metrics tracking | `./performance-dashboard.md` |

### Status Reports

| Document | Purpose | Path |
|----------|---------|------|
| **Implementation Status** | Feature implementation tracking | `../IMPLEMENTATION_STATUS.md` |
| **Benchmark Status** | Benchmark suite status | `../BENCHMARK_STATUS.md` |
| **LLVM Backend Status** | LLVM backend progress | `../LLVM_BACKEND_PRODUCTION_STATUS.md` |

---

## 🗂️ Repository Structure

```
seenlang/
├── seen_*/                     # Compiler crates (24+modules)
│   ├── seen_lexer/            # Tokenization
│   ├── seen_parser/           # AST construction
│   ├── seen_typechecker/      # Type inference
│   ├── seen_ir/               # Intermediate representation
│   ├── seen_mlir/             # MLIR optimizations
│   ├── seen_cranelift/        # Cranelift JIT backend ✅
│   ├── seen_runtime/          # Runtime system
│   ├── seen_core/             # Core utilities
│   ├── seen_cli/              # CLI interface
│   ├── seen_lsp/              # Language server
│   ├── seen_oop/              # OOP features
│   ├── seen_concurrency/      # Async/concurrency
│   ├── seen_reactive/         # Reactive programming
│   ├── seen_effects/          # Effect system
│   ├── seen_advanced/         # Advanced features
│   ├── seen_memory_manager/   # Memory management
│   ├── seen_shaders/          # Shader support
│   ├── seen_tooling/          # Dev tools
│   └── seen_self_hosting/     # Self-hosting infra
│
├── docs/                       # Documentation
│   ├── bmm-*.md               # Generated BMM docs
│   ├── Seen Language Spec.md
│   ├── Seen Design Document.md
│   └── [development plans]
│
├── benchmarks/                 # Performance benchmarks
├── examples/                   # Example programs
├── tests/                      # Integration tests
├── tools/                      # Development utilities
├── vscode-seen/                # VSCode extension
├── installer/                  # Installation tooling
└── performance_validation/     # Performance tests
```

---

## 🔧 Getting Started

### For New Developers

1. **Understand the Language**
   - Read [Seen Language Spec.md](./Seen Language Spec.md)
   - Review [Seen Design Document.md](./Seen Design Document.md)

2. **Understand the Architecture**
   - Read [bmm-architecture.md](./bmm-architecture.md)
   - Review [bmm-project-overview.md](./bmm-project-overview.md)

3. **Build & Test**
   - Follow [quickstart.md](./quickstart.md)
   - See development commands in [bmm-development-guide.md](./bmm-development-guide.md) _(To be generated)_

4. **Choose an Area**
   - Frontend: `seen_lexer`, `seen_parser`, `seen_typechecker`
   - Backend: **LLVM implementation needed** ⚠️
   - Features: `seen_oop`, `seen_concurrency`, etc.
   - Tooling: `seen_cli`, `seen_lsp`

### For AI Agents Planning Changes

#### Adding New Features
1. Review [Architecture](./bmm-architecture.md) to understand integration points
2. Check [Implementation Status](../IMPLEMENTATION_STATUS.md) for existing work
3. Identify affected crates in [Component Inventory](./bmm-component-inventory.md) _(To be generated)_
4. Follow development workflow in [bmm-development-guide.md](./bmm-development-guide.md) _(To be generated)_

#### Implementing LLVM Backend (High Priority)
1. **Context:** Read "Code Generation Backends" section in [bmm-architecture.md](./bmm-architecture.md)
2. **Gap Analysis:** Review "Critical Gaps" section detailing missing components
3. **Dependencies:** Understand `seen_ir` module structure
4. **Plan:** Create new `seen_llvm` crate following existing backend patterns
5. **Reference:** See `seen_cranelift` for similar backend implementation

#### Fixing Bugs
1. Check [crash-triage.md](./crash-triage.md) for debugging procedures
2. Review [operations.md](./operations.md) for operational context
3. Run tests per [bmm-development-guide.md](./bmm-development-guide.md) _(To be generated)_

---

## 🎯 Current Priorities

### Critical Path Items

1. **LLVM Backend Implementation** ⚠️
   - **Status:** Not started
   - **Impact:** Blocks AOT compilation, production benchmarks
   - **Docs:** See [bmm-architecture.md](./bmm-architecture.md) section "Code Generation Backends"
   - **Effort:** 4-6 weeks (estimated)

2. **Self-Hosting Completion**
   - **Status:** In progress (type errors remaining)
   - **Impact:** Enables Rust removal
   - **Docs:** See [SELF_HOSTING_PLAN.md](./SELF_HOSTING_PLAN.md)

3. **Production Benchmarks**
   - **Status:** Blocked by LLVM backend
   - **Docs:** See [BENCHMARK_STATUS.md](../BENCHMARK_STATUS.md)

---

## 📋 Workflow Integration

### BMM Workflow Status

This project is using the BMM (BMad Method) workflow for structured development. Current status tracked in [bmm-workflow-status.yaml](./bmm-workflow-status.yaml).

**Current Phase:** Documentation (Prerequisite)  
**Next Phase:** Planning (PRD creation)  
**Track:** BMad Method (Brownfield)

### Next Steps After Documentation

1. **Create PRD** for LLVM backend implementation
2. **Architecture Design** for LLVM integration
3. **Epic Breakdown** into implementable stories
4. **Sprint Planning** for development work

---

## 🔍 Search & Discovery

### Finding Information

**Language Features:**
- Syntax: [Seen Language Spec.md](./Seen Language Spec.md)
- Design rationale: [Seen Design Document.md](./Seen Design Document.md)
- Patterns: [concurrency-patterns.md](./concurrency-patterns.md)

**Implementation Details:**
- Architecture: [bmm-architecture.md](./bmm-architecture.md)
- Module breakdown: [bmm-component-inventory.md](./bmm-component-inventory.md) _(To be generated)_
- Source code: [bmm-source-tree.md](./bmm-source-tree.md) _(To be generated)_

**Development:**
- Build: [bmm-development-guide.md](./bmm-development-guide.md) _(To be generated)_
- Testing: [operations.md](./operations.md)
- Performance: [performance-baseline.md](./performance-baseline.md)

**Planning:**
- Roadmap: MVP/Alpha/Beta/Release plans in `docs/`
- Status: [IMPLEMENTATION_STATUS.md](../IMPLEMENTATION_STATUS.md)
- Workflows: [bmm-workflow-status.yaml](./bmm-workflow-status.yaml)

---

## 🚀 Quick Reference

### Build Commands
```bash
cargo build              # Development build
cargo build --release    # Optimized build
cargo test               # Run tests
cargo bench              # Run benchmarks
```

### Seen Commands (after build)
```bash
seen run <file>         # JIT execution ✅
seen build <file>       # AOT compilation ⚠️ (needs LLVM)
seen test               # Test runner
seen fmt                # Format code
seen check              # Type check only
```

### Useful Cargo Commands
```bash
cargo tree              # Dependency tree
cargo audit             # Security audit
cargo clippy            # Lint
cargo doc --open        # Generate & view docs
```

---

## 📞 External Resources

- **Repository:** https://github.com/seen-lang/seen (referenced in README)
- **License:** MIT
- **Version:** 1.0.0-alpha

---

## 📝 Document Metadata

- **Generated by:** BMM document-project workflow (Mary, Business Analyst Agent)
- **Workflow version:** 1.2.0
- **Scan level:** Exhaustive
- **Last updated:** 2025-11-17
- **Purpose:** Primary AI retrieval source for brownfield PRD creation and development planning

---

## ✅ Documentation Completeness

**Generated Documents:** ✅  
- bmm-project-overview.md
- bmm-architecture.md
- bmm-index.md (this file)

**To Be Generated:** ⏳  
- bmm-development-guide.md
- bmm-source-tree.md
- bmm-component-inventory.md

**Note:** Documents marked "To be generated" can be created on-demand via the document-project workflow's incomplete documentation generation feature.

---

*This index is designed to give AI agents and developers immediate context about the SeenLang compiler project, with special emphasis on the critical LLVM backend gap that needs implementation.*
