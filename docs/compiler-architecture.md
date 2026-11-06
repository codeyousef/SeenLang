# Compiler Architecture

The Seen compiler is self-hosted and compiles through LLVM for native code
generation. The shipped release binary uses `compiler_seen/src/main_compiler.seen`
as its command entrypoint.

## Pipeline Overview

```text
Source (.seen)
  -> Lexer
  -> Parser
  -> Type checker
  -> Multi-module LLVM IR generation
  -> opt/llc or target compiler tools
  -> Native binary or target artifact
```

## Frontend

### Lexer

Location: `compiler_seen/src/lexer/`

- Loads keyword/operator tables from `languages/<lang>/`.
- Preserves source locations for diagnostics.
- Supports line comments and standalone-delimited `/// ... ///` block comments.
- Emits language-neutral token types so later stages do not need to know which
  human language was used.

### Parser

Location: `compiler_seen/src/parser/`

- Recursive-descent parser centered on `real_parser.seen`.
- Produces program, declaration, statement, and expression nodes.
- Parses current syntax including package imports, `effect(Token)`,
  `@using`, `fun operator+` declarations, nullable/nullish forms, `when`, closures, sealed
  classes, traits/interfaces, module namespace aliases, facade `component`
  functions, named arguments, trailing/named slot blocks, UI `state` /
  `computed` / `uiEffect` constructs, and hot-reload-facing shared-module
  patterns.

### Type Checker

Location: `compiler_seen/src/typechecker/`

Type validation is split across the bootstrap/frontend path, focused
typechecker modules, and whole-program checks in `main_compiler.seen`; there is
not one monolithic authoritative pass. The `TypeChecker` interface module also
serves compatibility callers. Together these paths track scoped symbols,
nullable information, deterministic-mode checks, effect/capability
requirements, and conservative unused/unreachable warnings.

Large project graphs establish one aggregate visibility domain per owning
`Seen.toml` before declaration collection. Declarations in modules explicitly
owned by the entry manifest remain project-visible in both forked and
`--no-fork` Pass 1b, while dependency manifests retain separate domains.
Imports and local declarations still take precedence; genuinely unknown names
remain errors, and conflicting project-wide declarations produce deterministic
ambiguity diagnostics. Exact duplicate immutable constants are compatible, but
mutable globals, types, functions, and unequal constant definitions are not.

Global type inference is completed against the aggregate declaration registry
after Pass 1 has collected every module. This lets an unannotated constant use
another project module's typed constant without treating an unresolved
uppercase value name as a type. Function parameters named `r` are ordinary
parameters; the `r: Type` return form is recognized only after the closing
parameter parenthesis, where it is syntactically unambiguous.

### Bootstrap Frontend

Location: `compiler_seen/src/bootstrap/`

The bootstrap frontend wraps lexing, parsing, and type checking into the
compatibility entrypoints used by Stage 1, Stage 2, the LSP, and package
declaration scanning.

New bootstrap helper modules must be reachable from `main_compiler.seen`
imports as well as from the embedded compiler-module list. That keeps older
bootstrap compilers from treating new helper calls as external declarations with
the wrong ABI during self-hosted rebuilds.
Package-map framing uses one caller-owned reference record for an entire scan.
The decoder borrows it exclusively, overwrites all offsets and validity for each
row, retains no input, and performs no per-row state allocation. Both driver
consumers release the record after scanning. This avoids the immutable seed's
unsupported cross-module aggregate-return ABI without rewriting source or IR.

Bootstrap-sensitive `parseTypeRef` calls supply text, line, column and file
explicitly; the immutable seed must not be relied on to fill an empty String
default across that call boundary.

### Deterministic CLI policy

CORE-004C resolves deterministic command-line options once in the shipped
`main_compiler.seen` entrypoint. The canonical policy records whether the
reproducibility mode was selected plus its effective semantic profile and SIMD
policy. `--deterministic` resolves to profile `deterministic` and SIMD `none`;
explicit contradictory values fail with `core.004c.conflict` before package,
frontend, cache, code-generation, or execution work. The checker consumes the
effective profile directly. Deterministic execution uses the AOT compile path
because the JIT path has no SIMD-policy input.

The higher-level `compiler_seen/src/main.seen` wrapper is not a release CLI
entrypoint and does not define this contract. The legacy `seen determinism`
command and non-LLVM wrapper paths are rejected rather than treated as
compatibility aliases.

### Resolved deterministic effect graph

CORE-004D runs after package-aware declaration collection and lexical semantic
validation. Each parsed module is streamed into bounded declaration, call-edge,
annotation, type, and source-provenance tables; effect propagation then follows
canonical symbols across imports, packages, aliases, generic call spellings,
closures, methods, traits, foreign declarations, and module initializers. The
same gate is called by `check` and `compile`, while deterministic `run` reaches
it through the policy-aware AOT path.

