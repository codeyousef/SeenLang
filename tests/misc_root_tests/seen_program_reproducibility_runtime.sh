#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -P -- "${BASH_SOURCE[0]%/*}/../.." && pwd -P)
COMPILER=${COMPILER:-${SEEN_BIN:-$ROOT/compiler_seen/target/seen}}
PRODUCER="$ROOT/scripts/produce_program_reproducibility_evidence.py"
CAPPED="$ROOT/scripts/run_capped_regression.sh"
ARTIFACT_CHECKER="$ROOT/scripts/check_program_artifacts.py"
REPRO_CHECKER="$ROOT/scripts/check_program_reproducibility.py"
CERTIFIER="$ROOT/scripts/certify_two_builder_programs.py"

fail() {
    echo "FAIL: CORE-004FG runtime evidence: $*" >&2
    exit 1
}

test -x "$COMPILER" || fail "compiler is unavailable: $COMPILER"
test -f "$PRODUCER" || fail "producer is unavailable"
test -x "$CAPPED" || fail "capped entry is unavailable"
python3 -m py_compile "$PRODUCER" "$ARTIFACT_CHECKER" \
    "$REPRO_CHECKER" "$CERTIFIER" || fail syntax
python3 "$PRODUCER" self-test || fail producer-self-test

mkdir -p "$ROOT/.seen"
SESSION=$(mktemp -d "$ROOT/.seen/core-004fg-runtime.XXXXXX")
cleanup() {
    local status=$?
    if [ "$status" -eq 0 ]; then
        case "$SESSION" in
            "$ROOT"/.seen/core-004fg-runtime.*)
                chmod -R u+w "$SESSION/installed-b" 2>/dev/null || true
                rm -rf -- "$SESSION"
                ;;
            *) return 1 ;;
        esac
    else
        echo "Preserved failed CORE-004FG runtime evidence: $SESSION" >&2
    fi
    return "$status"
}
trap cleanup EXIT INT TERM

python3 "$PRODUCER" prepare --repo "$ROOT" --session "$SESSION" \
    --compiler "$COMPILER" || fail prepare

for builder in builder-a builder-b; do
    builder_compiler=$COMPILER
    if [ "$builder" = builder-b ]; then
        builder_compiler="$SESSION/installed-b/bin/seen"
    fi
    bash "$CAPPED" "seen-core-004fg-${builder}" --compiler "$builder_compiler" -- \
        python3 "$PRODUCER" worker --repo "$ROOT" --session "$SESSION" \
            --compiler "$builder_compiler" --builder "$builder" || fail "$builder"
done

mapfile -t trust_a < <(python3 "$PRODUCER" print-trust \
    --session "$SESSION" --builder builder-a)
mapfile -t trust_b < <(python3 "$PRODUCER" print-trust \
    --session "$SESSION" --builder builder-b)
test "${#trust_a[@]}" -eq 3 && test "${#trust_b[@]}" -eq 3 || \
    fail trust-pin-readback
test "${trust_a[0]}" != "${trust_b[0]}" || fail distinct-builder-keys
test ! -e "$SESSION/builder-a.private.pem" && \
    test ! -e "$SESSION/builder-b.private.pem" || fail private-keys-survived-workers

python3 "$PRODUCER" finalize --repo "$ROOT" --session "$SESSION" || \
    fail finalize

artifact_args=()
for result_id in \
    root-a-cold root-a-warm root-a-disabled \
    root-b-cold root-b-warm root-b-disabled; do
    artifact_args+=(--result-root \
        "$result_id=$SESSION/core-004f/$result_id")
done
python3 "$ARTIFACT_CHECKER" --evidence "$SESSION/program-artifacts.json" \
    "${artifact_args[@]}" --output "$SESSION/program-artifacts.certified.json" || \
    fail core-004f-certification
cmp -s "$SESSION/program-artifacts.json" \
    "$SESSION/program-artifacts.certified.json" || fail core-004f-canonical

python3 "$REPRO_CHECKER" --evidence "$SESSION/program-reproducibility.json" \
    >"$SESSION/program-reproducibility.checked.json" || fail core-004g-check
cmp -s "$SESSION/program-reproducibility.json" \
    "$SESSION/program-reproducibility.checked.json" || fail core-004g-canonical

certifier_args=(
    --candidate "$SESSION/program-reproducibility.json"
    --builder-a-root "$SESSION/builder-a"
    --builder-b-root "$SESSION/builder-b"
    --cache-a-root "$SESSION/source-a"
    --cache-b-root "$SESSION/source-b"
    --signature-a "$SESSION/builder-a.sig"
    --signature-b "$SESSION/builder-b.sig"
    --public-key-a "$SESSION/${trust_a[2]}"
    --public-key-b "$SESSION/${trust_b[2]}"
    --expected-identity-a "${trust_a[0]}"
    --expected-identity-b "${trust_b[0]}"
    --expected-issuer-a "${trust_a[1]}"
    --expected-issuer-b "${trust_b[1]}"
)
python3 "$CERTIFIER" "${certifier_args[@]}" \
    --output "$SESSION/program-reproducibility.certified.json" || \
    fail core-004g-certification
cmp -s "$SESSION/program-reproducibility.json" \
    "$SESSION/program-reproducibility.certified.json" || fail core-004g-certified-canonical

# CORE-004F must reject both changed evidence identity and changed producer bytes.
python3 "$PRODUCER" mutate-evidence --kind core004f-input \
    --source "$SESSION/program-artifacts.json" \
    --output "$SESSION/program-artifacts.input-change.json"
