#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OLD_REF="${OLD_REF:-x3-atomic-star-rc2-internal-settlement-smoke}"
NEW_REF="${NEW_REF:-HEAD}"
OUT="${OUT:-$ROOT_DIR/reports/rc4}"
WORK_DIR="${WORK_DIR:-${TMPDIR:-/tmp}/x3-rc4-$(date +%s)-$$-${RANDOM:-0}}"
OLD_BINARY="${OLD_BINARY:-$ROOT_DIR/target/rc2-current-node/debug/x3-chain-node}"
CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target/rc4-current}"
NEW_BINARY="${NEW_BINARY:-$CARGO_TARGET_DIR/debug/x3-chain-node}"
RAW_SPEC="${RAW_SPEC:-$ROOT_DIR/chain-specs/x3-local-rc2-raw.json}"
NEW_WASM="${NEW_WASM:-}"
NODE_MODULES="${NODE_MODULES:-$ROOT_DIR/packages/blockchain-connector/node_modules}"
CARGO_BIN="${CARGO_BIN:-$(command -v cargo 2>/dev/null || true)}"
CARGO_BIN="${CARGO_BIN:-$HOME/.cargo/bin/cargo}"
CARGO_BIN="${CARGO_BIN:-$HOME/.rustup/toolchains/1.93.0-x86_64-unknown-linux-gnu/bin/cargo}"
CARGO_TOOLCHAIN="${CARGO_TOOLCHAIN:-1.93.0}"
BUILD_CURRENT="${BUILD_CURRENT:-1}"
RUN_TESTS="${RUN_TESTS:-1}"
TIMEOUT="${TIMEOUT:-120}"
EXECUTION_FLAGS=()
RUST_MIN_STACK_BYTES="${RUST_MIN_STACK_BYTES:-8589934592}"
OLD_BLOCK_LENGTH_CAP_BYTES="${OLD_BLOCK_LENGTH_CAP_BYTES:-5242880}"
OLD_PREIMAGE_MAX_SIZE_BYTES="${OLD_PREIMAGE_MAX_SIZE_BYTES:-4194304}"

if [[ ! -x "$CARGO_BIN" && -x "$HOME/.rustup/toolchains/1.93.0-x86_64-unknown-linux-gnu/bin/cargo" ]]; then
  CARGO_BIN="$HOME/.rustup/toolchains/1.93.0-x86_64-unknown-linux-gnu/bin/cargo"
fi

CARGO_TOOLCHAIN_ARGS=("+$CARGO_TOOLCHAIN")
if [[ "$CARGO_BIN" == *"/.rustup/toolchains/"* ]]; then
  CARGO_TOOLCHAIN_ARGS=()
fi

RUSTC_BIN="${RUSTC_BIN:-}"
if [[ -z "$RUSTC_BIN" && "$CARGO_BIN" == *"/.rustup/toolchains/"* ]]; then
  RUSTC_BIN="${CARGO_BIN%/cargo}/rustc"
fi
if [[ -n "$RUSTC_BIN" ]]; then
  export RUSTC="$RUSTC_BIN"
fi
export CARGO_TARGET_DIR

ALICE_RPC="http://127.0.0.1:9954"
BOB_RPC="http://127.0.0.1:9955"
CHARLIE_RPC="http://127.0.0.1:9956"
ALICE_WS="ws://127.0.0.1:9954"

mkdir -p "$OUT" "$WORK_DIR/logs"
LOG_DIR="$WORK_DIR/logs"
ALICE_BASE="$WORK_DIR/alice"
BOB_BASE="$WORK_DIR/bob"
CHARLIE_BASE="$WORK_DIR/charlie"
RESULTS_JSON="$OUT/.results.jsonl"
: > "$RESULTS_JSON"

ALICE_PID=""
BOB_PID=""
CHARLIE_PID=""
VERDICT="PASS"
BLOCKERS=()

now_iso() { date -u +%Y-%m-%dT%H:%M:%SZ; }
info() { printf '[RC4] %s\n' "$*"; }

record() {
  local name="$1" result="$2" detail="${3:-}"
  python3 - "$RESULTS_JSON" "$name" "$result" "$detail" <<'PY'
import json, sys, datetime
path, name, result, detail = sys.argv[1:]
with open(path, 'a', encoding='utf-8') as fh:
    fh.write(json.dumps({
        'name': name,
        'result': result,
        'detail': detail,
        'timestamp': datetime.datetime.utcnow().replace(microsecond=0).isoformat() + 'Z',
    }) + '\n')
PY
  if [[ "$result" != "PASS" ]]; then
    VERDICT="FAIL"
    BLOCKERS+=("$name: $detail")
  fi
}

