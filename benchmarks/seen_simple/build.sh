#!/bin/bash
# Build script for Seen arithmetic benchmarks
# Compiles Seen source to optimized executable

set -e  # Exit on any error

echo "🚀 Building Seen Arithmetic Benchmarks"
echo "   Using Seen compiler with complete optimization pipeline"

# Paths
SEEN_COMPILER="../../compiler_seen/target/native/debug/seen_compiler"
SOURCE_FILE="src/main.seen"
OUTPUT_EXECUTABLE="arithmetic_benchmark"

# Check if Seen compiler exists
if [ ! -f "$SEEN_COMPILER" ]; then
    echo "❌ Error: Seen compiler not found at $SEEN_COMPILER"
    echo "   Please build the Seen compiler first:"
    echo "   cd ../../compiler_seen && ../../target-wsl/debug/seen build"
    exit 1
fi

# Check if source file exists
if [ ! -f "$SOURCE_FILE" ]; then
    echo "❌ Error: Source file not found: $SOURCE_FILE"
    exit 1
fi

echo "   📁 Source: $SOURCE_FILE"
echo "   📦 Output: $OUTPUT_EXECUTABLE"
echo ""

# Compile with Seen compiler
echo "🔥 Phase 1: Seen Compilation"
echo "   Using E-graph optimization + LLVM backend"

# Run the Seen compiler
if $SEEN_COMPILER compile "$SOURCE_FILE" "$OUTPUT_EXECUTABLE"; then
    echo "   ✅ Compilation successful!"
else
    echo "   ❌ Compilation failed!"
    exit 1
fi

# Verify executable was created
if [ -f "$OUTPUT_EXECUTABLE" ]; then
    echo "   📦 Executable size: $(stat -c%s "$OUTPUT_EXECUTABLE") bytes"
    echo "   🔧 Making executable: chmod +x $OUTPUT_EXECUTABLE"
    chmod +x "$OUTPUT_EXECUTABLE"
else
    echo "   ❌ Error: Executable not created"
    exit 1
fi

echo ""
echo "✅ Build completed successfully!"
echo "   Run with: ./$OUTPUT_EXECUTABLE"
echo "   Or run benchmarks: ./run_benchmarks.sh"