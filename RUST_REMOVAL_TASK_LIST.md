# Rust Removal Task List  
**Goal:** Complete MVP self-hosting and remove all Rust compiler code
**Current Status:** 🟡 Type checking complete, runtime issues remain

## Critical Path to Rust Removal

### Phase 1: Fix Runtime Build Issues (BLOCKING)
**Status:** 🔴 In Progress  
**Blocker:** Build command fails with Debug AST dump instead of clean errors

- [ ] **Task 1.1:** Fix error handling in build command
  - Location: `seen_cli/src/main.rs` build subcommand
  - Issue: Runtime errors dump raw AST Debug instead of user-friendly messages
  - Fix: Properly catch and format interpreter/codegen errors
  
- [ ] **Task 1.2:** Identify build failure root cause
  - Add verbose logging to see which phase fails (IR gen, codegen, linking)
  - Test with minimal program first (Hello World)
  - Isolate whether issue is in interpreter, IR, or codegen
  
- [ ] **Task 1.3:** Test simple program compilation
  - Create `test_hello.seen` with just `fun main() { __PrintLn("Hello") }`
  - Verify: `seen_cli build test_hello.seen -o test_hello`
  - Verify: `./test_hello` runs correctly

### Phase 2: Complete Core Standard Library
**Status:** 🟡 Partial - Intrinsics exist, high-level APIs missing

- [ ] **Task 2.1:** String operations module (`seen_std/string.seen`)
  - `String.length()`, `String.charAt(i)`, `String.substring(start, end)`
  - `String.indexOf(substr)`, `String.replace(old, new)`
  - `String.split(delim)`, `String.join(parts, delim)`
  - `String.toUpperCase()`, `String.toLowerCase()`
  - `String.trim()`, `String.startsWith()`, `String.endsWith()`

- [ ] **Task 2.2:** Array operations module (`seen_std/array.seen`)
  - `Array.length()`, `Array.get(i)`, `Array.set(i, val)`
  - `Array.push(val)`, `Array.pop()`, `Array.shift()`, `Array.unshift(val)`
  - `Array.map(fn)`, `Array.filter(fn)`, `Array.reduce(fn, init)`
  - `Array.slice(start, end)`, `Array.concat(other)`

- [ ] **Task 2.3:** Map operations module (`seen_std/map.seen`)
  - `Map.get(key)`, `Map.set(key, val)`, `Map.has(key)`, `Map.delete(key)`
  - `Map.keys()`, `Map.values()`, `Map.entries()`
  - `Map.size()`, `Map.clear()`

- [ ] **Task 2.4:** File I/O module (`seen_std/io.seen`)
  - High-level wrappers over `__ReadFile`, `__WriteFile`
  - `File.readText(path)`, `File.writeText(path, content)`
  - `File.readLines(path)`, `File.writeLines(path, lines)`
  - `File.exists(path)`, `File.delete(path)`
  - `Dir.create(path)`, `Dir.list(path)`

- [ ] **Task 2.5:** Math module (`seen_std/math.seen`)
  - Constants: `Math.PI`, `Math.E`
  - Functions: `Math.abs()`, `Math.min()`, `Math.max()`
  - `Math.floor()`, `Math.ceil()`, `Math.round()`
  - `Math.sqrt()`, `Math.pow()`, `Math.log()`
  - `Math.sin()`, `Math.cos()`, `Math.tan()`

### Phase 3: Codegen Implementation
**Status:** 🟡 Multiple versions exist, none fully functional

- [ ] **Task 3.1:** Choose canonical codegen implementation
  - Evaluate: `simple.seen`, `real_codegen.seen`, `complete_codegen.seen`
  - Pick one and mark others as deprecated/experimental
  - Document: Why this one, what it supports, what it doesn't

- [ ] **Task 3.2:** Implement C backend codegen
  - Generate C code from Seen IR
  - Handle: functions, structs, enums, arrays, strings
  - Emit runtime helpers for GC, concurrency if needed
  - Test: Compile generated C with gcc/clang

- [ ] **Task 3.3:** Implement LLVM backend codegen (future)
  - Generate LLVM IR from Seen IR
  - Use llvm-sys bindings or shell out to llc
  - Requires LLVM installed on system
  - Optional for MVP, required for performance

### Phase 4: Bootstrap Verification
**Status:** 🔴 Not Started - Blocked on Phase 1

- [ ] **Task 4.1:** Create minimal self-hosting test
  - Write `minimal_compiler.seen` with just lexer + parser
  - Test: Can it compile itself?
  - Verify: Output is byte-for-byte identical or has known diffs

- [ ] **Task 4.2:** Three-stage bootstrap script
  - **Stage 0:** Rust compiler → Seen compiler binary (stage1)
  - **Stage 1:** stage1 → Seen compiler binary (stage2)
  - **Stage 2:** stage2 → Seen compiler binary (stage3)
  - **Verify:** stage2 == stage3 (determinism)

- [ ] **Task 4.3:** Automated bootstrap CI
  - Script: `scripts/bootstrap_self_hosting.sh`
  - Run: On every commit to main
  - Assert: All stages succeed, determinism holds
  - Time: Should complete in < 5 minutes

### Phase 5: Rust Removal
**Status:** 🔴 Not Started - Blocked on Phase 4

- [ ] **Task 5.1:** Archive Rust implementation
  - Move all `seen_*` Rust crates to `legacy/` directory
  - Keep for reference and comparison
  - Document: "Historical implementation, superseded by self-hosted compiler"

