# Seen Language — Unified **MVP** Plan

**Last Updated:** 2025-12-30  
**Core Principle:** Safety by default, nondeterminism explicitly opt-in via annotation.  
**Target Platforms:** Linux, Windows, RISC-V, UWW-Compatible WASM

---

## Current Status Summary

| Component | Status |
|-----------|--------|
| Rust Compiler | ✅ Production Ready |
| Self-Host Type-Check | ❌ Failing (Parser Enum Resolution) |
| Self-Host IR Gen | ⏸️ Paused (Fixing Parser) |
| Self-Host Native Codegen | ⏸️ Paused |
| Stage1→Stage2→Stage3 | ⏳ Blocked on Stage1 |
| LLVM Tracing Infrastructure | ✅ Complete |

**Blocking Issue:** Compiling `compiler_seen/src/main.seen` fails with `Unknown enum variant 'BangEqual' in enum 'SeenTokenType'` during IR lowering. The self-hosted parser's enum variants don't all match the `SeenTokenType` definition. Build progresses past type-checking but fails in LLVM IR generation.

**Recent Progress (2025-12-30):**
1. Implemented `LlvmTraceOptions` with `--trace-llvm` CLI flag for debugging
2. Fixed `IRValue::Struct` array field handling (load content from pointer, not store pointer)
3. Fixed `ConstructObject` array field handling (same fix)
4. `test_array_push.seen` now passes - basic Array field in struct works
5. Stage1 `--help` and `--version` work correctly
6. Narrowed crash to Vec_push accessing its internal Array fields when Vec is a class pointer stored in Map
7. Refactored `TokenType` into `SeenTokenType` in `lexer/token_type.seen`
8. **[FIXED]** Resolved `Type mismatch: expected SeenTokenType, found ??` by removing nullable enum returns
9. **[NEW]** Added `None` variant to `SeenTokenType` enum as workaround for nullable enum issues
10. **[NEW]** Changed lexer helper functions (`getTwoCharOperatorType`, `getSingleCharOperatorType`, `getSingleCharSeenTokenType`) to return `SeenTokenType` instead of `SeenTokenType?`, returning `SeenTokenType.None` instead of `null`
11. **[NEW]** Fixed `c_gen.seen` to import `ParserExpressionNode` instead of `ExpressionNode`
12. **[NEW]** Fixed `real_parser.seen` enum variants: `Greater` → `GreaterThan`, `Less` → `LessThan`
13. **[NEW]** Added helper functions to `token_type.seen` (e.g., `getLeftParenType()`) to work around enum resolution
14. **[NEW]** Added `KeywordUse` variant to `SeenTokenType` enum
15. **[REMAINING]** Need to add `BangEqual` variant or fix parser to use `NotEqual`

---

# NEXT IMMEDIATE STEPS: Debug Struct Layout Mismatch & Invalid Pointer Access

**Objective:** Instrument the compiler and runtime to detect **Struct Layout Mismatches** and **Invalid Pointer Access** during the `Vec_push` call within `Map`.

---

## Step 1: Runtime Instrumentation (Standard Library)

**Target File:** `seen_std/src/collections/vec.seen`  
**Goal:** Verify the `this` pointer passed to `Vec.push` is valid and not null/misaligned.

**Instructions:**

1. Locate the `push(value: T)` method in the `Vec` class (around line 52).
2. Insert the following intrinsic calls at the **very first line** of the function body:
```seen
__Print("[DEBUG] Vec_push called. 'this' address: ")
__PrintInt(__PtrToInt(this))
__Println("")
```

3. **Constraint:** Do NOT access any fields of `this` (like `this.length` or `this.chunks`) in these print statements, as that may trigger the crash immediately if the pointer is invalid.

4. **Note:** If `__PtrToInt` intrinsic doesn't exist, create it in the runtime. It should convert a pointer to an Int for diagnostic printing.

---

## Step 2: Compiler Instrumentation (Rust Backend)

**Target File:** `rust_backup/seen_ir/src/llvm_backend.rs`  
**Goal:** Expose the compiler's internal field index mapping vs. LLVM's generated struct layout.

### Task A: Implement `dump_layouts` Flag

1. **Update Config:** Add `dump_layouts: bool` to `LlvmTraceOptions` (around line 396).

2. **Update Parsing:** In the trace options parsing logic, add support for `SEEN_TRACE_LLVM=layouts` or extend the existing flag.

3. **Instrumentation Point:** Locate `register_struct_type` function (around line 1048).

4. **Implementation:** When defining a struct body, iterate through fields and print to `stderr`:
```text
[LAYOUT] Struct Name: {struct_name}
[LAYOUT]   Field 0: {field_name} (Type: {ir_type})
[LAYOUT]   Field 1: {field_name} (Type: {ir_type})
...
```

*This confirms the order of fields in the compiler's memory.*

### Task B: Enhanced GEP Tracing

1. **Instrumentation Point:** Locate `emit_field_get` function (around line 700) and `emit_field_set` (around line 902).

2. **Implementation:** Before the `build_struct_gep` call, if `trace_options.trace_instructions` or `trace_options.dump_layouts` is true, print:
```text
[GEP TRACE] accessing member '{member_name}' of struct '{struct_name}'
[GEP TRACE]   - Base Pointer Type: {llvm_type_of_base}
[GEP TRACE]   - Calculated Field Index: {index}
```