if python3 "$ARTIFACT_CHECKER" \
    --evidence "$SESSION/program-artifacts.input-change.json" \
    >"$SESSION/core-004f-input.out" 2>"$SESSION/core-004f-input.err"; then
    fail core-004f-input-mutation-accepted
fi
grep -Fq core.004f.input_change "$SESSION/core-004f-input.err" || \
    fail core-004f-input-mutation-code

mutated_program="$SESSION/core-004f/root-b-disabled/program"
cp -- "$mutated_program" "$SESSION/program.backup"
python3 "$PRODUCER" mutate-file --path "$mutated_program"
if python3 "$ARTIFACT_CHECKER" --evidence "$SESSION/program-artifacts.json" \
    "${artifact_args[@]}" >"$SESSION/core-004f-bytes.out" \
    2>"$SESSION/core-004f-bytes.err"; then
    fail core-004f-byte-mutation-accepted
fi
grep -Fq core.004f.mismatch "$SESSION/core-004f-bytes.err" || \
    fail core-004f-byte-mutation-code
cp -- "$SESSION/program.backup" "$mutated_program"

# CORE-004G must reject mutated producer bytes and an invalid sidecar.
first_artifact="$SESSION/builder-b/single-file-cold.bin"
cp -- "$first_artifact" "$SESSION/artifact.backup"
python3 "$PRODUCER" mutate-file --path "$first_artifact"
if python3 "$CERTIFIER" "${certifier_args[@]}" \
    --output "$SESSION/mutated-artifact.certified.json" \
    >"$SESSION/core-004g-bytes.out" 2>"$SESSION/core-004g-bytes.err"; then
    fail core-004g-byte-mutation-accepted
fi
grep -Fq core.004g.raw_mismatch "$SESSION/core-004g-bytes.err" || \
    fail core-004g-byte-mutation-code
test ! -e "$SESSION/mutated-artifact.certified.json" || \
    fail core-004g-byte-mutation-published
cp -- "$SESSION/artifact.backup" "$first_artifact"

cp -- "$SESSION/builder-b.sig" "$SESSION/signature.backup"
python3 "$PRODUCER" mutate-file --path "$SESSION/builder-b.sig"
if python3 "$CERTIFIER" "${certifier_args[@]}" \
    --output "$SESSION/mutated-signature.certified.json" \
    >"$SESSION/core-004g-signature.out" \
    2>"$SESSION/core-004g-signature.err"; then
    fail core-004g-signature-mutation-accepted
fi
grep -Fq core.004g.unsigned "$SESSION/core-004g-signature.err" || \
    fail core-004g-signature-mutation-code

python3 "$PRODUCER" mutate-evidence --kind core004g-signature-pin \
    --source "$SESSION/program-reproducibility.json" \
    --signature "$SESSION/builder-b.sig" \
    --output "$SESSION/program-reproducibility.forged-signature.json"
forged_args=("${certifier_args[@]}")
forged_args[1]="$SESSION/program-reproducibility.forged-signature.json"
if python3 "$CERTIFIER" "${forged_args[@]}" \
    --output "$SESSION/forged-signature.certified.json" \
    >"$SESSION/core-004g-forgery.out" 2>"$SESSION/core-004g-forgery.err"; then
    fail core-004g-cryptographic-forgery-accepted
fi
grep -Fq core.004g.unsigned "$SESSION/core-004g-forgery.err" || \
    fail core-004g-cryptographic-forgery-code
test ! -e "$SESSION/forged-signature.certified.json" || \
    fail core-004g-cryptographic-forgery-published
cp -- "$SESSION/signature.backup" "$SESSION/builder-b.sig"

root_mutation_log=$(python3 "$PRODUCER" mutate-evidence \
    --kind core004g-root --source "$SESSION/program-reproducibility.json" \
    --output "$SESSION/program-reproducibility.root-mutation.json")
mutated_signature=${root_mutation_log#signature=}
root_args=("${certifier_args[@]}")
root_args[1]="$SESSION/program-reproducibility.root-mutation.json"
root_args[11]="$mutated_signature"
if python3 "$CERTIFIER" "${root_args[@]}" \
    --output "$SESSION/mutated-root.certified.json" \
    >"$SESSION/core-004g-root.out" 2>"$SESSION/core-004g-root.err"; then
    fail core-004g-root-mutation-accepted
fi
grep -Fq core.004g.root_identity "$SESSION/core-004g-root.err" || \
    fail core-004g-root-mutation-code

cancel_status=0
python3 "$CERTIFIER" "${certifier_args[@]}" \
    --output "$SESSION/cancelled.certified.json" --test-cancel-before-commit \
    >"$SESSION/core-004g-cancel.out" 2>"$SESSION/core-004g-cancel.err" || \
    cancel_status=$?
test "$cancel_status" -eq 130 || fail core-004g-cancel-status
grep -Fq core.004g.cancelled "$SESSION/core-004g-cancel.err" || \
    fail core-004g-cancel-code
test ! -e "$SESSION/cancelled.certified.json" || fail core-004g-cancel-published

test -z "$(find "$SESSION" \( -name '.program-certification-*' -o \
    -name '.program-artifacts-*' -o -name '.program-ed25519-*' \) \
    -print -quit)" || fail atomic-scratch-cleanup
echo "PASS: CORE-004F/004G measured two-root program reproducibility evidence"
