//! Arbitrary bytes into every X3BC reader and both X3 engines.
//!
//! Properties, beyond "nothing panics and nothing allocates without bound":
//! - the runtime's format reader (`mini_x3::read_x3bc`) and x3-backend's agree on readability;
//! - the runtime's on-chain validator never admits a module x3-backend cannot read;
//! - the runtime engine never runs a module its validator refuses.
#![no_main]

use libfuzzer_sys::fuzz_target;
use x3_backend::BytecodeModule;
use x3_x3_integration::mini_x3;
use x3_x3_integration::{X3Executor, X3ExecutorConfig};

fuzz_target!(|data: &[u8]| {
    let runtime_reads = mini_x3::read_x3bc(data).is_ok();
    let backend_reads = BytecodeModule::from_bytes(data).is_ok();
    assert_eq!(
        runtime_reads, backend_reads,
        "the two X3BC readers disagree about readability"
    );

    let admitted = mini_x3::validate_x3bc(data).is_ok();
    if admitted {
        assert!(
            backend_reads,
            "the chain admitted a module x3-backend cannot read"
        );
    }

    let ran = mini_x3::execute_x3bc(data, 10_000);
    if !admitted {
        assert!(ran.is_err(), "mini_x3 ran a module its validator refused");
    }

    let _ = X3Executor::execute_on_chain(data, 10_000, &[], false);
    let mut config = X3ExecutorConfig::on_chain();
    config.gas_limit = 10_000;
    let _ = X3Executor::execute(data, &[], config);
    let _ = x3_vm::Verifier::verify_module_bytes(data, &x3_vm::VerifyOptions::on_chain());
});
