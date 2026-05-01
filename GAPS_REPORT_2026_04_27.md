# X3 Chain — Gaps Report
**Generated:** 2026-04-27 from `repomix-output.md`  
**Scope:** 4,123 files across full monorepo  
**Status:** Pre-Testnet / Pre-Mainnet  

---

## Executive Summary

| Category | Finding | Severity |
|---|---|---|
| Mainnet-blocking TODOs (internal count) | **549** | CRITICAL |
| Total TODO/FIXME/unimplemented markers | **454** | HIGH |
| `panic!()` calls in non-test code | **~457** | HIGH |
| `unwrap()` / `.expect()` in non-test code | **~10,317** | HIGH |
| `unsafe {}` blocks | **215** | MEDIUM |
| Placeholder / TBD fields (contacts, peer IDs, etc.) | **64** | HIGH |
| TICKET items still TODO | **95 references** | HIGH |

The project is architecturally ambitious — dual-VM (EVM + SVM) on Substrate, cross-chain GPU validation, tri-VM genesis, Tauri desktop, multi-SDK — but **core execution paths remain mocked or stubbed**, the deployment surface has placeholder values throughout, and several frontend applications are empty shells.

---

## GAP-1 — EVM (Frontier) Integration: MOCKED

**Severity: CRITICAL**  
**File evidence:** `create_frontier_stub<C>()` (line 825653), `compute_mock_state_root()` (line 119174), `test_mock_executor_*` tests in EVM pallet

The EVM executor is a stub. `create_frontier_stub` returns a bare struct with no real Frontier pallet wiring. `compute_mock_state_root` hashes changes in-process with no Merkle trie. Solidity contracts cannot actually be deployed or executed today.

**What's missing:**
- Replace `create_frontier_stub` with real `pallet-evm` + `pallet-ethereum` Frontier integration
- Wire Frontier JSON-RPC endpoints in `node/src/rpc.rs` (currently a TODO at line 1308 of the PRD)
- Implement EVM-to-canonical-ledger state synchronization
- Real Merkle Patricia trie state root computation

---

## GAP-2 — SVM Integration: MOCKED

**Severity: CRITICAL**  
**File evidence:** `create_svm_stub<C>()` (line 825859), `test_mock_executor_*` in SVM pallet, `/// SHA-256 helper (mock implementation)` (svm.ts line 134)

The SVM executor follows the same pattern as EVM — it's a stub dispatcher. `create_svm_stub` has no Sealevel BPF runtime. The TypeScript SDK's SHA-256 needed for Solana-style program IDs is explicitly marked mock.

**What's missing:**
- Real BPF/eBPF runtime for Sealevel program execution
- SVM account model with lamport balances, data, owner
- SVM-to-canonical-ledger state sync
- Real SHA-256 in `packages/ts-sdk/src/svm.ts`
- SS58/Base58 validation in `packages/ts-sdk/src/utils.ts`

---

## GAP-3 — Cross-VM Bridge: Verifier is a TODO

**Severity: CRITICAL**  
**File evidence:** `// TODO replace mock verifier in bridge/finality.rs` (line 2739529), TICKET-4.5-002/003/004/006 all marked `— TODO`

The cross-VM bridge exists structurally but the finality verifier that prevents double-spends and replay across the EVM↔SVM bridge is explicitly noted as a placeholder. The vault/band invariants, lane storage/freeze mechanics, inventory reserve/release, and global unsettled notional tracking are all open tickets.

**What's missing:**
- Real finality verifier in `crates/cross-vm-bridge/bridge/finality.rs`
- TICKET-4.5-002: Vault storage and band invariants
- TICKET-4.5-003: Lane storage and freeze mechanics  
- TICKET-4.5-004: Inventory reserve and release
- TICKET-4.5-006: Global and lane unsettled notional tracking
- Atomic cross-VM transaction ordering guarantees
- EVM↔SVM message passing round-trip tests

---

## GAP-4 — GPU Executor Signature Validation: STUB

**Severity: CRITICAL**  
**File evidence:** `// GPU executor signature validation stub` (line 492766), `// For now: return mock result` (line 496714), `return placeholder` in claim broadcast (line 111338)

GPU proof verification — the primary security guarantee of the cross-chain GPU validator — is a stub. When an EVM GPU claim broadcast fails, the code returns a placeholder receipt. This means the slashing/reward mechanism has no verified input.

**What's missing:**
- Real GPU computation proof verification (ZK proof or attestation)
- Validated GPU claim submission path in the EVM executor
- Slashing conditions wired to verified proof failures
- Reward distribution tied to verified outcomes

---

## GAP-5 — RPC Layer: Gaps in WebSocket and Frontier RPC

**Severity: HIGH**  
**File evidence:** PRD lines for "Implement WebSocket server", "Implement Frontier RPC module integration (node/src/rpc.rs line 1308)", TODO comments at lines 48630, 48907, 1333062, 1333339

