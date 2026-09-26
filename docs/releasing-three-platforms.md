# Three-platform release procedure

The required v0.22.7-and-later release set is Linux x86-64, macOS arm64, and
Windows x64. A macOS x64 build is not implied by “macOS”; do not publish a
Homebrew formula that refers to an absent macOS x64 archive. Publishing is one
draft-to-public transaction, never three unrelated releases or a Linux-only
success followed by optional uploads.

## Source and containment

1. Work in an isolated clean checkout at the exact candidate commit. Run
   focused tests, full `scripts/run_ci_required.sh`, and clean-tree
   `scripts/prepare_release_toolchain_artifact.sh` before pushing. Every
   build-capable command runs serially through `run_with_project_artifacts.sh`
   and `run_in_hard_memory_scope.sh`, with numerical memory/swap/task read-back,
   a current-memory-derived cap no greater than 64 GiB, and a timeout. If this
   Linux scope is unavailable, stop; do not substitute an uncapped Mac build.
2. Merge/push the exact tree to `main` and obtain the successful authoritative
   `CI / required` push run for that SHA. Do not poll it. Preserve the run ID.
3. Build both cross-host inputs from the clean final commit, using a locally
   copied, hash-recorded Apple SDK and the verified candidate compiler:

   ```bash
   SEEN_MACOS_SDKROOT=/absolute/ignored/MacOSX.sdk \
   SEEN_RELEASE_CROSS_BUILDER=/absolute/verified/seen \
   SEEN_GO=/absolute/pinned/go SEEN_JOBS=1 SEEN_OPT_JOBS=1 \
   scripts/build_three_platform_inputs.sh VERSION
   ```

   This invokes the ABI-transformed Windows compiler build, ZIP/NSIS packaging,
   and macOS arm64 compiler/package-client cross-build serially in one Linux
   hard scope. It creates a source-commit/hash manifest in the ignored input
   directory. Require the transformed Windows ABI contract, PE compiler
   `--version` and `seen-pkg` protocol under the bounded headless Wine fixture,
   and check Mac SDK hash, Mach-O format, linked libraries, archive layout,
   and source-commit provenance. The direct LLVM Windows target is not a
   substitute for the validated ABI transformer. If a hard-bounded native Mac
   smoke is unavailable, report that limitation explicitly; static Mach-O
   validation is not native execution evidence.

## One tag and one release

1. Rebuild both cross-platform inputs **after the final source commit**. The
   Windows ZIP and macOS archive each carry `release-provenance.env` with the
   exact commit. Do not reuse an archive from a dirty/pre-commit tree.
2. After exact-SHA main CI is green, create and push one annotated `vVERSION`
   tag peeling to that SHA. Tag push no longer auto-publishes.
3. Copy the macOS arm64 archive, Windows x64 ZIP, and NSIS installer to one
   ignored input directory. Run `scripts/stage_release_platform_inputs.sh
   VERSION ABSOLUTE_INPUT_DIR`. It validates the payloads, writes a manifest of
   names/sizes/SHA-256/source commit/tree, and creates an unpublished draft.
   A pre-existing release is a stop condition. Record the draft's numeric release
   ID and four asset IDs/digests; do not rely on tag-name lookup from an Actions
   runner token. Before spending the release tag attempt, dispatch
   `.github/workflows/release.yml` on `main` with `probe_only=true`, the exact
   `version`, and `draft_release_id`. Require its read-only Windows runner-token
   access job to succeed; the signed-release job must be skipped.

   ```bash
   gh workflow run release.yml --ref main \
     -f version=VERSION -f draft_release_id=NUMERIC_ID -f probe_only=true
   ```

   Record the single probe run ID and inspect it after completion. A failed
   probe blocks tag dispatch; do not substitute a maintainer-token check.
4. Dispatch `.github/workflows/release.yml` **on the tag ref**, with its exact
   `version` and numeric `draft_release_id` inputs and `probe_only=false`, once.

   ```bash
   gh workflow run release.yml --ref vVERSION \
     -f version=VERSION -f draft_release_id=NUMERIC_ID -f probe_only=false
   ```

   The required `windows-smoke` job downloads and hashes assets by validated
   numeric IDs on a real Windows x64 runner and executes the compiler and package
   client with short timeouts. Only then does the Linux release job check
   merged-main CI, download the exact certified Linux toolchain, and
   build Linux artifacts inside the hard scope. It signs the four Linux
   components/manifest and signs the combined `SHA256SUMS` covering all three
   platforms. Upload, re-download, and publication use the same validated
   numeric draft ID. Only after re-downloading and byte-comparing the complete draft
   asset set does it publish the draft. A failed run leaves the release draft;
   never delete or clobber it without an explicit remediation decision.
5. When the workflow is reported complete, inspect it once. Download every
   published asset independently; verify the annotated tag object and peel,
   source commit/tree, three archives, asset SHA-256, `SHA256SUMS` signature,
   component bundles, compiler/package-client/manifest identity, CPU baseline,
   and extracted read-only installed-layout smoke. Do not claim release success
   because a tag or workflow run merely exists.
6. Only then install the audited Linux archive system-wide with `pkexec` if
   needed. Never use `sudo`. Compare installed files and version to the
   audited archive and record exact evidence in the owning Linear issue.

`docs/bootstrap.md` defines the Linux containment policy.
`releases/VERIFICATION.md` defines the consumer signature checks. Generated
archives, SDK copies, Wine prefixes, and evidence stay in ignored artifact
roots; no private release plan is committed.
