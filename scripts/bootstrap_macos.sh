#!/bin/bash
# Bootstrap macOS Seen compiler from Linux stage1_frozen via Docker
#
# This script:
# 1. Runs the Linux stage1_frozen inside Docker to compile the Seen compiler
# 2. Extracts the generated LLVM IR (.ll) files
# 3. Compiles them natively on macOS to produce a Mach-O binary
#
# Prerequisites:
# - Docker Desktop for Mac
# - LLVM/Clang installed (brew install llvm)
# - Run from the repository root

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# Detect host architecture
HOST_ARCH=$(uname -m)
echo "=== Seen macOS Bootstrap ==="
echo "Host architecture: $HOST_ARCH"
echo ""

# Step 0: Check prerequisites
if ! command -v docker &>/dev/null; then
    echo -e "${RED}ERROR: Docker not found. Install Docker Desktop for Mac.${NC}"
    exit 1
fi

if ! command -v clang &>/dev/null; then
    echo -e "${RED}ERROR: clang not found. Install Xcode Command Line Tools.${NC}"
    exit 1
fi

if ! command -v llc &>/dev/null; then
    echo -e "${YELLOW}WARNING: llc not found in PATH. Checking Homebrew LLVM...${NC}"
    if [ -f "/opt/homebrew/opt/llvm/bin/llc" ]; then
        export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
        echo "  Using Homebrew LLVM at /opt/homebrew/opt/llvm/bin"
    elif [ -f "/usr/local/opt/llvm/bin/llc" ]; then
        export PATH="/usr/local/opt/llvm/bin:$PATH"
        echo "  Using Homebrew LLVM at /usr/local/opt/llvm/bin"
    else
        echo -e "${RED}ERROR: llc not found. Install LLVM: brew install llvm${NC}"
        exit 1
    fi
fi

# Step 1: Compile in Docker (Linux) and emit LLVM IR
echo ""
echo "Step 1: Compiling Seen compiler in Docker (Linux x86_64)..."

DOCKER_IMAGE="seen-bootstrap"
DOCKER_WORKDIR="/workspace"
IR_OUTPUT_DIR="/tmp/seen_bootstrap_ir"

# Create a minimal Dockerfile if it doesn't exist
DOCKERFILE_TMP=$(mktemp)
cat > "$DOCKERFILE_TMP" << 'DOCKERFILE'
FROM ubuntu:22.04
RUN apt-get update && apt-get install -y clang llvm lld && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
DOCKERFILE

echo "  Building Docker image..."
docker build -t "$DOCKER_IMAGE" -f "$DOCKERFILE_TMP" . > /dev/null 2>&1
rm -f "$DOCKERFILE_TMP"

# Clean up previous IR output
rm -rf "$IR_OUTPUT_DIR"
mkdir -p "$IR_OUTPUT_DIR"

