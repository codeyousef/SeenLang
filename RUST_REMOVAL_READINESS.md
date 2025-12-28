# Rust Removal Readiness Assessment

## Summary: NOT YET READY

The self-hosted Seen compiler is **NOT** ready for Rust removal. While significant progress has been made, there are critical blockers.

---

## What Works ✅

1. **Basic Programs** - Simple programs with integers, strings, functions compile and run correctly
2. **File I/O** - `readText`, `writeText`, `__ReadFile_SRET`, `__FileError_SRET` work correctly (after the recent SRET fix)
3. **String Operations** - String concatenation, length, toString() work
4. **Control Flow** - if/else, while loops, function calls work
5. **Bootstrap Compiler** - The Rust-based `seen` compiler successfully builds the self-hosted compiler executable
6. **LLVM Backend** - Code generation with optimization works
7. **Self-hosted Binary Exists** - `compiler_seen/target/native/release/seen_compiler` is built and runs

---

## Critical Blockers ❌

### 1. Vec<T> Generic Implementation Broken
The `Vec<T>` implementation in `seen_std/src/collections/vec.seen` crashes at runtime:
- **Cause**: `__default<T>()` returns `i64::0` for ALL types
- **Impact**: Any code using `Vec`, `Map`, or other generic collections crashes
- **Affected**: The self-hosted compiler uses `Vec` extensively for tokens, AST nodes, etc.

### 2. Self-Hosted Compiler Cannot Compile ANY Code
```
$ ./seen_compiler check /tmp/minimal.seen
Vec index out of bounds
```
Even empty files cause the Vec bounds error because the lexer/parser initialization uses Vec-based data structures (Map for keywords, Array for tokens).

### 3. `__default<T>()` Generic Intrinsic Not Properly Implemented
The backend treats all `__default<T>()` calls as returning `i64::0`, but:
- `__default<VecChunk<T>>` should return a struct
- `__default<String>` should return an empty string struct
- `__default<SomeClass>` should return a properly initialized instance

---

## Required Fixes Before Rust Removal

### Priority 1: Fix Generic Default Initialization
```rust
// In call.rs, "__default" handler:
// Must detect type parameter T and generate appropriate default:
// - For integers: const_zero()
// - For structs: zeroed struct or call static constructor
// - For strings: empty SeenString {len: 0, data: null}
// - For pointers: null pointer
```

### Priority 2: Verify Vec/Map Work End-to-End
After fixing `__default<T>`, test that:
1. `Vec<Int>.new()` + `push()` + `get()` works
2. `Vec<String>.new()` + `push()` + `get()` works
3. `Vec<SomeStruct>.new()` + `push()` + `get()` works
4. `Map<String, T>.new()` + `put()` + `get()` works

### Priority 3: Self-Hosted Compiler Can Compile Simple Programs
Once Vec works, the self-hosted compiler should be able to:
1. Compile a "hello world" program
2. Compile itself (bootstrap)

---

## Test Results

| Test | Bootstrap Compiler | Self-Hosted Compiler |
|------|-------------------|---------------------|
| Hello World (no imports) | ✅ Works | ❌ Vec crash |
| File I/O | ✅ Works | ❌ Vec crash |
| Vec operations | ❌ Segfault | ❌ Vec crash |
| Self-compilation | N/A | ❌ Vec crash |

---

## Conclusion

The Seen language has a working LLVM backend and most core features work when compiled by the Rust bootstrap compiler. However, **generic collections (Vec, Map) are broken**, which cascades to break the self-hosted compiler since it relies on these collections.

**Estimated work remaining**: 1-2 days to fix `__default<T>` properly, then testing and bug fixes for edge cases.

---

*Generated: 2024-12-27*
