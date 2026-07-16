# Seen package registry contract v1

This directory is a provisional design package shared by the Seen 0.10 compiler
and registry service. It is not yet the normative v1 contract. Public upload
endpoints must remain disabled until FEL-632 supplies and reviews the complete
OpenAPI, manifest, lockfile, record, error, archive, resolution, and signing
contracts. Any review change must update the schemas, fixtures, compiler test,
and service test atomically.

Contract paths are host-neutral. Development uses
`https://seen.dev.yousef.codes/packages`; production promotion later changes
the configured base URL to `https://seen.yousef.codes/packages` without
rewriting schema identities or trust rules.

A canonical hosted registry origin is a byte-exact absolute HTTPS URL with a
lowercase ASCII DNS host and one or more lowercase path segments. It contains
no credentials, port, query, fragment, percent encoding, dot segment, or
trailing slash. Clients compare the canonical string exactly and never repair
or follow a manifest origin to a different registry.

## Package identity and aliases

A registry package identity is exactly `owner/name`.

- Each segment is one to 63 lowercase ASCII letters, digits, or internal
  hyphens.
- A segment cannot start or end with a hyphen.
- Clients and servers reject non-canonical input. They never trim, lowercase,
  decode, Unicode-normalize, or otherwise repair an identity.
- Registry authorization, not the compiler grammar, decides whether an actor
  may publish under a syntactically valid owner.

ASCII-only canonical names make Unicode lookalikes invalid. Registration still
compares a candidate against active, reserved, and tombstoned names using a
pinned Unicode confusable-skeleton data revision plus similarity heuristics.
Skeletons are review keys, never replacement canonical names. Exact skeleton
collisions are rejected; other high-similarity candidates require manual
review.

The dependency key is a project-local alias and import root. The `package`
field is the registry identity:

```toml
[dependencies]
json_pkg = { package = "seen/json", version = "^2.1.0" }
tls_pkg = { package = "alice/tls", version = "~1.4", allow = ["network", "ffi"] }
```

In this example source imports begin with `json_pkg` or `tls_pkg`; registry
lookup, provenance, lock records, and cache ownership use `seen/json` or
`alice/tls`.
For v1 portability, aliases use the deliberately narrower ASCII grammar
`[A-Za-z_][A-Za-z0-9_]{0,63}`. This is an import-alias grammar, not the full
multilingual Seen identifier grammar. Two aliases may refer to the same
canonical package, but duplicate aliases are rejected. An alias also cannot
collide with the current project's import root or a compiler-reserved import
root.

## Version requirements and deterministic resolution

Hosted dependency requirements use one of four canonical v1 forms:

| Form | Example | Meaning |
| --- | --- | --- |
| Exact | `2.1.0` or `2.1.0-rc.1+build.5` | Match the complete canonical version string byte-for-byte, including build metadata. |
| Caret | `^2.1.0` | Inclusive declared lower bound and exclusive next leftmost-nonzero bound. `^0.2.3` ends before `0.3.0`; `^0.0.3` ends before `0.0.4`. |
| Tilde | `~1.4` or `~1.4.2` | Start at `1.4.0` or the full declared version and end before `1.5.0`. |
| Comparator conjunction | `>=1.2.3 <2.0.0` | Exactly one lower and one upper full-version bound, in that order, separated by one ASCII space. Lower operators are `>` or `>=`; upper operators are `<` or `<=`. |

Caret operands are full versions. Tilde alone permits the canonical
`major.minor` shorthand. Range operands cannot contain build metadata, and a
comparator interval must be nonempty with its lower version preceding its
upper version by SemVer precedence. V1 rejects rather than repairs OR ranges,
wildcards, tags such as `latest`, hyphen ranges, unbounded comparators, `=`
prefixes, leading or trailing whitespace, tabs, commas, and repeated spaces.
The grammar and accept/reject cases are frozen by
`schemas/semver-requirement-v1.schema.json` and
`fixtures/semantic-requirement-cases-v1.json`.