The graph classifies unordered hash collections, floating point, time, random,
I/O, process, environment, network, unsafe foreign calls, declared capability
effects, and resolved user calls. One exact, argument-free, non-conflicting
`@nondeterministic` annotation makes that declaration an explicit propagation
boundary. Unknown or ambiguous calls and unresolved effect tokens fail closed.
Diagnostics use stable `core.004d.*` codes and include the source location,
canonical owning declaration, bounded effect path, facility identity, and remediation. Node,
edge, direct-effect, propagation-round, path-length, and diagnostic limits are
explicit; exhaustion produces `core.004d.limit` rather than partial approval.

Builtin method effects require resolved receiver identity. For example,
`Array<T>.get` is a deterministic collection read whose result retains `T`;
user classes named `Array`, aliases to those classes and unrelated methods do
not acquire that intrinsic contract.

### Deterministic execution context

CORE-004E defines `seen-deterministic-context-v1`. Deterministic compile and
run capture a bounded, typed context from explicit epoch, seed, hash-seed,
locale, timezone, and allowlisted external-input policy. Missing, malformed,
or conflicting values fail closed with `core.004e.*` diagnostics. Native Seen
policy owns validation; the process boundary is the narrow nondeterministic
adapter. Required CI executes the context contract with the freshly accepted
Stage-1 compiler, including deterministic output parity and installed-source
payload identity.

### Reproducible user-program artifacts

CORE-004F keeps user-program reproducibility separate from normalized compiler
bootstrap comparison. `seen-program-build-input-v1` identifies complete build
inputs, `seen-object-cache-record-v1` and `seen-release-lto-cache-record-v1`
bind cache entries, and `seen-program-artifacts-v1` records raw hashes for
cold, warm, no-cache, release, LTO, package-graph, installed-compiler, and
path-remapped builds. Stable source identity and object ordering exclude
physical checkout roots, while temporary outputs are published atomically.

CORE-004G defines `seen-program-reproducibility-v1` for independent-builder
certification of that corpus. The canonical record binds source/toolchain
inputs, builder identities, exact artifact bytes, cleanup, cancellation, and
bounded diagnostics. Required CI validates these contracts before bootstrap;
release-toolchain preparation revalidates them without starting a second full
compiler/package build.

The measured native matrix requires a Linux x86-64 host and pins the CPU
baseline to `x86-64`. It does not request cross-target compilation, which has
separate full-LTO eligibility rules. One flag authority supplies actual
invocations, recorded commands and build-input digests, including release/LTO
mode and frozen-package flags.

Native runtime C compilation retains the caller's working directory, original
`pkg-config` arguments and include environment. Clang remains the authority for
forced-header lookup, system/framework paths, sysroots and search precedence;
the compiler does not parse or rebase those option families.

On native Linux, each Clang command opens the runtime directory as descriptor 3
and reads `/proc/self/fd/3/<unit>.c`. This command-scoped, zero-copy input identity
is stable across checkout/install roots while runtime-local quoted headers still
resolve through the same descriptor. It neither changes the working directory
nor creates shared source aliases. Failure to open or use that input fails the
compile rather than falling back to a physical-path-dependent identity.
Other native hosts use their physical runtime source paths without changing
the caller directory. Cross-target, JIT, platform-shim and bundled CUDA paths
retain their separate compilation contracts.

Runtime-object cache schema `runtime-object-v5` excludes earlier path-dependent
objects. Cache identity also binds the caller directory and the unset/empty/value
states of `CPATH`, `C_INCLUDE_PATH`, `CPLUS_INCLUDE_PATH` and `OBJC_INCLUDE_PATH`,
without interpreting or rewriting them.
Checkout-root discovery and common TEE/SGX/SEV source-availability probes
shell-quote their paths, including roots with spaces. Native consumer checks
exercise retained common-shim linkage and outside-enclave state; they do not
certify physical enclave hardware.


### Two-builder reproducibility

CORE-004A defines `seen-bootstrap-reproducibility-v1` in
`release.reproducibility`. Certification requires the same pinned bootstrap
builder binary hash in two distinct build roots, while pinning the same source
commit and tree digest, source-date epoch, toolchain signature,
command, target, and read-back containment limits. Both final compiler
artifacts must be byte-identical either raw (`normalization=none`) or after the
strict `elf64-x86_64-v1` transform. That transform requires a valid little-endian
ELF64 x86-64 image, zeroes only the GNU build-ID descriptor, and sorts the
fixed-width `.rela.dyn` records after proving their relocation offsets are
unique. All other bytes remain covered by the normalized SHA-256.

`scripts/release_bootstrap_matrix.sh` accepts an independently produced peer
Stage-3 artifact and its builder/root identities, re-verifies the active hard
scope, and delegates safe file hashing plus atomic evidence output to
`scripts/certify_two_builder_bootstrap.py`. Missing peer inputs, equal builder
or root identities, toolchain drift, unsafe files, and artifact mismatch fail
closed. A changed bootstrap-seed hash is rejected rather than confused with
environmental independence. The evidence schema is `releases/manifest.schema.json`; the checker is
`scripts/check_bootstrap_reproducibility.py`. No new foreign symbol is added.

### Signed release component pins

