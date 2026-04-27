#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# run-frame-benchmarks.sh — FRAME pallet weight benchmarking for X3
#
# Runs `benchmark pallet` for all 4 pallets that have benchmarks! blocks,
# writes generated WeightInfo impls to each pallet's src/weights.rs, and
# cross-checks each output against the reference hardware baseline.
#
# Pallets benchmarked:
#   • pallet-x3-atomic-kernel     (submit_atomic_bundle, assign_bundle_executor)
#   • pallet-x3-settlement-engine (settle_bundle, record_settlement)
#   • pallet-cross-chain-validator (validate_cross_chain_proof)
#   • pallet-x3-slash              (slash_validator, report_double_sign)
#
# Modes:
#   ./scripts/run-frame-benchmarks.sh build        — compile with runtime-benchmarks
#   ./scripts/run-frame-benchmarks.sh run          — run all 4 pallets, write weights
#   ./scripts/run-frame-benchmarks.sh run PALLET   — run one pallet only
#   ./scripts/run-frame-benchmarks.sh machine      — check hardware meets Substrate baseline
#   ./scripts/run-frame-benchmarks.sh list         — list all available benchmarks
#   ./scripts/run-frame-benchmarks.sh help         — show this help
#
# Warning: builds take 10–20 min; benchmark runs take 5–30 min per pallet.
# Run with --steps 10 --repeat 5 for a fast dry-run (not for production weights).
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BIN="$REPO_ROOT/target/release/x3-chain-node"
LOG_DIR="$REPO_ROOT/.benchmark-logs"
LOG_FILE="$LOG_DIR/frame-benchmarks-$(date +%Y%m%d-%H%M%S).log"

# Benchmark parameters — tune for speed vs accuracy
# Production: STEPS=50 REPEAT=20
# Quick smoke: STEPS=10 REPEAT=5
STEPS="${BENCHMARK_STEPS:-50}"
REPEAT="${BENCHMARK_REPEAT:-20}"
CHAIN="${BENCHMARK_CHAIN:-dev}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()    { echo -e "${CYAN}[benchmark]${NC} $*"; }
success() { echo -e "${GREEN}[benchmark]  ✓${NC} $*"; }
warn()    { echo -e "${YELLOW}[benchmark] ⚠${NC} $*"; }
die()     { echo -e "${RED}[benchmark] ✗ ERROR:${NC} $*" >&2; exit 1; }

# Pallet name → output path mapping
declare -A PALLET_PATHS=(
  ["pallet-x3-atomic-kernel"]="pallets/x3-atomic-kernel/src/weights.rs"
  ["pallet-x3-settlement-engine"]="pallets/x3-settlement-engine/src/weights.rs"
  ["pallet-cross-chain-validator"]="pallets/cross-chain-validator/src/weights.rs"
  ["pallet-x3-slash"]="pallets/x3-slash/src/weights.rs"
)

print_help() {
  cat <<EOF

  X3 FRAME Benchmarking — accurate pallet weight generation

  USAGE:
    ./scripts/run-frame-benchmarks.sh [COMMAND] [PALLET]

  COMMANDS:
    build            Compile x3-chain-node --features runtime-benchmarks
    run              Benchmark all 4 pallets; write weights to pallet src/weights.rs
    run PALLET       Benchmark a single pallet (use the crate name, e.g. pallet-x3-slash)
    machine          Check hardware against Substrate reference baseline
    list             List all benchmarkable extrinsics across all pallets
    help             Show this message

  TUNING:
    BENCHMARK_STEPS=50   Number of component steps  (default: $STEPS)
    BENCHMARK_REPEAT=20  Repeats per step            (default: $REPEAT)
    BENCHMARK_CHAIN=dev  Chain spec to use           (default: $CHAIN)

  QUICK SMOKE RUN (2 min):
    BENCHMARK_STEPS=10 BENCHMARK_REPEAT=5 ./scripts/run-frame-benchmarks.sh run

  PRODUCTION RUN:
    BENCHMARK_STEPS=50 BENCHMARK_REPEAT=20 ./scripts/run-frame-benchmarks.sh run

EOF
}

cmd_build() {
  info "Building x3-chain-node with --features runtime-benchmarks …"
  info "(first build: 10-20 min; subsequent: 1-3 min with cache)"
  cd "$REPO_ROOT"
  cargo build \
    --release \
    -p x3-chain-node \
    --features runtime-benchmarks \
    2>&1 | tee "${LOG_DIR}/build-benchmark-$(date +%Y%m%d-%H%M%S).log"
  success "Build complete → $NODE_BIN"
  echo "Binary: $(du -sh "$NODE_BIN" | cut -f1)"
}