Resolution is over the intersection of every requirement for one canonical
`(registry_origin, package)` key. The manifest registry alias is resolved once
to a canonical origin before solving; metadata and locks from another origin
are errors, and the resolver never falls back to another registry. Stable
candidates are eligible by ordinary SemVer precedence. A prerelease candidate
is eligible only when every requirement on that package contains a prerelease
operand with the same major, minor, and patch tuple. A stable release following
an opted-in prerelease remains eligible.

Fresh resolution and update exclude yanked and security-quarantined releases.
Normal or frozen resolution may retain a yanked release only when an existing
lock matches its origin, exact version, and archive digest. Security quarantine
always invalidates a lock. Among eligible candidates the resolver chooses the
highest SemVer precedence. Build metadata does not affect precedence: if a
range has multiple highest candidates differing only by build metadata, the
resolver reports `ambiguous_build_metadata` instead of using a lexical
tie-break. The registry returning one exact version with different digests is
`metadata_equivocation`.

Frozen mode requires a valid lock and never reselects. Normal mode tries a
valid locked candidate first, then canonical descending candidate order if the
lock is missing or no longer satisfies the graph. Update mode ignores lock
preference. Package decisions are ordered by the UTF-8 bytes of canonical
origin then identity; constraints are ordered by requester then requirement;
candidate traversal and depth-first backtracking use the frozen order. The
first complete graph wins. If none exists,
`dependency_constraint_conflict` reports requesters in canonical order. A new
lock is committed atomically only after the complete graph and every archive
digest verify. `fixtures/resolution-policy-v1.json` and
`fixtures/deterministic-resolution-cases-v1.json` are the executable reference
contract for the per-package candidate-selection kernel. Transitive dependency
graph expansion, canonical depth-first backtracking, and atomic whole-graph lock
commit fixtures remain explicitly deferred to FEL-629 alongside the full hosted
resolver; the current fixture names must not be read as executable graph-solver
coverage.

The current compiler implements only the exact-requirement subset represented
by `fixtures/scoped-dependencies.toml`; it does not yet consume a lock for
resolution. FEL-629 owns the complete requirement parser, solver, lock modes,
and capability consent. Hosted resolution remains disabled until that work
enforces this contract.

A publishable project declares its canonical identity independently from its
local project name:

```toml
manifest-version = 1

[project]
name = "math_core"
version = "1.2.3"

[package]
identity = "alice/mathx"
visibility = "public"
include = ["src/**/*.seen", "README.md", "LICENSE"]
assets = []
capabilities = []
```

The local project/import root (`math_core`), dependency alias (`calc`), package
identity (`alice/mathx`), registry origin, and resolved version are five
separate values. No equality between the project name and the identity's final
segment is implied. The compiler maps a scoped release to the content-addressed
path `.seen/packages/<owner>/<name>/<version>/<archive-sha256>/`; it validates
every component before forming that path. Two registries therefore cannot
silently share a cache entry for different bytes. Legacy unscoped
static-registry dependencies remain a
temporary local-development compatibility path and are not valid identities
for the hosted v1 registry.

`Seen.lock` is the dependency resolver lock format. Version 2 records an explicit
alias, canonical package identity, exact resolved version, canonical registry
origin, source kind, and the digest of the exact downloaded archive. It never
records a mutable manifest registry alias as the origin. The current compiler
writes this shape as a provisional resolution report but does not yet enforce
it on a later run; `--locked`/`--frozen` consumption is required before hosted
resolution can be enabled. The stdlib ABI/module hash snapshot is a different
artifact named `Seen.modules.lock`.

## Namespace lifecycle

The `seen` owner and operational owners listed in the identity fixture are
reserved. The `seen` owner can be controlled only by the registry operator and
cannot be transferred.

Namespace ownership transfer requires acceptance by the current owner and the
new owner, a seven-day cooling period, recent passkey authentication by both,
and an immutable audit record. Transfer changes authorization only; package
identities, versions, digests, attestations, and history do not change.

Public package identities are never renamed or recycled. Moving a package to a
different namespace publishes a new identity and tombstones the old identity;
the resolver never follows an identity redirect automatically. A namespace
that has ever owned a public release is never recycled. An empty namespace with
no release history may be reclaimed only after 180 days of inactivity, a
30-day notice, recovery review, and an auditable operator decision.

