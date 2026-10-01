//! The CLI hunt: many seeds, one verdict, and no packet unless something failed.

use std::process::Command;

#[test]
fn hunt_over_clean_seeds_exits_zero_and_writes_no_packet() {
    let dir = std::env::temp_dir().join(format!("x3-sim-hunt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_x3-sim"))
        .args([
            "--hunt",
            "4",
            "--seed",
            "1000",
            "--scenario",
            "happy-path",
            "--sessions",
            "2",
            "--steps",
            "60",
            "--nodes",
            "3",
            "--packet",
            dir.to_str().unwrap(),
            "--no-minimize",
        ])
        .output()
        .expect("run the simulator binary");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "a clean hunt must exit 0: {stdout} {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("passes = 4"), "stdout was: {stdout}");
    assert!(!dir.exists(), "no failure means no packet directory");
}

#[test]
fn packet_directory_is_only_created_when_a_failure_is_written() {
    // Negative control for the test above: a config the binary is told to fail
    // on (a usage error) must exit 2 and still write nothing.
    let dir = std::env::temp_dir().join(format!("x3-sim-usage-{}", std::process::id()));
    let output = Command::new(env!("CARGO_BIN_EXE_x3-sim"))
        .args(["--hunt", "0", "--packet", dir.to_str().unwrap()])
        .output()
        .expect("run the simulator binary");
    assert_eq!(output.status.code(), Some(2), "a usage error is exit 2");
    assert!(!dir.exists());
}
