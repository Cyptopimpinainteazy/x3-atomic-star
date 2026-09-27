#!/usr/bin/env bash
# ProofForge - Security Gates Runner (S0 & S1)
# Executes security verification gates
# Usage: ./scripts/run-security-gates.sh [gate_level]
#   gate_level: all (default), s0, s1

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PROOF_BINARY="${REPO_ROOT}/target/release/x3-proof"
RESULTS_DIR="${REPO_ROOT}/.proof-results"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Functions
log_header() {
    echo -e "${CYAN}════════════════════════════════════════${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}════════════════════════════════════════${NC}"
}

log_gate() {
    echo -e "\n${BLUE}▶ $1${NC}"
}

log_pass() {
    echo -e "${GREEN}✓ $1${NC}"
}

log_fail() {
    echo -e "${RED}✗ $1${NC}"
}

# S0 SECURITY GATE
run_s0_gate() {
    log_gate "S0 Security Gate - Basic Verification"
    
    mkdir -p "$RESULTS_DIR"
    local results_file="${RESULTS_DIR}/s0-security-gate.txt"

    # Fail with the reason that matters when the TLA+ toolchain is absent. The
    # proof binary shells out to `java -cp tools/tla2tools.jar tlc2.TLC`, so with
    # no JRE on PATH every spec reports `TLA+ invocation error … No such file or
    # directory (os error 2)` — a message that names neither Java nor the fix.
    # Measured 2026-09-27: that is exactly how `S0: formal_verification_blocked`
    # appeared on a box with `tools/tla2tools.jar` present and no `java`.
    if [ -n "$(find "${REPO_ROOT}/formal-proofs/tla" -name '*.tla' -print -quit 2>/dev/null)" ] \
        && ! command -v java >/dev/null 2>&1; then
        {
            echo "=== S0 Security Gate Execution ==="
            echo "Timestamp: $(date -u)"
            echo ""
            echo "✗ TLA+ specs exist under formal-proofs/tla but no 'java' is on PATH."
            echo "  The proof binary runs 'java -cp tools/tla2tools.jar tlc2.TLC',"
            echo "  so formal verification cannot run at all."
            echo "  Fix: install a JRE (e.g. default-jre-headless) or put one on PATH."
        } > "$results_file"
        log_fail "S0 gate could not run: no JRE on PATH for the TLA+ model check"
        return 1
    fi
    
    {
        echo "=== S0 Security Gate Execution ==="
        echo "Timestamp: $(date -u)"
        echo ""
        echo "Running basic security verification..."
        echo ""
    } > "$results_file"
    
    # The exit code is not the check. `x3-proof security-gate` returns 0 while
    # its own report ends in `Gate Status: 1 BLOCKER(S) REMAIN` (measured
    # 2026-09-27, on the missing-JRE blocker), so a wrapper that only looked at
    # `if "$PROOF_BINARY" …` logged "✓ S0 gate passed" over a report that said a
    # blocker remained. Read the report.
    local s0_output
    if ! s0_output="$("$PROOF_BINARY" security-gate -v 2>&1)"; then
        printf '%s\n' "$s0_output" >> "$results_file"
        log_fail "S0 gate failed - see $results_file"
        return 1
    fi
    printf '%s\n' "$s0_output" >> "$results_file"

    if grep -q "BLOCKER(S) REMAIN" <<<"$s0_output"; then
        grep -E "BLOCKER\(S\) REMAIN|⛔" <<<"$s0_output" | sed 's/^/  /' >&2
        log_fail "S0 gate reports a remaining blocker - see $results_file"
        return 1
    fi
    log_pass "S0 gate passed"
    
    {
        echo ""
        echo "=== Blockers Check ==="
    } >> "$results_file"
    
    # Informational only: the verdict is the report above. This used to log
    # "No critical blockers detected" whenever the command exited 0, whatever it
    # printed — which is how `reports/rc6/security_gates_*.log` came to say
    # "✓ S0 gate passed / ✓ No critical blockers detected" four minutes after
    # `.proof-results/s0-security-gate.txt` recorded `1 BLOCKER(S) REMAIN`.
    local explain
    if explain="$("$PROOF_BINARY" explain-blockers all 2>&1)"; then
        printf '%s\n' "$explain" >> "$results_file"
        log_pass "Blocker explanation written (the S0 report above is the verdict)"
    else
        printf '%s\n' "$explain" >> "$results_file"
        log_fail "explain-blockers failed - see $results_file"
        return 1
    fi
    
    return 0
}

# S1 SECURITY GATE
run_s1_gate() {
    log_gate "S1 Security Gate - Extended Verification"
    
    mkdir -p "$RESULTS_DIR"
    local results_file="${RESULTS_DIR}/s1-security-gate.txt"
    
    {
        echo "=== S1 Security Gate Execution ==="
        echo "Timestamp: $(date -u)"
        echo ""
    } > "$results_file"
    
    # Critical modules to verify
    local critical_modules=(
        "consensus:P7:Consensus Mechanism"
        "bridge:P7:Cross-Chain Bridge"
        "runtime:P7:Runtime Environment"
        "asset_kernel:P7:Asset Kernel"
        "custody:P7:Custody System"
    )
    
    echo "Verifying critical modules with strict validation:" >> "$results_file"
    echo "" >> "$results_file"
    
    local all_passed=true
    for entry in "${critical_modules[@]}"; do
        IFS=':' read -r module level name <<< "$entry"
        
        echo "→ Verifying $name ($module) as $level..." 
        echo "→ Verifying $name ($module) as $level..." >> "$results_file"
        
        if "$PROOF_BINARY" prove "$module" --strict -v >> "$results_file" 2>&1; then
            log_pass "$name verified successfully"
            echo "  ✓ PASSED" >> "$results_file"
        else
            log_fail "$name verification failed"
            echo "  ✗ FAILED" >> "$results_file"
            all_passed=false
        fi
        echo "" >> "$results_file"
    done
    
    if $all_passed; then
        log_pass "S1 gate passed - all critical modules verified"
        return 0
    else
        log_fail "S1 gate failed - some modules did not verify"
        return 1
    fi
}

# MAIN
main() {
    log_header "ProofForge Security Gates"
    
    local gate_level="${1:-all}"
    local exit_code=0
    
    # Build if needed
    if [ ! -f "$PROOF_BINARY" ]; then
        echo "Building ProofForge..."
        cd "$REPO_ROOT"
        cargo build -p proof-forge --release 2>&1 | tail -3
    fi
    
    echo ""
    
    case "$gate_level" in
        all)
            run_s0_gate || exit_code=$?
            run_s1_gate || exit_code=$?
            ;;
        s0)
            run_s0_gate || exit_code=$?
            ;;
        s1)
            run_s1_gate || exit_code=$?
            ;;
        *)
            echo "Invalid gate level: $gate_level"
            echo "Valid options: all, s0, s1"
            exit 1
            ;;
    esac
    
    echo ""
    log_header "Security Gates Complete"
    
    exit $exit_code
}

trap 'echo -e "\n${RED}Interrupted${NC}"; exit 130' INT TERM

main "$@"