## Package contents and capabilities

Hosted archives are source-only. They may contain Seen source, `Seen.toml`,
documentation, tests, license files, and small assets matched by explicit
include globs. They may not contain executables, object files, dynamic or static
libraries, device entries, links, or install/build lifecycle scripts.

The root `Seen.toml` inside the uploaded archive is authoritative. The registry
hashes its exact raw bytes, parses it strictly, and byte/field-matches the
reservation's manifest digest, complete parsed manifest, path identity, version,
capabilities, include globs, and assets. Include membership is recomputed from
validated effective archive paths. Client-supplied parsed manifest fields never
replace this check, and any mismatch rejects before ready state or promotion.

Initial hard limits are 25 MiB compressed and 100 MiB expanded. The ingestion
contract also pins configurable file-count, per-file, path-length, path-depth,
compression-ratio, and validation-time limits. Limit or scanner failure rejects
closed with a stable machine-readable error code.

The canonical capability vocabulary is `file`, `network`, `process`,
`environment`, `dynamic-load`, `ffi`, `unsafe`, `native-link`, and `macro`.
Packages declare requested capabilities; dependency edges declare `allow`.
Capabilities are consent and policy signals, not an operating-system sandbox.
In particular, FFI, unsafe operations, and native linking can escape language
enforcement. The current compiler parser does not enforce package capabilities
or dependency `allow`; these fields are contract fixtures only, and hosted
resolution remains disabled until consent enforcement is implemented.

## Release state and immutability

The executable transition fixture keeps four orthogonal dimensions:
lifecycle, visibility, resolver availability, and retention. A public release
cannot use the private first-scan-to-ready edge: it passes a distinct first
scan, waits at least 72 hours, and then enters a distinct second-scan state with
a fresh source-proof check. Scanner outages and inconclusive results retry from
quarantine or delay while remaining unavailable; only definitive policy
failure rejects a reserved version.

Package identity, version, and archive digest are reserved atomically before a
release enters quarantine. Manifest/capability data derived from the archive is
bound immutably to that digest. Later provenance and scan rechecks are
append-only attestations. Blob promotion must complete before resolver-visible
signed metadata is published. Yanking and security quarantine change signed
availability, not lifecycle history or bytes; neither allows version reuse.

Private package names and versions are not enumerable. A private release may be
soft-deleted, but its identity/version reservation and audit record remain; blob
purge follows the retention policy. Converting private content to public starts
the public quarantine and 72-hour delay from the beginning.

Registry and dashboard language may say “origin verified”, “integrity
verified”, or “passed scans”. It must never claim that a package is safe.

## Signed resolver metadata

Public TUF targets represent only releases whose state is exactly public,
active, and retained. The releases role may publish `available` or `yanked`
availability; the security role is authoritative for quarantine or reviewed
reinstatement of the identical target key and cannot change its archive digest.
Private releases never enter this anonymously served metadata.

Every target key has the byte-exact form
`packages/<owner>/<name>/<version>/<sha256>/<name>-<version>.seenpkg.tgz`.
The owner/name, exact version, digest, and archive filename must match the target
custom fields, and the target hash must match the same digest. Clients and
signers reject rather than decode, normalize, or repair any mismatch.

Metadata signatures and metadata-file hashes use
`seen-tuf-canonical-json-v1`, frozen in the signing-policy schema and fixture.
The same JSON serialization primitive applies to the `signed` value for
signature input and to the complete envelope (including signatures) for
snapshot/timestamp length and SHA-256 fields.

Bearer credentials for operations marked with a recent-auth maximum must carry
the JWT NumericDate `auth_time` claim. The registry compares its server clock to
that claim; `exp` and token issue time are not substitutes for fresh
reauthentication.

## Contract verification

Run:

```bash
python3 tests/package_registry_contracts.py
```

The test uses only the Python standard library so compiler and service builds
can consume the same fixtures without a package-manager bootstrap dependency.
Passing it establishes fixture consistency, not approval of the unfinished v1
contract.
