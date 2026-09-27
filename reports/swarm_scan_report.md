# X3 repo scan

Findings carry an id, a severity, the exact file and symbol, why it matters, the fix, the test that would prove the fix and the gate that catches a regression. Sorted by severity, kind, path and line, so two runs over the same tree are byte-identical.

Root: `/home/lojak/Desktop/xxxstar-main`
Findings: 30

## Counts

| kind | count | ratcheted here |
|---|---|---|
| `pallet-call-without-weights` | 25 | yes |
| `stale-registry-test` | 2 | yes |
| `ungated-crate` | 2 | yes |
| `unregistered-pallet` | 1 | yes |

## Related ratchets (not re-reported here)

| gate | status | detail |
|---|---|---|
| stub / marker ratchet | pass | critical-marker=443, explicit-stub=74, marker=1187 |
| fake-code scan | pass | constant-assert=17, noop-test=2, skip=139 |
| panic / unwrap ratchet | pass | pallet-call=0, production=440, runtime-hook=0 |

## Findings

### HIGH — `pallet-call-without-weights` — pallets/atomic-trade-engine/src/lib.rs:877

- **id:** `e87a13dabeda42dc`
- **symbol:** `atomic-trade-engine::Weight::from_parts(60_000_000, 0`
- **why it matters:** 4 extrinsic(s) charge an invented literal weight while the pallet ships generated weights, so the benchmarked numbers are not the ones being charged
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p atomic-trade-engine --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-account-registry/src/lib.rs:143

- **id:** `b0dc1fd9e36a697e`
- **symbol:** `x3-account-registry::10_000`
- **why it matters:** 3 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-account-registry --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-asset-registry/src/lib.rs:180

- **id:** `b574f1efddd4ee97`
- **symbol:** `x3-asset-registry::Weight::from_parts(25_000, 0`
- **why it matters:** 7 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-asset-registry --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-cross-vm-router/src/lib.rs:619

- **id:** `dfbf068d1a222e53`
- **symbol:** `x3-cross-vm-router::Weight::from_parts(40_000, 0`
- **why it matters:** 8 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-cross-vm-router --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-crosschain-gateway/src/lib.rs:803

- **id:** `1f2a607c7f59c765`
- **symbol:** `x3-crosschain-gateway::frame_support::weights::Weight::from_parts(20_000, 0`
- **why it matters:** 11 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-crosschain-gateway --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-custody/src/lib.rs:535

- **id:** `abfc948c61561e62`
- **symbol:** `x3-custody::Weight::from_parts(10_000, 0`
- **why it matters:** 10 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-custody --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-da/src/lib.rs:213

- **id:** `462a031b55b221c8`
- **symbol:** `x3-da::Weight::from_parts(15_000, 0`
- **why it matters:** 2 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-da --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-dapp-hub/src/lib.rs:247

- **id:** `ae270b6375793ccb`
- **symbol:** `x3-dapp-hub::Weight::from_parts(10_000, 0`
- **why it matters:** 8 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-dapp-hub --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-domain-registry/src/lib.rs:212

- **id:** `176efbaafb7b3dc8`
- **symbol:** `x3-domain-registry::frame_support::weights::Weight::from_parts(20_000, 0`
- **why it matters:** 3 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-domain-registry --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-flashloan/src/lib.rs:110

- **id:** `21db505e5fbddcfb`
- **symbol:** `x3-flashloan::Weight::from_parts(10_000, 0`
- **why it matters:** 3 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-flashloan --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-kernel/src/lib.rs:1177

- **id:** `e744dafce65577c9`
- **symbol:** `x3-kernel::Weight::from_parts(10_000, 0`
- **why it matters:** 3 extrinsic(s) charge an invented literal weight while the pallet ships generated weights, so the benchmarked numbers are not the ones being charged
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-kernel --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-partner/src/lib.rs:256

