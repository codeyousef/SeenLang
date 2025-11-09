#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
DEFAULT_MATRIX="$ROOT_DIR/releases/bootstrap_matrix.toml"
DEFAULT_OUTPUT="$ROOT_DIR/artifacts/bootstrap"
CLI_BIN="${CARGO_TARGET_BIN:-${CARGO_TARGET_DIR:-$HOME/.cargo/target-shared}/release/seen_cli}"
SIGN_BIN="${CARGO_TARGET_DIR:-$ROOT_DIR/target}/release/sign_bootstrap_artifact"

MATRIX_FILE="$DEFAULT_MATRIX"
OUTPUT_DIR="$DEFAULT_OUTPUT"
SIGNING_KEY=""
VERIFY_KEY=""
ABI_MANIFEST="$ROOT_DIR/seen_std/Seen.toml"
ABI_LOCK="$ROOT_DIR/seen_std/Seen.lock"
ABI_SNAPSHOT=""
SKIP_ABI_VERIFY=0

usage() {
  cat <<'EOF'
release_bootstrap_matrix.sh - Build and attest Stage1/2/3 outputs for every matrix entry.

Usage: scripts/release_bootstrap_matrix.sh [options]

Options:
  --matrix <path>       Path to bootstrap_matrix.toml (default: releases/bootstrap_matrix.toml)
  --output-dir <path>   Directory to store per-entry artifacts (default: artifacts/bootstrap)
  --cli-bin <path>      Path to seen_cli binary (default: ${CARGO_TARGET_BIN:-${CARGO_TARGET_DIR:-$HOME/.cargo/target-shared}/release/seen_cli})
  --signer-bin <path>   Path to sign_bootstrap_artifact binary (default: target/release/sign_bootstrap_artifact)
  --signing-key <path>  Ed25519 signing key to sign each manifest (raw/hex seed)
  --public-key <path>   Ed25519 public key used to verify signatures (optional)
  --abi-manifest <path> Seen stdlib manifest for ABI guard (default: seen_std/Seen.toml)
  --abi-lock <path>     Seen stdlib lock file (default: seen_std/Seen.lock)
  --abi-snapshot <path> Output path for ABI snapshot JSON (optional)
  --skip-abi-verify     Skip running abi_guard verification
  -h, --help            Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --matrix)
      MATRIX_FILE="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_DIR="$2"
      shift 2
      ;;
    --cli-bin)
      CLI_BIN="$2"
      shift 2
      ;;
    --signer-bin)
      SIGN_BIN="$2"
      shift 2
      ;;
    --signing-key)
      SIGNING_KEY="$2"
      shift 2
      ;;
    --public-key)
      VERIFY_KEY="$2"
      shift 2
      ;;
    --abi-manifest)
      ABI_MANIFEST="$2"
      shift 2
      ;;
    --abi-lock)
      ABI_LOCK="$2"
      shift 2
      ;;
    --abi-snapshot)
      ABI_SNAPSHOT="$2"
      shift 2
      ;;
    --skip-abi-verify)
      SKIP_ABI_VERIFY=1
      shift 1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ ! -f "$MATRIX_FILE" ]]; then
  echo "Matrix file not found: $MATRIX_FILE" >&2
  exit 1
fi

if [[ ! -x "$CLI_BIN" ]]; then
  echo "seen_cli not found at $CLI_BIN" >&2
  echo "Build it with: cargo build -p seen_cli --release --features llvm" >&2
  exit 1
fi

if [[ ! -x "$SIGN_BIN" ]]; then
  echo "Building sign_bootstrap_artifact (missing at $SIGN_BIN) ..."
  cargo build -p sign_bootstrap_artifact --release
fi

mkdir -p "$OUTPUT_DIR"

CLI_VERSION="$("$CLI_BIN" --version | head -n 1 | tr -d '\r')"
TMP_BINARIES=()

cleanup() {
  for path in "${TMP_BINARIES[@]}"; do
    if [[ -f "$path" ]]; then
      rm -f "$path"
    fi
  done
}
trap cleanup EXIT

log() {
  printf '[bootstrap-matrix] %s\n' "$*"
}

if [[ $SKIP_ABI_VERIFY -eq 0 ]]; then
  if [[ -f "$ABI_MANIFEST" && -f "$ABI_LOCK" ]]; then
    log "[abi] Verifying stdlib using $ABI_MANIFEST + $ABI_LOCK"
    cargo run --quiet -p abi_guard -- verify \
      --manifest "$ABI_MANIFEST" \
      --lock "$ABI_LOCK"
  else
    log "[abi] Manifest or lock missing; skipping verification"
  fi
fi

if [[ -n "$ABI_SNAPSHOT" ]]; then
  log "[abi] Writing snapshot to $ABI_SNAPSHOT"
  mkdir -p "$(dirname "$ABI_SNAPSHOT")"
  cargo run --quiet -p abi_guard -- snapshot \
    --manifest "$ABI_MANIFEST" \
    --output "$ABI_SNAPSHOT" \
    --lock "$ABI_LOCK"
fi