CORE-004B defines `seen-release-artifact-manifest-v1` in
`release.artifact_pins`. A production release has exactly four ordered
components: compiler, runtime, standard library, and version-coupled package
client. Each component is a regular non-symlink file with a bounded byte size,
SHA-256 checksum, verified signature bundle, and bundle digest. The canonical
manifest also pins the source commit, source archive digest, target, version,
signing mode, identity, and issuer. Unknown fields, unsafe names, reordered or
duplicate roles, absent sidecars, mismatched bytes, and post-sign verification
fail closed with `core.004b.*` diagnostics.

The native module owns validation and policy. The Python checker mirrors the
contract at the artifact boundary, while `sign_release.sh` and
`verify_release.sh` invoke the external signature tool as a ledger-neutral
process. Release packaging never builds or substitutes a missing package
client and produces deterministic runtime and standard-library archives. This
adds no native ABI symbol.

### Deterministic import graph

Location: `compiler_seen/src/imports/graph.seen`

Compiler module discovery is finalized by
`resolveRecursiveImportGraph`, the native Seen CORE-003A state machine. Its
fallible entry point returns `Result<ImportGraphResolution, SeenError>` and
accepts the immutable `OperationContext` used by release validation. The input
uses exact canonical module identities and bounded adjacency arrays; absolute
paths, backslashes, empty or traversal segments, duplicate identities, unknown
edges, duplicate edges, malformed ranges, unsupported platforms, and cycles
are rejected rather than normalized or repaired.

Edges are sorted by UTF-8 module identity before an iterative depth-first walk.
The declared root remains first, reachable dependencies follow deterministically,
and disconnected nodes use the same byte ordering. Limits cover module count,
edge count, identity bytes, and traversal depth. Cancellation is checked before
allocation and throughout edge validation and traversal. Failures use stable
`core.003a.*` codes from `release/diagnostic_schema.seen`, never retry validation
or policy errors, bound messages to 4 KiB, and carry explicit redaction policy.

The compile, check, and JIT paths all call the same resolver and have no legacy
cycle-detection fallback. Project graphs consume its canonical order.
Compiler-owned graphs are also validated fail-closed, while their existing
deterministic curated execution schedule is retained because that schedule is
part of the bounded-memory bootstrap contract. The compiling API example is
`compiler_seen/examples/import_graph_resolution.seen`. CORE-003A adds no foreign
symbol, so the native-boundary ledger remains unchanged.

### Deterministic global initialization

Location: `compiler_seen/src/imports/graph.seen` and
`compiler_seen/src/codegen/ir_module_emit.seen`

CORE-003B derives a dependency-first postorder from the same validated import
graph state machine used by CORE-003A. The public
`planDeterministicGlobalInitialization` entry point returns
`Result<GlobalInitializationPlan, SeenError>`; malformed graphs, unsupported
platforms, invalid bounds, cycles, and cancellation fail closed with stable
`core.003b.*` diagnostics. No second traversal or source-order repair path is
used.

Compile, check, and JIT consume the checked initialization schedule while
preserving the canonical module discovery/codegen order and the bounded compiler
bootstrap schedule. Each emitted module receives a unique, bounded LLVM
constructor priority derived from its dependency-first rank, so global
initialization does not depend on equal-priority linker order. The synthesized
per-module initializer remains internal and all its assignments retain source
order.

The compiling API example is
`compiler_seen/examples/global_initialization_plan.seen`, and Stage-1 acceptance
also executes a three-module runtime chain whose globals observe initialized
dependency state. The object-cache compatibility identity is
`seen-object-cache-abi-v3`. CORE-003B adds no foreign symbol, so the
native-boundary ledger remains unchanged.

The frozen Stage-1 compiler remains bound to its hash-verified v2 compatibility
manifest under `bootstrap/`. The bootstrap overlay copies that immutable record
instead of exposing the live v3 release manifest; newly produced compilers use
the live checkout record. This permits a fail-closed ABI transition without
teaching either compiler to ignore or repair a compatibility mismatch.

### Unmodified production IR

Location: `compiler_seen/src/imports/graph.seen`,
`compiler_seen/src/main_compiler.seen`, and `scripts/safe_rebuild.sh`

CORE-003C makes native codegen output the only production optimizer input. The
public `validateUnmodifiedProductionIr` entry point returns
`Result<ProductionIrPlan, SeenError>` and accepts only canonical bounded
artifact paths whose emitted and optimizer-input SHA-256 identities are exact
matches. Repair requests, changed bytes, unsupported platforms, invalid bounds,
and cancellation fail closed with stable `core.003c.*` diagnostics. The plan is
rendered deterministically as `seen-production-ir-policy-v1` and always reports
`repair_allowed: false`.

Current compiler, recovery, Windows cross-build, and saved-IR preflight paths
pass emitted IR directly to LLVM. Optimizer and object-emission failures retain
the raw artifact and return typed diagnostics; there is no retry with rewritten
IR. The immutable frozen Stage-1 compiler has one explicitly marked bootstrap
compatibility adapter because its historical output predates the native fixes.
That marker is cleared at rebuild entry, granted only to the exact frozen
compile or its captured raw artifacts, and any adapter failure aborts. A
repaired Stage-2 seed can build Stage 3 but can never be selected for production
installation; only unmodified current-compiler output is eligible.

