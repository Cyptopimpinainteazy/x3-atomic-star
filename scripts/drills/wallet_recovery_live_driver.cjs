#!/usr/bin/env node
'use strict';
//
// The wallet pallet's biometric registration and social recovery, driven on a live chain.
//
// `pallets/x3-wallet-pallet` gained a real recovery lifecycle on 2026-09-26 (`d85f79c85`):
// guardian registration validated by the `x3-wallet` library, a guardian-initiated request carrying
// a delay, per-guardian approvals with a threshold, and a finalize that is the only place the
// stored recovery owner changes. Every proof of it was a unit test against the pallet's mock, and
// its own earlier test had to seed a guardian record straight into storage because no extrinsic
// created one.
//
// This drives the whole thing through the real extrinsics on a booted chain, including the refusals
// that matter: an unsupported biometric type, a finalize before the delay, and a cancel from
// someone who is not the recovery owner. The final assertion is the state change — the stored
// recovery owner after finalize — not the event.
//
// Inputs (environment):
//   X3_WS_URL    websocket endpoint of a node on the chain
//   X3_OUT_JSON  optional path for the machine-readable evidence

const fs = require('fs');
const { ApiPromise, WsProvider } = require('@polkadot/api');
const { Keyring } = require('@polkadot/keyring');
const { blake2AsU8a } = require('@polkadot/util-crypto');

const POLL_INTERVAL_MS = 400;
const INCLUSION_TIMEOUT_MS = 120_000;
// Long enough that the approvals cannot outrun it: each extrinsic costs two or three blocks, and
// with a three-block delay the "early" finalize below arrived after the delay had already elapsed —
// it succeeded, which is correct behaviour and a broken test.
const RECOVERY_DELAY_BLOCKS = 20;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const evidence = { status: 'started', steps: [] };
function flush() {
  if (process.env.X3_OUT_JSON) {
    fs.writeFileSync(process.env.X3_OUT_JSON, JSON.stringify(evidence, null, 2));
  }
}
function record(label, extra = {}) {
  evidence.steps.push({ label, ...extra });
  console.log(`[wallet] ${label}${extra.detail ? ` — ${extra.detail}` : ''}`);
}
function die(message) {
  evidence.status = 'failed';
  evidence.error = message;
  flush();
  console.error(`[wallet] FAIL: ${message}`);
  process.exit(1);
}
function requireEnv(name) {
  const value = process.env[name];
  if (!value) die(`${name} is required`);
  return value;
}

const hex = (value) => `0x${Buffer.from(value).toString('hex')}`;

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

async function submitExpectingRefusal(extrinsic, signer, label, expectedName) {
  const signed = await extrinsic.signAsync(signer);
  const startBlock = (await api.rpc.chain.getHeader()).number.toNumber();
  await api.rpc.author.submitExtrinsic(signed);
  const observed = await waitFor(
    label,
    startBlock,
    (event) => api.events.system.ExtrinsicFailed.is(event),
  );
  if (!observed.dispatchError) throw new Error(`${label}: expected a refusal, the chain accepted it`);
  if (expectedName && observed.dispatchError !== expectedName) {
    throw new Error(`${label}: refused for the wrong reason — ${observed.dispatchError}`);
  }
  return observed.dispatchError;
}

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

