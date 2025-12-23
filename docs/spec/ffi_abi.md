# Seen Language — FFI & ABI Specification

## 1. Calling Conventions
- Default calling convention mirrors the target platform C ABI (`extern "C"`).
- Additional conventions (e.g., `"system"`, `"vectorcall"`) follow LLVM naming where supported; unsupported combinations trigger compile errors.
- Varargs functions are permitted on platforms that expose C varargs; Seen enforces that trailing parameters are trivially copyable.

## 2. Layout Attributes

- `@repr(C)` ensures C-compatible field order and alignment.
- `@repr(transparent)` ensures layout identical to the single non-zero-sized field.
- `@repr(packed(N))` packs data structs to `N` alignment; accessing misaligned fields is `unsafe`.
- `@align(N)` raises alignment to `N` bytes (power of two).
- Unions follow C semantics; reading inactive fields is `unsafe`.

## 3. Visibility & Symbol Policy

- Deterministic symbol mangling stabilizes across platforms; `@export(name = "...")` overrides the symbol string.
- Projects automatically export capitalized items.
- `@no_mangle` disables mangling entirely — recommended for interop entry points.
- CLI `--declaration-map` emits JSON mapping Seen declarations to exported symbols for tooling integration.

## 4. Struct Interop Example
```seen
@repr(C)
data Extent3D {
  Width: u32,
  Height: u32,
  Depth: u32,
}

@no_mangle
extern "C" fun SeenInit() -> i32 { 0 }
```

## 5. Deterministic Object Emission
- Linker inputs are generated with stable section ordering; timestamps removed under `--profile deterministic`.
- Embedded assets (`@embed(path=...)`) preserve byte-for-byte payloads; hashed during Stage1→Stage2 verification.
- Multi-platform targets rely on platform-specific linkers but share the same intermediate IR hashing workflow.

## 6. Safety Rules
- Calling Seen functions from C requires matching ABI declarations; mismatches are compile-time errors when prototypes are generated via `seen bindgen`.
- Inline assembly is gated behind `unsafe` and target feature checks.
- FFI modules must document ownership transfer: `take`/`borrow` conventions surface via attributes (`@ffi(borrow)`,
  `@ffi(transfer)`).

## 7. Testing & Validation
- `tests/ffi_layout` contains golden tests comparing Seen layout hashes to C headers using bindgen snapshots.
- Deterministic builds run `llvm-dwarfdump`/`pdbutil` to validate debug symbol stability.
- CI jobs fail when exported symbols change without updating the recorded ABI hash emitted by the FFI layout tests.