The compiling API example is
`compiler_seen/examples/production_ir_policy.seen`. CORE-003C adds no foreign
symbol, so the native-boundary ledger remains unchanged. The existing
`seen-object-cache-abi-v3` identity also binds the unmodified production-IR
handoff policy; IR bytes and object semantics are unchanged for current
compilers.

### Unmodified production source

Location: `compiler_seen/src/imports/graph.seen` and
`scripts/safe_rebuild.sh`

CORE-003D makes checked-out source bytes the only production compiler input.
The public `validateUnmodifiedProductionSources` entry point returns
`Result<ProductionSourcePlan, SeenError>` and accepts only canonical bounded
paths whose checkout and compiler-input SHA-256 identities match exactly.
Rewrite requests, changed bytes, unsupported platforms, invalid bounds, and
cancellation fail closed with stable `core.003d.*` diagnostics. The
deterministic `seen-production-source-policy-v1` plan always reports
`rewrite_allowed: false`.

The bootstrap source view may relocate regular files to provide an immutable
frozen compatibility manifest and a symlink-free package layout, but it copies
every Seen source file unchanged and verifies each copy with `cmp`. The former
triple-slash body stripping and the obsolete Rust codegen rewrite utility are
removed. Current compilers, frozen builders, recovery builders, and platform
builds therefore see source bytes that are identical to the checkout; a source
compatibility failure stops the build instead of synthesizing alternate source.

The compiling API example is
`compiler_seen/examples/production_source_policy.seen`. CORE-003D adds no
foreign symbol, so the native-boundary ledger remains unchanged. The existing
`seen-object-cache-abi-v3` identity also binds the unmodified production-source
handoff policy.

### Stable machine diagnostics

Location: `compiler_seen/src/release/diagnostic_schema.seen`

CORE-REL-001 defines the `seen-machine-diagnostic-v1` envelope around the
existing `SeenError` contract. `validateMachineDiagnostic` returns
`Result<MachineDiagnostic, SeenError>` and validates every caller-owned field
before serialization: stable code/subsystem/operation identities, a 4 KiB
message, at most eight causes per node and eight nested levels, optional native
code, retry/redaction classes, canonical relative source locations, and bounded
accelerator context. Existing diagnostic codes are payload values and are not
renamed by the envelope.

Accelerator metadata records backend, target, device capability, entry point,
source line/column, explicit fallback reason, and exactly one maturity state:
`unsupported`, `compile-only`, `experimental-hardware`, `verified`, or
`production-certified`. Compile-only evidence is therefore distinguishable
from real hardware execution. Sensitive errors replace the message, source
path, and fallback reason with `<redacted>` before canonical JSON is emitted.
Validation, limit, cancellation, and unsupported-platform failures use stable
`core.rel.001.*` codes and never retry implicitly.

The compiling example is `compiler_seen/examples/machine_diagnostic.seen`.
The release compatibility schema binds `seen-machine-diagnostic-v1` to the
existing `seen-object-cache-abi-v3` compiler identity. This avoids adding a
structural compatibility bridge that older bootstrap compilers cannot parse.
CORE-REL-001 adds no foreign symbol and does not change the native-boundary
ledger.

## Code Generation

Location: `compiler_seen/src/codegen/`

The LLVM generator is now split into focused driver and helper modules rather
than a single monolithic implementation. `llvm_ir_gen.seen` is the public facade;
state-based helpers handle declarations, modules, functions, calls, binary
expressions, method calls, statements, literals, member/index access, control
flow, runtime declarations, and target-specific state.

Generation is organized around:

1. Declaration/signature collection.
2. Type/layout and registry preparation.
3. Function and module body lowering to LLVM IR.
4. Object emission, optimization, and linking.

Cross-module function declarations are retained in one indexed registry used
directly by module emission. The compiler does not duplicate that registry as
growing pipe-delimited strings; doing so creates quadratic retained allocation
during a self-host scan. On large graphs, full lexical-semantic ASTs and their
declaration registry are built and checked in bounded child processes, so only
the code-generation declaration state survives into module IR generation.

Package artifacts participate in code generation through interface indexes and
object manifests: dependency declarations are scanned, provided modules are
skipped for codegen, and prebuilt objects are linked into the final binary.

Native runtime ABI bridges are selected by the resolved foreign symbol, not by
the eligibility of a source declaration for intrinsic optimization.
`__ExecuteCommand` returns a native pointer to a command-result record; the
dedicated command bridge loads its `%CommandResult` aggregate before ordinary
source return lowering. Lexical math-name shadowing cannot bypass that bridge.

### Concrete aggregate generic layouts

Semantic and effect checks consume the original program. Before final native
declaration registration, `prepareGenericAggregateClasses` closes the demands
for source generic classes with closed aggregate/array arguments. Demand identity
includes the lexical declaration owner and resolved type arguments. Each
instantiation starts from a fresh parsed template, so rewriting one lowering AST
cannot mutate another shape or the ordinary word-erased template.

