//! Both X3 engines against bytecode an attacker writes by hand.
//!
//! `mini_x3` is the engine the runtime runs (`WasmX3Adapter` in the wasm build), so a panic in it
//! is a panic in block execution on bytes an extrinsic chose, and a result it makes up is a
//! receipt the chain persists for work that never happened. Each case below was one or the other
//! before it was fixed; each now has to end the same way on both engines — refused, or executed to
//! exactly the value the code computes — and neither may panic.

#![cfg(feature = "std")]

use std::panic::{catch_unwind, AssertUnwindSafe};
use x3_backend::{BytecodeModule, FunctionEntry};
use x3_x3_integration::mini_x3::{self, MiniValue, X3Error};
use x3_x3_integration::{X3Executor, X3ExecutorConfig};

/// (param_count, local_count, entry) for each function, in table order.
type Functions<'a> = &'a [(u8, u16, u32)];

fn module(functions: Functions, integers: &[i64], code: Vec<u8>) -> Vec<u8> {
    let mut module = BytecodeModule::new();
    for value in integers {
        module
            .const_pool
            .add_integer(*value)
            .expect("the writer accepts an integer constant");
    }
    for (index, (param_count, local_count, entry)) in functions.iter().enumerate() {
        module.functions.push(FunctionEntry {
            name: format!("f{index}"),
            entry_point: *entry,
            param_count: *param_count,
            local_count: *local_count,
            max_stack: 16,
            return_type_tag: 1,
        });
    }
    module.code = code;
    module.to_bytes()
}

/// How one engine ended: `Ok(value)` for a successful run, `Err(reason)` for any refusal.
type Outcome = Result<i64, String>;

fn on_std(bytes: &[u8]) -> Outcome {
    let result = catch_unwind(AssertUnwindSafe(|| {
        X3Executor::execute(bytes, &[], X3ExecutorConfig::on_chain())
    }))
    .unwrap_or_else(|_| panic!("x3-vm panicked on {bytes:02x?}"));
    match result {
        Ok(receipt) if receipt.success => <[u8; 8]>::try_from(receipt.return_data.as_slice())
            .map(i64::from_le_bytes)
            .map_err(|_| format!("non-integer result {:?}", receipt.return_data)),
        Ok(receipt) => Err(String::from_utf8_lossy(&receipt.return_data).into_owned()),
        Err(error) => Err(format!("{error:?}")),
    }
}

fn on_kernel(bytes: &[u8]) -> Result<i64, X3Error> {
    let result = catch_unwind(AssertUnwindSafe(|| mini_x3::execute_x3bc(bytes, 100_000)))
        .unwrap_or_else(|_| panic!("mini_x3 panicked on {bytes:02x?}"));
    match result?.return_val {
        MiniValue::I64(v) => Ok(v),
        other => panic!("non-integer result {other:?}"),
    }
}

fn validates(bytes: &[u8]) -> Result<(), X3Error> {
    catch_unwind(AssertUnwindSafe(|| mini_x3::validate_x3bc(bytes)))
        .unwrap_or_else(|_| panic!("the validator panicked on {bytes:02x?}"))
}

/// Both engines refuse, and the runtime's validator refuses with `expected`.
fn refused_by_both(bytes: &[u8], expected: X3Error) {
    assert_eq!(validates(bytes), Err(expected.clone()), "validator verdict");
    assert_eq!(on_kernel(bytes), Err(expected), "mini_x3 verdict");
    assert!(
        on_std(bytes).is_err(),
        "x3-vm must refuse too: {:?}",
        on_std(bytes)
    );
}

// Instruction encodings (see `x3-backend::opcode`).
fn load_imm(dst: u8, imm: i8) -> Vec<u8> {
    vec![0x18, dst, imm as u8]
}
fn load_const(dst: u8, index: u32) -> Vec<u8> {
    let mut v = vec![0x10, dst];
    v.extend_from_slice(&index.to_le_bytes());
    v
}
fn ret(src: u8) -> Vec<u8> {
    vec![0x05, src]
}
fn call(dst: u8, function: u32, args: &[u8]) -> Vec<u8> {
    let mut v = vec![0x04, dst];
    v.extend_from_slice(&function.to_le_bytes());
    v.extend_from_slice(&(args.len() as u16).to_le_bytes());
    v.extend_from_slice(args);
    v
}
fn jump(target: u32) -> Vec<u8> {
    let mut v = vec![0x01];
    v.extend_from_slice(&target.to_le_bytes());
    v
}
fn binary(op: u8, dst: u8, a: u8, b: u8) -> Vec<u8> {
    vec![op, dst, a, b]
}
fn atomic(op: u8) -> Vec<u8> {
    vec![op, 0, 0]
}

