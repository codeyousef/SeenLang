#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)"
F="$ROOT/tests/fixtures/core-004b"; C="$ROOT/scripts/check_release_artifact_manifest.py"
B="$ROOT/scripts/benchmark_release_artifact_manifest.py"; V="$ROOT/scripts/measure_release_artifact_manifest_coverage.py"
WORK="$ROOT/.seen/core-004b-contract"; fail(){ echo "FAIL: CORE-004B contract: $*" >&2; exit 1; }
rm -rf "$WORK"; mkdir -p "$WORK/bin" "$WORK/artifacts"; trap 'rm -rf "$WORK"' EXIT
python3 -m py_compile "$C" "$B" "$V" "$ROOT/tests/runner/test_release_artifact_manifest_unit.py" || fail syntax
python3 "$ROOT/tests/runner/test_release_artifact_manifest_unit.py" >/dev/null || fail unit
python3 "$V" || fail coverage
python3 "$C" --manifest "$F/happy/manifest.json" >"$WORK/canonical.json" || fail CORE-004B_happy
cmp -s "$F/happy/manifest.json" "$WORK/canonical.json" || fail canonical
if python3 "$C" --manifest "$F/invalid/manifest.json" >/dev/null 2>"$WORK/invalid.err"; then fail CORE-004B_invalid; fi
grep -Fq core.004b.limit "$WORK/invalid.err" || fail invalid-code
if python3 "$C" --manifest "$F/limit/manifest.json" >/dev/null 2>"$WORK/limit.err"; then fail CORE-004B_limit; fi
grep -Fq core.004b.limit "$WORK/limit.err" || fail limit-code
status=0; python3 "$C" --manifest "$F/cancel/manifest.json" --test-cancel-after-read >/dev/null 2>"$WORK/cancel.err" || status=$?
[[ "$status" -eq 130 ]] || fail CORE-004B_cancel
python3 "$C" --manifest "$F/happy/manifest.json" --fuzz-seconds "${SEEN_CORE_004B_FUZZ_SECONDS:-1}" --seed 1101 >/dev/null 2>"$WORK/fuzz.err" || fail fuzz
grep -Fq seed=1101 "$WORK/fuzz.err" || fail fuzz-seed
python3 "$B" "$F/happy/manifest.json" "$F/happy/benchmark.json" | grep -Fq 'warmups=5 samples=30 status=pass' || fail benchmark

cat >"$WORK/bin/cosign" <<'COSIGN'
#!/usr/bin/env bash
set -euo pipefail
mode="$1"; shift
bundle=""; artifact=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bundle) bundle="$2"; shift 2 ;;
        --key|--certificate-identity-regexp|--certificate-oidc-issuer) shift 2 ;;
        --yes) shift ;;
        *) artifact="$1"; shift ;;
    esac
done
[[ -n "$bundle" && -n "$artifact" && -f "$artifact" ]] || exit 1
if [[ "$mode" == "sign-blob" ]]; then
    printf 'signed:%s\n' "$(sha256sum "$artifact" | awk '{print $1}')" >"$bundle"
elif [[ "$mode" == "verify-blob" ]]; then
    [[ "$(cat "$bundle")" == "signed:$(sha256sum "$artifact" | awk '{print $1}')" ]] || exit 1
else
    exit 1
fi
COSIGN
chmod +x "$WORK/bin/cosign"
for role in compiler runtime stdlib package-client; do printf '%s\n' "$role" >"$WORK/artifacts/$role"; done
printf '#!/usr/bin/env bash\nexit 0\n' >"$WORK/fake-seen"; chmod +x "$WORK/fake-seen"
if SEEN_PACKAGE_CLIENT_BIN="$WORK/missing-seen-pkg" "$ROOT/scripts/build_release.sh" \
    --version 0.10.1 --compiler "$WORK/fake-seen" --output-dir "$WORK/no-fallback" \
    --skip-verify >/dev/null 2>"$WORK/no-fallback.err"; then fail package-client-fallback; fi
grep -Fq 'release packaging never builds or substitutes a missing client' "$WORK/no-fallback.err" || fail package-client-fallback-message
PATH="$WORK/bin:$PATH" "$ROOT/scripts/sign_release.sh" --key "$WORK/test.key" \
    --version 0.10.1 --source-commit "$(printf '1%.0s' {1..40})" \
    --source-digest "$(printf '2%.0s' {1..64})" --manifest "$WORK/artifacts/manifest.json" \
    --signer-identity test-key --signer-issuer local-test \
    --artifact compiler="$WORK/artifacts/compiler" --artifact runtime="$WORK/artifacts/runtime" \
    --artifact stdlib="$WORK/artifacts/stdlib" --artifact package-client="$WORK/artifacts/package-client" >/dev/null || fail sign-verify
PATH="$WORK/bin:$PATH" "$ROOT/scripts/verify_release.sh" --key "$WORK/test.pub" \
    --manifest "$WORK/artifacts/manifest.json" --artifact-dir "$WORK/artifacts" >/dev/null || fail verify
printf 'tampered\n' >>"$WORK/artifacts/runtime"
if PATH="$WORK/bin:$PATH" "$ROOT/scripts/verify_release.sh" --key "$WORK/test.pub" \
    --manifest "$WORK/artifacts/manifest.json" --artifact-dir "$WORK/artifacts" >/dev/null 2>&1; then fail fail-closed; fi
[[ -f "$ROOT/compiler_seen/src/release/artifact_pins.seen" ]] || fail native-policy
[[ -f "$ROOT/compiler_seen/tests/reproducibility/core_004b_artifact_pins.seen" ]] || fail native-test
[[ -f "$ROOT/compiler_seen/examples/release_artifact_pins.seen" ]] || fail example
[[ -f "$ROOT/tests/fixtures/soak/core_004b_artifact_pins.seen" ]] || fail soak
grep -Fq seen-release-artifact-manifest-v1 "$ROOT/schemas/compatibility-manifest.schema.json" || fail compatibility
[[ -z "$(find "$ROOT/.seen" -maxdepth 1 -name '.core-004b-*' -print -quit)" ]] || fail CORE-004B_cleanup
[[ -z "$(jobs -pr)" ]] || fail CORE-004B_cleanup
echo "PASS: signed release artifact pins"