*This confirms the compiler is calculating the correct index (e.g., 0 for `keys`) at the call site.*

---

## Step 3: Isolation Test Case (Reproduction)

**Target File:** `repro_crash.seen` (Create new file in project root)  
**Goal:** Isolate the crash to "Class-nested-in-Class" without `Map` logic overhead.

**Content:**
```seen
class Inner {
    data: Array<Int>
    
    fun init() r: Void {
        this.data = __ArrayNew(Int, 4)
    }
    
    fun touch() r: Void {
        __ArraySet(this.data, 0, 99) // Access array field
    }
}

class Outer {
    inner: Inner
    
    fun init() r: Void {
        this.inner = Inner.new() // Heap allocation
    }
}

fun main() r: Int {
    var o = Outer.new()
    __Print("Outer created. Inner address: ")
    __PrintInt(__PtrToInt(o.inner))
    __Println("")
    
    // Trigger the potential crash
    o.inner.touch()
    __Println("Touch successful.")
    return 0
}
```

---

## Step 4: Execution & Validation Workflow

**Instructions for validation:**

1. **Rebuild Compiler:**
```bash
cargo build --release
```

2. **Compile Repro with Tracing:**
```bash
SEEN_TRACE_LLVM=layouts ./target/release/seen_cli build repro_crash.seen --backend llvm -o repro_test --trace-llvm 2>&1 | tee repro_trace.log
```

3. **Analyze Output:**
   - **Check Layout:** Does `[LAYOUT]` show `inner` at Index 0 for `Outer`? Does `data` appear at Index 0 for `Inner`?
   - **Check GEP:** Does `[GEP TRACE]` in `main` show accessing Index 0 of `Outer` when getting `o.inner`?

4. **Run Repro:**
```bash
./repro_test
```

5. **Interpret Results:**
   - If `Inner address` prints `0`, the initialization in `Outer.init()` failed (Constructor injection bug).
   - If `Inner address` is valid (non-zero) but it crashes on `touch()`, the memory layout of `Inner` is mismatched.
   - If it prints "Touch successful.", the isolated case works and the bug is specific to `Map`→`Vec` interaction.

---

## Step 5: Apply Findings to Vec/Map

Based on Step 4 results:

- **If isolated test passes:** The bug is in `Map`'s storage of `Vec` pointers. Add same debug prints to `Map.put()` to trace `keysVec` and `valuesVec` addresses.

- **If isolated test fails:** The bug is in nested-class field access generally. Compare `[LAYOUT]` output with actual LLVM IR struct definitions using:
```bash
SEEN_TRACE_LLVM=dump ./target/release/seen_cli build repro_crash.seen --backend llvm -o repro_test 2>&1 | grep -A 20 "define.*Inner\|define.*Outer"
```

---

## Acceptance Criteria

- [ ] `Vec.push` prints `this` pointer address before any field access
- [ ] `--trace-llvm` with `layouts` mode prints `[LAYOUT]` for all struct types
- [ ] GEP operations print field index being accessed
- [ ] `repro_crash.seen` test case created and runnable
- [ ] Root cause of Vec_push crash identified (pointer corruption vs layout mismatch)

---

# PART 1: COMPLETED WORK

All tasks below are ✅ complete and verified.

---

## 1. Core Language & Compiler (Complete)

### 1.1 Lexer/Parser ✅
- Unicode NFC normalization during lexing
- `spec` keyword (renamed from `trait`)
- Caps visibility policy (no `pub` keyword)
- `Seen.toml` switches for visibility modes
- Operator precedence tables frozen
- Formatter enforces frozen precedence

### 1.2 Type System ✅
- Hindley-Milner type inference
- Generics with monomorphization
- Specs (interfaces) and sealed specs
- Nullable types (T?)
- Smart casting after null checks
- Phantom generics for typestates
- Method resolution/inference complete
- Enum variant/member access parity
- Operator typing (>=, <=, +) over all types
- Default params + constructor returns
- Prelude builtin export for manifest modules

### 1.3 Memory Model ✅
- Region-based allocation with O(1) bulk deallocation
- Generational references with runtime detection (debug builds)
- Deterministic drop semantics (RAII)
- Vale-style hybrid handles in `seen_memory_manager`
- `defer` stack + scope unwinding
- Region strategy hints (`bump`, `stack`, `cxl_near`)
- 32-bit arena indices for cache-oblivious layouts
- Safety checks gated behind debug profiles

### 1.4 FFI/ABI ✅
- `extern "C"` bindings
- `repr(C)` struct layout
- Unions, align/pack attributes
- Stable symbol mangling
- ABI snapshot locked (`artifacts/stdlib_abi/snapshot_20251116.json`)

### 1.5 Error Model ✅
- Consistent `Result<T, E>` type across compiler
- `abort` intrinsic for unrecoverable failures
- `exit()`, `super()`, `throw()` built-in functions

---

## 2. Code Generation (Complete)

### 2.1 LLVM Backend ✅
- AOT compilation to native binaries
- Optimization levels 0-3 mapped correctly
- SIMD vectorization with deterministic policy
- Float arithmetic (fadd, fsub, fmul, fdiv, frem)
- Cast instruction lowering (Int↔Float, Bool↔Int)
- Hardware-aware codegen (APX, AVX10, SVE hints)
- Memory topology hints (CXL near/far)