/// A callee frame naming a register past the end of the register file: `mini_x3` indexed it
/// unchecked and panicked.
#[test]
fn a_register_past_the_file_in_a_callee_is_refused() {
    // main: 200 locals, calls f; f's window starts at 200, and r250 is absolute 450.
    let mut code = call(0, 1, &[]);
    code.extend(ret(0));
    let f_entry = code.len() as u32;
    code.extend(load_imm(250, 1));
    code.extend(ret(250));
    let bytes = module(&[(0, 200, 0), (0, 50, 0)], &[], code);
    let bytes = {
        // f's entry is not 0: rebuild with the right entry.
        let mut m = BytecodeModule::from_bytes(&bytes).unwrap();
        m.functions[1].entry_point = f_entry;
        m.to_bytes()
    };
    assert_eq!(
        validates(&bytes),
        Ok(()),
        "well-formed; the fault is at run time"
    );
    assert_eq!(on_kernel(&bytes), Err(X3Error::RegisterOutOfBounds));
    assert!(on_std(&bytes).is_err());
}

/// A call passing more arguments than the callee has parameters: `argc` is a `u16`, and the
/// arguments were written into the callee's window unchecked.
#[test]
fn a_call_with_more_arguments_than_parameters_is_refused() {
    let args = vec![0u8; 300];
    let mut code = load_imm(0, 1);
    code.extend(call(1, 1, &args));
    code.extend(ret(1));
    let f_entry = code.len() as u32;
    code.extend(ret(0));
    let mut m = BytecodeModule::from_bytes(&module(&[(0, 16, 0), (0, 16, 0)], &[], code)).unwrap();
    m.functions[1].entry_point = f_entry;
    refused_by_both(&m.to_bytes(), X3Error::ArgumentCountMismatch);
}

/// The entry function is called with no arguments, so an entry that declares parameters would
/// run on whatever its parameter registers held.
#[test]
fn an_entry_function_with_parameters_is_refused() {
    let mut code = load_imm(0, 5);
    code.extend(ret(0));
    refused_by_both(
        &module(&[(2, 16, 0)], &[], code),
        X3Error::ArgumentCountMismatch,
    );
}

#[test]
fn a_jump_into_the_middle_of_an_instruction_is_refused() {
    let mut code = jump(1);
    code.extend(load_imm(0, 1));
    code.extend(ret(0));
    refused_by_both(
        &module(&[(0, 16, 0)], &[], code),
        X3Error::InvalidJumpTarget(1),
    );
}

#[test]
fn a_jump_past_the_code_is_refused() {
    let mut code = jump(10_000);
    code.extend(ret(0));
    refused_by_both(
        &module(&[(0, 16, 0)], &[], code),
        X3Error::InvalidJumpTarget(10_000),
    );
}

/// Opcodes the runtime engine used to "execute" by writing a made-up result. Each must be refused
/// by name now; none may produce a successful receipt.
#[test]
fn opcodes_without_an_implementation_are_refused_not_faked() {
    let cases: &[(&str, Vec<u8>, u8)] = &[
        // EvmCall dst, then operands: the cross-VM call returned 0 and "succeeded".
        ("evm_call", vec![0xB0, 0, 0, 0, 0, 0], 0xB0),
        ("svm_invoke", vec![0xC0, 0, 0, 0, 0, 0], 0xC0),
        ("gpu_device_count", vec![0xD4, 0], 0xD4),
        // CtxSender: the zero address.
        ("ctx_sender", vec![0x80, 0], 0x80),
        // CtxChainId: a hard-coded 3375.
        ("ctx_chain_id", vec![0x85, 0], 0x85),
        // Emit event 0 with no arguments: skipped.
        ("emit", vec![0xA2, 0, 0, 0, 0, 0, 0], 0xA2),
        // LoadIndex: 0.
        ("load_index", vec![0x14, 0, 1, 2], 0x14),
        // Inc: implemented here, unimplemented in x3-vm, so the engines disagreed on it.
        ("inc", vec![0x26, 0, 0], 0x26),
        // AtomicCheck: `false`.
        ("atomic_check", vec![0x93, 0], 0x93),
    ];
    for (name, instr, op) in cases {
        let mut code = load_imm(0, 1);
        code.extend(instr.iter().copied());
        code.extend(ret(0));
        let bytes = module(&[(0, 16, 0)], &[], code);
        assert_eq!(
            validates(&bytes),
            Err(X3Error::UnimplementedOpcode(*op)),
            "{name}: validator"
        );
        assert_eq!(
            on_kernel(&bytes),
            Err(X3Error::UnimplementedOpcode(*op)),
            "{name}: mini_x3"
        );
        assert!(on_std(&bytes).is_err(), "{name}: x3-vm reported success");
    }
}

