# HeartOn No-Project-C Split

This note answers a narrower question than "remove all C from Seen."

The target is:

> **Seen projects such as HeartOn should not need handwritten C in their own repos.**

That does **not** require the Seen repo itself to become zero-C immediately.

## Recommendation

Use a layered split:

1. **Seen core / compiler / runtime / stdlib**
   - owns native-plumbing infrastructure
   - owns runtime/link fixes that every project would otherwise duplicate
   - owns the minimal low-level ABI boundary needed so packages can be written in Seen

2. **First-party official packages**
   - own the heavy, fast-moving, domain-specific APIs
   - expose ergonomic Seen interfaces for SDL3, Vulkan, images, audio, and rendering

3. **Project repos**
   - only contain project-specific Seen code
   - do not build custom shim libraries
   - do not maintain handwritten override C

## What belongs in Seen core

These pieces should stay in the Seen repo because they are cross-cutting infrastructure.

### 1. Compiler support for hidden native plumbing

Files:

- `compiler_seen/src/main_compiler.seen`

Why:

- the compiler already special-cases upstream Linux platform shims
- this is the right place to auto-build and auto-link hidden native support
- projects should not need custom `Seen.toml` native entries or shell glue for common upstream facilities

### 2. Runtime/link model fixes

Files:

- `seen_runtime/seen_runtime.c`

Why:

- if the runtime exports weak symbols that shadow real system libraries, every project pays the price
- project repos should not need files like `vk_overrides.c`

### 3. FFI and binding-generation tooling

Files:

- `compiler_seen/src/tools/c_import_gen.seen`
- related compiler FFI/type-checking paths

Why:

- official bindings are only maintainable if Seen can generate more than flat function externs
- this is foundational tooling, not app-level API surface

### 4. Minimal low-level platform boundary

Examples:

- raw window creation / input/event memory layout access
- raw Vulkan surface / device / submission entrypoints
- proc-loading and system-symbol access needed by official packages

Why:

- packages need a stable low-level foundation
- projects should not have to invent one privately

## What should be first-party packages

These should be official packages rather than stdlib modules, because they are large, optional, and likely to evolve faster than the compiler/runtime.

### Recommended packages

- `seen-sdl3`
- `seen-vulkan`
- `seen-renderer`
- `seen-png` or `seen-image`
- `seen-audio`

### Why packages are the better home

- keep stdlib small and stable
- allow faster release cadence than compiler releases
- make platform/media APIs semver independently
- let applications choose what they depend on

## What this means for HeartOn

HeartOn should end up with:

- no project-authored `.c` files
- no project-specific native shim library
- no custom C compilation step in its build
- only Seen modules and package dependencies

Any unavoidable native glue should be maintained once upstream in Seen core or in first-party packages, not duplicated in the game repo.

## Immediate Seen-repo work

To move HeartOn toward zero project C, the Seen repo should do this first:

1. fix the runtime Vulkan weak-stub issue so projects do not need override C
2. expand the upstream low-level platform boundary enough that packages can sit on top of it
3. improve binding-generation tooling so official packages are maintainable
4. keep persistence on the Seen stdlib path instead of treating it as a C-only problem

## Practical near-term split

### Keep in Seen repo now

- runtime/link fixes
- hidden platform shim plumbing
- minimal raw low-level ABI modules
- FFI generation improvements

### Build as official packages on top

- ergonomic SDL3 wrapper
- ergonomic Vulkan wrapper
- renderer abstractions
- PNG/image loading
- audio wrapper layers

This gives the best near-term outcome:

> **HeartOn stops writing C before Seen itself becomes zero-C internally.**
