#!/usr/bin/env node
'use strict';
//
// The commit-reveal ordering window, driven on a live chain.
//
// `pallets/private-execution` has had the window since `9ba1f2c20` and a chain-derived beacon
// since `fd95445d3`, but every proof of it was a unit test against the pallet's mock. This driver
// is the missing half: it opens a window through the real extrinsic, commits and reveals from real
// accounts, settles, and then *recomputes* the canonical order from what the chain stored and
// requires the two to agree.
//
// It also exercises the reason no live run existed before `c917917d6`: window open/commit used to
// require private execution *and* the confidential-validator quorum, and this runtime's
// `AttestationVerifier = RefuseAllAttestations` means that quorum can never be met. The lane has
// its own switch now, and this driver turns it on the way an operator would — a council motion
// (`AdminOrigin = EnsureRootOrHalfCouncil`), the same governance path the runtime upgrade uses.
//
// Inputs (environment):
//   X3_WS_URL    websocket endpoint of a node on the chain
//   X3_OUT_JSON  optional path for the machine-readable evidence
//
// Exit 0 → the settled order is the canonical order the driver recomputed, and the refusals held.

const fs = require('fs');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');
const { blake2AsU8a } = require('@polkadot/util-crypto');
const { hexToU8a } = require('@polkadot/util');

const COUNCIL_THRESHOLD = 2; // council = Alice + Bob on local3; half-council gate
const WEIGHT_BOUND = { refTime: '120000000000', proofSize: '2000000' };
const POLL_INTERVAL_MS = 400;
const INCLUSION_TIMEOUT_MS = 120_000;
const BOND = BigInt(process.env.X3_ORDERING_BOND || '10000000000000'); // 10 DOLLARS
const COMMITMENT_DOMAIN = Buffer.from('X3:FAIR_ORDER:V1', 'utf8');

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/// Byte arguments (`Vec<u8>` / `[u8; N]` in the metadata) are passed as hex strings.
///
/// A Node `Buffer` is a `Uint8Array`, and the client's `Bytes` codec reads one as if its contents
/// were already length-prefixed — measured: a 10-byte plaintext was decoded as a 190 MB `Bytes`
/// and refused. The runtime-upgrade driver passes `system.set_code`'s blob the same way.
const hex = (value) => `0x${Buffer.from(value).toString('hex')}`;

const evidence = { status: 'started', steps: [], settled_order: null, recomputed: null };
function flush() {
  if (process.env.X3_OUT_JSON) {
    fs.writeFileSync(process.env.X3_OUT_JSON, JSON.stringify(evidence, null, 2));
  }
}
function record(label, extra = {}) {
  evidence.steps.push({ label, ...extra });
  console.log(`[ordering] ${label}${extra.detail ? ` — ${extra.detail}` : ''}`);
}
function die(message) {
  evidence.status = 'failed';
  evidence.error = message;
  flush();
  console.error(`[ordering] FAIL: ${message}`);
  process.exit(1);
}

function requireEnv(name) {
  const value = process.env[name];
  if (!value) die(`${name} is required`);
  return value;
}

let api;
const keyring = {};

/// The 20-byte label a commitment binds for an account: `H160(blake2_256(SCALE(account))[..20])`.
/// An `AccountId32` SCALE-encodes as its own 32 bytes, so this is the address in bytes.
function senderLabel(address) {
  const accountId = new Keyring({ type: 'sr25519' }).decodeAddress(address);
  return Buffer.from(blake2AsU8a(accountId, 256)).subarray(0, 20);
}

/// `commitment_hash(sender, plaintext, nonce)` exactly as `crates/x3-order-window` computes it.
function commitmentHash(address, plaintext, nonce) {
  const buffer = Buffer.concat([
    COMMITMENT_DOMAIN,
    Buffer.from(plaintext, 'utf8'),
    Buffer.from(nonce),
    senderLabel(address),
  ]);
  return Buffer.from(blake2AsU8a(buffer, 256));
}

/// `order_key(Some(beacon), hash)` — the key the lane sorts a settled window by.
///
/// Both arguments arrive as `0x…` hex from storage, so they are decoded to bytes first: hashing the
/// *text* of a hex string produces a different (and consistent-looking) key, which is how the first
/// run of this drill "recomputed" the exact reverse of the chain's order.
function orderKey(beacon, commitHash) {
  return Buffer.from(blake2AsU8a(Buffer.concat([hexToU8a(beacon), hexToU8a(commitHash)]), 256));
}

function describeDispatchResult(result) {
  if (result.isOk) return 'Ok';
  const error = result.asErr;
  if (error?.isModule) {
    try {
      const decoded = api.registry.findMetaError(error.asModule);
      return `${decoded.section}.${decoded.name}`;
    } catch (_e) {
      /* fall through */
    }
  }
  return error ? error.toString() : result.toString();
}