read_matrix_rows() {
  python3 <<'PY' "$MATRIX_FILE"
import sys, json
from pathlib import Path
try:
    import tomllib
except ModuleNotFoundError:
    import tomli as tomllib

path = Path(sys.argv[1])
with path.open("rb") as fh:
    data = tomllib.load(fh)

entries = data.get("entries") or []
for entry in entries:
    host = entry.get("host")
    if not host:
        continue
    backend = entry.get("backend", "llvm")
    profile = entry.get("profile", "release")
    target = entry.get("target", host)
    name = entry.get("name") or f"{host}-{backend}-{profile}"
    print("\t".join([name, host, target, backend, profile]))
PY
}

mapfile -t MATRIX_ROWS < <(MATRIX_FILE="$MATRIX_FILE" read_matrix_rows)
if [[ ${#MATRIX_ROWS[@]} -eq 0 ]]; then
  echo "No entries defined in $MATRIX_FILE" >&2
  exit 1
fi

MANIFESTS=()

for row in "${MATRIX_ROWS[@]}"; do
  IFS=$'\t' read -r entry_name host target backend profile <<< "$row"
  log "Processing entry ${entry_name} (host=${host}, target=${target}, backend=${backend}, profile=${profile})"

  stage1_tmp="$ROOT_DIR/stage1_${entry_name}"
  stage2_tmp="$ROOT_DIR/stage2_${entry_name}"
  stage3_tmp="$ROOT_DIR/stage3_${entry_name}"
  rm -f "$stage1_tmp" "$stage2_tmp" "$stage3_tmp"
  TMP_BINARIES=("$stage1_tmp" "$stage2_tmp" "$stage3_tmp")

  profile_args=()
  if [[ -n "$profile" && "$profile" != "release" ]]; then
    profile_args=(--profile "$profile")
  fi

  log "  [1/4] Stage-1 via seen_cli"
  cmd=("$CLI_BIN" build "compiler_seen/src/main.seen" --backend "$backend" --target "$target")
  cmd+=("${profile_args[@]}")
  cmd+=(--output "$stage1_tmp")
  SEEN_ENABLE_MANIFEST_MODULES=1 "${cmd[@]}"

  log "  [2/4] Stage-2 via Stage-1"
  "$stage1_tmp" build "compiler_seen/src/main.seen" "$stage2_tmp"

  log "  [3/4] Stage-3 via Stage-2"
  "$stage2_tmp" build "compiler_seen/src/main.seen" "$stage3_tmp"

  entry_dir="$OUTPUT_DIR/$entry_name"
  mkdir -p "$entry_dir"
  mv "$stage1_tmp" "$entry_dir/stage1_seen"
  mv "$stage2_tmp" "$entry_dir/stage2_seen"
  mv "$stage3_tmp" "$entry_dir/stage3_seen"

  TMP_BINARIES=()

  manifest_path="$entry_dir/manifest.json"
  log "  [4/4] Generating manifest $manifest_path"
  sign_cmd=(
    "$SIGN_BIN" sign
    --stage1 "$entry_dir/stage1_seen"
    --stage2 "$entry_dir/stage2_seen"
    --stage3 "$entry_dir/stage3_seen"
    --backend "$backend"
    --profile "$profile"
    --host "$host"
    --target "$target"
    --entry "$entry_name"
    --output "$manifest_path"
    --cli-path "$CLI_BIN"
    --cli-version "$CLI_VERSION"
  )
  if [[ -n "$SIGNING_KEY" ]]; then
    sign_cmd+=(--signing-key "$SIGNING_KEY")
  fi
  "${sign_cmd[@]}"

  if [[ -n "$VERIFY_KEY" ]]; then
    sig_path="$manifest_path.sig"
    if [[ ! -f "$sig_path" ]]; then
      echo "Signature not found at $sig_path for verification" >&2
      exit 1
    fi
    "$SIGN_BIN" verify \
      --manifest "$manifest_path" \
      --signature "$sig_path" \
      --public-key "$VERIFY_KEY"
  fi

  MANIFESTS+=("$manifest_path")
done

if [[ ${#MANIFESTS[@]} -gt 0 ]]; then
  INDEX_PATH="$OUTPUT_DIR/index.json"
  python3 <<'PY' "$INDEX_PATH" "${MANIFESTS[@]}"
import sys, json, datetime
out_path = sys.argv[1]
manifest_paths = sys.argv[2:]
entries = []
for manifest_path in manifest_paths:
    with open(manifest_path, "r", encoding="utf-8") as fh:
        data = json.load(fh)
    entries.append({
        "matrix_entry": data.get("matrix_entry"),
        "manifest": manifest_path,
        "host_triple": data.get("host_triple"),
        "backend": data.get("backend"),
        "profile": data.get("profile"),
        "stage2_sha256": data.get("stage2", {}).get("sha256"),
        "stage3_sha256": data.get("stage3", {}).get("sha256"),
        "stage2_equals_stage3": data.get("stage2_equals_stage3"),
    })

payload = {
    "generated_at": datetime.datetime.utcnow().isoformat(timespec="milliseconds") + "Z",
    "entries": entries,
}
with open(out_path, "w", encoding="utf-8") as fh:
    json.dump(payload, fh, indent=2)
PY
  log "Wrote matrix index to $INDEX_PATH"
fi

log "Bootstrap matrix completed for ${#MANIFESTS[@]} entr$( [[ ${#MANIFESTS[@]} -eq 1 ]] && echo "y" || echo "ies")."
