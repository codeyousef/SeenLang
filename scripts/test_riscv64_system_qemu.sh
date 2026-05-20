#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")"/.. && pwd)"
COMPILER_BIN="$ROOT_DIR/compiler_seen/target/seen"
SOURCE_FILE="$ROOT_DIR/examples/hello_world/hello_english.seen"
OUTPUT_ROOT="$ROOT_DIR/artifacts/riscv64-qemu-system"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
REQUIRE=0
QEMU_SYSTEM="${SEEN_QEMU_SYSTEM_RISCV64:-}"
KERNEL="${SEEN_RISCV64_QEMU_KERNEL:-}"
ROOTFS="${SEEN_RISCV64_QEMU_ROOTFS:-}"
ROOTFS_FORMAT="${SEEN_RISCV64_QEMU_DRIVE_FORMAT:-qcow2}"
SSH_USER="${SEEN_RISCV64_QEMU_USER:-root}"
SSH_PORT="${SEEN_RISCV64_QEMU_SSH_PORT:-10022}"
SSH_IDENTITY="${SEEN_RISCV64_QEMU_IDENTITY:-}"
MEMORY="${SEEN_RISCV64_QEMU_MEMORY:-2048}"
SMP="${SEEN_RISCV64_QEMU_SMP:-2}"

usage() {
  cat <<'USAGE'
test_riscv64_system_qemu.sh - boot a RISC-V Linux guest and run a Seen smoke binary.

Usage: scripts/test_riscv64_system_qemu.sh [options]

Options:
  --compiler <path>       Path to Seen compiler binary
  --source <file>         Seen source to compile and run in the guest
  --output-dir <dir>      Output directory for logs and artifacts
  --qemu <path>           qemu-system-riscv64 binary
  --kernel <path>         RISC-V Linux kernel image
  --rootfs <path>         RISC-V root filesystem image with sshd
  --rootfs-format <fmt>   QEMU drive format, default qcow2
  --ssh-user <user>       Guest SSH user, default root
  --ssh-port <port>       Host forwarded SSH port, default 10022
  --ssh-identity <path>   SSH private key for guest login
  --memory <mb>           Guest memory in MiB, default 2048
  --smp <n>               Guest vCPU count, default 2
  --require               Fail instead of skipping when inputs are missing
  -h, --help              Show this help message

Required environment alternatives:
  SEEN_RISCV64_QEMU_KERNEL
  SEEN_RISCV64_QEMU_ROOTFS
  SEEN_RISCV64_QEMU_USER
  SEEN_RISCV64_QEMU_IDENTITY

The rootfs must boot Linux, run sshd, and allow the configured user to execute
uploaded test binaries. This is intentionally optional; user-mode QEMU remains
the fast required RISC-V gate.
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --compiler)
      COMPILER_BIN="$2"
      shift 2
      ;;
    --source)
      SOURCE_FILE="$2"
      shift 2
      ;;
    --output-dir)
      OUTPUT_ROOT="$2"
      shift 2
      ;;
    --qemu)
      QEMU_SYSTEM="$2"
      shift 2
      ;;
    --kernel)
      KERNEL="$2"
      shift 2
      ;;
    --rootfs)
      ROOTFS="$2"
      shift 2
      ;;
    --rootfs-format)
      ROOTFS_FORMAT="$2"
      shift 2
      ;;
    --ssh-user)
      SSH_USER="$2"
      shift 2
      ;;
    --ssh-port)
      SSH_PORT="$2"
      shift 2
      ;;
    --ssh-identity)
      SSH_IDENTITY="$2"
      shift 2
      ;;
    --memory)
      MEMORY="$2"
      shift 2
      ;;
    --smp)
      SMP="$2"
      shift 2
      ;;
    --require)
      REQUIRE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

skip_or_fail() {
  local message="$1"
  echo "[riscv64-system-qemu] unavailable: $message" >&2
  echo "[riscv64-system-qemu] Provide a RISC-V kernel/rootfs with sshd, or use scripts/test_riscv64_qemu.sh for the required user-mode gate." >&2
  if [[ "$REQUIRE" -eq 1 ]]; then
    exit 1
  fi
  exit 0
}

if [[ -z "$QEMU_SYSTEM" ]]; then
  QEMU_SYSTEM="$(command -v qemu-system-riscv64 || true)"