async function waitForExpectedEvent(label, startBlock, isExpected, timeoutMs = INCLUSION_TIMEOUT_MS) {
  const deadline = Date.now() + timeoutMs;
  let next = startBlock + 1;
  while (Date.now() < deadline) {
    const best = (await api.rpc.chain.getHeader()).number.toNumber();
    while (next <= best) {
      const blockHash = await api.rpc.chain.getBlockHash(next);
      const records = await api.query.system.events.at(blockHash);
      const failed = records.find((record) => api.events.system.ExtrinsicFailed.is(record.event));
      if (failed) {
        return {
          blockNumber: next,
          dispatchError: describeDispatchError(failed.event.data),
        };
      }
      const found = records.find((record) => isExpected(record.event));
      if (found) return { blockNumber: next, event: found.event };
      next += 1;
    }
    await sleep(POLL_INTERVAL_MS);
  }
  throw new Error(`${label}: expected event was not observed within ${timeoutMs} ms`);
}

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

/// Submit and require the call's own event. Returns the observed block.
async function submitAndConfirm(extrinsic, signer, label, isExpected) {
  const signed = await extrinsic.signAsync(signer);
  const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  await api.rpc.author.submitExtrinsic(signed);
  const observed = await waitForExpectedEvent(label, startBlock, isExpected);
  if (observed.dispatchError) {
    throw new Error(`${label}: dispatch failed — ${observed.dispatchError}`);
  }
  return observed;
}

/// Submit and *require* a refusal, naming it. This chain puts failed dispatches in blocks, so the
/// absence of the success event is the point.
async function submitExpectingRefusal(extrinsic, signer, label) {
  const signed = await extrinsic.signAsync(signer);
  const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  await api.rpc.author.submitExtrinsic(signed);
  const observed = await waitForExpectedEvent(
    label,
    startBlock,
    (event) => api.events.system.ExtrinsicFailed.is(event),
  );
  if (!observed.dispatchError) {
    throw new Error(`${label}: expected a refusal, the chain accepted it`);
  }
  return observed.dispatchError;
}

/// Execute `call` as a council motion (propose, both approve, close) — the dispatch origin the
/// runtime sees is `Council(Members)`, which satisfies `EnsureRootOrHalfCouncil`.
async function councilDispatch(call, label) {
  const lengthBound = call.toU8a().length;
  const proposed = await submitAndConfirm(
    api.tx.council.propose(COUNCIL_THRESHOLD, call, lengthBound),
    keyring.alice,
    `${label}: council propose`,
    (event) => api.events.council.Proposed.is(event),
  );
  const proposalIndex = proposed.event.data[1].toNumber();
  const proposalHash = proposed.event.data[2].toHex();

  await submitAndConfirm(
    api.tx.council.vote(proposalHash, proposalIndex, true),
    keyring.bob,
    `${label}: council vote Bob`,
    (event) => api.events.council.Voted.is(event),
  );
  await submitAndConfirm(
    api.tx.council.vote(proposalHash, proposalIndex, true),
    keyring.alice,
    `${label}: council vote Alice`,
    (event) => api.events.council.Voted.is(event),
  );
  const closed = await submitAndConfirm(
    api.tx.council.close(proposalHash, proposalIndex, WEIGHT_BOUND, lengthBound),
    keyring.alice,
    `${label}: council close`,
    (event) => api.events.council.Executed.is(event),
  );
  const result = closed.event.data[1];
  if (!result.isOk) {
    throw new Error(`${label}: motion executed but the dispatch failed — ${describeDispatchResult(result)}`);
  }
  record(`${label}: motion executed`, { detail: `block=${closed.blockNumber}` });
}

/// Wait until the chain has produced at least `count` blocks past `from`.
async function advanceBlocks(from, count) {
  const target = from + count;
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    const best = (await api.rpc.chain.getHeader()).number.toNumber();
    if (best >= target) return best;
    await sleep(POLL_INTERVAL_MS);
  }
  throw new Error(`the chain did not reach block ${target} within 120 s`);
}

