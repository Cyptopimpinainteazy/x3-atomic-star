//! `x3 compile` writes an artifact a reader can load, and can sign it (X3-LANG-009).
//!
//! Before this, `x3 compile` and `x3 build` wrote `output.bytecode.code`: the code section alone,
//! with no magic, no header, no function table and no checksum. No reader of the X3BC format could
//! load the `.x3b` they produced, and signing it — the out-of-band authentication X3-LANG-009 asks
//! for — was a library call with nothing in the build to call it.

use std::path::{Path, PathBuf};
use std::process::Command;

use sp_core::Pair;
use x3_vm::{Value, VM};

fn x3() -> Command {
    Command::new(env!("CARGO_BIN_EXE_x3"))
}

fn workdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3-cli-artifact-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn source(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("program.x3");
    std::fs::write(&path, text).expect("write source");
    path
}

fn compile(src: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    x3().arg("compile")
        .arg(src)
        .arg("--output")
        .arg(out)
        .args(extra)
        .output()
        .expect("run x3 compile")
}

fn run(artifact: &[u8]) -> Value {
    let mut vm = VM::from_bytes(artifact).expect("the artifact loads");
    vm.call_function(0, &[])
        .expect("main runs")
        .value
        .expect("main returns a value")
}

#[test]
fn the_compiled_artifact_is_a_whole_x3bc_module_that_runs() {
    let dir = workdir("loads");
    let src = source(
        &dir,
        "fn add(a: i64, b: i64) -> i64 { return a + b; }\n\
         fn main() -> i64 { return add(40, 2); }\n",
    );
    let out = dir.join("program.x3b");
    let result = compile(&src, &out, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    let artifact = std::fs::read(&out).expect("artifact");
    assert_eq!(
        &artifact[..4],
        b"X3BC",
        "the envelope starts with the format's magic"
    );
    // Two functions: the call only resolves because the function table was written.
    assert_eq!(run(&artifact), Value::I64(42));
}

fn seed_hex(byte: u8) -> String {
    format!("{byte:02x}").repeat(32)
}

fn public_hex(byte: u8) -> String {
    let pair = sp_core::ed25519::Pair::from_seed(&[byte; 32]);
    pair.public().0.iter().map(|b| format!("{b:02x}")).collect()
}

fn registry(dir: &Path, name: &str, entries: &[(&str, String, &str)]) -> PathBuf {
    let keys: Vec<String> = entries
        .iter()
        .map(|(id, key, status)| {
            format!(r#"{{"key_id": "{id}", "public_key": "{key}", "status": "{status}"}}"#)
        })
        .collect();
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, format!(r#"{{"keys": [{}]}}"#, keys.join(", "))).expect("registry");
    path
}

fn verify(artifact: &Path, registry: &Path) -> std::process::Output {
    x3().arg("verify-artifact")
        .arg(artifact)
        .arg("--registry")
        .arg(registry)
        .output()
        .expect("run x3 verify-artifact")
}

#[test]
fn a_signed_artifact_verifies_and_an_edited_one_does_not() {
    let dir = workdir("signed");
    let src = source(&dir, "fn main() -> i64 { return 7; }\n");
    let out = dir.join("program.x3b");
    let active = registry(&dir, "active", &[("release", public_hex(5), "active")]);

    let result = compile(
        &src,
        &out,
        &[
            "--sign-key-hex",
            &seed_hex(5),
            "--key-id",
            "release",
            "--signing-registry",
            active.to_str().unwrap(),
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let sidecar = dir.join("program.x3b.sig.json");
    assert!(
        sidecar.exists(),
        "the attestation is written next to the artifact"
    );

    let ok = verify(&out, &active);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(String::from_utf8_lossy(&ok.stdout).contains("signed by 'release' (active)"));

    // Rotated out: what it signed still verifies.
    let retired = registry(
        &dir,
        "retired",
        &[
            ("release", public_hex(5), "retired"),
            ("next", public_hex(6), "active"),
        ],
    );
    assert!(verify(&out, &retired).status.success());

    // Revoked: nothing it signed verifies.
    let revoked = registry(
        &dir,
        "revoked",
        &[
            ("release", public_hex(5), "revoked"),
            ("next", public_hex(6), "active"),
        ],
    );
    let refused = verify(&out, &revoked);
    assert!(
        !refused.status.success(),
        "a revoked signer must not verify"
    );
    assert!(String::from_utf8_lossy(&refused.stderr).contains("revoked"));

    // One byte of the artifact changed: the attestation is about a different artifact.
    let mut bytes = std::fs::read(&out).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&out, &bytes).unwrap();
    let edited = verify(&out, &active);
    assert!(
        !edited.status.success(),
        "an edited artifact must not verify"
    );
    assert!(
        String::from_utf8_lossy(&edited.stderr).contains("different artifact"),
        "{}",
        String::from_utf8_lossy(&edited.stderr)
    );
}

#[test]
fn a_key_the_registry_does_not_let_sign_writes_nothing() {
    let dir = workdir("refused");
    let src = source(&dir, "fn main() -> i64 { return 7; }\n");

    for (name, entries, key_id, expected) in [
        (
            "retired",
            vec![
                ("release", public_hex(5), "retired"),
                ("next", public_hex(6), "active"),
            ],
            "release",
            "has it as retired",
        ),
        (
            "unlisted",
            vec![("next", public_hex(6), "active")],
            "release",
            "has it as not listed",
        ),
        (
            // Active under the id, but the registry lists a different public key there.
            "mismatch",
            vec![("release", public_hex(9), "active")],
            "release",
            "does not verify against the registry",
        ),
    ] {
        let reg = registry(&dir, name, &entries);
        let out = dir.join(format!("{name}.x3b"));
        let result = compile(
            &src,
            &out,
            &[
                "--sign-key-hex",
                &seed_hex(5),
                "--key-id",
                key_id,
                "--signing-registry",
                reg.to_str().unwrap(),
            ],
        );
        assert!(!result.status.success(), "{name}: must not sign");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(stderr.contains(expected), "{name}: {stderr}");
        assert!(!out.exists(), "{name}: no artifact was written");
        assert!(
            !dir.join(format!("{name}.x3b.sig.json")).exists(),
            "{name}: no attestation was written"
        );
    }
}