The node RPC does not expose WebSocket endpoints. Frontier ETH JSON-RPC (`eth_sendTransaction`, `eth_getBalance`, etc.) is not wired. Telemetry RPC pipelines in the Tauri desktop are all TODO comments.

**What's missing:**
- WebSocket server in `node/src/rpc.rs`
- `system_*`, `chain_*`, `state_*` standard Substrate RPC methods
- Frontier ETH RPC module wiring
- Desktop: replace 4 TODO placeholders with real taurpc/IPC wiring

---

## GAP-6 — TypeScript & Python SDKs: Collateral Calls Implemented (Updated 2026-04-30)

**Severity: RESOLVED**  
**File evidence:** `collateral.ts` and `collateral.py` now have real implementations; existing `depositBond`/`requestWithdrawBond`/`finalizeWithdraw`/`getBondState` are fetch/httpx RPC calls; new plan-named methods added to cover full API surface.

The SDK collateral module implementations have been completed. The gap report's "unimplemented" status was stale — existing methods were already real RPC calls, and additional methods were added to complete the API surface. The `registry.hash` compatibility issue in the substrate adapter may still need verification against newer Substrate versions.

**Status update:**
- `collateral.ts` and `collateral.py`: Methods are implemented with real RPC calls
- SDK integration tests against live node: Pending
- Publish to npm and PyPI: Pending
- `registry.hash` compatibility: Needs verification

---

## GAP-7 — Pallet Executor Authorization: Not Wired

**Severity: HIGH**  
**File evidence:** `// TODO: Integrate with x3-kernel pallet for executor authorization` appears 3 times (lines 2061641, 2075047, 2106884); `// TODO: Add executor authorization check via x3-kernel pallet` (line 2069615)

Three separate pallets have executor authorization checks replaced with TODO comments. Any caller can invoke executor-gated operations without proving they hold an authorized executor role.

**What's missing:**
- Wire `x3-kernel` pallet authorization hook in all three pallets
- Define executor registration and rotation logic
- Add authorization failure tests

---

## GAP-8 — Placeholder Weight Implementations

**Severity: HIGH**  
**File evidence:** `/// Default implementation using placeholder weights` (line 1004788), `/// Substrate weight implementation with placeholder values.` (line 1021276), `fn placeholder() -> Result<(), BenchmarkError>` (line 1070031)

Substrate pallet weights are placeholder values. This means the block-weight accounting is incorrect, making the chain vulnerable to DoS via under-priced extrinsics and invalidating any TPS benchmarks that depend on weight limits.

**What's missing:**
- Run `cargo benchmark` for all pallets with placeholder weights
- Replace placeholder values with measured benchmark output
- Verify block-weight limits are enforced correctly
- Re-run TPS benchmarks after correction

---

## GAP-9 — Cryptographic Signing: Placeholder Implementations

**Severity: HIGH**  
**File evidence:** `// Cryptographic signing functions (placeholder implementations)` (line 589173), `// For now: hash and return as placeholder` (line 589179)

Cryptographic signing is partially placeholder. For a chain whose validator security depends on correct key handling, placeholder signing paths are a critical trust boundary failure.

**What's missing:**
- Replace placeholder signing with real `sr25519`/`ed25519`/`secp256k1` implementations via `sp_core` or `k256`/`ed25519-dalek`
- Audit all signature verification code paths
- Test against known attack vectors (malleability, low-order points, etc.)

---

## GAP-10 — Frontend Applications: Empty Shells

**Severity: HIGH**  
**File evidence:** Only 12 files found under `apps/dex`, `apps/wallet`, `apps/explorer` combined; `apps/dex` contains only `package.json`, `.eslintrc.json`, `tsconfig.json`

The DEX, Wallet, and Explorer frontend applications consist of config files only — no implementation. The Inferstructor Dashboard is partially built but GPU metrics display is wired to a `start_mock_stream` function.

**What's missing:**
- DEX: liquidity pool UI, swap interface, order book
- Wallet: key management, transaction signing, multi-asset support
- Explorer: block list, transaction history, account viewer, search
- Inferstructor: real-time GPU metrics wired to actual node RPC, not mock stream

---

## GAP-11 — Governance and Treasury Pallets: Unimplemented

**Severity: HIGH**  
**File evidence:** PRD Phase 10 — all governance and treasury items are unchecked; no `pallets/governance/lib.rs` or `pallets/treasury/lib.rs` found with real implementation

On-chain governance (proposals, voting, time-locked execution) and treasury management are listed in the PRD as Phase 10 work with zero items checked. Without governance, there is no upgrade path for a live network.

**What's missing:**
- `pallets/governance`: proposal submission, voting, time-lock execution
- `pallets/treasury`: funding requests, spending approval, reporting
- Governance UI in frontend
- Token economics documentation and tokenomics whitepaper

---

## GAP-12 — Agent Memory Off-Chain Workers: 50% Complete

**Severity: MEDIUM**  
**File evidence:** `| 6 | Agent Memory Offchain | 🟡 50% | 24 | 50% | Spec: ✅ Complete, Workers: ⏳ TODO, RPC: ⏳ TODO |` (line 2725109)

