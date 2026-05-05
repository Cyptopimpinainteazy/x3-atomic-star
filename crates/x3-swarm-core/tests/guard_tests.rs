use x3_swarm_core::guard::{evaluate_path, GuardAction};

#[test]
fn guard_allows_docs_tests_reports() {
    assert_eq!(evaluate_path("docs/guide.md"), GuardAction::Allow);
    assert_eq!(evaluate_path("reports/swarm_health_report.md"), GuardAction::Allow);
    assert_eq!(evaluate_path("tests/unit/test_scanner.rs"), GuardAction::Allow);
}

#[test]
fn guard_allows_tauri_ui() {
    assert_eq!(evaluate_path("apps/tauri-os/src/apps/SwarmCommand/SwarmCommand.tsx"), GuardAction::Allow);
}

#[test]
fn guard_blocks_env_files() {
    assert_eq!(evaluate_path(".env"), GuardAction::Block);
    assert_eq!(evaluate_path(".env.local"), GuardAction::Block);
}

#[test]
fn guard_blocks_private_keys() {
    assert_eq!(evaluate_path("keys/private.pem"), GuardAction::Block);
    assert_eq!(evaluate_path("validator-keys/node.key"), GuardAction::Block);
}

#[test]
fn guard_blocks_mainnet_scripts() {
    assert_eq!(evaluate_path("mainnet-deploy/deploy.sh"), GuardAction::Block);
    assert_eq!(evaluate_path("chain-specs/mainnet/config.json"), GuardAction::Block);
}

#[test]
fn guard_requires_approval_for_runtime() {
    assert_eq!(evaluate_path("runtime/src/lib.rs"), GuardAction::RequireApproval);
}

#[test]
fn guard_requires_approval_for_pallets() {
    assert_eq!(evaluate_path("pallets/x3-dex/src/lib.rs"), GuardAction::RequireApproval);
}

#[test]
fn guard_requires_approval_for_bridge() {
    assert_eq!(evaluate_path("bridge/bridge.rs"), GuardAction::RequireApproval);
}

#[test]
fn guard_requires_approval_for_btc_gateway() {
    assert_eq!(evaluate_path("btc/gateway.rs"), GuardAction::RequireApproval);
}