async function main() {
  const wsUrl = requireEnv('X3_WS_URL');
  const provider = new WsProvider(wsUrl, 1_000);
  api = await ApiPromise.create({ provider, noInitWarn: true });
  evidence.chain = (await api.rpc.system.chain()).toString();

  const sr = new Keyring({ type: 'sr25519' });
  keyring.alice = sr.addFromUri('//Alice');
  keyring.bob = sr.addFromUri('//Bob');

  // The pallet and its window extrinsics must actually be in this runtime's metadata.
  const pe = api.tx.privateExecution;
  for (const name of [
    'setOrderingWindowsEnabled',
    'openOrderingWindow',
    'commitOrdering',
    'revealOrdering',
    'settleOrderingWindow',
    'installOrderingBeacon',
  ]) {
    if (!pe?.[name]) die(`privateExecution.${name} is missing from metadata`);
  }

  const enabledBefore = await api.query.privateExecution.orderingWindowsEnabled();
  record('read the window switch', { detail: `enabled=${enabledBefore.toHuman()}` });

  // An operator turns the lane on through governance, not with a key.
  if (!enabledBefore.isTrue) {
    await councilDispatch(pe.setOrderingWindowsEnabled(true), 'enable ordering windows');
  }
  const enabledAfter = await api.query.privateExecution.orderingWindowsEnabled();
  if (!enabledAfter.isTrue) die('the switch is still off after the motion');
  record('the ordering lane is enabled');

  const now = (await api.rpc.chain.getHeader()).number.toNumber();
  // Each governance/extrinsic step below costs two or three blocks (submit, then wait for the
  // event), and the window must still be open when the reveals land — the first run of this
  // drill used eight blocks and every reveal was refused with `OrderingWindowNotOpen`, which
  // is a refusal for the wrong reason.
  const openBlock = now + 1;
  const closeBlock = now + 40;

  const opened = await submitAndConfirm(
    pe.openOrderingWindow(openBlock, closeBlock),
    keyring.alice,
    'open a window',
    (event) => api.events.privateExecution.OrderingWindowOpened.is(event),
  );
  const windowId = opened.event.data[0].toNumber();
  record('window opened', { detail: `id=${windowId} range=${openBlock}..=${closeBlock}` });

  // Two participants, each with its own plaintext and nonce, committed in the reverse of the
  // order they will settle in (the settle order is the key order, not arrival).
  const participants = [
    { who: 'alice', plaintext: 'alpha-swap', nonce: new Uint8Array(32).fill(1) },
    { who: 'bob', plaintext: 'beta-swap', nonce: new Uint8Array(32).fill(2) },
  ].map((p) => ({
    ...p,
    label: senderLabel(keyring[p.who].address),
    hash: commitmentHash(keyring[p.who].address, p.plaintext, p.nonce),
  }));

  for (const p of participants) {
    const committed = await submitAndConfirm(
      pe.commitOrdering(windowId, hex(p.hash), BOND),
      keyring[p.who],
      `commit from ${p.who}`,
      (event) => api.events.privateExecution.OrderingCommitted.is(event),
    );
    record(`committed from ${p.who}`, { detail: `block=${committed.blockNumber}` });
  }

  // A reveal that does not hash to its commit must be refused, and refused *for that reason*: any
  // refusal would satisfy a looser check, so this one names the binding it is testing.
  const refusal = await submitExpectingRefusal(
    pe.revealOrdering(windowId, hex(participants[0].hash), hex('not-what-was-committed'), hex(participants[0].nonce)),
    keyring.alice,
    'a reveal that does not match its commit',
  );
  if (refusal !== 'privateExecution.OrderingRevealMismatch') {
    die(`the mismatched reveal was refused for the wrong reason: ${refusal}`);
  }
  record('a reveal that does not hash to its commit was refused', { detail: refusal });

  for (const p of participants) {
    await submitAndConfirm(
      pe.revealOrdering(windowId, hex(p.hash), hex(p.plaintext), hex(p.nonce)),
      keyring[p.who],
      `reveal from ${p.who}`,
      (event) => api.events.privateExecution.OrderingRevealed.is(event),
    );
  }
  record('both reveals accepted at their own blocks');

  // Settle needs the window closed *and* the beacon block produced: the beacon is
  // `BlockHash(close_block + 1)`, so wait for `close_block + 2`.
  await advanceBlocks(now, closeBlock - now + 2);
  const settled = await submitAndConfirm(
    pe.settleOrderingWindow(windowId),
    keyring.alice,
    'settle the window',
    (event) => api.events.privateExecution.OrderingWindowSettled.is(event),
  );
  record('window settled', { detail: `block=${settled.blockNumber}` });

  const record_ = await api.query.privateExecution.orderingSettlements(windowId);
  if (record_.isNone) die('the settled window has no settlement record');
  const settlement = record_.unwrap();
  const beacon = settlement.beacon.unwrap().toHex();
  const ordered = settlement.ordered.map((hash) => hash.toHex());
  evidence.settled_order = ordered;
  evidence.beacon = beacon;

  // Recompute the order from the beacon the chain stored, and require the two to agree. This is
  // the property the pallet asserts internally; here it is checked from outside, over RPC.
  const expected = participants
    .map((p) => ({
      key: orderKey(beacon, `0x${Buffer.from(p.hash).toString('hex')}`),
      hash: `0x${Buffer.from(p.hash).toString('hex')}`,
    }))
    .sort((a, b) => Buffer.compare(a.key, b.key))
    .map((entry) => entry.hash);
  evidence.recomputed = expected;

  if (JSON.stringify(ordered) !== JSON.stringify(expected)) {
    die(`the settled order is not the canonical order:\n  chain      ${ordered}\n  recomputed ${expected}`);
  }
  record('the settled order equals the order recomputed from the stored beacon');

  evidence.status = 'passed';
  flush();
  await api.disconnect();
  console.log('[ordering] PASS');
}

main().catch((error) => die(error && error.message ? error.message : String(error)));