- [ ] **Task 5.2:** Create pure-Seen distribution
  - Package: Self-hosted compiler binary
  - Include: Standard library (`.seen` files)
  - Include: Runtime libraries if needed
  - Include: Build script to recompile compiler from source

- [ ] **Task 5.3:** Update documentation
  - README: Remove Rust setup instructions
  - README: Add "Building from source" for self-hosted compiler
  - Contributing: Update with new pure-Seen development flow
  - Architecture docs: Reflect self-hosted design

- [ ] **Task 5.4:** Release v1.0.0
  - Tag: `v1.0.0-self-hosted`
  - Announce: "Seen compiler now 100% written in Seen"
  - Artifacts: Binaries for Linux, macOS, Windows
  - Milestone: MVP complete, Alpha development begins

## Current Blockers (Prioritized)

### P0: Critical - Prevents All Progress
1. ❌ **Build command Debug dump** - Can't compile anything until fixed
2. ❌ **Build command failure** - Unknown root cause, needs investigation

### P1: High - Blocks Self-Hosting
3. ⚠️ **Missing standard library** - Programs can't do basic operations
4. ⚠️ **Incomplete codegen** - Can't generate executables even if IR works

### P2: Medium - Blocks Polish
5. ⚠️ **Error message quality** - Hard to debug when things go wrong
6. ⚠️ **Type structure alignment** - main_compiler.seen doesn't match ast.seen

### P3: Low - Can Be Deferred
7. ℹ️ **Performance optimization** - Functional first, fast later
8. ℹ️ **LLVM backend** - C backend sufficient for MVP
9. ℹ️ **Advanced features** - Traits, effects, etc. not needed yet

## Success Criteria

### Minimum Viable Product (MVP) ✅ When:
- ✅ Self-hosted compiler has **zero type errors** (DONE!)
- ✅ All intrinsic functions registered (DONE!)
- ⏳ Self-hosted compiler **builds to working binary**
- ⏳ Binary can **compile simple programs** (Hello World)
- ⏳ Binary can **compile itself** (bootstrap works)
- ⏳ Three-stage bootstrap **produces identical output** (determinism)
- ⏳ Standard library **core modules** implemented
- ⏳ **No Rust code** in critical path

### Stretch Goals (Not MVP, Nice to Have)
- 🎯 LLVM backend for performance
- 🎯 Full standard library (collections, networking, etc.)
- 🎯 Package manager integration
- 🎯 Cross-compilation support
- 🎯 Multi-platform binaries (Windows, macOS, Linux)

## Time Estimates

### Optimistic (Everything goes smoothly)
- Phase 1: 2-4 hours
- Phase 2: 4-8 hours  
- Phase 3: 8-16 hours
- Phase 4: 2-4 hours
- Phase 5: 1-2 hours
- **Total: 17-34 hours** (~2-4 days of focused work)

### Realistic (Some debugging needed)
- Phase 1: 4-8 hours (runtime errors are tricky)
- Phase 2: 8-16 hours (stdlib is tedious)
- Phase 3: 16-32 hours (codegen is complex)
- Phase 4: 4-8 hours (bootstrap issues)
- Phase 5: 2-4 hours (documentation)
- **Total: 34-68 hours** (~5-9 days of focused work)

### Pessimistic (Major issues discovered)
- Phase 1: 8-16 hours (deep runtime bugs)
- Phase 2: 16-24 hours (stdlib edge cases)
- Phase 3: 32-48 hours (codegen redesign)
- Phase 4: 8-16 hours (bootstrap failures)
- Phase 5: 4-8 hours (migration issues)
- **Total: 68-112 hours** (~9-14 days of focused work)

## Next Immediate Actions (This Session)

Since you're going to sleep, here's what I recommend tackling next:

### Priority 1: Fix the Debug Dump Issue
1. Look at `seen_cli/src/main.rs` build command error handling
2. Find where it's printing Debug instead of Display
3. Add proper error formatting for all error types
4. Test with simple program to verify clean errors

### Priority 2: Minimal Working Build
1. Create `test/hello.seen` with minimal code
2. Try building it: `seen_cli build test/hello.seen -o hello`
3. If it fails, add logging to see which phase breaks
4. Fix the immediate blocker (probably in interpreter or IR gen)

### Priority 3: Document the Journey
1. Keep notes on what you find during debugging
2. Update this task list as things get completed
3. Add lessons learned to a "debugging guide" doc

## Files to Focus On

### For Phase 1 (Build Issues)
- `seen_cli/src/main.rs` - Build command implementation
- `seen_core/src/lib.rs` - Compilation pipeline
- `seen_interpreter/src/lib.rs` - Runtime execution
- `seen_ir/src/generator.rs` - IR generation

### For Phase 2 (Stdlib)
- `seen_std/string.seen` - Create new
- `seen_std/array.seen` - Create new
- `seen_std/map.seen` - Create new
- `seen_std/io.seen` - Create new
- `seen_std/math.seen` - Create new

### For Phase 3 (Codegen)
- `compiler_seen/src/codegen/` - Pick one implementation
- `seen_ir/src/codegen_c.rs` - Rust reference for C backend
- Test generated C code compilation

---

**Status Legend:**
- ✅ Complete
- 🟢 In progress, going well  
- 🟡 In progress, some issues
- 🔴 Blocked or not started
- ❌ Critical blocker
- ⚠️ High priority
- ℹ️ Low priority
- 🎯 Stretch goal
- ⏳ Waiting/dependent
