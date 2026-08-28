#!/usr/bin/env bash
set -euo pipefail

# CORE-004C removed the legacy non-LLVM `determinism` command. Keep this
# historical entrypoint fail-closed so automation cannot silently exercise a
# wrapper-only compiler surface and report it as shipped behavior.
echo "core.004c.invalid: scripts/nightly_backends.sh is retired; the shipped compiler supports the LLVM backend only" >&2
echo "use scripts/validate_determinism.sh for the deterministic CLI contract" >&2
exit 1
