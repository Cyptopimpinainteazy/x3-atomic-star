# The on-chain interpreter answered 44 opcodes it could not execute

Date: 2026-09-26. Tree state at start: `0d903e8b1 chore(matrix): the ordering window's chain path
carries its evidence`.

## The finding

`crates/x3-integration::mini_x3` is the interpreter a block runs. `node/src/service.rs` builds the
executor with `sc_service::new_wasm_executor`, so there is no native runtime execution to fall back
on: the runtime's `x3-x3-integration/std` feature is off in the wasm build, and the `no_std` arm of
`X3Executor::execute` is the one `pallets/x3-kernel` reaches through `X3VmAdapter`.

That interpreter held a block of arms that decoded their operands, wrote a placeholder into the
destination register and moved on. Measured, byte by byte, with the compiler's own operand widths:

```
0x14 / 0x15  LoadIndex / StoreIndex      dst = I64(0)
0x16 / 0x17  LoadField / StoreField      dst = I64(0)
0x70         NewArray                    silent no-op
0x71         ArrayLen                    dst = I64(0)
0x72 / 0x73  ArrayPush / ArrayPop        silent no-op
0x74         NewTuple                    skip
0x75         TupleGet                    dst = Unit
0x80         CtxSender                   dst = 20 zero bytes
0x81         CtxBlockHeight              dst = I64(0)
0x82         CtxTimestamp                dst = I64(0)
0x83         CtxValue                    dst = I64(0)
0x85         CtxChainId                  dst = I64(3375)
0x90..0x92   AtomicBegin/Commit/Rollback tracked and skipped, so a failed window still committed
0x93         AtomicCheck                 dst = Bool(false), always
0xA0         AgentSelf                   dst = Unit
0xA1         AgentInit                   skip
0xA2         Emit                        event dropped
0xB0..0xB9   EVM intrinsics              dst = I64(0), skip 6 bytes
0xC0..0xC7   SVM intrinsics              dst = I64(0), skip 6 bytes
0xD0..0xD7   GPU intrinsics              dst = I64(0), skip 6 bytes
```

`AGENTS.md` §5 (never turn UNKNOWN into SUCCESS), §18 (an accelerator must never be accepted merely
because it returned) and §4 (no mock production infrastructure) each name a violation here. The
six-byte skip was also the wrong width for most of the intrinsics — `EvmSstore` is three bytes,
`SvmCreateAccount` seven — so a program containing one could have its next instruction decoded from
the middle of the previous instruction's operands.

## The fix

`crates/x3-integration/src/mini_x3.rs`:

* every opcode above is now refused with `X3Error::UnsupportedOpcode(byte)`, which names the opcode
  rather than the payload;
* the atomic window is implemented for real instead of skipped — `AtomicBegin` snapshots the
  module's globals, `AtomicCommit` discards the snapshot, `AtomicRollback` restores it and aborts
  the execution with `AtomicAborted` (mirroring `crates/x3-vm/src/vm.rs`), and `AtomicCheck` answers
  whether a window is open. Globals are the only state this interpreter has, so the guarantee is
  small but it is now complete rather than absent;
* atomic nesting is bounded by `MAX_ATOMIC_DEPTH = 32`, because each open window holds a copy of the
  globals and a loop of `AtomicBegin` would otherwise be a memory amplification a program can ask
  the chain to perform.

New tests: `crates/x3-integration/tests/mini_x3_fail_closed.rs` (5 tests, one table of 44 bytes) and
`crates/x3-integration/tests/interpreter_agreement.rs` (3 tests, holding both interpreters to "no
value for these opcodes" across the same table, plus the one divergence that remains).

## Commands and results

Break-it-first, before the fix, with the whole table exercised:

```
cargo test -p x3-x3-integration --test mini_x3_fail_closed
    test result: FAILED. 1 passed; 4 failed
    44 of 44 instructions were answered ... (the assertion lists every one)
```

Control, run after the fix by restoring the four fabricated arm families one at a time:

```
# aggregate / context / agent-emit arms restored
18 of 44 instructions were answered rather than refused:
  load_index (0x14): answered Unit instead of refusing
  store_index (0x15): answered Unit instead of refusing
  ...
  emit (0xA2): answered Unit instead of refusing
test result: FAILED. 4 passed; 1 failed

# cross-vm / gpu intrinsic arm restored
26 of 44 instructions were answered rather than refused:
  intrinsic (0xB0): answered Unit instead of refusing
  ...
  intrinsic (0xD7): answered Unit instead of refusing
test result: FAILED. 4 passed; 1 failed
```

18 + 26 = 44 of 44. Both controls were reverted; the arms are refusals in the committed tree.

Green, after the fix:

```
cargo test -p x3-x3-integration --features compile
    13 passed (lib) / 6 passed (bc_const_pool_parity) / 5 passed (bytecode_robustness)
    8 passed (bytecode_version_compat) / 6 passed (compiler_bridge) / 3 passed (gas_accounting)
    3 passed (interpreter_agreement) / 5 passed (mini_x3_fail_closed)
    0 failed, 0 ignored anywhere

cargo test -p x3-vm
    165 passed / 8 passed / 0 failed

cargo test -p pallet-x3-kernel
    225 passed / 2 passed / 0 failed

cargo check -p x3-x3-integration --no-default-features --target wasm32-unknown-unknown
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.51s    (the runtime path builds)

cargo check -p x3-chain-runtime --features std
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 16s

cargo clippy -p x3-x3-integration --all-targets
    no warnings
```

The compiler's own fixture corpus still executes to the value its source states in both engines
(`compiler_bridge.rs`), and the seed data shows the refusals are not vacuous: a scalar program in the
same harness runs in both engines.

## What the fix does not do

* `crates/x3-vm` implements `evm_sstore`/`evm_sload` and the runtime interpreter refuses them, so
  X3VM contract storage is still off-chain-only. `interpreter_agreement.rs` pins both halves of that
  in one test; the row is `X3-MEV-004` and the ticket is TICKET-147.
* The `std` VM still has no arm for the aggregate family, `inc`/`dec`, `mod_f`, the numeric
  conversions, `ctx_gas` and `agent_check` — so a program using them runs on chain and fails
  off-chain. That direction is fail-closed on both sides (nothing is fabricated), but it is a
  divergence and TICKET-149 records it.
* The verifier still admits every one of these opcodes at intake (`VerifyOptions::on_chain` denies
  floats, not intrinsics), so refusal happens at execution rather than at validation. Fail-closed
  either way, but later than it needs to be.