### 2.2 IR Emission ✅
- Deterministic IR with sorted structures
- SSA form
- Vector types (`IRType::Vector`)
- SIMD splat/reduce instructions
- Scope/spawn/select IR instructions

### 2.3 Alternative Backends ✅
- MLIR emission path (core dialect + Transform)
- Cranelift backend for fast compilation
- `--backend mlir` and `--backend clif` CLI flags
- Deterministic hashes across all backends

---

## 3. Async & Concurrency (Complete)

### 3.1 Structured Concurrency ✅
- `scope { ... }` blocks govern spawned tasks
- Non-detached spawns outside scope produce compile errors
- `spawn detached` for background tasks
- `cancel taskHandle` primitive
- Scoped tasks join deterministically at runtime

### 3.2 Channels & Job System ✅
- `Channel()` constructor with Sender/Receiver endpoints
- Optional capacity support (bounded channels)
- Fair, waker-driven `select` outcomes
- Generational channel handles with validation
- Channel futures on shared async runtime

### 3.3 Runtime Scheduling ✅
- Scope-bound coroutine frames on structured stacks
- Per-priority dispatch counts and queue promotions
- Cooperative backoff/yield for fairness
- Wake latency tracking and starvation detection (>5ms)
- Frame + scheduler snapshots for PB-Perf dashboards

---

## 4. Optimization Pipeline (Complete)

### 4.1 E-Graph Equality Saturation ✅
- `egg`-powered rewrite pass in `seen_ir/src/optimizer/egraph.rs`
- Canonicalizes arithmetic (+0, *1, commutativity)
- Enabled at `-O2`/`-O3`

### 4.2 ML-Guided Heuristics ✅
- Inlining and register-allocation pressure via `seen_ir/src/optimizer/ml.rs`
- JSON weight files via `SEEN_ML_HEURISTICS`
- `InlineHint` attributes passed to LLVM
- Decision logging (`SEEN_ML_DECISION_LOG`) and replay (`SEEN_ML_DECISION_REPLAY`)

### 4.3 LENS Superoptimizer ✅
- Hot-loop rewrites for linear instruction chains
- `IROptimizer::superoptimize_loop_chains`
- Collapse temporary registers in add/sub/mul/shl sequences

### 4.4 SIMD Baseline ✅
- CLI flags: `--simd=off|auto|max`, `--target-cpu`, `--simd-report`
- Deterministic builds coerce scalar mode
- Per-function JSON summaries (policy, mode, reason, ops, speedup)
- Hardware-aware heuristics (loop detection, arithmetic density, register pressure)

---

## 5. Tooling (Complete)

### 5.1 CLI Commands ✅
- `seen build` — AOT compilation with `--target <triple>`
- `seen run` — JIT execution
- `seen test` — test runner
- `seen fmt` / `seen fmt --check` — formatter
- `seen determinism` — hash verification
- `seen trace` — IR/control-flow graph inspection
- `seen pkg` — deterministic zip packaging
- `seen doctor` — build ID inspection
- `seen shaders` — SPIR-V validation/transpilation

### 5.2 LSP Server ✅
- Hover information
- Go-to definition
- Diagnostics
- Code formatting
- Find references

### 5.3 Performance Baselines ✅
- `perf_baseline` harness in `tools/perf_baseline`
- Runtime, peak RSS, binary sizes, compile timings
- CI runs baseline suite on every push/PR
- Rust/C++ parity dashboard (`docs/performance-dashboard.md`)

### 5.4 Determinism Profile ✅
- `--profile deterministic` pins timestamps/temp roots
- Identical ELF/WASM binaries across compile hosts
- Stage2 == Stage3 hash equality verified

---

## 6. Standard Library (Complete)

### 6.1 Core Types ✅
- String, Int, Float, Bool, Array
- Option<T>, Result<T, E>
- Vec<T> with chunked doubling growth
- StdString (allocator-backed mutable string)
- StringHashMap (open-addressed, deterministic)

### 6.2 Collections ✅
- HashMap<K, V> with robin-hood hashing
- HashSet<T>
- LinkedList<T> with O(1) operations
- BitSet (64-bit word-based)
- ByteBuffer

### 6.3 I/O & Networking ✅
- File operations (read, write, append, directory management)
- Buffered I/O (BufferedReader/Writer)
- TCP sockets (TcpListener, TcpStream)
- Non-blocking mode support
- Epoll/kqueue wrappers

### 6.4 Concurrency ✅
- Thread spawn/join
- Mutex synchronization
- Channel message passing
- JoinHandle for results
- Atomic operations

### 6.5 Time & Duration ✅
- Duration (secs, millis, micros, nanos)
- Instant for measurement
- Sleep functionality
- Timestamp parsing

### 6.6 Math ✅
- Constants (PI, E)
- Basic ops (abs, min, max, clamp, sign)
- Trig functions (sin, cos, tan)
- Power/log (pow, exp, log, sqrt)
- Interpolation (lerp, remap, smoothstep)

### 6.7 String Operations ✅
- StringBuilder with length tracking
- Split, trim, search, replace
- Padding, prefix/suffix checks
- CString bridges
- JSON escaping

