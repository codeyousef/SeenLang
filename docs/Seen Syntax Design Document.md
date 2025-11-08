# Seen Language — Syntax Design (Multi‑Platform, No Benchmarks)

> Canonical syntax guide aligned with the updated MVP/Alpha/Beta/Release plans. Removes all benchmark references. Shows idioms for systems, graphics, and engine code.

---

## 1. Lexical & Identifiers
- **Unicode policy:** NFC normalization; reject mixed‑script confusables.
- **Visibility modes (project‑level):** `Seen.toml`
  ```toml
  [lang]
  visibility = "caps"      # Latin: Uppercase public, lowercase private
  # or
  visibility = "explicit"  # Non‑cased scripts: use `pub` for exports
  export_alias = "ascii"   # ASCII symbol alias for FFI/export
  ```
- **Keywords (words as operators):** `and`, `or`, `not`, `is`, `match`, `defer`, `using`, `spawn`, `scope`, `cancel`.

---

## 2. Declarations & Modules
```seen
module graphics.vulkan

pub struct Image {
  handle: Ptr<VkImage>,
  extent: Extent3D,
}

pub fun create_image(dev: Device, info: ImageInfo) -> Result<Image, CreateError> { /* ... */ }
```

- `module` names map to directories; public items per visibility mode.
- `pub` required in `visibility = "explicit"` projects.

---

## 3. Types, Traits, Generics
```seen
trait Resource { fun destroy(self) }

sealed trait CmdState {}
struct Recording: CmdState {}
struct Executable: CmdState {}

struct CommandBuffer<S: CmdState> { raw: Ptr<VkCommandBuffer>, phantom: Phantom<S> }

fun end(cb: CommandBuffer<Recording>) -> CommandBuffer<Executable> { /* ... */ }
```
- **Traits** with associated types (not shown) and orphan rule.
- **Sealed traits** to restrict unsafe external impls.
- **Phantom** parameters enable typestates.

---

## 4. Enums & Pattern Matching
```seen
enum VkResult {
  Success,
  OutOfDate,
  DeviceLost,
}

fun present(r: VkResult) -> Result<Unit, VkResult> {
  match r {
    VkResult::Success => Ok(()),
    VkResult::OutOfDate => Err(r),
    VkResult::DeviceLost => panic("device lost"),
  }
}
```
- Exhaustiveness required unless a wildcard arm is provided.

---

## 5. Memory, RAII, Regions
```seen
struct GpuBuffer { dev: Ptr<VkDevice>, buf: Ptr<VkBuffer> }

impl Drop for GpuBuffer {
  fun drop(self) { frame_deletes.defer_after(2, || vkDestroyBuffer(self.dev, self.buf)); }
}

region upload {
  let tmp = allocate_temp();
  defer free(tmp);
}
```
- RAII `Drop` at scope end; `defer` is LIFO; **regions** bulk‑destroy contents.

---

## 6. Errors & Control Flow
```seen
fun load(path: &str) -> Result<[u8], IoError> {
  let f = open(path)?;      // `?` desugars to match on Result
  let bytes = read_all(f)?;
  Ok(bytes)
}

@[cold]
fun unlikely_case() { /* ... */ }
```
- Recoverable errors use `Result`; `panic` aborts.

---

## 7. Concurrency & Jobs
```seen
scope(|s| {
  let (tx, rx) = channel<Work>(256);
  s.spawn(|| producer(tx));
  s.spawn(|| consumer(rx));
}); // joins on scope exit

spawn(|| task());
cancel(token);
```
- Structured concurrency: `scope` joins; cancellation is cooperative.
- Rule: no `await` while holding region borrows; capture ownership with `move`.

---

## 8. Numerics & SIMD
```seen
@[float_env(ftz, daz, round="nearest")]
fun dot(a: vec4, b: vec4) -> f32 { /* ... */ }

let q: quat = quat_from_axis_angle(axis, angle);
```
- Float environment attributes; SIMD‑friendly `vec/mat/quat` value types.

---

## 9. FFI/ABI & Layout
```seen
@[repr(C)]
struct Extent3D { width: u32, height: u32, depth: u32 }

@[no_mangle]
pub extern "C" fun seen_init() -> i32 { 0 }

@[repr(C, packed(1))]
union Bytes4 { u: u32, b: [u8; 4] }
```
- Exact layout/align/pack; function pointers and callbacks supported.

---

## 10. Shaders & Render‑Graph DSL
```seen
@[embed(path="shaders/triangle.spv")] const TRIANGLE_SPV: [u8];

graph pass "gbuffer" { write color, depth; run |vk| draw_world(vk); }

graph pass "lighting" { read color, depth; write lit; run |vk| shade(vk); }
```

- `@[embed]` injects byte blobs.
- Render‑graph DSL declares passes/resources; compiler emits barriers/lifetime checks.

---

## 11. Packaging & Plugins
```seen
# seen.pkg
[package]
name = "seen-vulkan-min"
version = "0.1.0"

[dependencies]
seen-ecs = "^0.1"

@[plugin_abi(version="1.0")]
module physics_plugin { /* exports */ }
```
- `seen pkg` manages lockfiles, versions, and publishing.
- Plugin ABI macro + capability tokens for sandboxing.

---

## 12. Platforms & Toolchains (syntax‑adjacent)
- Android (NDK r26+), iOS/macOS (Xcode + codesign), Windows (MSVC), Web (Emscripten flags).  
- Target triples select codegen; attributes control export, align, packing, float env.

---

## 13. Style & Formatting
- `seen fmt` rules: word‑operator spacing, public/private casing (caps/explicit), import order, brace style.  
- `seen fmt --check` for CI.

---

## 14. Examples Index
- **Systems:** Result/`?`, RAII `Drop`, `defer`, channels, jobs.
- **Graphics:** typestate command buffers, render‑graph, shader embed.
- **Interop:** FFI init function, repr(C) structs, unions.

*No benchmark data appears in this document; examples are minimal and illustrative.*
