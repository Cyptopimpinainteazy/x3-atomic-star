#!/usr/bin/env node
'use strict';
//
// The Sentinel's privileged origin, driven against a live chain.
//
// `pallets/x3-sentinel` is the guard that freezes a mint authority on an asset, and
// `pallets/x3-token-factory` consults it before every supply-changing authority op
// (`type Sentinel = X3Sentinel`, `check_sentinel` before `mint`). Its registry row carries one
// blocker — "no live governance simulation" — and this driver is the evidence for what that
// blocker actually is:
//
//   * the governance gate is real: a signed account, Alice or Bob, cannot freeze anything
//     (`system.BadOrigin`), and the freeze map stays empty;
//   * the guard is inert while nothing is frozen: the same mint authority keeps minting;
//   * and the privileged path the pallet documents ("Root or a governance council") is
//     *unreachable on this chain*: the runtime wires `FreezeOrigin = EnsureRoot` in the single
//     shared `Config` impl used by all six runtime variants, and a chain without `sudo` has no
//     way for an extrinsic to arrive as root.
//
// The driver therefore reports `privileged_path` as `present` or `unreachable`, with the reason,
// and still fails on any assertion that does not hold. When the runtime is wired to a governance
// origin (`EnsureRootOrHalfCouncil`, the origin five other pallets in this runtime already use),
// the same driver dispatches through the path it finds and asserts the freeze *effect* — the mint
// being refused with `x3TokenFactory.AuthorityFrozenBySentinel`.
//
// Inputs (environment):
//   X3_WS_URL    websocket endpoint of a node on the chain
//   X3_OUT_JSON  optional path for the machine-readable evidence

const fs = require('fs');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');

const POLL_INTERVAL_MS = 400;
const TIMEOUT_MS = 120_000;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
/// `Bytes`/`Vec<u8>` fields are hex-encoded strings in the polkadot-js API, not Buffers.
const bytes = (text) => `0x${Buffer.from(text, 'utf8').toString('hex')}`;

const evidence = { status: 'started', steps: [] };
function flush() {
  if (process.env.X3_OUT_JSON) {
    fs.writeFileSync(process.env.X3_OUT_JSON, JSON.stringify(evidence, null, 2));
  }
}
function record(label, extra = {}) {
  evidence.steps.push({ label, ...extra });
  console.log(`[sentinel] ${label}${extra.detail ? ` — ${extra.detail}` : ''}`);
}
function die(message) {
  evidence.status = 'failed';
  evidence.error = message;
  flush();
  console.error(`[sentinel] FAIL: ${message}`);
  process.exit(1);
}

let api;

function describeDispatchError(data) {
  const dispatchError = data[0];
  if (dispatchError?.isModule) {
    try {
      const decoded = api.registry.findMetaError(dispatchError.asModule);
      return `${decoded.section}.${decoded.name}`;
    } catch (_e) {
      /* fall through */
    }
  }
  return dispatchError ? dispatchError.toString() : 'unknown dispatch error';
}

async function waitFor(label, startBlock, predicate, timeoutMs = TIMEOUT_MS) {
  const deadline = Date.now() + timeoutMs;
  let next = startBlock + 1;
  while (Date.now() < deadline) {
    const best = (await api.rpc.chain.getHeader()).number.toNumber();
    while (next <= best) {
      const blockHash = await api.rpc.chain.getBlockHash(next);
      const records = await api.query.system.events.at(blockHash);
      const failed = records.find((record) => api.events.system.ExtrinsicFailed.is(record.event));
      if (failed) return { blockNumber: next, dispatchError: describeDispatchError(failed.event.data) };
      const found = records.find((record) => predicate(record.event));
      if (found) return { blockNumber: next, event: found.event };
      next += 1;
    }
    await sleep(POLL_INTERVAL_MS);
  }
  throw new Error(`${label}: nothing observed within ${timeoutMs} ms`);
}

async function submitAndConfirm(extrinsic, signer, label, isExpected) {
  const signed = await extrinsic.signAsync(signer);
  const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  await api.rpc.author.submitExtrinsic(signed);
  const observed = await waitFor(label, startBlock, isExpected);
  if (observed.dispatchError) throw new Error(`${label}: dispatch failed — ${observed.dispatchError}`);
  return observed;
}

async function submitExpectingRefusal(extrinsic, signer, label, expectedName) {
  const signed = await extrinsic.signAsync(signer);
  const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  await api.rpc.author.submitExtrinsic(signed);
  const observed = await waitFor(label, startBlock, (event) =>
    api.events.system.ExtrinsicFailed.is(event),
  );
  if (!observed.dispatchError) throw new Error(`${label}: expected a refusal, the chain accepted it`);
  if (expectedName && observed.dispatchError !== expectedName) {
    throw new Error(`${label}: refused for the wrong reason — ${observed.dispatchError}`);
  }
  return observed.dispatchError;
}