echo "  Running stage1_frozen in Docker to emit LLVM IR..."
docker run --rm \
    -v "$REPO_ROOT:$DOCKER_WORKDIR" \
    -v "$IR_OUTPUT_DIR:/output" \
    "$DOCKER_IMAGE" \
    bash -c "
        cd $DOCKER_WORKDIR && \
        chmod +x bootstrap/stage1_frozen && \
        ./bootstrap/stage1_frozen compile compiler_seen/src/main_compiler.seen /tmp/stage2_docker --fast --emit-llvm && \
        cp /tmp/*.ll /output/ 2>/dev/null || true && \
        cp compiler_seen/src/*.ll /output/ 2>/dev/null || true && \
        ls -la /output/
    "

# Check if we got IR files
IR_FILES=$(ls "$IR_OUTPUT_DIR"/*.ll 2>/dev/null | wc -l)
if [ "$IR_FILES" -eq 0 ]; then
    echo -e "${RED}ERROR: No .ll files generated. Check Docker output above.${NC}"
    exit 1
fi
echo -e "${GREEN}  Generated $IR_FILES LLVM IR file(s)${NC}"

# Step 2: Fix target triple and data layout for macOS
echo ""
echo "Step 2: Fixing LLVM IR for macOS $HOST_ARCH..."

if [ "$HOST_ARCH" = "arm64" ]; then
    TARGET_TRIPLE="arm64-apple-darwin"
    DATA_LAYOUT="e-m:o-i64:64-i128:128-n32:64-S128"
else
    TARGET_TRIPLE="x86_64-apple-macosx"
    DATA_LAYOUT="e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
fi

for ll_file in "$IR_OUTPUT_DIR"/*.ll; do
    # Replace target triple
    sed -i '' "s|target triple = \"x86_64-unknown-linux-gnu\"|target triple = \"$TARGET_TRIPLE\"|g" "$ll_file"
    # Replace data layout (ELF -> Mach-O mangling)
    sed -i '' "s|e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128|$DATA_LAYOUT|g" "$ll_file"
done
echo -e "${GREEN}  Triple set to: $TARGET_TRIPLE${NC}"

# Step 3: Compile LLVM IR to object files
echo ""
echo "Step 3: Compiling LLVM IR to native object files..."

OBJ_DIR="/tmp/seen_bootstrap_obj"
rm -rf "$OBJ_DIR"
mkdir -p "$OBJ_DIR"

for ll_file in "$IR_OUTPUT_DIR"/*.ll; do
    base=$(basename "$ll_file" .ll)
    echo "  Compiling: $base.ll -> $base.o"
    llc -filetype=obj -O2 "$ll_file" -o "$OBJ_DIR/$base.o"
done

OBJ_COUNT=$(ls "$OBJ_DIR"/*.o 2>/dev/null | wc -l)
echo -e "${GREEN}  Compiled $OBJ_COUNT object file(s)${NC}"

# Step 4: Link with runtime
echo ""
echo "Step 4: Linking macOS binary..."

OUTPUT_BINARY="bootstrap/stage1_frozen_macos_${HOST_ARCH}"

# Compile runtime for macOS
echo "  Compiling runtime..."
clang -O2 -c -I seen_runtime seen_runtime/seen_runtime.c -o "$OBJ_DIR/seen_runtime.o"
clang -O2 -c -I seen_runtime seen_runtime/seen_region.c -o "$OBJ_DIR/seen_region.o"
clang -O2 -c -I seen_runtime seen_runtime/seen_pinning.c -o "$OBJ_DIR/seen_pinning.o"
clang -O2 -c -I seen_runtime seen_runtime/seen_numa.c -o "$OBJ_DIR/seen_numa.o"
clang -O2 -c -I seen_runtime seen_runtime/seen_hotreload.c -o "$OBJ_DIR/seen_hotreload.o"
clang -O2 -c -I seen_runtime seen_runtime/seen_identity.c -o "$OBJ_DIR/seen_identity.o"

# Link everything
echo "  Linking..."
clang -O2 -arch "$HOST_ARCH" \
    "$OBJ_DIR"/*.o \
    -o "$OUTPUT_BINARY" \
    -lm -lpthread

chmod +x "$OUTPUT_BINARY"

# Generate SHA256
if command -v shasum &>/dev/null; then
    shasum -a 256 "$OUTPUT_BINARY" > "${OUTPUT_BINARY}.sha256"
elif command -v sha256sum &>/dev/null; then
    sha256sum "$OUTPUT_BINARY" > "${OUTPUT_BINARY}.sha256"
fi

echo -e "${GREEN}  Linked: $OUTPUT_BINARY${NC}"

# Step 5: Verify
echo ""
echo "Step 5: Verifying bootstrap binary..."

if file "$OUTPUT_BINARY" | grep -q "Mach-O"; then
    echo -e "${GREEN}  Binary is valid Mach-O executable${NC}"
else
    echo -e "${RED}ERROR: Binary is not a valid Mach-O executable${NC}"
    file "$OUTPUT_BINARY"
    exit 1
fi

# Quick sanity test
echo "  Running quick test..."
if "$OUTPUT_BINARY" --help > /dev/null 2>&1 || [ $? -eq 1 ]; then
    echo -e "${GREEN}  Bootstrap binary runs successfully${NC}"
else
    echo -e "${YELLOW}  Warning: Bootstrap binary returned unexpected exit code${NC}"
fi

# Clean up
rm -rf "$IR_OUTPUT_DIR" "$OBJ_DIR"

echo ""
echo -e "${GREEN}=== macOS Bootstrap Complete ===${NC}"
echo ""
echo "Bootstrap binary: $OUTPUT_BINARY"
echo "Hash file: ${OUTPUT_BINARY}.sha256"
echo ""
echo "Next steps:"
echo "  1. Test self-hosting:"
echo "     $OUTPUT_BINARY compile compiler_seen/src/main_compiler.seen /tmp/stage2_mac --fast"
echo "  2. Run safe_rebuild.sh to rebuild with the new bootstrap"
echo "  3. Commit the bootstrap binary if verification passes"
