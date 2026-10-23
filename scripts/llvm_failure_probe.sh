#!/usr/bin/env bash
# Test-only, monotonic failure control. Never edits IR or produces an object.
set -euo pipefail

fail() { echo "RESOURCE STOP: LLVM failure probe: $*" >&2; exit 126; }
[ "$#" -ge 2 ] || fail 'tool and artifact root are required'
tool=$1
root=$2
shift 2
[ "${SEEN_CAPPED_REGRESSION_SCOPE:-}" = seen-llvm-failure-probes ] ||
    fail 'reserved probe scope is required'
stage=${SEEN_LLVM_FAILURE_PROBE_STAGE:-}
case "$stage" in opt|object) ;; *) fail 'invalid failure stage' ;; esac
raw_input=0
object_input=0
thinlto=0
for argument in "$@"; do
    case "$argument" in
        "$root"/seen_compile_*/seen_module_1.ll) raw_input=1 ;;
        "$root"/seen_compile_*/seen_module_1.opt.ll) object_input=1 ;;
        --thinlto-bc) thinlto=1 ;;
    esac
done
if [ "$stage" = opt ] && [ "$tool" = opt ] && [ "$raw_input" = 1 ]; then
    echo 'test.002a.injected.opt: forced opt failure for module 1' >&2
    exit 86
fi
if [ "$stage" = object ] && [ "$object_input" = 1 ] &&
    { [ "$tool" = llc ] || { [ "$tool" = opt ] && [ "$thinlto" = 1 ]; }; }; then
    echo 'test.002a.injected.object: forced object emission failure for module 1' >&2
    exit 86
fi
# Queries, other modules and non-selected phases execute the real bounded tool.
exit 0