The `ir_generic_aggregate`, `_visit` and `_layout` modules reuse the ordinary
class layout, method ABI and cleanup paths. Inline aggregate and inherited-layout
dependencies are registered before enclosing layouts. String aggregates and
Array references retain their native field and call types across getters/setters
and function boundaries; runtime-owned containers keep their existing ABI.

The function ABI/body checks are separate from class specialization. A type
argument alone does not prove an ABI change: fixed container signatures may
remain valid, but dependent value storage or materialization must also be safe.
Inferred direct calls bind the existing source argument types and enter the same
proof queue before argument emission. Analysis retains move/borrow qualifiers so
implicit cleanup cannot select an incompatible runtime family. Compiler-owned
defaults use the common typed-zero authority. Null-capable Ptr and Array source
parameters, including methods, do not acquire contradictory `nonnull`
attributes; null-accepting Array runtime entrypoints retain the same guarantee.
The standard-library `Ok`/`Err` declarations retain their existing boxed Result
payload ABI, including an uninferred inactive label. That exception requires
the canonical source module, trusted file and constructor signature; a
same-spelled user function does not acquire it.
Unsupported aggregate-by-value free-function and method-own generic paths are
rejected rather than repaired with boxes, casts or an LLVM rewrite. This pass
changes lowering ASTs, not source files or post-emission production IR.

Field storage and expression ABIs are distinct. Opaque `Ptr` and Array/List
expressions retain LLVM `ptr`; typed `*T` addresses and class handles retain
their existing `i64` expression ABI even when a field stores a native pointer.
Field projection and pointer/word casts use `mapTypeState` as the expression
authority, avoiding both erased native pointers and duplicate conversions.

Generated callback identities and function definitions belong to the module,
not recursive expression/statement snapshots. Lowering reserves an identity
before entering a callback or parallel body and appends its definition after
restoring function-local scratch. Recursive lowering cannot discard inner
definitions or rewind either module-owned identity counter.
This does not add capture semantics to named callback blocks.

### Refactored Codegen Layout

The refactor intentionally leaves `llvm_ir_gen.seen` boring. It owns the
compatibility API, bridges legacy facade fields into shared state, and delegates
real lowering work to smaller modules. A quick rule of thumb:

| Module family | What belongs there |
|---------------|--------------------|
| `ir_decl_*` | declaration scanning, runtime declarations, type registration |
| `ir_module_*` | module entry/tail emission, string constants, object-unit flow |
| `ir_function_*` | function identity, attributes, entry/exit state, body setup |
| `ir_call_*` and `ir_method_*` | call planning, receiver handling, argument lowering |
| `ir_stmt_*` and `ir_*_driver` | statement/expression orchestration |
| `ir_*_emit` and `ir_*_plan` | leaf emission and small planning decisions |

Comments in these files should explain the boundary or invariant, not restate
the line of code below them. Good comments answer questions such as "why is this
state copied here?", "why does this pass run before that one?", or "what must be
true when this helper returns?".

## Backend and Targets

The shipped compiler supports the LLVM backend. It can emit native binaries and
target artifacts for the platforms listed in [CLI Reference](cli-reference.md).
Important target controls include `--target`, `--target-cpu`, `--simd`,
`--sanitize`, `--pgo-generate`, `--pgo-use`, `--pic`, and
`--object-manifest`.

## Native-boundary ledger

Native ABI use is explicit and versioned in
[`architecture/native-boundaries.json`](architecture/native-boundaries.json).
It records the owning subsystem, purpose, ABI, supported platforms, and each
foreign symbol. The ledger is a fail-closed contract: update it alongside any
production FFI addition, then run `tests/misc_root_tests/seen_native_boundaries_ledger.sh`.
The JSON shape is defined by
[`../schemas/native-boundaries.schema.json`](../schemas/native-boundaries.schema.json).

[`architecture/native-inventory.json`](architecture/native-inventory.json)
is the deterministic source inventory behind that review contract. It records
every production Seen `extern fun` symbol with its declaring source files, all
backend implementations present in compiler source, and which backend the
shipped CLI exposes. `scripts/ci_required.sh` regenerates the inventory in
memory and rejects any byte-level drift before a pull request can merge.

## Required CI contract

The authoritative `.github/workflows/ci.yml` preserves the required check
`CI / required` with a source-bound dependency graph:

1. `bootstrap` (120 minutes) provisions the pinned tools and builds a fresh
   Stage2/Stage3 toolchain with actual smoke checks. It uploads an **unverified**
   Stage3 candidate, not an installed or release-eligible compiler.
2. `acceptance` (195 minutes per job) runs the exact `core`, `io-simd`,
   `gpu-emitter`, `gpu-runtime`, and `async` matrix on separate hosted machines,
   with `fail-fast: false`. Every group downloads the same run's source-SHA-bound
   candidate and verifies its commit/tree, portable x86-64 baseline, file sizes,
   and hashes before execution. A group receipt is uploaded only after success.
3. `native-arm-inputs` (145 minutes) cross-builds the unchanged full Linux ARM64
   compiler and matching package client. Its exact three-member archive is
   cross-built input, not evidence of native execution.
