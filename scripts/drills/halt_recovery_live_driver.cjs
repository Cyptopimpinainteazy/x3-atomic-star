#!/usr/bin/env node
'use strict';
//
// Trip the constitutional halt on a live chain and clear it again, through real extrinsics.
//
// Why this exists: three P0 rows (`X3-RT-001`, `X3-RT-006`, `X3-RT-007`) and `TICKET-153` all said
// the same thing — the economic halt had never been tripped on a network, so every claim about it
// was a single-process claim. Doing it found a P0 the unit tests could not: the halt refused every
// signed extrinsic, including the council motion that is the only route to its own remedy, so a
// tripped halt could not be undone by any transaction the chain accepts.
//
// The assertions here are deliberately state- and pool-level, not event-level:
//
//   * the transfer submitted while halted must be refused with the runtime's halt code, by the
//     pool of *two different validators*, and the destination balance must not move;
//   * the council motion that clears the halt must be submittable *while still halted* — that is
//     the property the fix added, and without it `api.rpc.author.submitExtrinsic` throws;
//   * after the remedy, both flags must read false and a transfer must land and move the balance.
//
// Inputs (environment):
//   X3_WS_URL     websocket endpoint of a validator on the chain
//   X3_WS_URL_B   optional second validator, used to show the refusal is chain state, not a
//                 node-local switch
//   X3_OUT_JSON   optional path for the machine-readable evidence

const fs = require('fs');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');

const POLL_INTERVAL_MS = 400;
const INCLUSION_TIMEOUT_MS = 120_000;
// The local3 council is Alice and Bob, and the runtime's half-council gate is
// `EnsureProportionAtLeast<_, _, 1, 2>`. Proposing with threshold 2 means the motion only closes
// once both have approved, so the gate is met with a full majority rather than by how non-voters
// are counted.
const COUNCIL_THRESHOLD = 2;
// `council.close`'s weight bound is a `Weight`, not a number — passing a number makes
// `createType` fail before the extrinsic is even built. Same bound the governance driver uses.
const WEIGHT_BOUND = { refTime: '120000000000', proofSize: '2000000' };
// `pallet_x3_invariants::INVARIANT_HALT_CODE`.
const HALT_CODE = 1;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const evidence = { status: 'started', steps: [] };
function flush() {
  if (process.env.X3_OUT_JSON) {
    fs.writeFileSync(process.env.X3_OUT_JSON, JSON.stringify(evidence, null, 2));
  }
}
function record(label, extra = {}) {
  evidence.steps.push({ label, ...extra });
  console.log(`[halt] ${label}${extra.detail ? ` — ${extra.detail}` : ''}`);
}
function die(message) {
  evidence.status = 'failed';
  evidence.error = message;
  flush();
  console.error(`[halt] FAIL: ${message}`);
  process.exit(1);
}
function requireEnv(name) {
  const value = process.env[name];
  if (!value) die(`${name} is required`);
  return value;
}

let api;
const keyring = {};

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

async function waitFor(label, startBlock, predicate, timeoutMs = INCLUSION_TIMEOUT_MS) {
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

/// Execute `call` as a council motion: propose, both members approve, close.
///
/// While the halt is set this is the only route to `clear_halted` (and on a mainnet-rc1 chain it is
/// the only route at all, since there is no sudo). Every extrinsic below is pool-gated by the halt,
/// so if the exemption list regresses this function throws at `submitExtrinsic` instead of failing
/// silently later.
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
    throw new Error(
      `${label}: council motion executed but the dispatch failed — ${describeDispatchError([result])}`,
    );
  }
  record(`${label}: council motion executed`, {
    detail: `index=${proposalIndex} hash=${proposalHash} block=${closed.blockNumber}`,
    proposalIndex,
    proposalHash,
    blockNumber: closed.blockNumber,
  });
  return closed;
}

