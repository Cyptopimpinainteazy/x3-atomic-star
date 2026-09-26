//! The two interpreters must refuse the same opcodes.
//!
//! `crates/x3-vm` (reached by tests, tools and simulation) and
//! `crates/x3-integration::mini_x3` (reached by a block — the node builds a `WasmExecutor`, so the
//! runtime executes the `no_std` one) are two implementations of one ISA. `compiler_bridge.rs`
//! already holds them to the same *answer* on the compiler's fixture corpus; this file holds them
//! to the same *refusals*, which is the half a corpus of working programs cannot show.
//!
//! The rule is the fail-closed one: for every opcode in the table below, neither engine may hand the
//! program a value. Either engine completing one of these is a fail-open path — the program receives
//! a result no implementation produced. The engines need not refuse *identically* to satisfy this
//! (the runtime names the opcode, the `std` VM may report a missing hostcall instead); they must
//! refuse. The divergences that remain after that are listed at the bottom of this file, so an
//! opcode only one engine implements is recorded rather than silently averaged away.

#![cfg(feature = "std")]

use x3_backend::{BytecodeModule, FunctionEntry};
use x3_vm::VM;
use x3_x3_integration::mini_x3::{execute_x3bc, X3Error};

/// A module whose `main` is `instruction`, then `Nop`s and a `RetVoid`, written by the real
/// envelope writer.
fn module_with_code(code: &[u8]) -> Vec<u8> {
    let mut module = BytecodeModule::new();
    module.functions.push(FunctionEntry {
        name: "main".to_string(),
        entry_point: 0,
        param_count: 0,
        local_count: 16,
        max_stack: 16,
        return_type_tag: 1,
    });
    module.code = code.to_vec();
    module.to_bytes()
}

/// `instruction`, then `Nop`s and a `RetVoid`, so an engine that skips the instruction reaches a
/// clean end instead of an unrelated `UnexpectedEof`.
fn module_with(instruction: &[u8]) -> Vec<u8> {
    let mut code = instruction.to_vec();
    code.extend_from_slice(&[0x00; 8]);
    code.push(0x06); // RetVoid
    module_with_code(&code)
}

/// Opcodes the compiler can emit and neither interpreter implements, with one reasoning note each.
/// Kept as a table so a divergence shows up as a changed refusal rather than as a silent success.
fn agreed_refusals() -> Vec<(u8, &'static str)> {
    let mut table: Vec<(u8, &'static str)> = vec![
        (0x14, "load_index"),
        (0x15, "store_index"),
        (0x16, "load_field"),
        (0x17, "store_field"),
        (0x70, "new_array"),
        (0x71, "array_len"),
        (0x72, "array_push"),
        (0x73, "array_pop"),
        (0x74, "new_tuple"),
        (0x75, "tuple_get"),
        (0x80, "ctx_sender"),
        (0x81, "ctx_block_height"),
        (0x82, "ctx_timestamp"),
        (0x83, "ctx_value"),
        (0x85, "ctx_chain_id"),
        (0xA0, "agent_self"),
        (0xA1, "agent_init"),
        (0xA2, "emit"),
    ];
    for byte in (0xB0u8..=0xB9).chain(0xC0..=0xC7).chain(0xD0..=0xD7) {
        table.push((byte, "cross-vm / gpu intrinsic"));
    }
    table
}