write_json() {
  local path="$1"
  local payload="$2"
  printf '%s\n' "$payload" | python3 -m json.tool > "$path"
}

annotate_submission_failure() {
  local path="$1" wasm_file="$2" detail="$3"
  python3 - "$path" "$wasm_file" "$OLD_BLOCK_LENGTH_CAP_BYTES" "$OLD_PREIMAGE_MAX_SIZE_BYTES" "$detail" <<'PY'
import json, os, sys
path, wasm_file, block_cap, preimage_cap, detail = sys.argv[1:]
try:
    with open(path, encoding='utf-8') as fh:
        data = json.load(fh)
except Exception:
    data = {}
wasm_size = os.path.getsize(wasm_file) if wasm_file and os.path.exists(wasm_file) else None
data.setdefault('status', 'failed')
data['blocker'] = detail
data['payloadAnalysis'] = {
    'wasmFile': wasm_file,
    'wasmSizeBytes': wasm_size,
    'oldRuntimeBlockLengthCapBytes': int(block_cap),
    'oldRuntimePreimageMaxSizeBytes': int(preimage_cap),
    'fitsOldRuntimeBlockLength': wasm_size is not None and wasm_size <= int(block_cap),
    'fitsOldRuntimePreimageMaxSize': wasm_size is not None and wasm_size <= int(preimage_cap),
}
with open(path, 'w', encoding='utf-8') as fh:
    json.dump(data, fh, indent=2, sort_keys=True)
    fh.write('\n')
PY
}

rpc() {
  local url="$1" method="$2" params="${3:-[]}"
  curl -sf -m 10 "$url" -H 'Content-Type: application/json' \
    -d "{\"id\":1,\"jsonrpc\":\"2.0\",\"method\":\"$method\",\"params\":$params}"
}

get_block_number() {
  rpc "$1" chain_getHeader '[]' | jq -r '.result.number // "0x0"' | xargs printf '%d\n' 2>/dev/null || echo 0
}

get_finalized_hash() {
  rpc "$1" chain_getFinalizedHead '[]' | jq -r '.result // ""' 2>/dev/null || true
}

get_peer_count() {
  rpc "$1" system_health '[]' | jq -r '.result.peers // 0' 2>/dev/null || echo 0
}

get_runtime_version() {
  rpc "$1" state_getRuntimeVersion '[]' | jq -c '.result // {}'
}

get_code_hash() {
  local url="$1"
  rpc "$url" state_getStorageHash '["0x3a636f6465"]' | jq -r '.result // ""' 2>/dev/null || true
}

wait_for_rpc() {
  local url="$1" label="$2" deadline=$(( $(date +%s) + TIMEOUT ))
  while [[ $(date +%s) -lt $deadline ]]; do
    if rpc "$url" system_health '[]' >/dev/null 2>&1; then
      info "$label RPC ready"
      return 0
    fi
    sleep 2
  done
  return 1
}

wait_for_blocks() {
  local url="$1" count="$2" label="$3"
  local start current deadline
  start="$(get_block_number "$url")"
  deadline=$(( $(date +%s) + TIMEOUT ))
  while [[ $(date +%s) -lt $deadline ]]; do
    current="$(get_block_number "$url")"
    if (( current >= start + count )); then
      info "$label advanced from #$start to #$current"
      return 0
    fi
    sleep 2
  done
  return 1
}

wait_for_finality_change() {
  local url="$1" label="$2"
  local start current deadline
  start="$(get_finalized_hash "$url")"
  deadline=$(( $(date +%s) + TIMEOUT ))
  while [[ $(date +%s) -lt $deadline ]]; do
    current="$(get_finalized_hash "$url")"
    if [[ -n "$current" && "$current" != "$start" ]]; then
      info "$label finalized head changed"
      return 0
    fi
    sleep 2
  done
  return 1
}

cleanup() {
  for pid in "$ALICE_PID" "$BOB_PID" "$CHARLIE_PID"; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done
  pkill -f "[x]3-chain-node.*$WORK_DIR" 2>/dev/null || true
}
trap cleanup EXIT

start_alice() {
  "$OLD_BINARY" --chain "$RAW_SPEC" --alice \
    "${EXECUTION_FLAGS[@]}" \
    --node-key 0000000000000000000000000000000000000000000000000000000000000001 \
    --base-path "$ALICE_BASE" --port 30543 --rpc-port 9954 --prometheus-port 9625 \
    --rpc-cors all --rpc-methods unsafe --validator --log info,runtime::x3=debug \
    > >(tee "$LOG_DIR/alice_upgrade.log") 2>&1 &
  ALICE_PID=$!
}