/// Submit a transfer that the halt must refuse, and require the refusal to come from the pool.
///
/// A pool that rejects returns an error from `author_submitExtrinsic` whose text carries the
/// runtime's `InvalidTransaction::Custom(1)`. Anything else — accepted here, or refused for a
/// different reason — fails the drill.
async function poolMustRefuse(connection, extrinsic, signer, label) {
  const signed = await extrinsic.signAsync(signer);
  try {
    await connection.rpc.author.submitExtrinsic(signed);
  } catch (error) {
    const message = String(error?.message || error);
    if (!message.includes('1010') && !message.includes(`Custom error: ${HALT_CODE}`)) {
      throw new Error(`${label}: refused, but not by the halt — ${message.slice(0, 300)}`);
    }
    record(`${label}: refused by the pool`, { detail: message.slice(0, 200) });
    return message;
  }
  throw new Error(`${label}: the pool accepted an extrinsic the halt must refuse`);
}

async function freeBalance(address) {
  const account = await api.query.system.account(address);
  return BigInt(account.data.free.toString());
}

/// Submit an extrinsic that the *pallet* must refuse, and require the refusal to be a dispatch
/// error rather than a pool rejection.
///
/// This is the distinction that matters for the recovery path: `rollback_atomic_bundle` must get
/// past the halt gate and reach its own logic. If the halt refused it, the pool would throw
/// `Custom error: 1` and the extrinsic would never be included; here it must be included and fail
/// inside the pallet (a zero bundle id does not exist).
async function reachThePallet(extrinsic, signer, label, expectedName) {
  const signed = await extrinsic.signAsync(signer);
  const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  await api.rpc.author.submitExtrinsic(signed);
  const observed = await waitFor(
    label,
    startBlock,
    (event) => api.events.system.ExtrinsicFailed.is(event),
  );
  if (!observed.dispatchError) throw new Error(`${label}: the pallet accepted it`);
  if (observed.dispatchError !== expectedName) {
    throw new Error(`${label}: refused for the wrong reason — ${observed.dispatchError}`);
  }
  record(`${label}: reached the pallet while halted`, {
    detail: `included at block ${observed.blockNumber} and refused by the pallet (${observed.dispatchError}), so the halt did not refuse it`,
  });
  return observed;
}

