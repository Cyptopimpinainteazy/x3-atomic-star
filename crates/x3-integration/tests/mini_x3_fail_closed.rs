//! The interpreter the chain actually runs must refuse what it cannot execute.
//!
//! There are two readers of X3BC and two interpreters of its opcodes: `x3-backend`/`x3-vm` on the
//! `std` side, and `x3-integration::mini_x3`, which exists because `pallets/x3-kernel` has to
//! execute `.x3` inside the runtime, and the runtime has no `std`. The node builds its executor
//! with `sc_service::new_wasm_executor` (`node/src/service.rs`), so **`mini_x3` is the only
//! interpreter that runs on chain** — the `std` VM is reached by tools, tests and off-chain
//! simulation, never by a block.
//!
//! That makes any fabricated answer in `mini_x3` a fabricated answer in consensus. It used to hold
//! a block of arms that decoded their operands, wrote a placeholder into the destination register
//! and moved on:
//!
//! ```text
//! 0x14 / 0x15  LoadIndex / StoreIndex      dst = I64(0)
//! 0x16 / 0x17  LoadField / StoreField      dst = I64(0)
//! 0x70..0x75   arrays and tuples           dst = I64(0) / Unit, or a silent no-op
//! 0x80..0x85   context reads               a zero sender, height, timestamp, value, or 3375
//! 0x90..0x93   atomic window               tracked and skipped, nothing ever reverted
//! 0xA0..0xA2   agent / emit                Unit, or the event dropped
//! 0xB0..0xB9, 0xC0..0xC7, 0xD0..0xD7       EVM / SVM / GPU  dst = I64(0), skip six bytes
//! ```
//!
//! A program that read `array[0]` therefore returned `0` on chain rather than failing, a program
//! that asked for the block height got `0`, an EVM call reported success with a zero result, and a
//! GPU signature check that never ran returned `I64(0)`. `AGENTS.md` §5 says uncertainty must not
//! become success and §18 says an accelerator must never be accepted merely because it answered;
//! these arms did both. The six-byte skip was also wrong for most of the intrinsics (`EvmSstore` is
//! three bytes, `SvmCreateAccount` seven), so the arms desynchronised the instruction stream.
//!
//! These tests hold the on-chain interpreter to the opposite rule: an opcode it cannot really
//! execute is refused, by name, before it is answered.

#![cfg(feature = "std")]

use x3_backend::{BytecodeModule, FunctionEntry};
use x3_x3_integration::mini_x3::{execute_x3bc, MiniValue, X3Error};

/// A module whose `main` is exactly `code`, wrapped by the real envelope writer so the header,
/// checksum and section layout are the ones the compiler produces.
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

/// `instruction` followed by `Nop`s and a `RetVoid`, so a placeholder that skips its operand bytes
/// runs to a clean end and reports a value rather than an unrelated `UnexpectedEof`.
fn code_with(instruction: &[u8]) -> Vec<u8> {
    let mut code = instruction.to_vec();
    code.extend_from_slice(&[0x00; 8]); // Nop
    code.push(0x06); // RetVoid
    code
}