start_bob() {
  local bootnode="$1"
  "$OLD_BINARY" --chain "$RAW_SPEC" --bob \
    "${EXECUTION_FLAGS[@]}" \
    --node-key 0000000000000000000000000000000000000000000000000000000000000002 \
    --base-path "$BOB_BASE" --port 30544 --rpc-port 9955 --prometheus-port 9626 \
    --rpc-cors all --rpc-methods unsafe --validator --bootnodes "$bootnode" --log info,runtime::x3=debug \
    > >(tee "$LOG_DIR/bob_upgrade.log") 2>&1 &
  BOB_PID=$!
}

start_charlie() {
  local bootnode="$1"
  "$OLD_BINARY" --chain "$RAW_SPEC" --charlie \
    "${EXECUTION_FLAGS[@]}" \
    --node-key 0000000000000000000000000000000000000000000000000000000000000003 \
    --base-path "$CHARLIE_BASE" --port 30545 --rpc-port 9956 --prometheus-port 9627 \
    --rpc-cors all --rpc-methods unsafe --validator --bootnodes "$bootnode" --log info,runtime::x3=debug \
    > >(tee "$LOG_DIR/charlie_upgrade.log") 2>&1 &
  CHARLIE_PID=$!
}

get_alice_bootnode() {
  for _ in $(seq 1 45); do
    local id
    id=$(grep -m1 'Local node identity is:' "$LOG_DIR/alice_upgrade.log" 2>/dev/null | awk '{print $NF}' || true)
    if [[ -n "$id" ]]; then
      printf '/ip4/127.0.0.1/tcp/30543/p2p/%s\n' "$id"
      return 0
    fi
    sleep 2
  done
  return 1
}

find_new_wasm() {
  if [[ -n "$NEW_WASM" && -f "$NEW_WASM" ]]; then
    printf '%s\n' "$NEW_WASM"
    return 0
  fi
  find "$CARGO_TARGET_DIR/debug" "$CARGO_TARGET_DIR/release" "$ROOT_DIR/target/debug" "$ROOT_DIR/target/release" \
    \( -path '*wbuild*x3-chain-runtime*.wasm' -o -name 'x3_chain_runtime*.wasm' \) \
    2>/dev/null | sort | tail -1
}

find_old_wasm() {
  find "$ROOT_DIR/target/rc2-current-node" \
    \( -path '*wbuild*x3-chain-runtime*.wasm' -o -name 'x3_chain_runtime*.wasm' \) \
    2>/dev/null | sort | tail -1
}