/// Float arithmetic is forbidden on chain by `x3-vm`'s verifier; the runtime engine admitted it.
#[test]
fn float_arithmetic_is_refused_on_chain() {
    let mut code = binary(0x30, 2, 0, 1); // AddF
    code.extend(ret(2));
    refused_by_both(
        &module(&[(0, 16, 0)], &[], code),
        X3Error::ForbiddenOnChain(0x30),
    );
}

#[test]
fn a_constant_index_past_the_pool_is_refused() {
    let mut code = load_const(0, 9);
    code.extend(ret(0));
    refused_by_both(
        &module(&[(0, 16, 0)], &[1], code),
        X3Error::ConstPoolOutOfBounds,
    );
}

/// An atomic block that commits runs to its value; a rollback aborts on both engines. The
/// rollback used to be a no-op in the runtime engine, which carried on and succeeded.
#[test]
fn atomic_blocks_commit_and_roll_back_the_same_way_on_both_engines() {
    let mut commit = atomic(0x90);
    commit.extend(load_imm(0, 9));
    commit.extend(atomic(0x91));
    commit.extend(ret(0));
    let bytes = module(&[(0, 16, 0)], &[], commit);
    assert_eq!(on_kernel(&bytes), Ok(9));
    assert_eq!(on_std(&bytes), Ok(9));

    let mut rollback = atomic(0x90);
    rollback.extend(load_imm(0, 9));
    rollback.extend(atomic(0x92));
    rollback.extend(ret(0));
    let bytes = module(&[(0, 16, 0)], &[], rollback);
    assert_eq!(on_kernel(&bytes), Err(X3Error::AtomicAborted));
    assert!(on_std(&bytes).is_err());

    let mut unopened = atomic(0x91);
    unopened.extend(ret(0));
    let bytes = module(&[(0, 16, 0)], &[], unopened);
    assert_eq!(on_kernel(&bytes), Err(X3Error::AtomicEndWithoutBegin));
    assert!(on_std(&bytes).is_err());
}

/// `i64::MIN / -1` and `% -1` overflow, and Rust panics on both even in release builds.
#[test]
fn division_overflow_wraps_on_both_engines() {
    for (op, expected) in [(0x23u8, i64::MIN), (0x24u8, 0)] {
        let mut code = load_const(0, 0);
        code.extend(load_const(1, 1));
        code.extend(binary(op, 2, 0, 1));
        code.extend(ret(2));
        let bytes = module(&[(0, 16, 0)], &[i64::MIN, -1], code);
        assert_eq!(on_kernel(&bytes), Ok(expected), "mini_x3, op {op:#x}");
        assert_eq!(on_std(&bytes), Ok(expected), "x3-vm, op {op:#x}");
    }
}

/// A code stream that stops inside an instruction.
#[test]
fn a_truncated_instruction_is_refused() {
    let code = vec![0x18, 0]; // LoadImm missing its immediate
    let bytes = module(&[(0, 16, 0)], &[], code);
    assert_eq!(validates(&bytes), Err(X3Error::UnexpectedEof));
    assert_eq!(on_kernel(&bytes), Err(X3Error::UnexpectedEof));
    assert!(on_std(&bytes).is_err());
}

/// Random code streams behind a valid envelope: neither engine may panic, and the runtime engine
/// may only run what its validator admitted.
#[test]
fn random_code_never_panics_either_engine() {
    // A small deterministic generator, so a failure reproduces.
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..3_000 {
        let len = (next() % 48) as usize + 1;
        let code: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        let locals = (next() % 64) as u16;
        let bytes = module(&[(0, locals, 0), (1, 8, 0)], &[3, -1], code);
        let admitted = validates(&bytes).is_ok();
        let ran = on_kernel(&bytes);
        if !admitted {
            assert!(ran.is_err(), "mini_x3 ran a module its validator refused");
        }
        let _ = on_std(&bytes);
    }
}