The off-chain worker substrate and RPC layer for agent memory are both TODO. The spec is complete but no implementation exists.

---

## GAP-13 — Deployment: All Contacts and Peer IDs are Placeholder

**Severity: HIGH**  
**File evidence:** 64 occurrences of `XXX`, `TBD`, or `12D3KooWXXX`; CEO/CTO/Lead Dev/Ops/Comms contacts all `TBD`; bootnode peer IDs all `12D3KooWXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX`

The production launch checklist, incident response runbook, and chain spec all contain placeholder values. The chain spec bootnode entries use a dummy libp2p peer ID, so no node can bootstrap the network from the published spec.

**What's missing:**
- Generate real validator keypairs and bootnode peer IDs
- Populate incident response contacts
- Wire real Prometheus/Grafana monitoring (currently only listed as TODO)
- Set up ELK/Loki log aggregation
- Create real GitHub Actions CI/CD (55 file references exist but no `.github/workflows/` directory in main tree)

---

## GAP-14 — `panic!()` and `unwrap()` in Production Code Paths

**Severity: HIGH**  
**File evidence:** ~457 `panic!()` occurrences outside test blocks; ~10,317 `unwrap()`/`.expect()` occurrences in non-test code

This is a hardening gap. Substrate runtimes that `panic!` in consensus-critical paths will cause chain forks. `unwrap()` on `None` in RPC handlers will crash nodes.

**Priority paths to fix:**
- All `panic!()` in `runtime/`, `pallets/`, `crates/` — replace with `Result` propagation
- `unwrap()` on RPC input parsing and storage reads
- `expect()` calls that don't include meaningful context

---

## GAP-15 — Test Coverage: Integration and Multi-Node Tests Missing

**Severity: MEDIUM**  
**File evidence:** 2,715 test modules exist; PRD Phase 5 all unchecked; no tarpaulin coverage report found; no `tests/e2e/` E2E tests for cross-VM; no multi-node consensus test harness

Unit test infrastructure is solid (2,715 `mod tests` / `#[cfg(test)]` blocks), but the cross-cutting integration layer is absent:

**What's missing:**
- Cross-VM transaction E2E tests (EVM→SVM→canonical)
- Multi-node consensus tests (3+ validator setup)
- Network partition recovery tests
- Sustained TPS stress tests on live testnet
- `cargo tarpaulin` run to establish actual coverage baseline (target: 80%+)
- Tests for `TICKET-4.5-*` pallet invariants

---

## GAP-16 — Security: No Rate Limiting, No RBAC, No Emergency Pause

**Severity: HIGH**  
**File evidence:** PRD Phase 8 entirely unchecked; no `rate_limit` code found in RPC handlers; no `emergency_pause` extrinsic in runtime

**What's missing:**
- Rate limiting middleware on JSON-RPC endpoints
- DDoS / transaction spam prevention
- RBAC for admin extrinsics (`sudo` is not sufficient for mainnet)
- Emergency pause mechanism (`pallet-sudo` replacement or governor)
- Secure enclave support for validator key management
- Audit trail for governance-level operations

---

## Priority Remediation Order

```
P0 — Mainnet Blockers (do before testnet goes public):
  GAP-1  EVM Frontier: replace mock executor
  GAP-2  SVM: replace mock executor + fix SHA-256
  GAP-3  Cross-VM bridge finality verifier
  GAP-4  GPU proof verification (not a placeholder)
  GAP-13 Real bootnode peer IDs in chain spec

P1 — High (required before open testnet):
  GAP-5  RPC WebSocket + Frontier ETH JSON-RPC
  GAP-7  Executor authorization wiring in 3 pallets
  GAP-8  Real pallet weights from benchmarks
  GAP-9  Cryptographic signing (no placeholders)
  GAP-14 panic!/unwrap() cleanup in runtime/pallets/crates
  GAP-16 Rate limiting + RBAC + emergency pause

P2 — High (required before mainnet):
  GAP-6  SDK collateral methods + registry.hash fix
  GAP-11 Governance + treasury pallets
  GAP-15 Integration and multi-node tests

P3 — Medium (polish for mainnet):
  GAP-10 DEX / Wallet / Explorer frontends
  GAP-12 Agent memory off-chain workers
```

---

## Metrics Snapshot

| Metric | Value |
|---|---|
| Total files in repomix | 4,123 |
| TODO/FIXME/unimplemented markers | 454 |
| Mainnet-blocking TODOs (self-reported) | 549 |
| `panic!()` in non-test code | ~457 |
| `unwrap()` / `.expect()` in non-test | ~10,317 |
| `unsafe {}` blocks | 215 |
| Placeholder / TBD fields | 64 |
| Open TICKET-4.5 references | 95 |
| Test modules (`mod tests`) | 2,715 |
| Frontend apps that are empty shells | 3 (DEX, Wallet, Explorer) |
| CI/CD pipeline files (`.github/workflows`) | 0 confirmed in main tree |