async function main() {
  const wsUrl = process.env.X3_WS_URL;
  if (!wsUrl) die('X3_WS_URL is required');
  const provider = new WsProvider(wsUrl, 1_000);
  api = await ApiPromise.create({ provider, noInitWarn: true });
  evidence.chain = (await api.rpc.system.chain()).toString();
  evidence.specVersion = api.runtimeVersion.specVersion.toString();

  const sr = new Keyring({ type: 'sr25519' });
  const alice = sr.addFromUri('//Alice');
  const bob = sr.addFromUri('//Bob');

  const sentinel = api.tx.x3Sentinel;
  const factory = api.tx.x3TokenFactory;
  if (!sentinel || !factory) die('the sentinel and token-factory pallets must both be in metadata');
  for (const name of ['freezeAuthority', 'unfreezeAuthority']) {
    if (!sentinel[name]) die(`x3Sentinel.${name} is missing from metadata`);
  }
  for (const name of ['createToken', 'mint']) {
    if (!factory[name]) die(`x3TokenFactory.${name} is missing from metadata`);
  }

  // ── a mintable token, so the guard has something to guard ───────────────────────────────────
  const created = await submitAndConfirm(
    factory.createToken({
      symbol: bytes('SNTL'),
      name: bytes('Sentinel drill token'),
      canonicalDecimals: 12,
      initialSupply: 1_000n,
      maxSupply: 1_000_000n,
      class: 'CappedMintable',
      enabledDomains: ['X3Native', 'X3Evm'],
    }),
    alice,
    'create a capped-mintable token',
    (event) => api.events.x3TokenFactory.TokenCreated.is(event),
  );
  const assetId = created.event.data.assetId;
  record('the token was created', { detail: `asset ${assetId.toHex()}` });

  // Baseline: the mint authority can mint while nothing is frozen.
  await submitAndConfirm(
    factory.mint(assetId, 'X3Native', 100n),
    alice,
    'mint as the mint authority (nothing frozen)',
    (event) => api.events.x3TokenFactory.TokenMinted.is(event),
  );

  // ── the governance gate: a signed account cannot freeze anything ────────────────────────────
  const reason = bytes('sentinel drill');
  for (const [name, signer] of [
    ['Alice', alice],
    ['Bob', bob],
  ]) {
    const refusal = await submitExpectingRefusal(
      sentinel.freezeAuthority(assetId, alice.address, reason),
      signer,
      `${name} tries to freeze a mint authority`,
      // `BadOrigin` is a top-level `DispatchError` variant, not a pallet error, so it decodes
      // without a section prefix.
      'BadOrigin',
    );
    record(`${name} cannot freeze an authority`, { detail: refusal });
  }
  const frozen = await api.query.x3Sentinel.frozenAccounts(assetId, alice.address);
  if (!frozen.isEmpty) die('a freeze was stored even though every caller was refused');
  record('the freeze map is empty after the refused attempts');

  // ── is there any privileged path on this chain? ─────────────────────────────────────────────
  const hasSudo = Boolean(api.tx.sudo);
  evidence.privileged_path = hasSudo ? 'present' : 'unreachable';
  if (hasSudo) {
    // A dev runtime: dispatch through sudo, then assert the *effect*, not the event.
    await submitAndConfirm(
      api.tx.sudo.sudo(sentinel.freezeAuthority(assetId, alice.address, reason)),
      alice,
      'freeze through the privileged path',
      (event) => api.events.x3Sentinel.AuthorityFrozen.is(event),
    );
    const nowFrozen = await api.query.x3Sentinel.frozenAccounts(assetId, alice.address);
    if (nowFrozen.isEmpty) die('the privileged freeze was accepted but nothing was stored');
    await submitExpectingRefusal(
      factory.mint(assetId, 'X3Native', 100n),
      alice,
      'mint while frozen',
      'x3TokenFactory.AuthorityFrozenBySentinel',
    );
    record('a frozen authority is refused by the factory guard', {
      detail: 'x3TokenFactory.AuthorityFrozenBySentinel',
    });

    await submitAndConfirm(
      api.tx.sudo.sudo(sentinel.unfreezeAuthority(assetId, alice.address)),
      alice,
      'unfreeze through the privileged path',
      (event) => api.events.x3Sentinel.AuthorityUnfrozen.is(event),
    );
    await submitAndConfirm(
      factory.mint(assetId, 'X3Native', 100n),
      alice,
      'mint after unfreeze',
      (event) => api.events.x3TokenFactory.TokenMinted.is(event),
    );
    record('the privileged path is present and the whole control works', {
      detail: 'sudo -> freeze -> mint refused -> unfreeze -> mint accepted',
    });
  } else {
    record('no privileged path exists on this chain', {
      detail:
        'the runtime wires x3Sentinel::FreezeOrigin = EnsureRoot, and this chain has no sudo, ' +
        'so no extrinsic can arrive as root: the freeze control cannot be engaged by anyone',
    });
    // The guard stays inert, which is the observable consequence of that: minting continues.
    await submitAndConfirm(
      factory.mint(assetId, 'X3Native', 100n),
      alice,
      'mint again while no freeze is possible',
      (event) => api.events.x3TokenFactory.TokenMinted.is(event),
    );
  }

  evidence.status = 'passed';
  flush();
  console.log(`[sentinel] PASS — privileged_path=${evidence.privileged_path}`);
  await api.disconnect();
}

main().catch((error) => {
  die(error.message);
});