### 6.8 Crypto & Hashing ✅
- MD5 (RFC 1321 compliant)
- Hash spec + FNV/SipHash utilities
- Random RNGs (LCG, PCG, Xorshift)

### 6.9 Environment & Process ✅
- CLI arguments (`args`)
- Environment variables (get, set, remove)
- Process execution (`runProgram`, `runCommand`)
- Path manipulation (normalize, join, basename, dirname)

---

## 7. Self-Hosting Infrastructure (Complete)

### 7.1 Bootstrap Pipeline ✅
- Stage0→Stage1→Stage2→Stage3 architecture
- Manifest module system with prelude
- `SEEN_ENABLE_MANIFEST_MODULES` env flag
- Dependency resolution working
- Cross-module function visibility solved

### 7.2 Type System Fixes ✅
- Stale type problem resolved (multi-pass deep type fixup)
- Enum predeclaration with immediate variant extraction
- Unknown type handling in operations
- Case-insensitive enum variant lookup
- Constructor validation for `new` methods
- Map<K,V> built-in constructor

### 7.3 Parser Hardening ✅
- Class/struct generics
- Struct literals
- `<` expression disambiguation
- Statement blocks with newline terminators
- `when` expressions
- Removed Kotlin-era constructs (ranges, Elvis, safe-navigation)

### 7.4 Stdlib Syntax Normalization ✅
- Removed 112 `pub` keywords
- Converted `Result::Err` → `Err` (114 occurrences)
- Converted `&&` → `and`, `||` → `or`

---

## 8. Documentation (Complete)

### 8.1 Language Specification ✅
- `/docs/spec/lexical.md` — lexical structure
- `/docs/spec/grammar.md` — grammar rules
- `/docs/spec/types.md` — type system
- `/docs/spec/regions.md` — memory regions
- `/docs/spec/errors.md` — error handling
- `/docs/spec/ffi_abi.md` — FFI and ABI
- `/docs/spec/numerics.md` — numeric types + SIMD appendix

### 8.2 Examples ✅
- `examples/seen-vulkan-min` — deterministic triangle driver
- `examples/seen-ecs-min` — ECS micro-simulation
- `examples/linux/hello_cli`
- `examples/web/hello_wasm`
- `examples/android/hello_ndk`

### 8.3 Guides ✅
- Quickstart with toolchain prerequisites
- Concurrency patterns (`docs/concurrency-patterns.md`)
- Release playbook (`docs/release-playbook.md`)
- Crash triage (`docs/crash-triage.md`)
- Performance baseline (`docs/performance-baseline.md`)

---

## 9. Benchmarks (Complete Infrastructure)

### 9.1 Benchmark Harness ✅
- `run_production_benchmarks.sh`
- `run_all_production_benchmarks.sh`
- Automated compilation, timing, reporting
- Markdown report generation

### 9.2 Runtime Intrinsics ✅
- Timing: `__GetTime()`, `__GetTimestamp()`, `__GetTimestampNanos()`, `__Sleep()`
- Math: `__Sqrt()`, `__Sin()`, `__Cos()`, `__Pow()`, `__Abs()`, `__Floor()`, `__Ceil()`
- I/O: `__Print()`, `__Println()`, `__PrintInt()`, `__PrintFloat()`
- String: `__IntToString`, `__FloatToString`, `__BoolToString`, `__CharToString`, `__StringConcat`
- Array: `__ArrayNew`, `__ArrayPush`, `__ArrayGet`, `__ArraySet`, `__ArrayLen`

### 9.3 Language Features for Benchmarks ✅
- Mutable variables (`var` reassignment)
- While/for/loop expressions
- Array indexing and mutation
- Float literals and operations
- Struct field mutation
- Cast expressions (Int↔Float, Bool↔Int)

### 9.4 Benchmark Implementations ✅
All 10 benchmarks implemented in `benchmarks/production/`:
1. Matrix Multiplication (SGEMM) — 512x512, cache-blocked
2. Sieve of Eratosthenes — 10M primes, bit array
3. Binary Trees — GC stress, depth 20
4. FASTA Generation — 5M nucleotides
5. N-Body Simulation — 50M steps
6. Reverse Complement — 25M bp
7. Mandelbrot Set — 4000x4000, 1000 iterations
8. LRU Cache — 5M operations
9. JSON Serialization — 1M objects
10. HTTP Echo Server — 5M requests

### 9.5 Performance Results ✅
- Fibonacci: 1.0x Rust (identical)
- Recursive Sum: 1.0x Rust
- Ackermann: 4.5x slower (deep recursion)
- Average: 2.08x slower (geometric mean)
- 5/10 production benchmarks passing

---

## 10. Release Infrastructure (Complete)

### 10.1 Bootstrap Matrix ✅
- `releases/bootstrap_matrix.toml` with host/backend/profile tuples
- `scripts/release_bootstrap_matrix.sh` iterates matrix
- Ed25519 signing via `tools/sign_bootstrap_artifact`
- Manifest emission (git commit, CLI version, per-stage SHA256)
- ABI guard verification before release

### 10.2 Installers ✅
- `scripts/build_installers.sh`
- Linux: DEB, RPM, AppImage
- Android: `.aab` bundles with manifest/assets/res/dex

### 10.3 Platform CI ✅
- `scripts/platform_matrix.sh` for smoke tests
- Linux build/run verified
- JSON reports under `artifacts/platform-matrix/`

