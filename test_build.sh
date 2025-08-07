#!/bin/bash
cd /home/yousef/Development/SeenLang
export CARGO_TARGET_DIR=target-wsl
echo "Testing build system..."
cargo build -p seen_cli --bin seen 2>&1 | tail -5
echo "Build status: $?"