fi
[[ -n "$QEMU_SYSTEM" && -x "$QEMU_SYSTEM" ]] || skip_or_fail "qemu-system-riscv64 not found"
[[ -x "$COMPILER_BIN" ]] || skip_or_fail "compiler binary not found or not executable: $COMPILER_BIN"
[[ -f "$SOURCE_FILE" ]] || skip_or_fail "source file not found: $SOURCE_FILE"
[[ -f "$KERNEL" ]] || skip_or_fail "kernel image not found; set SEEN_RISCV64_QEMU_KERNEL or pass --kernel"
[[ -f "$ROOTFS" ]] || skip_or_fail "rootfs image not found; set SEEN_RISCV64_QEMU_ROOTFS or pass --rootfs"
command -v ssh >/dev/null 2>&1 || skip_or_fail "ssh not found"
command -v scp >/dev/null 2>&1 || skip_or_fail "scp not found"

RUN_DIR="$OUTPUT_ROOT/$TIMESTAMP"
mkdir -p "$RUN_DIR"
ARTIFACT="$RUN_DIR/seen_riscv64_system_smoke"
COMPILE_LOG="$RUN_DIR/compile.log"
QEMU_LOG="$RUN_DIR/qemu.log"
PID_FILE="$RUN_DIR/qemu.pid"
SSH_OPTS=(
  -p "$SSH_PORT"
  -o StrictHostKeyChecking=no
  -o UserKnownHostsFile=/dev/null
  -o ConnectTimeout=5
)
if [[ -n "$SSH_IDENTITY" ]]; then
  [[ -f "$SSH_IDENTITY" ]] || skip_or_fail "SSH identity not found: $SSH_IDENTITY"
  SSH_OPTS+=(-i "$SSH_IDENTITY")
fi

cleanup() {
  if [[ -f "$PID_FILE" ]]; then
    local pid
    pid="$(cat "$PID_FILE" 2>/dev/null || true)"
    if [[ -n "$pid" ]]; then
      kill "$pid" >/dev/null 2>&1 || true
      wait "$pid" >/dev/null 2>&1 || true
    fi
  fi
}
trap cleanup EXIT

echo "[riscv64-system-qemu] compiling smoke binary"
"$COMPILER_BIN" compile "$SOURCE_FILE" "$ARTIFACT" --backend llvm --target=linux-riscv64 >"$COMPILE_LOG" 2>&1
[[ -x "$ARTIFACT" ]] || {
  echo "[riscv64-system-qemu] compile failed; see $COMPILE_LOG" >&2
  exit 1
}

echo "[riscv64-system-qemu] booting guest on SSH port $SSH_PORT"
"$QEMU_SYSTEM" \
  -machine virt \
  -cpu rv64 \
  -m "$MEMORY" \
  -smp "$SMP" \
  -nographic \
  -bios default \
  -kernel "$KERNEL" \
  -append "root=/dev/vda rw console=ttyS0" \
  -drive "file=$ROOTFS,format=$ROOTFS_FORMAT,if=virtio" \
  -netdev "user,id=net0,hostfwd=tcp:127.0.0.1:$SSH_PORT-:22" \
  -device virtio-net-device,netdev=net0 \
  >"$QEMU_LOG" 2>&1 &
echo "$!" > "$PID_FILE"

echo "[riscv64-system-qemu] waiting for guest SSH"
ready=0
for _ in $(seq 1 90); do
  if ssh "${SSH_OPTS[@]}" "$SSH_USER@127.0.0.1" "true" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 2
done
[[ "$ready" -eq 1 ]] || {
  echo "[riscv64-system-qemu] guest SSH did not become ready; see $QEMU_LOG" >&2
  exit 1
}

REMOTE_BIN="/tmp/seen_riscv64_system_smoke"
echo "[riscv64-system-qemu] uploading and running smoke binary"
scp "${SSH_OPTS[@]}" "$ARTIFACT" "$SSH_USER@127.0.0.1:$REMOTE_BIN" >/dev/null
ssh "${SSH_OPTS[@]}" "$SSH_USER@127.0.0.1" "chmod +x '$REMOTE_BIN' && '$REMOTE_BIN'" | tee "$RUN_DIR/run.log"

echo "[riscv64-system-qemu] PASS"
echo "[riscv64-system-qemu] logs: $RUN_DIR"