4. `native-arm` (50 minutes, `ubuntu-24.04-arm`) verifies those inputs and runs
   the full native SIMD-C suite with real capability observations, four contract
   profiles, 60-second fuzz, four cross-object targets and thirty paired samples.
   Native ASan additionally requires fresh pinned-runtime clean/fault/finite-limit
   controls under the unchanged finite virtual and aggregate kernel limits.
5. `required` (30 minutes) always runs and explicitly rejects unsuccessful,
   skipped, or missing dependencies. It downloads the candidate, all five
   receipts and the ARM input/qualification bundles, verifies their common
   source identity and retained native evidence, then installs and certifies
   the compiler, performs installed Gate 0 checks, writes the full stamp,
   and produces `seen-release-toolchain-${sha}`. No other job produces an eligible
   release toolchain artifact.

The longest configured dependency path remains 345 minutes (20,700 seconds):
the 145+50-minute ARM branch does not exceed the 195-minute acceptance branch.
Build-capable jobs use Ubuntu 24.04, commit-pinned checkout/Go/artifact actions,
exact Go 1.26.5, read-only repository permissions and architecture-appropriate
pinned host provisioning. The portable x86-64 baseline remains required for x64
candidates; ARM qualification uses native ARM instructions and fresh bindings.
`scripts/run_ci_required.sh --phase bootstrap`, `--phase acceptance --group GROUP`,
`--phase native-arm-inputs`, `--phase native-arm` and `--phase finalize` are the
hosted entries. The default still runs the complete local CI surface with its
14,400-second full acceptance timeout.
Hosted workload deadlines subtract elapsed setup, a 600-second bounded
drain/publication/diagnostic reserve, and one second for clock rounding from
the unchanged phase limits. Release-toolchain preparation remains inside the
finalization hard scope and workload deadline.

Each phase independently derives its aggregate memory cap from live memory:
at most 60% of total, further bounded by available memory minus a 10%-of-total
reserve, with the existing half-available fallback if that reserve cannot be met.
Kernel-page rounding and smaller requested caps remain enforced. The v4 manifest
corrects stale quarter/half metadata without changing those execution limits.
Each phase enters a read-back-verified Linux cgroup v2 scope with zero swap,
a 24-task ceiling, serial compiler/LLVM work and the 2 GiB optimizer limit.
Scope-bound serializer handles are never artifacts and never cross machines.
Candidates travel as an exact three-member archive; the candidate helper rejects
unsafe paths, links, duplicates, excess sizes, and malformed identity before
publishing extracted files. An environment marker alone is never scope evidence.

Each phase/group retains separately named, bounded failure-log tails and gate
evidence, never failed bulk compiler IR. Candidate and receipt artifacts use
source-SHA-bound names and same-run downloads. Release verification additionally
requires all nine actual jobs to have completed successfully and the live
same-run exact-source final toolchain artifact; a workflow-level green conclusion
or candidate upload alone is not certification.

The exact reviewed limits and platform support are versioned in
[`architecture/ci-containment.json`](architecture/ci-containment.json), with
the JSON shape fixed by
[`../schemas/ci-containment.schema.json`](../schemas/ci-containment.schema.json).
Linux x86-64 is the required runtime lane and Linux ARM64 receives static
policy coverage. macOS and Windows are explicitly unsupported and fail closed
until an equivalent read-back-verified aggregate boundary exists.

Obsolete disabled workflows are not retained as fallbacks: they referenced
unsupported compiler commands and paths and bypassed current containment
policy. `scripts/check_ci_workflows.py` scans `.github` with explicit file and
byte bounds, rejects links and retired workflow paths, and validates the reviewed
YAML semantics, including the graph, commands, action pins, permissions, and
release authorization. Display-step labels, comments, and formatting are not
authorization. Its bounded, dependency-free YAML subset rejects aliases, tags,
duplicate keys, and unsupported syntax rather than interpreting them loosely. Run
`tests/misc_root_tests/seen_ci_workflow_contract.sh` and
`tests/misc_root_tests/seen_ci_containment_contract.sh` after any CI-policy
change.

The native `release.gate0_certification` policy validates the clean-checkout,
active-CI, pinned compiler, resource-limit, platform-applicability, and ordered
build/test/fuzz-smoke/package evidence before Gate 0 can close. Repair is
explicitly forbidden and disabled workflows must be absent. Linux x86-64 is
required; Linux ARM64 receives static-policy coverage; macOS and Windows fail
closed until equivalent containment exists. The compiling example is
`compiler_seen/examples/gate0_certification.seen`. This certification adds no
foreign symbol, so the native-boundary ledger remains unchanged.

## Release compatibility contract

Every release has a strict machine-readable compatibility record at
`releases/compatibility-manifest.json`. It identifies the compiler and package
client versions, runtime and compiler ABIs, package artifact schemas, standard
library module-manifest version, minimum LLVM major, and all advertised target
triples. The JSON contract is defined by
`schemas/compatibility-manifest.schema.json` and rendered deterministically by
`scripts/check_compatibility_manifest.py`.