---

# PART 2: REMAINING WORK

All tasks below are ⏳ pending and listed in sequential execution order.

---

## Phase 1: Complete Self-Hosting (MVP Critical)

### Task 1.1: Fix LLVM Backend Stdlib Import Resolution
**Status:** ✅ Complete
**Estimated:** 4-6 hours

**Problem:** `seen_cli build --backend llvm` fails with:
```
Type error: Undefined variable 'seen_std.env.env.args' at 706:8
```

**Tasks:**
- [x] Trace how stdlib modules are loaded during LLVM codegen
- [x] Fix module path resolution for `seen_std.*` imports
- [x] Ensure all stdlib symbols are available during IR lowering
- [x] Test with `seen_cli build compiler_seen/src/main_compiler.seen --backend llvm`

**Acceptance:** Native binary generated from self-hosted compiler source.

---

### Task 1.2: Stage1 Native Binary Generation
**Status:** 🔄 In Progress (Multiple Compilation Issues)
**Estimated:** 2-3 hours → Extended
**Last Updated:** 2025-12-30

**Completed:**
- [x] Build Stage1 compiler from `compiler_seen/src/main.seen`
- [x] Fix `ir_type_to_llvm` struct resolution (was returning i8*)
- [x] Fix default parameter bug (`peekChar(1)` workaround applied via sed)
- [x] Fix Array_push element size bug (was hardcoded to 16 bytes, now uses actual struct size)
- [x] Stage1 binary runs successfully (`--version` works, `--help` works)
- [x] Implemented comprehensive LLVM tracing infrastructure
- [x] Fixed IRValue::Struct array field handling (load content from pointer)
- [x] Fixed ConstructObject array field handling (same fix)
- [x] test_array_push.seen now passes (basic array push works)
- [x] Split `TokenType` enum into `lexer/token_type.seen`
- [x] Added `--dump-struct-layouts` CLI flag for debugging memory layouts
- [x] Added `dump_struct_layouts()` method to LLVM backend
- [x] Created `test_vec_map_crash.seen` minimal reproduction test (passes!)
- [x] Fixed BangEqual → NotEqual enum variant mismatch in real_parser.seen

**New Debug Tools Added (2025-12-30):**
```bash
# Dump struct layouts for all registered types:
./target/release/seen_cli build file.seen --backend llvm -o out --dump-struct-layouts

# Example output shows class vs struct types and field layouts:
=== STRUCT LAYOUT DEBUG INFO ===
Class types (heap-allocated, stored as i64): {"VecChunk", "Vec", "Map", "KeywordHolder"}

  Vec (CLASS - ptr/i64)
    LLVM type: { { i64, i64, ptr }, { i64, i64, ptr }, { i64, i64, ptr }, i64, i64, i64 }
    [0] chunks : Array(VecChunk) -> { i64, i64, ptr }
    [1] capacities : Array(Integer) -> { i64, i64, ptr }
    [2] usage : Array(Integer) -> { i64, i64, ptr }
    [3] length : Integer -> i64
    [4] totalCapacity : Integer -> i64
    [5] nextChunkSize : Integer -> i64
```

**Current Blocking Bugs (as of 2025-12-30):**

1. **Parameter Type Mismatch (LLVM Verification Error):**
   ```
   Call parameter type does not match function signature!
     %load_twoChar = load i64, ptr %var_twoChar, align 4
    { i64, ptr }  %call9 = call i64 @SeenLexer_getTwoCharOperatorType(ptr %arg_cast8, i64 %load_twoChar)
   ```
   The lexer's `twoChar` variable (String type = `{i64, ptr}`) is being loaded as i64 instead of the proper String struct.

2. **Root Cause:** Type inference is not properly tracking String types for local variables. When a String is assigned to a local variable, the LLVM backend emits i64 loads instead of struct loads.

**Vec/Map Runtime Crash - Status Update:**
The minimal reproduction test (`test_vec_map_crash.seen`) **passes successfully**, demonstrating:
- Direct `Vec<String>.push()` works
- Direct `Map<String, Int>.put()` works  
- `KeywordHolder` pattern (class with Map field) works

This suggests the original crash may be specific to:
1. The actual KeywordManager implementation's complexity
2. Generic monomorphization with complex nested types
3. A type mismatch that only manifests in certain call patterns

**Tracing Infrastructure:**
New `LlvmTraceOptions` struct in `rust_backup/seen_ir/src/llvm_backend.rs`:
```rust
pub struct LlvmTraceOptions {
    pub trace_instructions: bool,  // Show each IR instruction being compiled
    pub trace_values: bool,        // Show IRValue -> LLVM type conversions
    pub trace_types: bool,         // Show type resolutions
    pub dump_ir: bool,             // Dump full LLVM IR at end
}
```

**How to use tracing:**
```bash
# Via CLI flag:
./target/release/seen_cli build file.seen --backend llvm -o out --trace-llvm

# Dump struct layouts:
./target/release/seen_cli build file.seen --backend llvm -o out --dump-struct-layouts

# Via environment variable:
SEEN_TRACE_LLVM=1 ./target/release/seen_cli build file.seen --backend llvm -o out
```