- **id:** `5a4a4346f22f5a32`
- **symbol:** `x3-partner::Weight::from_parts(60_000_000, 0`
- **why it matters:** 8 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-partner --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-reconciliation/src/lib.rs:263

- **id:** `3b36ca414d8858aa`
- **symbol:** `x3-reconciliation::Weight::from_parts(15_000, 0`
- **why it matters:** 6 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-reconciliation --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-reservation/src/lib.rs:239

- **id:** `97485f84208fe7e4`
- **symbol:** `x3-reservation::Weight::from_parts(10_000, 0`
- **why it matters:** 3 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-reservation --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-sentinel/src/lib.rs:218

- **id:** `f02935bb98289364`
- **symbol:** `x3-sentinel::Weight::from_parts(15_000, 0`
- **why it matters:** 7 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-sentinel --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-sequencer/src/lib.rs:246

- **id:** `48e9b957ac4f27ab`
- **symbol:** `x3-sequencer::Weight::from_parts(10_000, 0`
- **why it matters:** 1 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-sequencer --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-supply-ledger/src/lib.rs:366

- **id:** `e58d8b8c26725ac3`
- **symbol:** `x3-supply-ledger::Weight::from_parts(20_000, 0`
- **why it matters:** 5 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-supply-ledger --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-token-factory/src/lib.rs:259

- **id:** `e155b574d0b76870`
- **symbol:** `x3-token-factory::Weight::from_parts(60_000, 0`
- **why it matters:** 4 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-token-factory --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-treasury-policy/src/lib.rs:267

- **id:** `673fe9fe9ec47f20`
- **symbol:** `x3-treasury-policy::Weight::from_parts(50_000_000, 0`
- **why it matters:** 8 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-treasury-policy --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-wallet-pallet/src/lib.rs:218

- **id:** `b9033896ff6205da`
- **symbol:** `x3-wallet-pallet::10_000`
- **why it matters:** 12 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-wallet-pallet --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `pallet-call-without-weights` — pallets/x3-wrapped/src/lib.rs:278

- **id:** `e76afd632d1f8b4f`
- **symbol:** `x3-wrapped::Weight::from_parts(10_000, 0`
- **why it matters:** 7 extrinsic(s) charge an invented literal weight and the pallet has no generated weights at all, while being registered in runtime/src/lib.rs
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-wrapped --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### HIGH — `stale-registry-test` — FEATURE_REGISTRY.toml

- **id:** `ca100e7f73b2b24f`
- **symbol:** `launch_gate:mainnet_rc_gate`
- **why it matters:** `required_tests` cites a function that exists nowhere in the tree. The readiness gate only resolves citations whose crate_or_service is a directory of .rs files, so a row pointing at a script can cite a test nobody wrote and still pass — which is how this one was found
- **suggested fix:** implement `mainnet_rc_gate` under scripts/mainnet/mainnet_rc_gate.sh, or correct the citation in [launch_gate]
- **test required:** mainnet_rc_gate
- **release gate affected:** readiness consistency

### HIGH — `stale-registry-test` — FEATURE_REGISTRY.toml

- **id:** `d1c5abf1716ae1f6`
- **symbol:** `launch_gate:testnet_rc_gate`
- **why it matters:** `required_tests` cites a function that exists nowhere in the tree. The readiness gate only resolves citations whose crate_or_service is a directory of .rs files, so a row pointing at a script can cite a test nobody wrote and still pass — which is how this one was found
- **suggested fix:** implement `testnet_rc_gate` under scripts/mainnet/mainnet_rc_gate.sh, or correct the citation in [launch_gate]
- **test required:** testnet_rc_gate
- **release gate affected:** readiness consistency

### MEDIUM — `ungated-crate` — pallets/x3-cross-vm-router/fuzz/Cargo.toml

