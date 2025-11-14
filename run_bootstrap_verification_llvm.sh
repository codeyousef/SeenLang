#!/usr/bin/env bash
set -euo pipefail
LOG_FILE="bootstrap_results.log"
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; BLUE='\033[0;34m'; PURPLE='\033[0;35m'; NC='\033[0m'
log(){ echo -e "$1" | tee -a "$LOG_FILE"; }
header(){ echo -e "\n==============================================\n$1\n==============================================\n" | tee -a "$LOG_FILE"; }

header "🚀 STARTING 3-STAGE BOOTSTRAP (LLVM)"
log "$(date): Starting LLVM bootstrap"

header "📋 PRECHECKS"
log "${YELLOW}Building Rust CLI with LLVM...${NC}"
cargo build -p seen_cli --release --features llvm --quiet
log "${GREEN}✅ Rust CLI ready${NC}"

rm -f stage{1,2,3}_compiler stage{1,2,3}_compiler.c stage{1,2,3}_compiler.exe || true

header "🎯 STAGE 1: Rust CLI → Stage1 (LLVM)"
SEEN_ENABLE_MANIFEST_MODULES=1 cargo run -p seen_cli --release --features llvm -- build compiler_seen/src/main.seen \
  --backend llvm --output stage1_compiler 2>&1 | tee -a "$LOG_FILE"
[ -x stage1_compiler ] || { log "${RED}✖ Stage1 binary missing${NC}"; exit 1; }
log "${GREEN}✅ Stage1 ready${NC}"

header "🎯 STAGE 2: Stage1 → Stage2 (LLVM)"
./stage1_compiler build compiler_seen/src/main.seen --backend llvm --output stage2_compiler 2>&1 | tee -a "$LOG_FILE"
[ -x stage2_compiler ] || { log "${RED}✖ Stage2 binary missing${NC}"; exit 1; }
log "${GREEN}✅ Stage2 ready${NC}"

header "🎯 STAGE 3: Stage2 → Stage3 (LLVM)"
./stage2_compiler build compiler_seen/src/main.seen --backend llvm --output stage3_compiler 2>&1 | tee -a "$LOG_FILE"
[ -x stage3_compiler ] || { log "${RED}✖ Stage3 binary missing${NC}"; exit 1; }
log "${GREEN}✅ Stage3 ready${NC}"

header "🔍 VERIFY DETERMINISM"
H2=$(sha256sum stage2_compiler | cut -d' ' -f1)
H3=$(sha256sum stage3_compiler | cut -d' ' -f1)
log "Stage2: $H2"; log "Stage3: $H3"
if [ "$H2" = "$H3" ]; then
  log "${GREEN}🎉 SELF-HOSTING ACHIEVED: Stage2 == Stage3${NC}"
  exit 0
else
  log "${RED}✖ DIVERGENCE: Stage2 != Stage3${NC}"
  exit 1
fi