**Key Files Modified:**
- `rust_backup/seen_ir/src/llvm_backend.rs`:
  - Added `LlvmTraceOptions` struct with `from_env()`, `all()` methods
  - Added `dump_struct_layouts()` method for memory layout debugging
  - Added `dump_struct_layouts_flag` field to enable via CLI
  - Added `trace_options` and `current_inst_idx` fields
- `rust_backup/seen_cli/src/main.rs`:
  - Added `--trace-llvm` flag to Build command
  - Added `--dump-struct-layouts` flag to Build command
  - Added `dump_struct_layouts` parameter to `compile_file_llvm()`
- `compiler_seen/src/parser/real_parser.seen`:
  - Fixed `BangEqual` → `NotEqual` enum variant reference

**Next Steps:**
- [ ] Fix String type inference for local variables (i64 vs {i64, ptr} struct)
- [ ] Track variable types through assignment chains
- [ ] Ensure `var_slot_types` correctly records String variables
- [ ] Add debug traces for variable type assignment

**Acceptance:** Stage1 binary compiles any valid Seen source file.

---

### Task 1.3: Stage2 Compilation
**Status:** ⏳ Blocked on Task 1.2
**Estimated:** 2-3 hours

**Tasks:**
- [ ] Use Stage1 to compile Stage2 from same sources (`stage2.out`)
- [ ] Compare Stage1 and Stage2 binaries (hashes differ)
- [ ] Debug any differences
- [ ] Record Stage2 hash

**Acceptance:** Stage2 binary generated successfully.

---

### Task 1.4: Stage3 and Determinism Verification
**Status:** ⏳ Blocked on Task 1.3
**Estimated:** 1-2 hours

**Tasks:**
- [ ] Use Stage2 to compile Stage3 (`stage3.out`)
- [ ] Verify Stage2 == Stage3 (hash equality)
- [ ] Record hashes in documentation
- [ ] Update `validate_d2_determinism.sh`

**Acceptance:** Stage2 and Stage3 are byte-identical.

---

### Task 1.5: Rust Removal Validation
**Status:** ⏳ Blocked on Task 1.4
**Estimated:** 2-3 hours

**Tasks:**
- [ ] Run `verify_rust_needed.sh` — should print "Rust NOT needed"
- [ ] Run `run_bootstrap_seen_only.sh` — 3-stage Seen-only bootstrap (stage2==stage3) and smoke tests
- [ ] Run full test suite with Stage1 compiler
- [ ] Execute `r4_release_playbook.sh` dry-run

**Acceptance:** All bootstrap scripts pass; Rust compiler not required.

---

## Phase 2: Platform Targets (Post-Self-Host)

### Task 2.1: Linux Completion
**Status:** 🔄 Mostly complete  
**Estimated:** 8-12 hours

**Completed:**
- [x] `extern "C"` bindings
- [x] Static/dynamic linking
- [x] LLVM native codegen

**Remaining:**
- [ ] Vulkan 1.3+ / SDL3 graphics bindings
- [ ] PipeWire audio with ALSA fallback
- [ ] evdev/libinput for input
- [ ] AppImage packaging finalization
- [ ] Steam for Linux integration

**Acceptance:** Full Linux game client builds and runs.

---

### Task 2.2: Windows Target
**Status:** ⏳ Pending (needs Windows host)  
**Estimated:** 16-24 hours

**Tasks:**
- [ ] Cross-compilation from Linux host
- [ ] Deterministic PE32+ binaries (`/TIMESTAMP:0`, `/BREPRO`, `/DYNAMICBASE:NO`)
- [ ] Vulkan 1.3+ / SDL3 backend
- [ ] WiX v4 MSI installer generation
- [ ] Steamworks SDK integration
- [ ] Visual C++ Redistributable bundling

**Acceptance:** Windows binary passes determinism verification; MSI installer works.

---

### Task 2.3: RISC-V Target
**Status:** ⏳ Pending  
**Estimated:** 12-16 hours

**Tasks:**
- [ ] Cross-compilation from x86_64 Linux
- [ ] Target `rv64gc` with `m`, `a`, `f`, `d`, `c` extensions
- [ ] VisionFive 2 / Milk-V Pioneer hardware testing
- [ ] Deterministic ELF64 across architectures
- [ ] CI pipeline with RISC-V hardware
- [ ] 24/7 uptime validation

**Acceptance:** RISC-V binary runs on VisionFive 2 for 1 week without crash.

---

### Task 2.4: UWW-Compatible WASM Target
**Status:** ⏳ Pending  
**Estimated:** 20-30 hours

**Tasks:**
- [ ] Deterministic `.wasm` with identical SHA-256 across hosts
- [ ] `.uww.metadata` section with hash and attestation
- [ ] WASM SIMD via `-C target-feature=+simd128`
- [ ] Forbid WASI imports (`fd_read`, `clock_time_get`, `random_get`)
- [ ] Provide `uww::timestamp`, `uww::deterministic_rand`, `uww::storage::*`
- [ ] WASM files under 1MB with aggressive dead code elimination
- [ ] Soft-float or fixed-point for deterministic numerics

**Acceptance:** WASM runs identically on 3+ UWW nodes with matching state hashes.

---

## Phase 3: Determinism Enforcement (Post-Self-Host)

### Task 3.1: `@deterministic` / `@nondeterministic` Annotations
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] Add `@deterministic` decorator for modules
- [ ] Add `@nondeterministic` decorator for functions
- [ ] Implement compile-time checking at call sites
- [ ] Error: "Use of nondeterministic type in deterministic profile"

