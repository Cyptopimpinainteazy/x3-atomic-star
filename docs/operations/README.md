# Local operations

For a disposable dev node, build the actual executable first:

```bash
cargo build --locked --release -p x3-chain-node
bash scripts/run-dev-node.sh
```

The dev launcher requires a real executable, uses `--chain dev --tmp`, and never substitutes a mock RPC. It does not isolate custom ports automatically; check ports before launching alongside an existing network. No dev-node launch is established by the repository extraction tests.

The retained testnet entry point is [x3_testnet_up.sh](../../scripts/testnet/x3_testnet_up.sh). Read its `--help` before use. Its defaults use checkout-specific `.local-state/`, generated validator material under `deployment/chain-specs/x3-atomic-star-local/generated/`, and separate testnet port ranges. Generated state/keys are ignored.

Cleanup uses [stop-owned-nodes.py](../../scripts/testnet/stop-owned-nodes.py): it requires the checkout ownership marker, a PID directory inside the selected base directory, and matching executable/base-path identity. Linux pidfds avoid signalling a different reused PID. Unknown state is refused. Legacy global `pkill` launchers are excluded.

Launcher help, refusal and ownership tests do not prove a multi-validator network or partition recovery. Public/funded networks, upgrades, credentials, genesis ceremonies and production deployments require separate reviewed procedures and exact authorization. The original XXXSTAR checkout and any running infrastructure remain unchanged.