/// The 32-byte label this pallet uses for an account in the `x3-wallet` managers: the encoded
/// account's first 32 bytes (an `AccountId32` encodes as itself).
function accountBytes(address) {
  return Buffer.from(new Keyring({ type: 'sr25519' }).decodeAddress(address)).subarray(0, 32);
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

  const wallet = api.tx.x3WalletPallet ?? api.tx.x3Wallet;
  if (!wallet) die('the wallet pallet is not in this runtime metadata');
  for (const name of [
    'registerBiometric',
    'registerRecoveryGuardians',
    'initiateRecovery',
    'approveRecovery',
    'finalizeRecovery',
    'cancelRecovery',
  ]) {
    if (!wallet[name]) die(`wallet.${name} is missing from metadata`);
  }

  // ── biometric registration ──────────────────────────────────────────────────────────────────
  await submitAndConfirm(
    wallet.registerBiometric(1, hex(Buffer.alloc(32, 1)), hex(Buffer.alloc(32, 2))),
    keyring.alice,
    'register a biometric profile',
    (event) => api.events.x3WalletPallet.BiometricProfileCreated.is(event),
  );
  const profile = await api.query.x3WalletPallet.biometricProfiles(keyring.alice.address);
  if (profile.isNone) die('the profile was not stored');
  const stored = profile.unwrap();
  if (hex(accountBytes(keyring.alice.address)) !== stored.owner.toHex()) {
    die(`the profile's owner is ${stored.owner.toHex()}, not the signer`);
  }
  record('the profile names its signer as owner', { detail: stored.owner.toHex() });

  const refusal = await submitExpectingRefusal(
    wallet.registerBiometric(3, hex(Buffer.alloc(32, 1)), hex(Buffer.alloc(32, 2))),
    keyring.alice,
    'an unsupported biometric type',
    'x3WalletPallet.InvalidBiometricType',
  );
  record('an unsupported biometric type was refused', { detail: refusal });

  // ── recovery: register guardians, request, approve, finalize ────────────────────────────────
  const guardianKeys = [accountBytes(keyring.bob.address), accountBytes(keyring.charlie.address)];
  await submitAndConfirm(
    wallet.registerRecoveryGuardians(guardianKeys.map(hex), 2, RECOVERY_DELAY_BLOCKS),
    keyring.alice,
    'register two guardians (threshold 2)',
    (event) => api.events.x3WalletPallet.RecoveryGuardiansRegistered.is(event),
  );

  const aliceBytes = accountBytes(keyring.alice.address);
  const newOwner = accountBytes(keyring.bob.address);
  const initiated = await submitAndConfirm(
    wallet.initiateRecovery(keyring.alice.address, hex(newOwner)),
    keyring.bob,
    'a guardian starts recovery',
    (event) => api.events.x3WalletPallet.RecoveryInitiated.is(event),
  );
  const executableBlock = initiated.event.data[3].toNumber();
  record('the request records the delay', { detail: `executable at block ${executableBlock}` });

  await submitAndConfirm(
    wallet.approveRecovery(keyring.alice.address),
    keyring.charlie,
    'the first guardian approves',
    (event) => api.events.x3WalletPallet.RecoveryApproved.is(event),
  );
  const second = await submitAndConfirm(
    wallet.approveRecovery(keyring.alice.address),
    keyring.bob,
    'the second guardian approves (threshold met)',
    (event) => api.events.x3WalletPallet.RecoveryApproved.is(event),
  );
  record('the threshold is met', { detail: `approvals=${second.event.data[2].toNumber()}` });

  // Before the delay: refused, and the owner must not move.
  const early = await submitExpectingRefusal(
    wallet.finalizeRecovery(keyring.alice.address),
    keyring.bob,
    'a finalize before the delay',
    'x3WalletPallet.RecoveryNotReady',
  );
  record('a finalize before the delay was refused', { detail: early });
  const beforeOwner = (await api.query.x3WalletPallet.recoveryAccounts(keyring.alice.address)).unwrap().owner.toHex();
  if (beforeOwner !== hex(aliceBytes)) die(`the owner moved before the delay: ${beforeOwner}`);

  // A caller who is not the recovery owner cannot cancel on the owner's behalf here.
  const cancelRefusal = await submitExpectingRefusal(
    wallet.cancelRecovery(keyring.alice.address),
    keyring.charlie,
    'a cancel from someone who is not the recovery owner',
    'x3WalletPallet.NotRecoveryOwner',
  );
  record('a non-owner cancel was refused', { detail: cancelRefusal });

  await advanceBlocks(executableBlock - RECOVERY_DELAY_BLOCKS, RECOVERY_DELAY_BLOCKS + 2);
  const finalized = await submitAndConfirm(
    wallet.finalizeRecovery(keyring.alice.address),
    keyring.bob,
    'finalize after the delay',
    (event) => api.events.x3WalletPallet.RecoveryExecuted.is(event),
  );
  record('recovery executed', { detail: `block=${finalized.blockNumber}` });

  const afterOwner = (await api.query.x3WalletPallet.recoveryAccounts(keyring.alice.address)).unwrap().owner.toHex();
  if (afterOwner !== hex(newOwner)) {
    die(`the stored recovery owner is ${afterOwner}, expected ${hex(newOwner)}`);
  }
  record('the stored recovery owner changed to the new owner', { detail: afterOwner });

  evidence.status = 'passed';
  flush();
  await api.disconnect();
  console.log('[wallet] PASS');
}

main().catch((error) => die(error && error.message ? error.message : String(error)));