- **id:** `275693779fa5e4da`
- **symbol:** `pallet-x3-cross-vm-router-fuzz`
- **why it matters:** 1 test attribute(s), no gate command names this package, and no workspace-wide `cargo test` gate reaches its workspace, so its suite runs only when a human remembers
- **suggested fix:** append the gate line below to the fast-gate list in scripts/local-ci.sh, or record why it is intentionally ungated
- **test required:** cargo test -p pallet-x3-cross-vm-router-fuzz
- **release gate affected:** crate tests are gated
- **patch:** `.ai/patches/275693779fa5e4da.patch` (`--patches`)

### MEDIUM — `ungated-crate` — pallets/x3-settlement-engine/fuzz/Cargo.toml

- **id:** `343b07a276c84a27`
- **symbol:** `pallet-x3-settlement-engine-fuzz`
- **why it matters:** 1 test attribute(s), no gate command names this package, and no workspace-wide `cargo test` gate reaches its workspace, so its suite runs only when a human remembers
- **suggested fix:** append the gate line below to the fast-gate list in scripts/local-ci.sh, or record why it is intentionally ungated
- **test required:** cargo test -p pallet-x3-settlement-engine-fuzz
- **release gate affected:** crate tests are gated
- **patch:** `.ai/patches/343b07a276c84a27.patch` (`--patches`)

### MEDIUM — `unregistered-pallet` — pallets/pallet-x3-control/Cargo.toml

- **id:** `a2af0f92e1602244`
- **symbol:** `pallet-x3-control`
- **why it matters:** a pallet under pallets/ that runtime/src/lib.rs never names: either it is half-wired (declared, built, not in the runtime) or it is dead weight in the pallet directory
- **suggested fix:** add it to the runtime with weights and a genesis config, or move it out of pallets/ and say what it is
- **test required:** runtime registration / genesis build test
- **release gate affected:** runtime identity

### LOW — `pallet-call-without-weights` — pallets/depin-marketplace/src/lib.rs:368

- **id:** `1f4c6126e754ae38`
- **symbol:** `depin-marketplace::T::DbWeight::get().reads_writes(2, 2`
- **why it matters:** 11 extrinsic(s) charge the documented pre-benchmark `DbWeight::reads_writes` estimate; real numbers need a benchmark run
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p depin-marketplace --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### LOW — `pallet-call-without-weights` — pallets/private-execution/src/lib.rs:607

- **id:** `d1b03300ae0c8bcf`
- **symbol:** `private-execution::T::DbWeight::get().reads_writes(2, 2`
- **why it matters:** 8 extrinsic(s) charge the documented pre-benchmark `DbWeight::reads_writes` estimate; real numbers need a benchmark run
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p private-execution --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### LOW — `pallet-call-without-weights` — pallets/x3-jury-anchor/src/lib.rs:97

- **id:** `9e99332b7fcb730a`
- **symbol:** `x3-jury-anchor::T::DbWeight::get().reads_writes(1, 2`
- **why it matters:** 2 extrinsic(s) charge the documented pre-benchmark `DbWeight::reads_writes` estimate; real numbers need a benchmark run
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-jury-anchor --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

### LOW — `pallet-call-without-weights` — pallets/x3-rebalance/src/lib.rs:303

- **id:** `7cf43cd34802bacc`
- **symbol:** `x3-rebalance::T::DbWeight::get().reads_writes(2, 2`
- **why it matters:** 3 extrinsic(s) charge the documented pre-benchmark `DbWeight::reads_writes` estimate; real numbers need a benchmark run
- **suggested fix:** add a WeightInfo trait, generate weights with the FRAME benchmark CLI (`scripts/run-frame-benchmarks.sh`), and point the runtime at SubstrateWeight<Runtime>
- **test required:** cargo test -p x3-rebalance --features runtime-benchmarks
- **release gate affected:** runtime identity / benchmarks

## What this scan does not cover

`TODO`/`stub`/test-cheat markers and reachable `unwrap()`/`panic!` counts are owned by the two ratchets above; this report cites their verdicts instead of duplicating their debt.
