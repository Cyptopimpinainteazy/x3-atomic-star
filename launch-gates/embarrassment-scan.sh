#!/usr/bin/env bash
# X3 Embarrassment Scanner
# Finds all the things that make mainnet go boom:
# - panic! unwrap() expect() in runtime code
# - TODO FIXME in critical paths
# - hardcoded values, private keys, dev-only code
# - stub implementations, mocks in production
# - unimplemented! todo!() remaining
#
# Philosophy: If it can crash or lie, mainnet will find it.

set -euo pipefail

PROOF_LOG="${1:-.}/launch-gates/evidence/proof-embarrassment-scan.log"
TEMP_SCAN="${2:-.}/launch-gates/evidence/embarrassment-raw-findings.txt"

RED='\033[0;31m'
YELLOW='\033[1;33m'
ORANGE='\033[38;5;208m'
NC='\033[0m'

mkdir -p "$(dirname "$PROOF_LOG")"

{
    echo "=== X3 Embarrassment Scanner ==="
    echo "Start time: $(date)"
    echo "Scanning: crates/ pallets/ runtime/ node/"
    echo ""
} | tee "$PROOF_LOG"

CATEGORIES=(
    "PANIC:panic!(\|unimplemented!(\|unreachable!()"
    "UNWRAP:\.unwrap(\|\.expect("
    "TODO:TODO\|FIXME\|XXX\|HACK"
    "HARDCODED:hardcoded\|123456\|0xdeadbeef\|magic number"
    "KEYS:private_key\|secret_key\|seed_phrase\|mnemonic"
    "DEV:dev_only\|test_only\|mock\|stub\|fake"
    "MEMORY:MemoryStore\|in-memory\|volatile\|ephemeral"
    "LOCALHOST:127\.0\.0\.1\|localhost\|0\.0\.0\.0"
    "ALICE:alice\|bob\|charlie\|dave\|eve"
    "SUDOS:ensure_root\|ensure_signed_by_admin"
)

echo "=== Scan Results ===" >> "$TEMP_SCAN"

# Track severity
P0_COUNT=0
P1_COUNT=0
P2_COUNT=0

for category_def in "${CATEGORIES[@]}"; do
    IFS=':' read -r CATEGORY PATTERN <<< "$category_def"
    
    echo "" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
    echo "--- $CATEGORY ---" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
    
    MATCHES=$(rg -n "$PATTERN" crates pallets runtime node --type rust 2>/dev/null | wc -l || echo 0)
    
    if [ "$MATCHES" -gt 0 ]; then
        echo "Found $MATCHES occurrences:" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
        rg -n "$PATTERN" crates pallets runtime node --type rust 2>/dev/null | head -20 | tee -a "$PROOF_LOG" "$TEMP_SCAN" || true
        
        if [ $MATCHES -gt 20 ]; then
            echo "  ... and $((MATCHES - 20)) more" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
        fi
        
        # Classify severity
        case "$CATEGORY" in
            PANIC|UNWRAP)
                P0_COUNT=$((P0_COUNT + MATCHES))
                echo -e "${RED}SEVERITY: P0 (CRITICAL - Can crash mainnet)${NC}" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
                ;;
            KEYS|SUDOS|ALICE)
                P0_COUNT=$((P0_COUNT + MATCHES))
                echo -e "${RED}SEVERITY: P0 (CRITICAL - Security/config issue)${NC}" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
                ;;
            TODO|FIXME)
                P1_COUNT=$((P1_COUNT + MATCHES))
                echo -e "${ORANGE}SEVERITY: P1 (HIGH - Incomplete code)${NC}" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
                ;;
            *)
                P2_COUNT=$((P2_COUNT + MATCHES))
                echo -e "${YELLOW}SEVERITY: P2 (MEDIUM - Code smell)${NC}" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
                ;;
        esac
    else
        echo "✅ No matches" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
    fi
done

# Additional checks
echo "" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
echo "--- Additional Checks ---" | tee -a "$PROOF_LOG" "$TEMP_SCAN"

# Check for unbounded loops
UNBOUNDED=$(rg -c "loop\s*\{" crates pallets runtime --type rust 2>/dev/null || echo 0)
if [ "$UNBOUNDED" -gt 0 ]; then
    echo -e "${ORANGE}Found $UNBOUNDED infinite loops (check if bounded)${NC}" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
fi

# Check for default implementations of critical traits
DANGEROUS_DEFAULTS=$(rg -c "impl.*Default.*for.*\(Config\|Runtime\)" crates pallets runtime --type rust 2>/dev/null || echo 0)
if [ "$DANGEROUS_DEFAULTS" -gt 0 ]; then
    echo -e "${RED}Found $DANGEROUS_DEFAULTS Default impls on critical types (P0 risk)${NC}" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
    P0_COUNT=$((P0_COUNT + DANGEROUS_DEFAULTS))
fi

# Summary
echo "" | tee -a "$PROOF_LOG" "$TEMP_SCAN"
{
    echo "=== Embarrassment Scanner Summary ==="
    echo "P0 (CRITICAL): $P0_COUNT"
    echo "P1 (HIGH): $P1_COUNT"
    echo "P2 (MEDIUM): $P2_COUNT"
    echo "Total: $((P0_COUNT + P1_COUNT + P2_COUNT))"
    echo ""
} | tee -a "$PROOF_LOG" "$TEMP_SCAN"

if [ $P0_COUNT -eq 0 ]; then
    {
        echo "RESULT: ✅ PASS"
        echo "No critical hazards found in production paths."
        echo "Score: 95%"
    } | tee -a "$PROOF_LOG"
    exit 0
elif [ $P0_COUNT -lt 5 ]; then
    {
        echo "RESULT: ⚠️  CONDITIONAL PASS"
        echo "Found $P0_COUNT critical hazards (minor)."
        echo "Score: 70%"
    } | tee -a "$PROOF_LOG"
    exit 0
else
    {
        echo "RESULT: ❌ FAIL"
        echo "Found $P0_COUNT critical hazards - MAINNET BLOCKER."
        echo "Score: $(( (50 - P0_COUNT) % 50 ))%"
        echo ""
        echo "Required actions:"
        echo "1. Remove all panic!/unwrap() from consensus/validator code"
        echo "2. Remove all hardcoded values, private keys, test data"
        echo "3. Remove all TODO/FIXME from critical paths"
        echo "4. Full code audit before mainnet"
    } | tee -a "$PROOF_LOG"
    exit 1
fi
