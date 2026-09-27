#!/usr/bin/env node
// ─────────────────────────────────────────────────────────────────────────────
// scripts/drills/finality_cert_evm_live_driver.cjs
//
// Drives the anvil chain the shell script started and asserts the finality-certificate producer's
// behaviour on real data. Every certificate is built by the `x3-finality-cert` binary — a fresh
// process per verdict — so the reloaded accepted tip in step 3 is genuinely reloaded, not shared.
//
// Inputs (environment):
//   X3_RPC_URL            anvil JSON-RPC endpoint
//   X3_CHAIN_ID           decimal chain id the node should answer
//   X3_ANCHOR_TX          the transaction hash the certificate is about (the contract deploy)
//   X3_CONTRACT           the deployed AtlasHTLC address (recorded in evidence)
//   X3_FINALITY_CERT_BIN  path to the built x3-finality-cert binary
//   X3_WORK_DIR           scratch directory for oracle stores
//   X3_EVIDENCE_DIR       where to write driver_report.json
// ─────────────────────────────────────────────────────────────────────────────
'use strict';

const http = require('http');
const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const RPC_URL = process.env.X3_RPC_URL;
const CHAIN_ID = process.env.X3_CHAIN_ID;
const ANCHOR_TX = process.env.X3_ANCHOR_TX;
const CONTRACT = process.env.X3_CONTRACT;
const BIN = process.env.X3_FINALITY_CERT_BIN;
const WORK_DIR = process.env.X3_WORK_DIR;
const EVIDENCE_DIR = process.env.X3_EVIDENCE_DIR;

let rpcId = 1;

function rpc(method, params = []) {
  const body = JSON.stringify({ jsonrpc: '2.0', id: rpcId++, method, params });
  const url = new URL(RPC_URL);
  return new Promise((resolve, reject) => {
    const req = http.request(
      {
        host: url.hostname,
        port: url.port,
        method: 'POST',
        headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(body) },
      },
      (res) => {
        let data = '';
        res.on('data', (chunk) => (data += chunk));
        res.on('end', () => {
          let parsed;
          try {
            parsed = JSON.parse(data);
          } catch (err) {
            return reject(new Error(`${method}: bad JSON response: ${data.slice(0, 200)}`));
          }
          if (parsed.error) {
            return reject(new Error(`${method}: ${parsed.error.code} ${parsed.error.message}`));
          }
          resolve(parsed.result);
        });
      }
    );
    req.on('error', reject);
    req.write(body);
    req.end();
  });
}

function runBinary(args) {
  const result = spawnSync(BIN, args, { encoding: 'utf8' });
  const stdout = (result.stdout || '').trim();
  try {
    return { exitCode: result.status, json: JSON.parse(stdout) };
  } catch (err) {
    return {
      exitCode: result.status,
      json: { outcome: 'error', code: 'UnparseableOutput', detail: stdout || result.stderr },
    };
  }
}

// Compare hex quantities (e.g. "0x7a69") to a decimal string.
function hexEquals(hex, decimal) {
  return BigInt(hex) === BigInt(decimal);
}