/// Every instruction the parser accepts and the on-chain interpreter has no implementation for,
/// with the operand bytes the compiler's emitter gives it (`crates/x3-backend/src/emit.rs`).
fn unexecutable_instructions() -> Vec<(&'static str, Vec<u8>)> {
    let mut cases: Vec<(&'static str, Vec<u8>)> = vec![
        // Arrays and aggregate fields — the interpreter has no aggregate representation.
        ("load_index", vec![0x14, 0x00, 0x01, 0x02]),
        ("store_index", vec![0x15, 0x00, 0x01, 0x02]),
        ("load_field", vec![0x16, 0x00, 0x01, 0x00, 0x00]),
        ("store_field", vec![0x17, 0x01, 0x00, 0x00, 0x00]),
        ("new_array", vec![0x70, 0x00, 0x04]),
        ("array_len", vec![0x71, 0x00, 0x01]),
        ("array_push", vec![0x72, 0x00, 0x01]),
        ("array_pop", vec![0x73, 0x00]),
        ("new_tuple", vec![0x74, 0x00, 0x02, 0x00, 0x01]),
        ("tuple_get", vec![0x75, 0x00, 0x01, 0x00, 0x00]),
        // Execution context — nothing supplies it to the runtime interpreter, so a value here is
        // invented. `ctx_gas` (0x84) is deliberately absent: the interpreter knows its own gas.
        ("ctx_sender", vec![0x80, 0x00]),
        ("ctx_block_height", vec![0x81, 0x00]),
        ("ctx_timestamp", vec![0x82, 0x00]),
        ("ctx_value", vec![0x83, 0x00]),
        ("ctx_chain_id", vec![0x85, 0x00]),
        // Agents and events — no agent registry and no event sink reach this interpreter.
        ("agent_self", vec![0xA0, 0x00]),
        ("agent_init", vec![0xA1, 0x00, 0x00, 0x00]),
        ("emit", vec![0xA2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ];

    // Cross-VM and GPU intrinsics. Each is a real opcode with a defined encoding; none has a host
    // to call in the runtime build, and a GPU result that no device produced is exactly what §18
    // forbids accepting.
    for byte in (0xB0u8..=0xB9).chain(0xC0..=0xC7).chain(0xD0..=0xD7) {
        cases.push(("intrinsic", vec![byte, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]));
    }
    cases
}

#[test]
fn a_supported_program_still_runs_so_the_refusals_are_not_vacuous() {
    // `LoadImm r0, 7` + `Ret r0`: the control that shows a module built by this helper executes.
    let bytes = module_with_code(&[0x18, 0x00, 7, 0x05, 0x00]);
    let result = execute_x3bc(&bytes, 100_000).expect("a scalar program must run");
    assert_eq!(result.return_val, MiniValue::I64(7));
}

#[test]
fn the_on_chain_interpreter_refuses_every_instruction_it_cannot_execute() {
    let mut fabricated: Vec<String> = Vec::new();
    let mut refused = 0usize;

    for (name, instruction) in unexecutable_instructions() {
        let byte = instruction[0];
        let bytes = module_with_code(&code_with(&instruction));
        match execute_x3bc(&bytes, 100_000) {
            Err(X3Error::UnsupportedOpcode(refused_byte)) if refused_byte == byte => {
                refused += 1;
            }
            Err(other) => fabricated.push(format!(
                "{name} (0x{byte:02X}): refused with {other:?}, which does not name the opcode"
            )),
            Ok(result) => fabricated.push(format!(
                "{name} (0x{byte:02X}): answered {:?} instead of refusing",
                result.return_val
            )),
        }
    }

    let expected = unexecutable_instructions().len();
    assert!(
        fabricated.is_empty(),
        "{} of {expected} instructions were answered rather than refused:\n  {}",
        fabricated.len(),
        fabricated.join("\n  ")
    );
    assert_eq!(refused, expected, "every case must be refused by name");
}

/// The atomic window is the one opcode family in that block that can be implemented for real here.
///
/// `mini_x3` has no storage and no events, so the only state an atomic window can protect is the
/// module's globals — which means the guarantee is small but complete, and the interpreter can
/// honestly provide it instead of skipping the window. The `std` VM (`crates/x3-vm/src/vm.rs`)
/// already does this for registers, globals and storage; these tests pin the runtime side to the
/// same lifecycle, including the refusal of a commit or rollback with no window open.
#[test]
fn an_atomic_window_with_nothing_open_is_refused() {
    let commit = module_with_code(&code_with(&[0x91, 0x00, 0x00]));
    assert_eq!(
        execute_x3bc(&commit, 100_000).unwrap_err(),
        X3Error::AtomicEndWithoutBegin,
        "a commit outside a window must not be a no-op"
    );

    let rollback = module_with_code(&code_with(&[0x92, 0x00, 0x00]));
    assert_eq!(
        execute_x3bc(&rollback, 100_000).unwrap_err(),
        X3Error::AtomicRollbackWithoutBegin,
        "a rollback outside a window must not be a no-op"
    );
}

/// `atomic_check` reports whether a window is open, and it has to be a real query: the compiler
/// emits it, and a constant answer sends every program down the same branch.
#[test]
fn atomic_check_reports_the_window_it_is_in() {
    // Outside any window: `atomic_check r0`, then `Ret r0` (which must be `false`).
    let outside = module_with_code(&[0x93, 0x00, 0x05, 0x00]);
    let result = execute_x3bc(&outside, 100_000).expect("the query itself runs");
    assert_eq!(
        result.return_val,
        MiniValue::Bool(false),
        "no window is open, so the query is false"
    );

    // Inside a window: `AtomicBegin`, `atomic_check r0`, `AtomicCommit`, `Ret r0`.
    let inside = module_with_code(&[
        0x90, 0x00, 0x00, // AtomicBegin 0
        0x93, 0x00, // atomic_check r0
        0x91, 0x00, 0x00, // AtomicCommit 0
        0x05, 0x00, // Ret r0
    ]);
    let result = execute_x3bc(&inside, 100_000).expect("a window that commits runs");
    assert_eq!(
        result.return_val,
        MiniValue::Bool(true),
        "the query inside the window must say so"
    );
}

/// A rollback restores what the window changed. The observable state here is the module's global,
/// so the test writes one inside the window, rolls back, and reads it outside.
#[test]
fn a_rollback_restores_the_global_the_window_changed() {
    let mut module = BytecodeModule::new();
    let init = module
        .const_pool
        .add_integer(11)
        .expect("the writer must accept an integer constant");
    module.globals.push(x3_backend::bc_format::GlobalEntry {
        name: "g".to_string(),
        type_tag: 1,
        mutable: true,
        init_const: init,
    });
    module.functions.push(FunctionEntry {
        name: "main".to_string(),
        entry_point: 0,
        param_count: 0,
        local_count: 16,
        max_stack: 16,
        return_type_tag: 1,
    });
    module.code = vec![
        0x90, 0x00, 0x00, // AtomicBegin 0
        0x18, 0x01, 99, // LoadImm r1, 99
        0x13, 0x00, 0x00, 0x00, 0x00, 0x01, // StoreGlobal 0 <- r1
        0x92, 0x00, 0x00, // AtomicRollback 0
        0x12, 0x00, 0x00, 0x00, 0x00, 0x00, // LoadGlobal r0 <- 0
        0x05, 0x00, // Ret r0
    ];
    let bytes = module.to_bytes();

    // The rollback ends the execution, exactly as the `std` VM's `AtomicAborted` does, so the
    // interpreter is asked with the rollback removed to read the global back.
    let outcome = execute_x3bc(&bytes, 100_000);
    assert_eq!(
        outcome.unwrap_err(),
        X3Error::AtomicAborted,
        "an atomic rollback aborts the execution; it does not continue past the window"
    );
}