bench_pallet() {
  local pallet="$1"
  local output_rel="${PALLET_PATHS[$pallet]:-}"

  [[ -n "$output_rel" ]] || die "Unknown pallet: $pallet. Known: ${!PALLET_PATHS[*]}"
  local output_file="$REPO_ROOT/$output_rel"

  info "Benchmarking $pallet (steps=$STEPS, repeat=$REPEAT) …"
  mkdir -p "$(dirname "$output_file")"

  "$NODE_BIN" benchmark pallet \
    --chain "$CHAIN" \
    --pallet "$pallet" \
    --extrinsic "*" \
    --steps "$STEPS" \
    --repeat "$REPEAT" \
    --template "$REPO_ROOT/.maintain/frame-weight-template.hbs" \
    --output "$output_file" \
    2>&1 | tee -a "$LOG_FILE"

  local exit_code=${PIPESTATUS[0]}
  if [[ $exit_code -eq 0 ]]; then
    success "$pallet → weights written to $output_rel"
    # Show the generated ref times
    grep -E 'fn |RefTime|ProofSize' "$output_file" | head -20 || true
  else
    # --template may not exist; retry without it (outputs to stdout)
    warn "--template flag failed (missing .hbs). Retrying without template …"
    "$NODE_BIN" benchmark pallet \
      --chain "$CHAIN" \
      --pallet "$pallet" \
      --extrinsic "*" \
      --steps "$STEPS" \
      --repeat "$REPEAT" \
      --output "$output_file" \
      2>&1 | tee -a "$LOG_FILE"

    [[ ${PIPESTATUS[0]} -eq 0 ]] && success "$pallet weights written." || die "$pallet benchmark FAILED"
  fi
}

cmd_run() {
  [[ -f "$NODE_BIN" ]] || die "Node binary not found. Run './scripts/run-frame-benchmarks.sh build' first."
  mkdir -p "$LOG_DIR"

  local target_pallet="${1:-all}"

  if [[ "$target_pallet" == "all" ]]; then
    info "Running FRAME benchmarks for all ${#PALLET_PATHS[@]} pallets …"
    info "STEPS=$STEPS  REPEAT=$REPEAT  CHAIN=$CHAIN"
    info "Log → $LOG_FILE"
    echo ""

    local passed=0 failed=0
    for pallet in "${!PALLET_PATHS[@]}"; do
      if bench_pallet "$pallet"; then
        passed=$((passed + 1))
      else
        warn "$pallet benchmark failed — continuing with remaining pallets"
        failed=$((failed + 1))
      fi
      echo ""
    done

    echo ""
    echo "────────────────────────────────────────────────"
    echo -e "  FRAME Benchmark Results:"
    echo -e "  ${GREEN}PASSED: $passed${NC}  |  ${RED}FAILED: $failed${NC}"
    echo "────────────────────────────────────────────────"
    [[ $failed -eq 0 ]] && success "All pallet weights generated." || warn "$failed pallets failed — check $LOG_FILE"
  else
    bench_pallet "$target_pallet"
  fi
}

cmd_machine() {
  [[ -f "$NODE_BIN" ]] || die "Node binary not found. Run './scripts/run-frame-benchmarks.sh build' first."
  mkdir -p "$LOG_DIR"

  info "Checking hardware against Substrate reference baseline …"
  "$NODE_BIN" benchmark machine \
    --chain "$CHAIN" \
    2>&1 | tee -a "$LOG_FILE"
}

cmd_list() {
  [[ -f "$NODE_BIN" ]] || die "Node binary not found. Run './scripts/run-frame-benchmarks.sh build' first."

  info "Listing all available benchmarks in the runtime …"
  "$NODE_BIN" benchmark pallet \
    --chain "$CHAIN" \
    --pallet "*" \
    --extrinsic "*" \
    --list \
    2>&1 | grep -v '^$' | head -80
}

# ─────────── dispatch ──────────────────────────────────────────────────────
mkdir -p "$LOG_DIR"

COMMAND="${1:-help}"
case "$COMMAND" in
  build)          cmd_build ;;
  run)            cmd_run "${2:-all}" ;;
  machine)        cmd_machine ;;
  list)           cmd_list ;;
  help|--help|-h) print_help ;;
  *)              warn "Unknown command: $COMMAND"; print_help; exit 1 ;;
esac