`compiler_seen/src/release/compatibility.seen` owns both layers of the native
contract. `validateCompatibilityManifest` retains the bounded, side-effect-free
`core.002a.*` schema checks. `generateCompatibilityManifest` accepts an
explicit `CompatibilityReleaseInputs`, and `renderCompatibilityManifest`
produces one canonical UTF-8 representation. The strict decoder rejects
unknown and duplicate fields before constructing the typed model;
`consumeCompatibilityManifest` compares the canonical decoded record with the
complete runtime expectation. Atomic output validates before writing and uses
the runtime's transactional replacement primitive, so cancellation and errors
leave no partial manifest.

Before `seen pkg` launches the version-coupled package client, the compiler
consumes `releases/compatibility-manifest.json` in a source checkout or the
same installer-shipped bytes beside the executable. The manifest supplies the
sidecar version and request protocol only after every compiler, runtime,
standard-library, ABI, platform, and target value matches. There is no PATH,
default-value, or partial-manifest fallback.

The manifest's `seen-package-interface-v2` component entry binds the
independently versioned `seen-package-layout-v1` compatibility identity, which
the same native module owns with its bounded `ReusablePackageLayout`
validation/rendering API.
Every path and platform claim is supplied explicitly. The API accepts only the
canonical `Seen.toml`, `src/mod.seen`, tests, examples, readme, and license
mapping and returns typed `pkg.layout.001.*` errors instead of normalizing a
different tree.

The compiler component's `seen-object-cache-abi-v3` identity binds both the
`seen-import-graph-v1` canonical discovery order and the
`seen-global-initialization-plan-v1` dependency-first emission order. Any
incompatible ordering change must advance that ABI identity so cached objects
cannot cross the ordering boundary. The same identity binds
`seen-production-ir-policy-v1`: current codegen bytes reach LLVM without a
repair transform. It also binds `seen-production-source-policy-v1`: source
bytes reach compilation without a rewrite transform.

### Debug, coverage, and sanitizer builds

`seen compile` exposes `-g`/`--debug`, `--coverage`, and
`--sanitize address|undefined|thread|memory`. The validated native
`BuildInstrumentationPolicy` applies the selected Clang instrumentation to
every Seen LLVM module, retained runtime object, and compiled ledgered ABI shim.
Unsupported targets and malformed or excessive report paths fail with stable
`core.rel.002.*` errors; instrumentation is never silently dropped.

`--instrumentation-report <relative-path>` writes canonical
`seen-build-instrumentation-evidence-v1` JSON after a successful link. Its
component states distinguish `source-only`, `compile-only`, and
`hardware-executed`. A compiler invocation may emit only source or compile
evidence; hardware execution must come from a separate hardware gate. This
contract is bound by `seen-object-cache-abi-v3`.

### PGO and explicit LTO modes

PGO and explicit LTO requests normalize to `ReleaseOptimizationPolicy` and remain
restricted to Linux x86-64. Plain native `--release` separately defaults to full
merged optimization on eligible targets; it does not request the platform-specific
selection policy. Existing full-LTO eligibility and toolchain checks still apply.
`--lto=full` requires a complete merged-IR path; `--lto=thin` retains bounded
per-module ThinLTO. Missing tools, partial IR, or optimizer/object failures are
typed `core.rel.003.*` errors and never switch modes implicitly.

`--pgo-generate` and `--pgo-use` require `--release`. Profile use accepts a
canonical relative `.profdata` path, rejects raw profiles, and keys compiler,
runtime, and merged-LTO caches by the profile bytes. PGO flags cover Seen
modules, retained runtime objects, and compiled ABI shims.
Seen IR generation runs `pgo-instr-gen,instrprof` before optimization; profile
use runs `pgo-instr-use` at the matching pipeline point. This applies to both
per-module and full-LTO merged IR. Runtime/link driver flags alone do not
establish Seen function coverage. Release
acceptance checks actual Seen entry/helper counters in the collected profile.
PGO ThinLTO bitcode uses LLVM's pre-link pipeline so post-link call-graph
profile metadata is produced once, by the LTO backend.
Full-LTO merged IR, and the single-module full-LTO case, emit native objects
instead of handing already optimized post-link profile metadata to ThinLTO.

## Incremental and Parallel Compilation

The compiler uses source-level and IR-level caches:

- `.seen_cache/`
- `<SEEN_ARTIFACT_ROOT>/seen_ir_cache/`
- `<SEEN_ARTIFACT_ROOT>/seen_thinlto_cache/`
- `target/seen-build/runtime-objects/`
- `target/seen-build/release-lto/`

Cache-v4 keys use stable module identities rather than temporary bootstrap
overlay paths. Source/object reuse is scoped by the compiler binary hash,
compiler ABI signature, project declaration hash, module body hash, LLVM tool
versions, target/profile settings, LTO/PIC/sanitizer/PGO flags, and runtime
payload signatures. Body-only edits should miss the changed module's object key
without flushing otherwise valid neighboring cache entries, while compiler
codegen/layout changes reject stale objects automatically.

Normal multi-module compiler builds use bounded worker pools for IR generation
and optimizer work. Guarded scripts derive `SEEN_JOBS` and `SEEN_OPT_JOBS` from
memory caps and CPU count; the compiler also accepts `--jobs <n>` and
`--opt-jobs <n>`. Low-memory and bootstrap verification paths can still force
serial execution with `--no-fork`; guarded scripts also export
`SEEN_MEMORY_LIMIT_BYTES` so runtime allocation-heavy compiler phases fail with
Seen diagnostics instead of depending on host OOM behavior.