**Acceptance:** Deterministic code cannot call nondeterministic code without annotation.

---

### Task 3.2: Collection Enforcement
**Status:** ⏳ Pending  
**Estimated:** 6-8 hours

**Tasks:**
- [ ] BTreeMap<K, V> / BTreeSet<T> as default (sorted, deterministic)
- [ ] HashMap<K, V> / HashSet<T> require `@nondeterministic`
- [ ] Vec<T> with hardcoded doubling (deterministic by default)
- [ ] `@allow_allocator_optimizations` for nondeterministic Vec growth

**Acceptance:** `--profile deterministic` rejects HashMap without annotation.

---

### Task 3.3: Fixed-Point Numerics
**Status:** ⏳ Pending  
**Estimated:** 12-16 hours

**Tasks:**
- [ ] `Fixed64` and `Fixed128` types
- [ ] `Qm.n` syntax (e.g., `fixed8.24`)
- [ ] Deterministic across all platforms
- [ ] Panic-on-overflow compiler switch
- [ ] Saturating and wrapping variants
- [ ] f32/f64 require `@nondeterministic` in deterministic profile

**Acceptance:** Fixed64 produces identical results on all platforms.

---

## Phase 4: Decorator System (Post-Self-Host)

### Task 4.1: Built-In Decorators
**Status:** ⏳ Pending  
**Estimated:** 16-24 hours

**Decorators to implement:**
- [ ] `@deterministic` — module-level determinism enforcement
- [ ] `@nondeterministic` — exemption from determinism rules
- [ ] `@component` — framework component with lifecycle hooks
- [ ] `@store` — state management with deterministic mutations
- [ ] `@middleware_stack` — composable middleware chain
- [ ] `@executor` — single-threaded async executor
- [ ] `@test` — unit test (deterministic by default)
- [ ] `@profile` — performance instrumentation
- [ ] `@hot_reload` — runtime code reloading
- [ ] `@derive(Reflect, Serialize, Deserialize)` — auto-generation
- [ ] `@syscall("uww::...")` — UWW runtime import
- [ ] `@_c_import("header.h")` — C library import
- [ ] `@preallocate(size = N)` — region pre-allocation

**Acceptance:** All decorators parse, type-check, and execute correctly.

---

### Task 4.2: User-Defined Decorators (Macro System)
**Status:** ⏳ Pending  
**Estimated:** 24-32 hours

**Tasks:**
- [ ] Design procedural macro syntax
- [ ] Implement macro expansion at compile time
- [ ] Ensure macro expansion is deterministic
- [ ] Macro-generated code passes determinism checks

**Acceptance:** Users can define custom decorators that expand correctly.

---

## Phase 5: UWW Infrastructure (Post-Self-Host)

### Task 5.1: Capability Tokens
**Status:** ⏳ Pending  
**Estimated:** 12-16 hours

**Tasks:**
- [ ] Static capability tokens for function-level constraints
- [ ] Syntax: `fn mix(p: Packet) -> Result using NetToken`
- [ ] Firefox Sidecar sandboxing (Seen modules barred from filesystem)
- [ ] Token validation at compile time

**Acceptance:** Functions without required tokens cannot access restricted syscalls.

---

### Task 5.2: Identity Protection
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] Generational handle masking (XOR with region-specific secret)
- [ ] Stealth Registry metadata protection
- [ ] Prevent memory probing attacks

**Acceptance:** Raw RAM reads cannot resolve identity handles without secret.

---

### Task 5.3: Trusted Execution (TEE)
**Status:** ⏳ Pending  
**Estimated:** 16-24 hours

**Tasks:**
- [ ] `enclave_call` intrinsic
- [ ] `seal_data` / `unseal_data` intrinsics
- [ ] Compile to Intel SGX or AMD SEV instructions
- [ ] Hardware attestation proofs

**Acceptance:** TEE intrinsics produce valid hardware attestation.

---

### Task 5.4: Deterministic Bit-Fields
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] First-class `bitfield` types
- [ ] Big-endian/little-endian control
- [ ] Guaranteed memory layout across targets
- [ ] Sphinx Mixnet 5-hop packet header matching

**Acceptance:** Bit-fields match across x86 and ARM architectures.

---

### Task 5.5: VSD Pointer Pinning
**Status:** ⏳ Pending  
**Estimated:** 6-8 hours

**Tasks:**
- [ ] `region` attribute preventing OS relocation
- [ ] VSD Mapper for 64KB shard paging
- [ ] 0-copy shard access

**Acceptance:** Pinned memory regions remain valid during paging operations.

---

## Phase 6: Framework-Building Features (Post-Self-Host)

### Task 6.1: Component Model
**Status:** ⏳ Pending  
**Estimated:** 12-16 hours

**Tasks:**
- [ ] `@component` decorator with lifecycle hooks
- [ ] Nested composition within deterministic parents
- [ ] Deterministic child enforcement by default

**Acceptance:** Components compose correctly with lifecycle management.

---

### Task 6.2: Virtual DOM Primitives
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] VNode structs with deterministic field types only
- [ ] BTreeMap for attributes (deterministic iteration)
- [ ] Vec with fixed growth for children

**Acceptance:** VDOM diffing produces deterministic results.