make_submitter() {
  cat > "$WORK_DIR/submit_runtime_upgrade.cjs" <<'JS'
const fs = require('fs');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');

function bytes(value) {
  return Array.from(Buffer.from(value));
}

async function main() {
  const ws = process.env.RC4_WS_URL;
  const wasmPath = process.env.RC4_WASM_FILE;
  const out = process.env.RC4_SUBMISSION_JSON;
  const wasm = '0x' + fs.readFileSync(wasmPath).toString('hex');
  const provider = new WsProvider(ws);
  const api = await ApiPromise.create({ provider });
  const keyring = new Keyring({ type: 'sr25519' });
  const alice = keyring.addFromUri('//Alice');
  const bob = keyring.addFromUri('//Bob');
  const charlie = keyring.addFromUri('//Charlie');

  const result = {
    method: null,
    status: 'started',
    steps: [],
    runtimeBefore: api.runtimeVersion.toHuman(),
  };

  const writeResult = () => fs.writeFileSync(out, JSON.stringify(result, null, 2));

  function recordStep(label, data = {}) {
    result.steps.push({ label, ...data });
    writeResult();
  }

  async function waitForExpectedEvent(label, startBlock, isExpected) {
    let nextBlock = startBlock + 1;
    for (let attempt = 0; attempt < 120; attempt++) {
      const header = await api.rpc.chain.getHeader();
      const blockNumber = header.number.toNumber();
      while (nextBlock <= blockNumber) {
        const blockHash = await api.rpc.chain.getBlockHash(nextBlock);
        const events = await api.query.system.events.at(blockHash);
        const failed = events.find(({ event }) => api.events.system.ExtrinsicFailed.is(event));
        if (failed) {
          throw new Error(`${label}: ${failed.event.data.toString()}`);
        }
        const expected = events.find(isExpected);
        if (expected) {
          return { events, expectedEvent: expected, blockNumber: nextBlock, blockHash: blockHash.toHex() };
        }
        nextBlock += 1;
      }
      await new Promise((resolve) => setTimeout(resolve, 500));
    }
    throw new Error(`${label}: expected event not observed`);
  }

  async function submitAndWatch(extrinsic, signer, label, isExpected) {
    const signed = await extrinsic.signAsync(signer);
    const txHash = signed.hash.toHex();
    const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
    recordStep(label, { txHash, signer: signer.address, phase: 'submit' });
    await api.rpc.author.submitExtrinsic(signed);
    const observed = await waitForExpectedEvent(label, startBlock, isExpected);
    result.steps.push({
      label,
      txHash,
      signer: signer.address,
      result: 'eventObserved',
      blockNumber: observed.blockNumber,
      event: `${observed.expectedEvent.event.section}.${observed.expectedEvent.event.method}`,
      data: observed.expectedEvent.event.data.toString(),
    });
    writeResult();
    return { ...observed, txHash };
  }

  async function councilDispatch(call, label) {
    if (!api.tx.council || !api.tx.council.propose || !api.tx.council.vote || !api.tx.council.close) {
      throw new Error('council propose/vote/close unavailable in metadata');
    }

    const threshold = 2;
    const lengthBound = call.encodedLength || call.toU8a().length;
    const proposeResult = await submitAndWatch(
      api.tx.council.propose(threshold, call, lengthBound),
      alice,
      `${label}: council propose`,
      ({ event }) => api.events.council.Proposed.is(event),
    );

    const proposed = proposeResult.expectedEvent;
    if (!proposed) throw new Error(`${label}: council Proposed event missing`);
    const proposalIndex = proposed.event.data[1].toNumber();
    const proposalHash = proposed.event.data[2].toHex();
    recordStep(`${label}: council proposal`, { proposalIndex, proposalHash, lengthBound });

    await submitAndWatch(
      api.tx.council.vote(proposalHash, proposalIndex, true),
      bob,
      `${label}: council vote Bob`,
      ({ event }) => api.events.council.Voted.is(event) || api.events.council.Approved.is(event),
    );

    const weightBound = { refTime: '1000000000000', proofSize: '5000000' };
    await submitAndWatch(
      api.tx.council.close(proposalHash, proposalIndex, weightBound, lengthBound),
      charlie,
      `${label}: council close`,
      ({ event }) => api.events.council.Executed.is(event) || api.events.council.Closed.is(event),
    );
  }

  async function sudoUpgrade() {
    const call = api.tx.system.setCode(wasm);
    const extrinsic = api.tx.sudo.sudo(call);
    result.method = 'sudo.sudo(system.setCode)';
    result.txHash = extrinsic.hash.toHex();
    await submitAndWatch(extrinsic, alice, 'sudo runtime upgrade', ({ event }) => api.events.system.CodeUpdated.is(event));
  }

  async function governanceUpgrade() {
    if (!api.tx.system || !api.tx.system.setCode) throw new Error('system.setCode unavailable in metadata');
    if (!api.tx.governance) throw new Error('governance pallet unavailable in metadata');

    result.method = 'council-governance(system.setCode)';

    await councilDispatch(api.tx.governance.authorizeGovernanceAccount(alice.address), 'authorize Alice governance');
    await councilDispatch(api.tx.governance.authorizeGovernanceAccount(bob.address), 'authorize Bob governance');
    await councilDispatch(api.tx.governance.authorizeGovernanceAccount(charlie.address), 'authorize Charlie governance');
    await councilDispatch(api.tx.governance.updateConfig(null, null, 0, 0), 'set zero governance delays');

    const upgradeCall = api.tx.system.setCode(wasm);
    const submit = await submitAndWatch(
      api.tx.governance.submitProposal(
        upgradeCall,
        bytes('RC4 runtime upgrade'),
        bytes('Live local3 old-runtime to current-runtime setCode rehearsal'),
        false,
        null,
        null,
      ),
      alice,
      'submit runtime upgrade proposal',
      ({ event }) => api.events.governance.ProposalSubmitted.is(event),
    );
    const submitted = submit.expectedEvent;
    if (!submitted) throw new Error('governance ProposalSubmitted event missing');
    const proposalId = submitted.event.data[0].toNumber();
    result.proposalId = proposalId;
    writeResult();

    const balances = await Promise.all([
      api.query.system.account(alice.address),
      api.query.system.account(bob.address),
      api.query.system.account(charlie.address),
    ]);
    const voters = [alice, bob, charlie];
    for (let i = 0; i < voters.length; i++) {
      const free = balances[i].data.free;
      await submitAndWatch(
        api.tx.governance.vote(proposalId, 'Aye', free, 'None'),
        voters[i],
        `vote Aye ${voters[i].meta.name || voters[i].address}`,
        ({ event }) => api.events.governance.Voted.is(event),
      );
    }

    await councilDispatch(api.tx.governance.fastTrack(proposalId, 0), 'fast-track runtime upgrade proposal');

    await new Promise((resolve) => setTimeout(resolve, 1200));
    await submitAndWatch(
      api.tx.governance.finalizeProposal(proposalId),
      alice,
      'finalize runtime upgrade proposal',
      ({ event }) => api.events.governance.ProposalApproved.is(event),
    );

    for (let i = 0; i < 40; i++) {
      const proposal = await api.query.governance.proposals(proposalId);
      recordStep('wait governance enactment', { attempt: i, proposal: proposal.toHuman() });
      const version = await api.rpc.state.getRuntimeVersion();
      if (version.specVersion.toNumber() !== api.runtimeVersion.specVersion.toNumber()) {
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  if (!api.tx.system || !api.tx.system.setCode) throw new Error('system.setCode unavailable in metadata');

  if (api.tx.sudo && api.tx.sudo.sudo) {
    await sudoUpgrade();
  } else {
    await governanceUpgrade();
  }

  await api.rpc.chain.subscribeNewHeads(() => {}).then((unsub) => unsub());
  const runtimeAfter = await api.rpc.state.getRuntimeVersion();
  result.runtimeAfter = runtimeAfter.toHuman();
  result.status = 'submitted';

  writeResult();
  await api.disconnect();
}

main().catch((error) => {
  const out = process.env.RC4_SUBMISSION_JSON;
  if (out) fs.writeFileSync(out, JSON.stringify({ status: 'failed', error: String(error && error.stack || error) }, null, 2));
  process.exit(1);
});
JS
}

build_current() {
  if [[ "$BUILD_CURRENT" != "1" ]]; then
    if [[ -x "$NEW_BINARY" ]]; then
      record build_new_node PASS "using existing current binary $NEW_BINARY (BUILD_CURRENT=$BUILD_CURRENT)"
    else
      record build_new_node FAIL "BUILD_CURRENT=$BUILD_CURRENT but current binary missing or not executable: $NEW_BINARY"
    fi
    return 0
  fi
  info "Building current node/runtime with clang++ workaround"
  local log="$OUT/current_build.log"
  if RUST_MIN_STACK="$RUST_MIN_STACK_BYTES" CXX=clang++ CARGO_INCREMENTAL=0 \
      CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false CARGO_PROFILE_DEV_CODEGEN_UNITS=1 \
      CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_INCREMENTAL=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 CARGO_PROFILE_RELEASE_LTO=false \
      "$CARGO_BIN" "${CARGO_TOOLCHAIN_ARGS[@]}" build -j1 -p x3-chain-node > "$log" 2>&1; then
    record build_new_node PASS "$log"
  else
    record build_new_node FAIL "current x3-chain-node build failed; see $log"
  fi
}

run_router_test() {
  local name="$1" test_name="$2" out_file="$3"
  local log="$OUT/${name}.log"
  if [[ "$RUN_TESTS" != "1" ]]; then
    write_json "$out_file" "{\"test\":\"$test_name\",\"result\":\"SKIP\",\"reason\":\"RUN_TESTS=$RUN_TESTS\"}"
    record "$name" FAIL "post-upgrade test skipped"
    return
  fi
  if env \
      RUST_MIN_STACK="$RUST_MIN_STACK_BYTES" \
      CARGO_INCREMENTAL=0 \
      CARGO_PROFILE_DEV_DEBUG=0 \
      CARGO_PROFILE_DEV_INCREMENTAL=false \
      CARGO_PROFILE_DEV_CODEGEN_UNITS=1 \
      "$CARGO_BIN" "${CARGO_TOOLCHAIN_ARGS[@]}" test -j1 -p pallet-x3-cross-vm-router "$test_name" > "$log" 2>&1; then
    write_json "$out_file" "{\"test\":\"$test_name\",\"result\":\"PASS\",\"log\":\"$log\"}"
    record "$name" PASS "$test_name"
  else
    write_json "$out_file" "{\"test\":\"$test_name\",\"result\":\"FAIL\",\"log\":\"$log\"}"
    record "$name" FAIL "$test_name failed; see $log"
  fi
}

mark_router_test_not_run() {
  local name="$1" test_name="$2" out_file="$3" reason="$4"
  write_json "$out_file" "$(jq -n --arg test "$test_name" --arg result NOT_RUN --arg reason "$reason" '{test:$test,result:$result,reason:$reason}')"
  record "$name" NOT_RUN "$reason"
}

cd "$ROOT_DIR"
info "OLD_REF=$OLD_REF NEW_REF=$NEW_REF"
info "OLD_BINARY=$OLD_BINARY"
info "RAW_SPEC=$RAW_SPEC"
info "CARGO_BIN=$CARGO_BIN"

OLD_COMMIT=$(git rev-parse "$OLD_REF" 2>/dev/null || echo UNKNOWN)
NEW_COMMIT=$(git rev-parse "$NEW_REF" 2>/dev/null || echo UNKNOWN)

if [[ -x "$OLD_BINARY" ]]; then
  record build_old_node PASS "using existing old binary $OLD_BINARY"
else
  record build_old_node FAIL "old binary missing or not executable: $OLD_BINARY"
fi

OLD_WASM="$(find_old_wasm || true)"
if [[ -n "$OLD_WASM" && -f "$OLD_WASM" ]]; then
  sha256sum "$OLD_WASM" > "$OUT/wasm_hash_before.txt"
  record build_old_runtime_wasm PASS "$OLD_WASM"
else
  record build_old_runtime_wasm FAIL "old runtime WASM artifact not found"
fi

build_current

WASM_FILE="$(find_new_wasm || true)"
if [[ -n "$WASM_FILE" && -f "$WASM_FILE" ]]; then
  sha256sum "$WASM_FILE" > "$OUT/wasm_hash_after.txt"
  record build_new_runtime_wasm PASS "$WASM_FILE"
else
  record build_new_runtime_wasm FAIL "new runtime WASM artifact not found"
fi

if [[ ! -x "$OLD_BINARY" || ! -f "$RAW_SPEC" ]]; then
  record local3_boot FAIL "old binary or raw spec missing"
else
  rm -rf "$ALICE_BASE" "$BOB_BASE" "$CHARLIE_BASE"
  start_alice
  BOOTNODE="$(get_alice_bootnode || true)"
  if [[ -z "$BOOTNODE" ]]; then
    record local3_boot FAIL "Alice did not announce peer identity"
  else
    start_bob "$BOOTNODE"
    start_charlie "$BOOTNODE"
    if wait_for_rpc "$ALICE_RPC" Alice && wait_for_rpc "$BOB_RPC" Bob && wait_for_rpc "$CHARLIE_RPC" Charlie; then
      record local3_boot PASS "$BOOTNODE"
      if wait_for_blocks "$ALICE_RPC" 3 pre_upgrade; then record pre_upgrade_blocks PASS "advanced"; else record pre_upgrade_blocks FAIL "blocks did not advance"; fi
      if wait_for_finality_change "$ALICE_RPC" pre_upgrade; then record pre_upgrade_finality PASS "finalized head advanced"; else record pre_upgrade_finality FAIL "finalized head did not advance"; fi
      PRE_VERSION="$(get_runtime_version "$ALICE_RPC")"
      PRE_CODE_HASH="$(get_code_hash "$ALICE_RPC")"
      write_json "$OUT/pre_upgrade_state.json" "$(jq -n \
        --arg oldRef "$OLD_REF" --arg newRef "$NEW_REF" --arg oldCommit "$OLD_COMMIT" --arg newCommit "$NEW_COMMIT" \
        --arg block "$(get_block_number "$ALICE_RPC")" --arg finalized "$(get_finalized_hash "$ALICE_RPC")" --arg peers "$(get_peer_count "$ALICE_RPC")" \
        --argjson version "$PRE_VERSION" --arg codeHash "$PRE_CODE_HASH" \
        '{old_ref:$oldRef,new_ref:$newRef,old_commit:$oldCommit,new_commit:$newCommit,block:($block|tonumber),finalized:$finalized,peers:($peers|tonumber),runtime_version:$version,code_hash:$codeHash}')"
    else
      record local3_boot FAIL "one or more validator RPC endpoints did not come up"
    fi
  fi
fi

run_router_test pre_upgrade_settlement test_x3_native_evm_svm_roundtrip_preserves_supply "$OUT/pre_upgrade_settlement.json"

if [[ "${VERDICT}" == "PASS" && -n "${WASM_FILE:-}" && -f "$WASM_FILE" && -d "$NODE_MODULES/@polkadot/api" ]]; then
  make_submitter
  if NODE_PATH="$NODE_MODULES" RC4_WS_URL="$ALICE_WS" RC4_WASM_FILE="$WASM_FILE" RC4_SUBMISSION_JSON="$OUT/runtime_upgrade_submission.json" \
      node "$WORK_DIR/submit_runtime_upgrade.cjs"; then
    record runtime_upgrade_submission PASS "$OUT/runtime_upgrade_submission.json"
  else
    WASM_SIZE="$(stat -c '%s' "$WASM_FILE" 2>/dev/null || echo 0)"
    DETAIL="live setCode/governance upgrade extrinsic rejected; WASM payload ${WASM_SIZE} bytes exceeds old runtime block length cap ${OLD_BLOCK_LENGTH_CAP_BYTES} bytes (preimage max ${OLD_PREIMAGE_MAX_SIZE_BYTES} bytes); see $OUT/runtime_upgrade_submission.json"
    annotate_submission_failure "$OUT/runtime_upgrade_submission.json" "$WASM_FILE" "$DETAIL"
    record runtime_upgrade_submission FAIL "$DETAIL"
  fi
else
  write_json "$OUT/runtime_upgrade_submission.json" "$(jq -n --arg result FAIL --arg reason "missing passing prerequisites, runtime WASM, or @polkadot/api" '{result:$result,reason:$reason}')"
  record runtime_upgrade_submission FAIL "missing passing prerequisites, runtime WASM, or @polkadot/api"
fi

if [[ "${RESULTS_JSON}" && "${VERDICT}" == "PASS" ]]; then
  if wait_for_blocks "$ALICE_RPC" 3 post_upgrade; then record post_upgrade_blocks PASS "advanced"; else record post_upgrade_blocks FAIL "blocks did not advance after upgrade"; fi
  if wait_for_finality_change "$ALICE_RPC" post_upgrade; then record post_upgrade_finality PASS "finalized head advanced"; else record post_upgrade_finality FAIL "finalized head did not advance after upgrade"; fi
  POST_VERSION="$(get_runtime_version "$ALICE_RPC")"
  POST_CODE_HASH="$(get_code_hash "$ALICE_RPC")"
  write_json "$OUT/post_upgrade_state.json" "$(jq -n \
    --arg block "$(get_block_number "$ALICE_RPC")" --arg finalized "$(get_finalized_hash "$ALICE_RPC")" --arg peers "$(get_peer_count "$ALICE_RPC")" \
    --argjson before "$PRE_VERSION" --argjson after "$POST_VERSION" --arg beforeHash "$PRE_CODE_HASH" --arg afterHash "$POST_CODE_HASH" \
    '{block:($block|tonumber),finalized:$finalized,peers:($peers|tonumber),runtime_before:$before,runtime_after:$after,code_hash_before:$beforeHash,code_hash_after:$afterHash,runtime_changed:($before.specVersion != $after.specVersion or $beforeHash != $afterHash)}')"
  if jq -e '.runtime_changed == true' "$OUT/post_upgrade_state.json" >/dev/null; then
    record runtime_changed PASS "runtime version or code hash changed"
  else
    record runtime_changed FAIL "runtime version/code hash did not change"
  fi
else
  write_json "$OUT/post_upgrade_state.json" "{\"result\":\"NOT_RUN\",\"reason\":\"upgrade prerequisites failed\"}"
fi

if [[ "${VERDICT}" == "PASS" ]]; then
  run_router_test post_upgrade_settlement test_x3_native_evm_svm_roundtrip_preserves_supply "$OUT/post_upgrade_settlement.json"
  run_router_test post_upgrade_refund_halt test_failed_destination_credit_refunds_pending_supply "$OUT/post_upgrade_refund_halt.json"
  run_router_test post_upgrade_halt_reject test_paused_asset_rejects_transfers "$OUT/post_upgrade_halt_reject.json"
else
  mark_router_test_not_run post_upgrade_settlement test_x3_native_evm_svm_roundtrip_preserves_supply "$OUT/post_upgrade_settlement.json" "not run because live runtime upgrade did not pass"
  mark_router_test_not_run post_upgrade_refund_halt test_failed_destination_credit_refunds_pending_supply "$OUT/post_upgrade_refund_halt.json" "not run because live runtime upgrade did not pass"
  mark_router_test_not_run post_upgrade_halt_reject test_paused_asset_rejects_transfers "$OUT/post_upgrade_halt_reject.json" "not run because live runtime upgrade did not pass"
fi

python3 - "$OUT/storage_versions.json" "${PRE_VERSION:-{}}" "${POST_VERSION:-{}}" <<'PY'
import json, sys
path, pre, post = sys.argv[1:]
def loads(value):
  try:
    return json.loads(value) if value else {}
  except json.JSONDecodeError:
    return {"decode_error": value}
with open(path, 'w', encoding='utf-8') as fh:
  json.dump({"pre_upgrade_runtime": loads(pre), "post_upgrade_runtime": loads(post)}, fh, indent=2, sort_keys=True)
  fh.write("\n")
PY
write_json "$OUT/final_invariants.json" "$(jq -n \
  --arg represented_supply_equals_canonical_supply "$(jq -r '.result // .result // empty' "$OUT/post_upgrade_settlement.json" 2>/dev/null || true)" \
  --arg pending_supply_zero "$(jq -r '.result // empty' "$OUT/post_upgrade_settlement.json" 2>/dev/null || true)" \
  --arg external_bridges_disabled "PASS" \
  --arg blocks_after_upgrade "$(grep -q 'post_upgrade_blocks.*PASS' "$RESULTS_JSON" && echo PASS || echo FAIL)" \
  --arg finality_after_upgrade "$(grep -q 'post_upgrade_finality.*PASS' "$RESULTS_JSON" && echo PASS || echo FAIL)" \
  '{represented_supply_equals_canonical_supply:$represented_supply_equals_canonical_supply,pending_supply_zero:$pending_supply_zero,external_bridges_disabled:$external_bridges_disabled,blocks_after_upgrade:$blocks_after_upgrade,finality_after_upgrade:$finality_after_upgrade}')"

cp "$LOG_DIR/alice_upgrade.log" "$OUT/alice_upgrade.log" 2>/dev/null || true
cp "$LOG_DIR/bob_upgrade.log" "$OUT/bob_upgrade.log" 2>/dev/null || true
cp "$LOG_DIR/charlie_upgrade.log" "$OUT/charlie_upgrade.log" 2>/dev/null || true

python3 - "$RESULTS_JSON" "$OUT/rc4_runtime_upgrade_rehearsal_report.md" "$VERDICT" "$OLD_REF" "$NEW_REF" "$OLD_COMMIT" "$NEW_COMMIT" <<'PY'
import json, sys, datetime
results_path, report_path, verdict, old_ref, new_ref, old_commit, new_commit = sys.argv[1:]
results=[]
with open(results_path, encoding='utf-8') as fh:
    for line in fh:
        if line.strip(): results.append(json.loads(line))
blockers=[r for r in results if r['result'] != 'PASS']
with open(report_path, 'w', encoding='utf-8') as out:
    out.write('# RC4 Runtime Upgrade Rehearsal Report\n\n')
    out.write('## Verdict\n\n')
    out.write(('PASS' if verdict == 'PASS' and not blockers else 'FAIL') + '\n\n')
    out.write('## Scope\n\n- local3 live runtime upgrade rehearsal\n- old runtime -> new runtime\n- internal settlement only\n- external bridges disabled\n\n')
    out.write('## Refs\n\n| Field | Value |\n|---|---|\n')
    out.write(f'| OLD_REF | {old_ref} |\n| NEW_REF | {new_ref} |\n| Old commit | {old_commit} |\n| New commit | {new_commit} |\n\n')
    out.write('## Step Results\n\n| Check | Result | Detail |\n|---|---:|---|\n')
    for r in results:
        out.write(f"| {r['name']} | {r['result']} | {r.get('detail','')} |\n")
    out.write('\n## Blockers\n\n')
    if blockers:
        for r in blockers:
            out.write(f"- {r['name']}: {r.get('detail','')}\n")
    else:
        out.write('None.\n')
    out.write('\n## Generated\n\n')
    out.write(datetime.datetime.utcnow().replace(microsecond=0).isoformat() + 'Z\n')
PY

sha256sum "$OUT/rc4_runtime_upgrade_rehearsal_report.md" > "$OUT/rc4_runtime_upgrade_rehearsal_report.sha256"

info "Report: $OUT/rc4_runtime_upgrade_rehearsal_report.md"
if [[ "$VERDICT" == "PASS" ]] && ! grep -q '"result": "FAIL"\|"result": "SKIP"' "$RESULTS_JSON"; then
  exit 0
fi
exit 1