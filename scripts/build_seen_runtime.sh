#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
TARGETS=("$@")

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  HOST_TRIPLE="$(rustc -Vv | awk '/host:/ { print $2; exit }')"
  TARGETS+=("$HOST_TRIPLE" "wasm32-unknown-unknown" "aarch64-linux-android")
fi

echo "[seen_runtime] Building static libraries for: ${TARGETS[*]}"

for triple in "${TARGETS[@]}"; do
  echo "[seen_runtime] -> cargo build -p seen_runtime --release --target ${triple}"
  (cd "$ROOT" && cargo build -p seen_runtime --release --target "${triple}")

  artifact="$ROOT/target/${triple}/release/libseen_runtime.a"
  if [[ ! -f "$artifact" ]]; then
    echo "[seen_runtime] !! expected artifact ${artifact} was not produced" >&2
    exit 1
  fi

  dest_dir="$ROOT/target/seen-runtime/${triple}"
  mkdir -p "$dest_dir"
  cp "$artifact" "$dest_dir/"
  echo "[seen_runtime] Copied artifact to ${dest_dir}/"
done

echo "[seen_runtime] Done."
