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
    // Signed by the root these tests trust: a registry is used only under a named root.
    let signed = x3()
        .arg("sign-registry")
        .arg(&path)
        .args(["--key-hex", &seed_hex(ROOT_SEED)])
        .output()
        .expect("run x3 sign-registry");
    assert!(
        signed.status.success(),
        "{}",
        String::from_utf8_lossy(&signed.stderr)
    );
    path
}

/// The registry root these tests sign with and trust.
const ROOT_SEED: u8 = 0x42;

fn root_hex() -> String {
    public_hex(ROOT_SEED)
}

fn verify(artifact: &Path, registry: &Path) -> std::process::Output {
    x3().arg("verify-artifact")
        .arg(artifact)
        .arg("--registry")
        .arg(registry)
        .args(["--registry-root", &root_hex()])
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
            "--registry-root",
            &root_hex(),
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
                "--registry-root",
                &root_hex(),
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

/// An artifact key registry is trusted only under a root the reader names (X3-LANG-009). Unsigned,
/// anyone who can edit the file decides which keys may sign artifacts.
#[test]
fn a_registry_is_trusted_only_under_the_named_root() {
    let dir = workdir("registry-trust");
    let src = source(&dir, "fn main() -> i64 { return 7; }\n");
    let out = dir.join("program.x3b");
    let reg = registry(&dir, "signed", &[("release", public_hex(5), "active")]);
    let sign_args = |extra: &[&str]| -> Vec<String> {
        let mut args: Vec<String> = [
            "--sign-key-hex",
            &seed_hex(5),
            "--key-id",
            "release",
            "--signing-registry",
            reg.to_str().unwrap(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        args.extend(extra.iter().map(|s| s.to_string()));
        args
    };
    let stderr = |o: &std::process::Output| String::from_utf8_lossy(&o.stderr).into_owned();
    let compile_with = |extra: &[&str]| {
        let args = sign_args(extra);
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        compile(&src, &out, &refs)
    };

    // Neither a root nor the explicit opt-out: refused, and nothing is written.
    let neither = compile_with(&[]);
    assert!(!neither.status.success());
    assert!(
        stderr(&neither).contains("must be signed"),
        "{}",
        stderr(&neither)
    );
    assert!(!out.exists());

    // Under the named root: signs, and verifies under the same root.
    let root = root_hex();
    let ok = compile_with(&["--registry-root", &root]);
    assert!(ok.status.success(), "{}", stderr(&ok));
    assert!(verify(&out, &reg).status.success());

    // Another root: refused, on both sides.
    let other = public_hex(0x43);
    let wrong = x3()
        .arg("verify-artifact")
        .arg(&out)
        .arg("--registry")
        .arg(&reg)
        .args(["--registry-root", &other])
        .output()
        .unwrap();
    assert!(!wrong.status.success());
    assert!(
        stderr(&wrong).contains("not the root you trust"),
        "{}",
        stderr(&wrong)
    );

    // A key appended to the registry after it was signed: the signature no longer covers it.
    let edited = std::fs::read_to_string(&reg).unwrap().replace(
        "]}",
        &format!(
            r#", {{"key_id": "attacker", "public_key": "{}", "status": "active"}}]}}"#,
            public_hex(0x77)
        ),
    );
    std::fs::write(&reg, edited).unwrap();
    let tampered = verify(&out, &reg);
    assert!(
        !tampered.status.success(),
        "an edited registry must not be trusted"
    );
    assert!(
        stderr(&tampered).contains("different file"),
        "{}",
        stderr(&tampered)
    );

    // The explicit opt-out still loads an unsigned registry, for local development only.
    let dev = x3()
        .arg("verify-artifact")
        .arg(&out)
        .arg("--registry")
        .arg(&reg)
        .arg("--unsigned-registry")
        .output()
        .unwrap();
    assert!(dev.status.success(), "{}", stderr(&dev));

    // Signing refuses a file that is not a valid registry.
    std::fs::write(&reg, "{\"keys\": []}").unwrap();
    let invalid = x3()
        .arg("sign-registry")
        .arg(&reg)
        .args(["--key-hex", &seed_hex(ROOT_SEED)])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
}