async function main() {
  const checks = [];
  const check = (name, ok, detail = '') => {
    checks.push({ name, ok: !!ok, detail: String(detail) });
  };

  // (0) The node is the chain we think it is.
  const chainIdHex = await rpc('eth_chainId');
  check('node_reports_the_expected_chain', hexEquals(chainIdHex, CHAIN_ID), `${chainIdHex} vs ${CHAIN_ID}`);

  // (1) The anchor is a real, successful transaction on this chain.
  const receipt = await rpc('eth_getTransactionReceipt', [ANCHOR_TX]);
  check('anchor_is_mined_and_succeeded', !!receipt && receipt.status === '0x1', JSON.stringify(receipt && receipt.status));
  if (!receipt) {
    return finish(checks, {});
  }
  const anchorHeight = Number(BigInt(receipt.blockNumber));
  const anchorHash = receipt.blockHash;

  // The producer's binding claim: the node's block at the anchor height carries the receipt's hash.
  const anchorBlock = await rpc('eth_getBlockByNumber', ['0x' + anchorHeight.toString(16), false]);
  check(
    'anchor_hash_is_the_block_at_its_height',
    anchorBlock && anchorBlock.hash === anchorHash,
    `${anchorBlock && anchorBlock.hash} vs ${anchorHash}`
  );

  // (2) Snapshot below the depth we are about to accept, so a later revert genuinely rewinds it.
  const snapshot = await rpc('evm_snapshot');
  await rpc('anvil_mine', ['0xb']); // 11 blocks; anchor at h0 gains its 12th confirmation.
  const tipBeforeRewind = Number(BigInt(await rpc('eth_blockNumber')));

  const storePath = path.join(WORK_DIR, 'oracle-store.json');
  const settled = runBinary([
    '--rpc', RPC_URL,
    '--chain', 'eth',
    '--expect-chain-id', String(CHAIN_ID),
    '--tx', ANCHOR_TX,
    '--store', storePath,
  ]);
  check(
    'certificate_settles_on_real_depth',
    settled.json.outcome === 'finalized' &&
      settled.json.confirmations >= 12 &&
      settled.json.observed_at === tipBeforeRewind,
    JSON.stringify(settled.json)
  );
  check(
    'accepted_tip_persisted_to_the_store',
    settled.json.accepted_tip === tipBeforeRewind && fs.existsSync(storePath),
    `store=${storePath} exists=${fs.existsSync(storePath)}`
  );

  // (3) Rewind the fork with real chain state: revert to the snapshot taken before the depth.
  const reverted = await rpc('evm_revert', [snapshot]);
  const tipAfterRewind = Number(BigInt(await rpc('eth_blockNumber')));
  check(
    'fork_rewound_below_the_accepted_tip',
    reverted === true && tipAfterRewind < tipBeforeRewind,
    `reverted=${reverted} tip ${tipAfterRewind} < ${tipBeforeRewind}`
  );

  // A *fresh process* reloads the accepted tip from the store and refuses the rewound certificate.
  const rewound = runBinary([
    '--rpc', RPC_URL,
    '--chain', 'eth',
    '--expect-chain-id', String(CHAIN_ID),
    '--tx', ANCHOR_TX,
    '--store', storePath,
  ]);
  check(
    'rewound_fork_refused_from_reloaded_tip',
    rewound.json.outcome === 'refused' && rewound.json.code === 'CertificateRewindsAcceptedAnchor',
    JSON.stringify(rewound.json)
  );

  // (4) A certificate for a chain this node is not is refused for the chain, not for the depth.
  const foreignStore = path.join(WORK_DIR, 'foreign-chain-store.json');
  const foreign = runBinary([
    '--rpc', RPC_URL,
    '--chain', 'eth',
    '--expect-chain-id', String(Number(CHAIN_ID) + 1),
    '--tx', ANCHOR_TX,
    '--store', foreignStore,
  ]);
  check(
    'foreign_chain_refused_for_the_chain',
    foreign.json.outcome === 'refused' && foreign.json.code === 'FinalityChainIdMismatch',
    JSON.stringify(foreign.json)
  );

  return finish(checks, {
    chain_id: CHAIN_ID,
    contract: CONTRACT,
    anchor_tx: ANCHOR_TX,
    anchor_height: anchorHeight,
    anchor_hash: anchorHash,
    tip_before_rewind: tipBeforeRewind,
    tip_after_rewind: tipAfterRewind,
    settled,
    rewound,
    foreign,
  });
}

function finish(checks, extra) {
  for (const c of checks) {
    console.log(`${c.ok ? '[ok]  ' : '[FAIL]'} ${c.name}${c.detail ? ' :: ' + c.detail : ''}`);
  }
  const allPass = checks.length > 0 && checks.every((c) => c.ok);
  if (EVIDENCE_DIR) {
    fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
    fs.writeFileSync(
      path.join(EVIDENCE_DIR, 'driver_report.json'),
      JSON.stringify({ rpc: RPC_URL, checks, ...extra }, null, 2)
    );
  }
  console.log(`finality_cert_evm_live_driver: ${allPass ? 'PASS' : 'FAIL'} (${checks.filter((c) => c.ok).length}/${checks.length})`);
  process.exitCode = allPass ? 0 : 1;
}

main().catch((err) => {
  console.error(`driver error: ${err.message}`);
  process.exitCode = 1;
});