async function main() {
  const wsUrl = requireEnv('X3_WS_URL');
  const provider = new WsProvider(wsUrl, 1_000);
  api = await ApiPromise.create({ provider, noInitWarn: true });
  evidence.chain = (await api.rpc.system.chain()).toString();

  const sr = new Keyring({ type: 'sr25519' });
  keyring.alice = sr.addFromUri('//Alice');
  keyring.bob = sr.addFromUri('//Bob');
  keyring.charlie = sr.addFromUri('//Charlie');

  // A second connection is optional but is what separates "chain state" from "a node setting".
  let second;
  if (process.env.X3_WS_URL_B) {
    second = await ApiPromise.create({ provider: new WsProvider(process.env.X3_WS_URL_B, 1_000), noInitWarn: true });
  }

  const kernel = api.tx.atlasKernel;
  const atomicKernel = api.tx.x3AtomicKernel;
  const invariants = api.tx.x3Invariants;
  const ledger = api.tx.x3SupplyLedger;
  if (!kernel?.emergencyHalt) die('atlasKernel.emergencyHalt is missing from metadata');
  if (!atomicKernel?.rollbackAtomicBundle) {
    die('x3AtomicKernel.rollbackAtomicBundle is missing from metadata');
  }
  if (!invariants?.clearHalted) die('x3Invariants.clearHalted is missing from metadata');
  if (!ledger?.resumeTransfers) die('x3SupplyLedger.resumeTransfers is missing from metadata');
  if (!api.tx.council?.propose || !api.tx.council?.vote || !api.tx.council?.close) {
    die('council propose/vote/close missing from metadata; the only route to the remedy is gone');
  }

  const members = (await api.query.council.members()).map((who) => who.toString());
  for (const who of [keyring.alice.address, keyring.bob.address]) {
    if (!members.includes(who)) {
      die(`the council is ${members.join(', ')} — the drill needs Alice and Bob as members`);
    }
  }
  record('council membership', { detail: `${members.length} members: ${members.join(', ')}` });

  const amount = 1_000_000_000n; // 0.001 X3 at 12 decimals; the dev accounts hold plenty.
  const bobBefore = await freeBalance(keyring.bob.address);

  // ── control: traffic works before the halt ─────────────────────────────────────────────────
  const control = await submitAndConfirm(
    api.tx.balances.transferKeepAlive(keyring.bob.address, amount),
    keyring.alice,
    'control transfer before the halt',
    (event) => api.events.balances.Transfer.is(event),
  );
  const bobAfterControl = await freeBalance(keyring.bob.address);
  if (bobAfterControl - bobBefore !== amount) {
    die(`the control transfer did not move the balance (${bobBefore} -> ${bobAfterControl})`);
  }
  record('control transfer landed', { detail: `block ${control.blockNumber}, +${amount}` });

  // ── trip the halt, through the only route this chain has ───────────────────────────────────
  await councilDispatch(kernel.emergencyHalt(), 'trip the emergency halt');

  const halted = await api.query.x3Invariants.halted();
  const transfersHalted = await api.query.x3SupplyLedger.transferHalted();
  if (!halted.isTrue) die('x3Invariants.halted is false after emergency_halt');
  if (!transfersHalted.isTrue) die('x3SupplyLedger.transferHalted is false after emergency_halt');
  record('both halt flags are set', { detail: 'x3Invariants.halted, x3SupplyLedger.transferHalted' });

  // ── a user call must be refused, by the pool, on more than one validator ───────────────────
  await poolMustRefuse(
    api,
    api.tx.balances.transferKeepAlive(keyring.bob.address, amount),
    keyring.alice,
    'a transfer while halted (node A)',
  );
  if (second) {
    await poolMustRefuse(
      second,
      second.tx.balances.transferKeepAlive(keyring.bob.address, amount),
      keyring.alice,
      'a transfer while halted (node B)',
    );
    record('a second validator refuses it for the same reason', {
      detail: 'the halt is consensus state, not a node-local switch',
    });
  }
  const bobWhileHalted = await freeBalance(keyring.bob.address);
  if (bobWhileHalted !== bobAfterControl) {
    die(`a refused transfer still moved the balance (${bobAfterControl} -> ${bobWhileHalted})`);
  }
  record('the refused transfer moved nothing', { detail: `balance still ${bobWhileHalted}` });

  // The bond-releasing rollback is the call the halt exists to leave open. It must get *past* the
  // gate: what stops it here is the pallet (a zero bundle id does not exist), not the halt.
  await reachThePallet(
    atomicKernel.rollbackAtomicBundle(
      '0x0000000000000000000000000000000000000000000000000000000000000000',
      'SubmitterCancelled',
    ),
    keyring.alice,
    'the bond-releasing rollback while halted',
    'x3AtomicKernel.BundleNotFound',
  );

  // ── the remedy, submitted while still halted ──────────────────────────────────────────────
  await councilDispatch(invariants.clearHalted(), 'clear the halt');
  await councilDispatch(ledger.resumeTransfers(), 'resume transfers');

  const haltedAfter = await api.query.x3Invariants.halted();
  const transfersHaltedAfter = await api.query.x3SupplyLedger.transferHalted();
  if (haltedAfter.isTrue) die('x3Invariants.halted is still set after the remedy');
  if (transfersHaltedAfter.isTrue) die('x3SupplyLedger.transferHalted is still set after the remedy');
  record('both halt flags are cleared', { detail: 'cleared by council motion, not by a restart' });

  // ── and traffic works again ───────────────────────────────────────────────────────────────
  const resumed = await submitAndConfirm(
    api.tx.balances.transferKeepAlive(keyring.bob.address, amount),
    keyring.alice,
    'transfer after the remedy',
    (event) => api.events.balances.Transfer.is(event),
  );
  const bobAfterResume = await freeBalance(keyring.bob.address);
  if (bobAfterResume - bobAfterControl !== amount) {
    die(`traffic did not resume (${bobAfterControl} -> ${bobAfterResume})`);
  }
  record('traffic resumed', { detail: `block ${resumed.blockNumber}, +${amount}` });

  evidence.status = 'passed';
  flush();
  console.log('[halt] PASS');
  await provider.disconnect();
  if (second) await second.disconnect();
  process.exit(0);
}

main().catch((error) => die(error?.stack || String(error)));
