# Release Gates

**Canonical source: `FEATURE_REGISTRY.toml`** — all readiness scores and blockers derive from it. Run `scripts/check-readiness-consistency.sh` to validate.

**Overall readiness: ~54.8%** (54.82% arithmetic mean across the 22 currently scored entries in `FEATURE_REGISTRY.toml`, recalculated 2026-09-26 after adding the Northern Swarm/Reactor release surface). A mainnet-ready claim is forbidden unless every feature scores ≥95%.

## Gate commands

- `make guard` — agent/stub/test-cheat guards
- `make test` — focused Python + Rust compiler tests
- `make audit` — invariant guard + mainnet release gate
- `make mainnet-check` — mainnet release gate
- `make fresh-machine-check` — bootstrap validation on fresh machine
- `python3 scripts/mainnet/swarm_reactor_gate.py` — Northern Swarm/Reactor on-chain compute hard gate

## Mainnet release gate (`make mainnet-check` → `scripts/mainnet_release_gate.py`)

Exit 0 = PASS. Exit 1 = FAIL — do NOT cut a release.

Validates: documentation existence, build, chain-spec, critical test suites, Northern Swarm/Reactor on-chain compute, reproducible builds, secret hygiene.


## Northern Swarm + Reactor on-chain compute hard gate

`scripts/mainnet/swarm_reactor_gate.py` is a **mainnet blocker**, not a documentation score. It is intentionally red until the deployed compute path is real.

Required before PASS:

- `pallet-northern-swarm` and `northern-swarm` remain workspace/runtime-integrated and both test suites pass.
- The on-chain pallet has real mock/unit coverage and benchmark-derived weights; hard-coded `Weight::from_parts` call weights are not accepted.
- Task results are finalized through an M-of-N verification/quorum path rather than accepting the first executor result.
- Reserved task rewards are actually settled to accepted executor(s), with balance invariants tested.
- The chain watcher consumes the pallet's real `Tasks` storage/events through metadata-compatible access; a fabricated/hard-coded `PendingTasks` key is forbidden.
- `submit_result` is signed by the registered executor and call encoding is metadata-driven; hard-coded pallet indices and unsigned-call shortcuts are forbidden.
- `TaskKind` survives into execution routing, with a typed compute-backend abstraction, a GPU backend, and an explicit CPU fallback.
- Accelerator use changes performance only. Consensus correctness and deterministic work must retain a CPU-verifiable path.

Current canonical score: `northern_swarm_reactor = 30` in `FEATURE_REGISTRY.toml`. Passing the structural gate alone does not raise that score; evidence and tests must justify any readiness change.

## CI enforcement

Enforced in `.github/workflows/mainnet-readiness.yml` on every push/PR to main.

## Mainnet-ready claims

Forbidden unless all gates pass AND `FEATURE_REGISTRY.toml` scores ≥95% for every feature. Currently: ~54.8% registry-scored readiness; this is **not** a mainnet-ready score.