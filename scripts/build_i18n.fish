#!/usr/bin/env fish
# Build stage2 compiler with i18n support (UTF-8 lexer + builtins)
# Module 5 (llvm_ir_gen.seen, 14312 lines) takes ~25 min — that's normal.

set -l COMPILER ./compiler_seen/target/seen
set -l OUTPUT /tmp/stage2_fresh
set -l LOG /tmp/build_fresh.log
set -l MEM_LIMIT 54G  # 54 of 61 GB — leaves ~7 GB for system

# Colors
set -l RED    (set_color red)
set -l GREEN  (set_color green)
set -l YELLOW (set_color yellow)
set -l CYAN   (set_color cyan)
set -l RESET  (set_color normal)

echo $CYAN"=== Seen i18n Compiler Build ===""$RESET"
echo "Memory limit: $MEM_LIMIT"
echo ""

# --- Step 0: Verify compiler exists ---
if not test -x $COMPILER
    echo $RED"ERROR: Compiler not found at $COMPILER""$RESET"
    exit 1
end

# --- Step 1: Clean all caches ---
echo $YELLOW"[1/5] Cleaning caches...""$RESET"
rm -rf /tmp/seen_ir_cache /tmp/seen_thinlto_cache .seen_cache/ 2>/dev/null
for i in (seq 0 49)
    rm -f "/tmp/seen_module_$i.ll" "/tmp/seen_module_$i.o" "/tmp/seen_module_$i.opt.ll" 2>/dev/null
end
echo "      Done."

# --- Step 2: Build stage2 with memory limit ---
echo $YELLOW"[2/5] Building stage2 (expect ~35 min total)...""$RESET"
echo "      Log: $LOG"
echo ""

set -l START (date +%s)

# Use systemd-run for hard memory limit without needing root (--user --scope)
systemd-run --user --scope -p MemoryMax=$MEM_LIMIT -p MemorySwapMax=0 \
    $COMPILER compile compiler_seen/src/main_compiler.seen $OUTPUT --fast \
    >$LOG 2>&1 &

set -l BUILD_PID (jobs -lp | tail -1)
echo "      Build PID: $BUILD_PID"
echo ""

# --- Monitor loop ---
while kill -0 $BUILD_PID 2>/dev/null
    set -l ELAPSED (math (date +%s) - $START)
    set -l MINS (math "floor($ELAPSED / 60)")
    set -l SECS (math "$ELAPSED % 60")

    # Count completed .ll files
    set -l LL_COUNT 0
    for i in (seq 0 49)
        test -f "/tmp/seen_module_$i.ll"; and set LL_COUNT (math $LL_COUNT + 1)
    end

    # Count completed .o files
    set -l O_COUNT 0
    for i in (seq 0 49)
        test -f "/tmp/seen_module_$i.o"; and set O_COUNT (math $O_COUNT + 1)
    end

    # Find the heavy child (module 5 = llvm_ir_gen.seen, uses ~2GB)
    set -l CHILD_INFO ""
    for line in (ps --ppid $BUILD_PID -o rss= 2>/dev/null)
        set -l rss_kb (string trim $line)
        if test -n "$rss_kb"
            set -l rss_mb (math "floor($rss_kb / 1024)")
            if test $rss_mb -gt 500
                set CHILD_INFO " | heavy child: $rss_mb""MB"
            end
        end
    end

    # Total RSS of all compiler processes
    set -l TOTAL_MB 0
    for line in (ps aux | grep "$COMPILER" | grep -v grep | awk '{print $6}')
        if test -n "$line"
            set TOTAL_MB (math "$TOTAL_MB + floor($line / 1024)")
        end
    end

    printf "\r      %s[%02d:%02d]%s  IR: %s/50  OBJ: %s/50  RAM: %sMB%s    " \
        $CYAN $MINS $SECS $RESET $LL_COUNT $O_COUNT $TOTAL_MB "$CHILD_INFO"

    sleep 10
end

# Get exit status
wait $BUILD_PID
set -l EXIT_CODE $status

set -l END (date +%s)
set -l TOTAL (math $END - $START)
set -l TOTAL_MINS (math "floor($TOTAL / 60)")
set -l TOTAL_SECS (math "$TOTAL % 60")

echo ""
echo ""

if test $EXIT_CODE -ne 0
    echo $RED"[2/5] Build FAILED (exit $EXIT_CODE) after $TOTAL_MINS""m$TOTAL_SECS""s""$RESET"
    echo "      Check log: $LOG"
    tail -20 $LOG
    exit 1
end

if not test -f $OUTPUT
    echo $RED"[2/5] Build produced no output binary""$RESET"
    echo "      Check log: $LOG"
    tail -20 $LOG
    exit 1
end

echo $GREEN"[2/5] Build succeeded in $TOTAL_MINS""m$TOTAL_SECS""s""$RESET"
echo "      Binary: $OUTPUT ("(math "floor("(stat -c%s $OUTPUT)" / 1024)")" KB)"

# --- Step 3: Test return value ---
echo ""
echo $YELLOW"[3/5] Testing return value...""$RESET"
rm -rf /tmp/seen_ir_cache .seen_cache/ 2>/dev/null
echo 'fun main() r: Int { return 42 }' > /tmp/seen_test_ret.seen
$OUTPUT compile /tmp/seen_test_ret.seen /tmp/seen_test_ret_out --fast 2>/dev/null
/tmp/seen_test_ret_out 2>/dev/null
set -l RET $status
if test $RET -eq 42
    echo $GREEN"      PASS: return 42 → exit $RET""$RESET"
else
    echo $RED"      FAIL: return 42 → exit $RET (expected 42)""$RESET"
    exit 1
end

# --- Step 4: Test Arabic UTF-8 ---
echo $YELLOW"[4/5] Testing Arabic UTF-8...""$RESET"
rm -rf /tmp/seen_ir_cache .seen_cache/ 2>/dev/null
echo 'دالة رئيسية() ن: عدد { اطبع("مرحبا") رجع 42 }' > /tmp/seen_test_ar.seen
$OUTPUT compile /tmp/seen_test_ar.seen /tmp/seen_test_ar_out --fast --language ar 2>/dev/null
set -l AR_OUT (/tmp/seen_test_ar_out 2>/dev/null)
set -l AR_RET $status
if test $AR_RET -eq 42
    echo $GREEN"      PASS: Arabic → exit $AR_RET, output: $AR_OUT""$RESET"
else
    echo $RED"      FAIL: Arabic → exit $AR_RET (expected 42)""$RESET"
    echo "      This means the UTF-8 lexer fix didn't work."
    echo "      English tests will still work. Non-English tests blocked."
    exit 1
end

# --- Step 5: Install and run E2E suite ---
echo $YELLOW"[5/5] Installing and running E2E suite...""$RESET"
cp $OUTPUT ./compiler_seen/target/seen
chmod +x tests/e2e_multilang/run_all_e2e.sh
echo ""
./tests/e2e_multilang/run_all_e2e.sh
set -l E2E_EXIT $status

echo ""
if test $E2E_EXIT -eq 0
    echo $GREEN"=== ALL DONE — full E2E suite passed ===""$RESET"
else
    echo $YELLOW"=== E2E suite had failures (exit $E2E_EXIT) ===""$RESET"
end
exit $E2E_EXIT
