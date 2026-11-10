#!/usr/bin/env bash
# Explicit Gate-B-only provisioning. Sourcing installs no trap and does no IO.
prepare_emitter_fixture_pool() {
    local pool=$1
    [ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" = seen-p0-accel-gate-b ] || return 126
    [ "${FIXTURE_TARGET_CPU:-x86-64}" = x86-64 ] || return 126
    [ ! -e "$pool" ] && [ ! -L "$pool" ] || return 126
    mkdir -m 700 -- "$pool" || return
    python3 "$ROOT_DIR/scripts/emitter_fixture_pool.py" fingerprint \
        --root "$ROOT_DIR" --compiler "$COMPILER" --pool "$pool" || return
    (
        # The generic cleanup trap belongs only to this producer subshell, not B.
        TMP_DIR=$pool
        FIXTURE_ARTIFACT_PREFIX=emitter-fixtures
        run_compiler() { bash "$ATTESTED_SEEN" "$COMPILER" "$@"; }
        source "$ROOT_DIR/scripts/compiled_fixture_support.sh" || exit "$?"
        local program source profile status
        for program in text integrity; do
            if [ "$program" = text ]; then
                source="$ROOT_DIR/tests/gpu_artifacts/text_layout_contract.seen"
            else
                source="$ROOT_DIR/tests/gpu_artifacts/integrity_contract.seen"
            fi
            for profile in fast release undefined address; do
                status=0
                compile_fixture "$profile" "$program" "$source" || status=$?
                # Preserve the actual failing compiler status, logs and IR.
                [ "$status" -eq 0 ] || exit "$status"
            done
        done
        python3 "$ROOT_DIR/scripts/emitter_fixture_pool.py" seal \
            --root "$ROOT_DIR" --compiler "$COMPILER" --pool "$pool" || exit "$?"
    ) || return
    SEEN_EMITTER_FIXTURE_POOL=$pool
    export SEEN_EMITTER_FIXTURE_POOL
}

compile_emitter_fixture() {
    local profile=$1 name=$2 source=$3
    if [ "${SEEN_EMITTER_FIXTURE_POOL+x}" != x ]; then
        compile_fixture "$profile" "$name" "$source"
        return
    fi
    # Presence means an explicit reuse request, including an invalid/empty pool.
    # Never silently turn an invalid published pool into a new compilation.
    if [ "${FIXTURE_TARGET_CPU:-x86-64}" != x86-64 ] ||
        [ "${#FIXTURE_COMPILE_FLAGS[@]}" -ne 0 ]; then
        local suffix
        for suffix in .compile.status '' .compile.log .log .compile.origin.json; do
            rm -f -- "$TMP_DIR/$name-$profile$suffix" || return 126
        done
        echo 'emitter fixture pool requires the exact default native flags' >&2
        return 126
    fi
    python3 "$ROOT_DIR/scripts/emitter_fixture_pool.py" clone \
        --root "$ROOT_DIR" --compiler "$COMPILER" --pool "$SEEN_EMITTER_FIXTURE_POOL" \
        --source "$source" --profile "$profile" --destination "$TMP_DIR/$name-$profile"
}

compile_emitter_case() {
    local profile=$1 name=$2 source=$3
    shift 3
    compile_emitter_fixture "$profile" "$name" "$source" || return
    run_fixture "$profile" "$name" "$@"
}