Release builds keep the full merged-IR LTO path by default for performance.
Memory-constrained callers can pass `--lto=thin` to stay on the
bounded per-module ThinLTO path. Warm release builds can reuse a
signature-keyed merged-LTO object while preserving the default merged-LTO mode.

`seen compile --emit-module-ir-dir <dir> --stop-after-ir` writes raw per-module
LLVM IR into a caller-owned directory and exits before object emission/linking.
Packaging and cross-build scripts use this instead of scraping compiler-owned
scratch artifacts. By default, compiler scratch and these caches live below
the checkout's ignored `.seen/agent-tools/compiler/` directory.

`SEEN_TRACE_BUILD=<path>` writes JSONL build events
from rebuild scripts and compiler phases such as module discovery, declaration
scan, cache hashing, IR/object emission, runtime object reuse, release merge,
release-LTO mode, and link. `SEEN_BUILD_TRACE=<path>` remains a compatibility
alias. Compiler trace events use millisecond timestamps and escaped JSON fields.

## Accelerator native dependencies

Accelerator SDKs are explicit native dependencies, never implicit compiler
dependencies. Importing Seen declarations does not probe an SDK. The CUDA
foundation under `seen_runtime/cuda` stops before CUDA language or package
discovery unless `SEEN_ENABLE_CUDA=ON` is supplied to its isolated build.
Consequently the default compiler, runtime, and standard-library build remains
CPU-only and has no CUDA/cuBLAS linkage.

Raw Vulkan declarations and imported module names do not select native linker
libraries. A project that calls the Vulkan loader declares `vulkan = {}` under
`[native.dependencies]`; importing Vulkan types alone requires neither that
declaration nor a Vulkan SDK probe. Native fixture projects use the same manifest
boundary rather than compiler-only automatic link inference.

The `seen_cuda_*` resource ABI is versioned separately from compiler lowering.
Native Linux x86-64 projects opt in with
`seen_cuda = { bundled = true }` under `[native.dependencies]`. The compiler
then builds only the packaged runtime source into its project-local,
digest-verified native-dependency cache and links that exact library; it never
searches for a system `libseen_cuda` or accepts an incompatible cached
artifact. Without the manifest opt-in, imports remain compile-only and CUDA SDK
discovery is forbidden.
It provides fixed-width statuses and opaque owned handles; scheduling,
allocation bounds, fallback, graph policy, algorithm caching, and model
semantics remain native Seen behavior. Version 0.14 ships Linux x86-64 `sm_89`
support at `experimental-hardware` maturity. Compile-only results never imply a
hardware certification, and fallback is never silent.

`seen-cuda-stream-launch-token-v1` is the narrow exception that permits a
separately built model-kernel adapter to enqueue on an exact Seen-owned stream.
`CudaStream.borrowLaunchToken()` asks the native adapter to validate the opaque,
generation-checked owner and expected device, then returns a fixed-width
short-lived view of `cudaStream_t`. The token transfers no ownership and is
invalidated by stream close. Borrowing has no allocation, CUDA synchronization,
fallback, or scheduling policy. Capture is supported explicitly and reported in
the token flags. The optional CUDA ABI remains outside CPU-only discovery and is
bound by the release manifest's `runtime-v15` compatibility identity.

## Key Source Areas

| Area | Purpose |
|------|---------|
| `compiler_seen/src/main_compiler.seen` | Shipped compiler CLI and bootstrap driver |
| `compiler_seen/src/main.seen` | Higher-level CLI wrapper source, not the current release entrypoint |
| `compiler_seen/src/bootstrap/` | Frontend orchestration and diagnostic compatibility |
| `compiler_seen/src/lexer/` | Tokenization and multilingual keyword loading |
| `compiler_seen/src/parser/` | AST construction |
| `compiler_seen/src/typechecker/` | Type, effect, and deterministic-mode checks |
| `compiler_seen/src/codegen/` | LLVM IR generation, runtime declarations, backend helpers |
| `seen_std/src/` | Standard library modules |
| `seen_runtime/` | C runtime primitives linked by Seen programs |

## Type Representation in LLVM IR

| Seen Type | LLVM IR Shape |
|-----------|---------------|
| `Int` | `i64` |
| `Float` | `double` |
| `Bool` | `i1` |
| `String` | `%SeenString` (`{ i64, ptr }`) |
| `Char` | `i64` |
| `Array<T>` | runtime array handle/pointer |
| Class/value handles | pointer or handle depending on lowering path |
| Simple enum | integer tag |
| Payload/data enum | experimental payload/tag paths; verify each use with a focused test |

## Contributing to the Compiler

1. Make source changes.
2. Run source-only gates first.
3. Run `scripts/safe_rebuild.sh` only with explicit memory limits derived from
   current system memory.
4. Commit only after the relevant checks pass.

See [Bootstrap System](bootstrap.md) for the staged rebuild workflow.