#[test]
fn neither_interpreter_executes_an_opcode_the_other_one_refuses() {
    let mut disagreements: Vec<String> = Vec::new();

    for (byte, name) in agreed_refusals() {
        let bytes = module_with(&[byte, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        // The runtime interpreter, which is the one a block runs. It has no implementation for any
        // of these, so its refusal must name the opcode.
        match execute_x3bc(&bytes, 100_000) {
            Err(X3Error::UnsupportedOpcode(refused)) if refused == byte => {}
            Err(other) => disagreements.push(format!(
                "{name} (0x{byte:02X}): the runtime interpreter refused it as {other:?} rather than naming the opcode"
            )),
            Ok(result) => disagreements.push(format!(
                "{name} (0x{byte:02X}): the runtime interpreter answered {:?}",
                result.return_val
            )),
        }

        // The `std` interpreter, which tests and tools reach. Whether it refuses because the opcode
        // has no arm or because the host is missing is an implementation detail; answering is not.
        let mut vm = VM::from_bytes(&bytes)
            .unwrap_or_else(|e| panic!("{name} (0x{byte:02X}): the std VM must load it: {e:?}"));
        if let Ok(result) = vm.call_function(0, &[]) {
            disagreements.push(format!(
                "{name} (0x{byte:02X}): the std VM answered {:?}",
                result.value
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "{} of {} agreed-refusal opcodes were executed by one of the engines:\n  {}",
        disagreements.len(),
        agreed_refusals().len(),
        disagreements.join("\n  ")
    );
}

/// The control: the table is not "every opcode refuses". A scalar program that both engines do
/// implement runs in both, so the assertions above are about these opcodes and not about a
/// broken harness.
#[test]
fn both_engines_execute_the_same_supported_program() {
    // `LoadImm r0, 21` + `Ret r0`, with no padding: the program returns what it loaded.
    let bytes = module_with_code(&[0x18, 0x00, 21, 0x05, 0x00]);

    let runtime = execute_x3bc(&bytes, 100_000).expect("the runtime interpreter runs it");
    assert_eq!(
        runtime.return_val,
        x3_x3_integration::mini_x3::MiniValue::I64(21)
    );

    let mut vm = VM::from_bytes(&bytes).expect("the std VM loads it");
    let result = vm.call_function(0, &[]).expect("the std VM runs it");
    assert_eq!(result.value, Some(x3_vm::Value::I64(21)));
}

/// The one place the engines genuinely differ today, pinned so it cannot drift unnoticed.
///
/// `crates/x3-vm` implements `evm_sstore`/`evm_sload` against its own journaled storage, so a
/// contract can carry a slot between calls **off chain**. The runtime interpreter has no storage at
/// all and refuses both opcodes, so the same contract cannot carry one on chain. The test asserts
/// both halves: the `std` VM stores and reads back the value, and the runtime interpreter refuses
/// the very same artifact by name. That is `X3-MEV-004`'s remaining gap expressed as an executable
/// claim rather than as a note.
#[test]
fn slot_storage_is_implemented_off_chain_and_refused_on_chain() {
    // slot = r1 (0), value = r2 (5); store; load back into r3; return r3.
    let bytes = module_with_code(&[
        0x18, 0x01, 0x00, // LoadImm r1, 0
        0x18, 0x02, 0x05, // LoadImm r2, 5
        0xB4, 0x01, 0x02, // EvmSstore slot=r1 val=r2
        0xB3, 0x03, 0x01, // EvmSload dst=r3 slot=r1
        0x05, 0x03, // Ret r3
    ]);

    let mut vm = VM::from_bytes(&bytes).expect("the std VM loads it");
    let result = vm
        .call_function(0, &[])
        .expect("the std VM implements slot storage");
    assert_eq!(
        result.value,
        Some(x3_vm::Value::I64(5)),
        "the std VM must read back what it stored"
    );

    assert_eq!(
        execute_x3bc(&bytes, 100_000).unwrap_err(),
        X3Error::UnsupportedOpcode(0xB4),
        "the runtime interpreter must refuse the store rather than answer it"
    );
}

// Known remaining divergences, none of which is a fabricated value:
//
// * `0x26`/`0x27` (`inc`/`dec`), `0x34` (`mod_f`), the numeric conversions `0x60`-`0x68`, and
//   `0x84` (`ctx_gas`) execute in the runtime interpreter and are refused by the `std` one. The
//   runtime is the stricter-fail direction of this pair: a program using them runs on chain and
//   fails off-chain, never the reverse. Closing it means implementing them in `crates/x3-vm`.
// * `0x90`/`0x91` (`AtomicBegin`/`AtomicCommit`) execute in the `std` VM and, before this change,
//   were silently skipped by the runtime one; the runtime interpreter now implements the window
//   over globals, so both engines have a real implementation.
// * The runtime interpreter refuses the whole aggregate family and every cross-vm/gpu intrinsic
//   (`AGENTS.md` §5, §18). `crates/x3-vm` refuses them too, except `0xB3`/`0xB4`
//   (`EvmSload`/`EvmSstore`), which it implements against its own journaled storage — so a contract
//   that persists a slot off-chain cannot yet persist one on chain. That gap is recorded on
//   `X3-MEV-004`, not papered over here.
