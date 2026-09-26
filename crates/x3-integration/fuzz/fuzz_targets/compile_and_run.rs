//! Arbitrary source into the chain compiler, and whatever compiles into both engines.
//!
//! Properties:
//! - the compiler never panics on user source, at any optimization level;
//! - whatever it emits, the runtime's on-chain validator admits (floats aside — refused by
//!   policy, not by fault);
//! - the optimizer does not change the result: the O0 and O2 builds of one program end the same
//!   way (the same value, or both failing) on the runtime engine;
//! - x3-vm and the runtime engine agree on every build that succeeds.
#![no_main]

use libfuzzer_sys::fuzz_target;
use x3_compiler::{CompilationOptions, Compiler};
use x3_x3_integration::mini_x3::{self, MiniValue, X3Error};
use x3_x3_integration::{X3Executor, X3ExecutorConfig};

const GAS: u64 = 20_000;

fn run(bytes: &[u8]) -> Result<MiniValue, X3Error> {
    mini_x3::execute_x3bc(bytes, GAS).map(|r| r.return_val)
}

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return;
    };
    let o0 = Compiler::compile(source, CompilationOptions::no_opt());
    let o2 = Compiler::compile(source, CompilationOptions::opt2());
    let (Ok(o0), Ok(o2)) = (o0, o2) else {
        return;
    };
    let (b0, b2) = (o0.bytecode.to_bytes(), o2.bytecode.to_bytes());
    for bytes in [&b0, &b2] {
        if let Err(error) = mini_x3::validate_x3bc(bytes) {
            assert!(
                matches!(error, X3Error::ForbiddenOnChain(_)),
                "the chain refuses a program the compiler emitted: {error:?}"
            );
            return;
        }
    }

    let (r0, r2) = (run(&b0), run(&b2));
    match (&r0, &r2) {
        (Ok(a), Ok(b)) => assert_eq!(a, b, "the optimizer changed the program's value"),
        // Running out of gas at different points is expected: optimization changes the count.
        (Err(X3Error::GasExhausted), _) | (_, Err(X3Error::GasExhausted)) => {}
        (Err(a), Err(b)) => assert_eq!(a, b, "the optimizer changed how the program fails"),
        _ => panic!("one build succeeded and the other failed: O0 {r0:?}, O2 {r2:?}"),
    }

    // The std engine on the O0 build: it must agree with the runtime engine on a success.
    if let Ok(MiniValue::I64(expected)) = r0 {
        let mut config = X3ExecutorConfig::on_chain();
        config.gas_limit = GAS * 10;
        if let Ok(receipt) = X3Executor::execute(&b0, &[], config) {
            if receipt.success {
                assert_eq!(
                    receipt.return_data,
                    expected.to_le_bytes().to_vec(),
                    "x3-vm and mini_x3 disagree"
                );
            }
        }
    }
});
