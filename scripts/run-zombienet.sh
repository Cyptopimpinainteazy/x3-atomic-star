#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# run-zombienet.sh — Spawn and test an ephemeral X3 multi-validator testnet
#
# Covers:
#   • 3-validator Aura + GRANDPA network
#   • Block production verification
#   • Finality advancement
#   • Network partition simulation (manual steps printed)
#
# Modes:
#   ./scripts/run-zombienet.sh spawn    — spawn network and tail logs
#   ./scripts/run-zombienet.sh test     — run zombienet DSSL assertions
#   ./scripts/run-zombienet.sh build    — build x3-chain-node first
#   ./scripts/run-zombienet.sh help     — show this help
#
# Dependencies: zombienet binary, built x3-chain-node
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NODE_BIN="$REPO_ROOT/target/release/x3-chain-node"
ZOMBIENET_BIN="${ZOMBIENET_BIN:-$(which zombienet 2>/dev/null || echo '')}"
CONFIG="$REPO_ROOT/zombienet/x3-local-testnet.toml"
TEST_FILE="$REPO_ROOT/zombienet/x3-assertions.zndsl"
LOG_DIR="$REPO_ROOT/.zombienet-logs"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'
info()    { echo -e "${CYAN}[zombienet]${NC} $*"; }
success() { echo -e "${GREEN}[zombienet]  ✓${NC} $*"; }
warn()    { echo -e "${YELLOW}[zombienet] ⚠${NC} $*"; }
die()     { echo -e "${RED}[zombienet] ✗ ERROR:${NC} $*" >&2; exit 1; }

print_help() {
  cat <<EOF

  X3 Zombienet — ephemeral multi-validator testnet launcher

  USAGE:
    ./scripts/run-zombienet.sh [COMMAND]

  COMMANDS:
    build    Compile x3-chain-node (no special features needed)
    spawn    Spawn 3-validator testnet and tail all node logs
    test     Run DSSL assertion suite (zombienet/x3-assertions.zndsl)
    help     Show this message

  The spawned network exposes:
    Alice  WS  ws://127.0.0.1:9944   RPC  http://127.0.0.1:9933
    Bob    WS  ws://127.0.0.1:9946   RPC  http://127.0.0.1:9935
    Charlie WS ws://127.0.0.1:9948   RPC  http://127.0.0.1:9937

  NETWORK PARTITION SIMULATION (manual):
    After spawning, use iptables/tc to drop packets between validators:
      sudo iptables -A INPUT -p tcp --dport 30334 -j DROP   # isolate Bob
      sudo iptables -D INPUT -p tcp --dport 30334 -j DROP   # restore
    Observe in logs: Alice+Charlie continue finalizing; Bob catches up on heal.

EOF
}

check_deps() {
  [[ -n "$ZOMBIENET_BIN" ]] || die "zombienet not found. Install: npm install -g @zombienet/cli"
  [[ -f "$NODE_BIN" ]]      || die "Node binary not found. Run './scripts/run-zombienet.sh build' first."
  [[ -f "$CONFIG" ]]        || die "Zombienet config not found: $CONFIG"
}

cmd_build() {
  info "Building x3-chain-node (release) …"
  cd "$REPO_ROOT"
  cargo build --release -p x3-chain-node 2>&1 | tail -5
  success "Built → $NODE_BIN  ($(du -sh "$NODE_BIN" | cut -f1))"
}

cmd_spawn() {
  check_deps
  mkdir -p "$LOG_DIR"

  info "Spawning X3 local testnet (3 validators: Alice, Bob, Charlie) …"
  info "Config: $CONFIG"
  info "Press Ctrl-C to tear down the network"
  echo ""

  # Export binary path so zombienet can find it
  export PATH="$REPO_ROOT/target/release:$PATH"

  "$ZOMBIENET_BIN" \
    -p native \
    spawn "$CONFIG" \
    2>&1 | tee "$LOG_DIR/zombienet-$(date +%Y%m%d-%H%M%S).log"

  # zombienet blocks here; network is torn down on Ctrl-C
}

cmd_test() {
  check_deps

  if [[ ! -f "$TEST_FILE" ]]; then
    warn "No DSSL test file found at $TEST_FILE — generating minimal assertions …"
    generate_dssl
  fi

  info "Running Zombienet assertion suite …"
  export PATH="$REPO_ROOT/target/release:$PATH"

  "$ZOMBIENET_BIN" \
    -p native \
    test "$TEST_FILE" \
    2>&1 | tee "$LOG_DIR/test-$(date +%Y%m%d-%H%M%S).log"

  local exit_code=${PIPESTATUS[0]}
  [[ $exit_code -eq 0 ]] && success "All Zombienet assertions PASSED." || die "Assertions FAILED. Check logs in $LOG_DIR"
}

generate_dssl() {
  cat > "$TEST_FILE" <<'DSSL'
Description: X3 chain - 3-validator finality and block production

Network: zombienet/x3-local-testnet.toml
Creds: config

# ── Alice must be producing blocks ────────────────────────────────────────────
alice: is up
alice: reports node_roles is 4
alice: reports best block height is at least 1 within 60 secs
alice: reports finalized block height is at least 1 within 120 secs

# ── Bob must be producing blocks ──────────────────────────────────────────────
bob: is up
bob: reports best block height is at least 1 within 60 secs
bob: reports finalized block height is at least 1 within 120 secs

# ── Charlie must be producing blocks ─────────────────────────────────────────
charlie: is up
charlie: reports best block height is at least 1 within 60 secs
charlie: reports finalized block height is at least 1 within 120 secs

# ── All peers connected ────────────────────────────────────────────────────────
alice: reports peers count is at least 2 within 30 secs
bob: reports peers count is at least 2 within 30 secs
charlie: reports peers count is at least 2 within 30 secs

# ── Chain advancing (not stalled) ─────────────────────────────────────────────
alice: reports best block height is at least 5 within 180 secs
alice: reports finalized block height is at least 3 within 180 secs
DSSL
  info "Generated DSSL assertions → $TEST_FILE"
}

# ─────────── dispatch ──────────────────────────────────────────────────────
COMMAND="${1:-help}"
case "$COMMAND" in
  build)         cmd_build ;;
  spawn)         cmd_spawn ;;
  test)          cmd_test ;;
  help|--help|-h) print_help ;;
  *)             warn "Unknown command: $COMMAND"; print_help; exit 1 ;;
esac