---

### Task 6.3: State Management
**Status:** ⏳ Pending  
**Estimated:** 12-16 hours

**Tasks:**
- [ ] `@store` decorator with deterministic mutations
- [ ] Auto-generated mutation log for replay
- [ ] Time-travel debugging via snapshots

**Acceptance:** State changes can be replayed deterministically.

---

### Task 6.4: Middleware System
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] `@middleware_stack` decorator
- [ ] Vec iteration order execution
- [ ] Sandbox isolation for nondeterministic user middleware

**Acceptance:** Middleware executes in deterministic order.

---

### Task 6.5: Routing
**Status:** ⏳ Pending  
**Estimated:** 6-8 hours

**Tasks:**
- [ ] Deterministic route registration/resolution
- [ ] BTreeMap storage for sorted routes
- [ ] Compile-time route pattern validation

**Acceptance:** Route resolution is deterministic and validated at compile time.

---

### Task 6.6: Async Executor
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] `@executor` decorator for single-threaded executor
- [ ] FIFO task order (VecDeque)
- [ ] No `Send`/`Sync` requirements (WASM-safe)

**Acceptance:** Executor runs tasks in deterministic FIFO order.

---

## Phase 7: Production Polish (Post-Self-Host)

### Task 7.1: Remaining Benchmark Execution
**Status:** ⏳ 5/10 passing  
**Estimated:** 8-12 hours

**Remaining benchmarks to fix:**
- [ ] Debug and fix failing 5 benchmarks
- [ ] Verify all checksums match
- [ ] Generate final performance comparison report

**Acceptance:** 10/10 benchmarks pass with correct checksums.

---

### Task 7.2: Break/Continue in Loops
**Status:** ⏳ Pending  
**Estimated:** 2-3 hours

**Tasks:**
- [ ] Add Break/Continue AST nodes
- [ ] Type-check (must be in loop)
- [ ] Add Break/Continue IR instructions
- [ ] Implement in interpreter
- [ ] Generate LLVM branch to loop exit/header

**Acceptance:** Break/continue work in all backends.

---

### Task 7.3: Operator Overloading
**Status:** ⏳ Pending  
**Estimated:** 4-5 hours

**Tasks:**
- [ ] Design syntax: `operator+(other: T)`
- [ ] Implement in parser and typechecker
- [ ] Generate proper IR for overloaded ops
- [ ] Add regression tests

**Acceptance:** Custom `+`, `*`, `[]` operators work for user types.

---

### Task 7.4: HSM/Sigstore Signing Integration
**Status:** ⏳ Pending  
**Estimated:** 8-12 hours

**Tasks:**
- [ ] Integrate HSM-backed signing (not just local files)
- [ ] Sigstore integration for public verification
- [ ] CI requires fresh manifests + signatures before publishing
- [ ] Publish public key with every release

**Acceptance:** Release artifacts are signed with HSM keys and publicly verifiable.

---

### Task 7.5: Platform Installers
**Status:** ⏳ Linux only  
**Estimated:** 16-24 hours

**Tasks:**
- [ ] Windows MSI installer with WiX v4
- [ ] macOS pkg installer with notarization
- [ ] iOS IPA builder
- [ ] Hook signing into release pipeline

**Acceptance:** All platform installers build and install correctly.

---

### Task 7.6: Incremental Compilation & Caching
**Status:** ⏳ Pending
**Estimated:** 20-30 hours

**Tasks:**
- [ ] Design `.seen_lib` binary format for pre-compiled headers/symbols
- [ ] Implement serialization of Type/AST state for modules
- [ ] Implement "check-only" mode that loads `.seen_lib` without re-parsing
- [ ] Add file-hash based invalidation logic
- [ ] Integrate with `seen build` to skip unchanged modules

**Acceptance:** Recompiling a project with 1 changed file takes <1s.

---

## Validation Gates

Before MVP closure, all must pass:

1. `cargo test --workspace` — Rust compiler tests green
2. `./target/release/seen_cli check compiler_seen/src/main.seen` — 0 errors
3. `./target/release/seen_cli build compiler_seen/src/main.seen --backend llvm` — native binary
4. Stage2 == Stage3 hash equality
5. `./verify_rust_needed.sh` — prints "Rust not needed"
6. `./validate_bootstrap_fixed.sh` — smoke test passes
7. All 10 benchmarks pass with correct checksums

---

## Definition of Done

MVP is complete when:

1. ✅ Self-hosting compiler passes complete type-check with zero errors
2. ⏳ Self-hosting compiler generates native binaries via LLVM
3. ⏳ Stage1→Stage2→Stage3 bootstrap produces identical hashes
4. ⏳ All four platforms (Linux, Windows, RISC-V, WASM) produce deterministic artifacts
5. ⏳ UWW demo runs identically on 5+ nodes with matching state hashes
6. ⏳ Hearthshire ships on Steam with 95%+ positive reviews
7. ⏳ HeartOn ECS runs 10k+ entities at 60 FPS
8. ⏳ Three third-party developers build apps with Seen frameworks
9. ⏳ RISC-V demo runs for 1 week without crash
10. ⏳ All nondeterministic features explicitly annotated and opt-in
11. ⏳ Framework decorators (`@component`, `@store`, etc.) production-ready
12. ⏳ Test suite passes with >90% coverage on all platforms

