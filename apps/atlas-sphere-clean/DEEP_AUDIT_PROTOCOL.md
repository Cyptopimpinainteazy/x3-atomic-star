# DEEP AUDIT PROTOCOL — X3 ATOMIC CROSS-VM SYSTEM

> **Purpose:** This document is the mandatory audit gate for all contributors and AI tools (Copilot, Traycer, etc.) before any public testnet release. Work through every section in order. Do not mark anything PASS without evidence from code and tests.

---

## SECTION 1: FORMAL STATE MACHINE CHECK

### Required Deliverable
List every state the system can be in. Prove every transition is reachable, guarded, and tested.

### All States (complete for each pallet/component)

| State | Pallet / Component | Storage Key |
|---|---|---|
| `Pending` | x3-atomic-kernel, x3-settlement-engine | `Bundles<T>`, `Intents<T>` |
| `Locked` | x3-settlement-engine | `AtomicLocks<T>` |
| `Executing` | x3-atomic-kernel | `Bundles<T>.status` |
| `ProvingPoAE` | x3-atomic-kernel | `PoaeProofs<T>` |
| `FinalityVerified` | x3-atomic-kernel | `FinalityCertAnchors<T>` |
| `Settled` | x3-settlement-engine | `AtomicLocks<T>` removed |
| `Refunded` | x3-settlement-engine | `AtomicLocks<T>` removed |
| `Expired` | x3-settlement-engine | `IntentDeadlineIndex<T>` → swept |
| `RolledBack` | x3-atomic-kernel | `Bundles<T>.status` |
| `BridgePending` | cross-vm-bridge | `PendingOperations<T>` |
| `BridgeExecuted` | cross-vm-bridge | `PendingOperations<T>` removed |
| `CoordinatorPhase1..4` | cross-vm-coordinator | `SwapSession` (in-memory/persistent) |

### Per-Transition Proof Table

For every state-to-state transition, fill this table:

| Transition | Function | Required Auth | Input Validation | Event Emitted | Storage Change | Test Covering It |
|---|---|---|---|---|---|---|
| Pending → Locked | `lock_atomic_bundle` | Signed user | amount > 0, target != source | `AtomicLockCreated` | `AtomicLocks` insert | ? |
| Locked → Settled | `settle_atomic_bundle` | Manager/Oracle | proof valid | `AtomicSettled` | `AtomicLocks` remove | ? |
| Locked → Refunded | `refund_atomic_lock` | Signed owner | after deadline | `AtomicRefunded` | `AtomicLocks` remove | ? |
| Pending → Expired | `on_initialize` sweep | None (automatic) | deadline passed | `IntentExpired` | `Intents` remove | ? |
| Executing → RolledBack | `rollback_atomic_bundle` | Kernel authority | bundle exists | `BundleRolledBack` | `Bundles` update | ? |
| BridgePending → BridgeExecuted | `execute_pending_with_dispatcher` | Offchain | dispatcher result | `OperationExecuted` | `PendingOperations` remove | ✅ integration.rs |

### Audit Tasks
- [ ] Flag every transition with no test as **MISSING TEST**
- [ ] Flag every transition reachable without authentication as **AUTH BYPASS**
- [ ] Flag every transition that can leave funds locked without a recovery path as **P0 BLOCKER**
- [ ] Flag impossible/missing/unsafe transitions explicitly

---

## SECTION 2: ATOMICITY INVARIANT AUDIT

### Required Deliverable
For each invariant: state PASS / FAIL / NOT PROVEN with file path, storage key, test name, and what test is missing.

| # | Invariant | Status | File:Line | Storage Keys | Test Proving It | Missing Test |
|---|---|---|---|---|---|---|
| 1 | A lock can only be settled once | NOT PROVEN | settlement-engine/lib.rs | `AtomicLocks<T>` | none | double-settle extrinsic test |
| 2 | A lock can only be refunded if not already settled | NOT PROVEN | settlement-engine/lib.rs | `AtomicLocks<T>` | none | refund-after-settle test |
| 3 | Settlement amount ≤ locked amount | NOT PROVEN | settlement-engine/lib.rs | `AtomicLocks<T>` | none | over-settlement test |
| 4 | Refund amount == locked amount (no slippage) | NOT PROVEN | settlement-engine/lib.rs | `AtomicLocks<T>` | none | partial-refund test |
| 5 | BTC proof cannot be replayed for two locks | NOT PROVEN | settlement-engine/lib.rs `submit_btc_proof` | `UsedProofs<T>` or similar | none | proof replay test |
| 6 | Bundle rollback cannot happen after any leg settled | NOT PROVEN | atomic-kernel/lib.rs | `Bundles<T>`, `AtomicLocks<T>` | none | rollback-after-settle test |
| 7 | `on_initialize` sweep cannot expire a lock that already settled | NOT PROVEN | settlement-engine/lib.rs | `IntentDeadlineIndex<T>`, `AtomicLocks<T>` | none | expire-already-settled test |
| 8 | Cross-VM bridge executor cannot double-execute an operation | NOT PROVEN | cross-vm-bridge/src/lib.rs | `PendingOperations<T>` | integration.rs (partial) | restart-and-replay test |
| 9 | No funds leave the system without an event | NOT PROVEN | all pallets | all currency transfers | none | event coverage test |
| 10 | Total locked funds == sum of all `AtomicLock` entries | NOT PROVEN | settlement-engine/lib.rs | `AtomicLocks<T>` | none | balance invariant property test |

### Flag: Any invariant with status NOT PROVEN and no missing test identified → **P0 BLOCKER**

---

## SECTION 3: REORG / FORK / FINALITY FAILURE AUDIT

### Required Deliverable
For every failure scenario: describe expected behavior, actual behavior, who handles it, whether it is tested.

| # | Failure Scenario | Expected Behavior | Actual Behavior | Handler | Tested? | Verdict |
|---|---|---|---|---|---|---|
| 1 | BTC block containing settlement tx gets reorged out | Proof invalidated, lock refundable | Unknown — no reorg detection | Nobody | No | **P0 BLOCKER** |
| 2 | EVM chain reorgs past HTLC expiry block | Refund should remain valid | Unknown | Nobody | No | **P0 BLOCKER** |
| 3 | Finality cert anchor points to block that got orpaned | `get_finality_cert_anchor` returns stale root | Unknown | Nobody | No | **P0 BLOCKER** |
| 4 | Cross-VM coordinator crashes mid-phase (after Phase 2, before Phase 3) | Sessions must survive restart | Depends on persistence backend | `SwapCoordinator` | Only if persistent backend | **P1 if in-memory** |
| 5 | X3 node lags behind EVM chain by >1 epoch | All pending EVM proofs stale | Unknown | Nobody | No | **P1** |
| 6 | Bridge executor receives duplicate event from reconnected adapter | Must be idempotent | Unknown | `execute_pending_with_dispatcher` | Partial (no restart test) | **P1** |
| 7 | Two validators disagree on BTC block height | Different proof acceptance windows | Unknown | Nobody | No | **P1** |
| 8 | Runtime upgrade during live atomic swap | Pending sessions must migrate | Unknown — no migration guard | Nobody | No | **P0 BLOCKER** |

### Missing Behavior → Mark as P0 BLOCKER

---

## SECTION 4: GENESIS / RUNTIME UPGRADE / MIGRATION CHECK

### Required Deliverable
For each item: is it implemented? Is it safe? What data is lost or corrupted on upgrade?

| # | Item | Implemented? | Safe? | Risk on Missing |
|---|---|---|---|---|
| 1 | StorageVersion declared in every modified pallet | CHECK: `grep -rn "StorageVersion\|current_storage_version" pallets/` | Unknown | Silent data corruption |
| 2 | `on_runtime_upgrade` provided for every storage layout change | CHECK: `grep -rn "on_runtime_upgrade\|OnRuntimeUpgrade" pallets/ runtime/` | Unknown | Stale/invalid storage keys survive |
| 3 | Every new StorageMap has a default or explicit migration | Audit each `pallets/*/src/lib.rs` | Unknown | `get()` returns wrong default |
| 4 | `pre_upgrade` / `post_upgrade` hooks implemented | CHECK: `grep -rn "pre_upgrade\|post_upgrade" pallets/` | Unknown | No upgrade safety net |
| 5 | AtomicLocks migrated correctly on storage layout change | Must serialize all live locks before upgrade | Unknown | Locked funds lost |
| 6 | Bundles migrated correctly | Same as above | Unknown | In-flight bundles ghost after upgrade |
| 7 | IntentDeadlineIndex migrated correctly | Sorted index must be rebuilt or migrated | Unknown | Intents never expire OR all expire instantly |
| 8 | EVM chain-id registry persists across upgrades | Must be stable storage | Unknown | All EVM proofs become un-verifiable |
| 9 | VRF seeds / randomness sources stable across upgrades | Must not reset | Unknown | Randomness manipulable during upgrade window |
| 10 | Genesis config provides valid initial state for every new storage item | `construct_runtime!` + `GenesisConfig` audit | Unknown | Node fails to start on fresh chain |

### Commands
```bash
grep -rn "StorageVersion\|current_storage_version" pallets/ runtime/
grep -rn "on_runtime_upgrade\|OnRuntimeUpgrade\|pre_upgrade\|post_upgrade" pallets/ runtime/
grep -rn "#\[pallet::storage\]" pallets/ | wc -l
grep -rn "fn genesis_build\|GenesisConfig" pallets/ runtime/
```

### List every missing migration or unsafe default as **P0 BLOCKER**

---

## SECTION 5: ECONOMIC SAFETY / FEE / WEIGHT AUDIT

### Required Deliverable
For every extrinsic and automatic operation: classify griefing / spam / DoS risk.

| Extrinsic / Hook | Weight Benchmarked? | Can attacker spam it free? | Can attacker grief other users? | Classification |
|---|---|---|---|---|
| `lock_atomic_bundle` | ? | Not if fee > cost | Locks are per-user | P1 — check min lock amount |
| `settle_atomic_bundle` | ? | Only if zero-weight | No | P1 — ensure weight covers DB reads |
| `refund_atomic_lock` | ? | No — user pays | No | P1 — verify weight |
| `submit_btc_proof` | ? | BTC proof is expensive to forge | Spamming invalid proofs? | P1 — check proof replay protection |
| `rollback_atomic_bundle` | ? | Root-only so no | No | P1 |
| `on_initialize` sweep | ? | Automatic — no fee | If weight unbounded, can halt chain | **P0 BLOCKER if unbounded** |
| `execute_pending_with_dispatcher` | N/A (offchain) | Depends on adapter | Adapter crash = all users stuck | P1 |
| `lock_htlc` / `claim_htlc` / `refund_htlc` | ? | ? | ? | Audit pending |

### Commands
```bash
grep -rn "#\[pallet::weight\]\|Weight\|WeightInfo" pallets/
grep -rn "fn on_initialize" pallets/
grep -rn "benchmark\|#\[benchmarks\]" pallets/
```

### Classification Key
- **P0 testnet blocker**: Can halt chain or drain funds with normal user activity
- **P1 public testnet blocker**: Requires targeted attack, will break real users
- **P2 mainnet blocker**: Requires sophisticated attack, unacceptable at mainnet scale

---

## SECTION 6: OBSERVABILITY / DEBUGGABILITY AUDIT

### Required Deliverable
Every operator must be able to answer these 6 questions from node logs and metrics alone, without reading source code.

| # | Operator Question | How It Is Answered Today | Missing |
|---|---|---|---|
| 1 | Which atomic swaps are currently in progress? | Events in block? Storage query? Metrics? | No dedicated metrics endpoint |
| 2 | How many locks have been pending for >N blocks? | `on_initialize` log? | No log emitted for age |
| 3 | Why did a specific swap fail? | Error event? Log line with session ID? | Errors not always event-tagged |
| 4 | Is the BTC adapter syncing correctly? | Adapter health endpoint? | Unknown |
| 5 | What is the current BTC proof acceptance window (confirmations)? | Config dump? Runtime storage? | Unknown |
| 6 | Is the cross-VM bridge executor healthy and processing? | Offchain worker log? Metrics? | Unknown |

### Commands
```bash
grep -rn "log::\|tracing::\|debug!\|info!\|warn!\|error!" crates/ pallets/ node/
grep -rn "deposit_event\|Event::" pallets/
grep -rn "prometheus\|#\[metric\]\|register_counter\|register_gauge" node/ crates/
```

### Flag every unanswerable question as **P1 public testnet blocker**

---

## SECTION 7: ABUSE / MALICIOUS ACTOR THREAT MODEL

### Required Deliverable
For every attacker class and scenario: document attack path, affected files, current protection, missing protection, test.

| # | Actor | Scenario | Attack Path | Affected Files | Current Protection | Missing Protection | Test |
|---|---|---|---|---|---|---|---|
| 1 | Malicious user | Submit fake BTC merkle proof | Craft invalid merkle path with valid-looking structure | settlement-engine/lib.rs `verify_btc_merkle_proof` | Direction-aware merkle (C-009) | No confirmation depth check | No |
| 2 | Malicious user | Double-claim settlement | Call `settle_atomic_bundle` twice on same lock ID | settlement-engine/lib.rs | Remove-on-settle? | No explicit double-settle guard | No |
| 3 | Malicious user | Replay BTC proof across two different locks | Same txid, different lock IDs | settlement-engine/lib.rs `submit_btc_proof` | None apparent | No proof-consumed tracking | No |
| 4 | Malicious operator/validator | Submit false EVM finality cert | Inject invalid state root into `FinalityCertAnchors` | atomic-kernel/lib.rs | Auth check on setter | Who is the authorized setter? | No |
| 5 | Griefing attacker | Spam lock creations to fill `IntentDeadlineIndex` | Create many locks near block limit | settlement-engine/lib.rs | Fees (if set correctly) | Rate limit per account? | No |
| 6 | Race condition attacker | Race refund vs. settlement | Submit both in same block | settlement-engine/lib.rs | First-write-wins? | Explicit ordering check | No |
| 7 | Malicious BTC adapter | Provide false BTC headers | Report incorrect block heights | settlement-engine adapter | Trusted source only? | No multi-source validation | No |
| 8 | Malicious cross-VM dispatcher | Dispatcher returns false success | `execute_pending_with_dispatcher` trusts result | cross-vm-bridge | Result checked in C-008 | Is dispatcher result validated beyond Ok/Err? | Partial |
| 9 | Supply chain attacker | Malicious crate injection | Compromised dependency in cargo tree | All | `cargo deny` | `cargo audit` in CI? | No |
| 10 | Network attacker | Eclipse BTC node to delay proof | Feed stale BTC view to adapter | BTC adapter | None apparent | Multiple BTC node endpoints | No |
| 11 | Bribery / validator collusion | Validators accept shorter BTC confirmation depth | Governance vote to reduce `MIN_CONFIRMATIONS` | Runtime governance | Normal governance veto period | Timelock on parameter changes | No |
| 12 | Reentrancy (Substrate pseudo-reentrancy) | Callback during cross-VM dispatch modifying shared state | If EVM call triggers substrate storage write in callback | cross-vm-bridge | FRAME storage model prevents reentrancy | Verify no recursive dispatch | No |
| 13 | Fee manipulation | Zero-weight extrinsic called in tight loop | Extrinsic weight = 0 | Any unbenchmarked extrinsic | Should be caught by weights | Audit all weights | No |

---

## SECTION 8: KILL-SWITCH / EMERGENCY CONTROLS AUDIT

### Required Deliverable
Document every emergency mechanism. Flag if it can be used to steal funds.

| # | Item | Implemented? | Who Controls It? | Can It Steal User Funds? | Documented? |
|---|---|---|---|---|---|
| 1 | Global pause for all atomic operations | ? | ? | N/A if paused | ? |
| 2 | Per-pallet pause (e.g., `pallet-maintenance-mode`) | ? | Root / Council | N/A if paused | ? |
| 3 | Emergency force-refund for stuck locks | ? | Root | **CRITICAL if yes — root can steal** | ? |
| 4 | Emergency force-settle (override proof) | ? | Root | **CRITICAL if yes — root can redirect funds** | ? |
| 5 | Upgrade veto (delay upgrade that would corrupt live sessions) | ? | Council / Democracy | No | ? |
| 6 | BTC adapter kill switch (stop accepting new BTC proofs) | ? | Root | No | ? |
| 7 | Cross-VM bridge pause (stop dispatching) | ? | Root/Operator | No (stops new, not in-flight) | ? |
| 8 | Timelock on critical parameter changes (`MIN_CONFIRMATIONS`, fees) | ? | Governance | No | ? |
| 9 | Multisig requirement for emergency extrinsics | ? | ? | If not, single key = single point of failure | ? |

### Commands
```bash
grep -rn "EnsureRoot\|EnsureNever\|PalletId\|force_\|emergency_" pallets/ runtime/
grep -rn "pause\|halt\|freeze\|disable" pallets/ runtime/ node/
```

### Critical Rule: If any root-only extrinsic can move, redirect, or claim user funds without user signature → **CRITICAL: MUST REQUIRE MULTISIG OR TIMELOCK**

---

## SECTION 9: DETERMINISM AUDIT

### Required Deliverable
For every check: PASS / FAIL / UNKNOWN. Flag every nondeterministic path as P0.

| # | Check | Status | File:Line | Notes |
|---|---|---|---|---|
| 1 | No `std::time::SystemTime` or `Instant` in runtime | CHECK | Run `rg -n "SystemTime\|Instant" runtime/ pallets/` | Any hit = **P0** |
| 2 | No `thread_rng` or unseeded RNG in consensus path | CHECK | Run `rg -n "thread_rng\|rand::" runtime/ pallets/` | Any hit = **P0** |
| 3 | No `HashMap` iteration order dependence in storage-affecting code | CHECK | Run `rg -n "HashMap" pallets/` and audit each use | Iteration order is nondeterministic |
| 4 | No `f32`/`f64` in any on-chain computation | CHECK | Run `rg -n "f32\|f64\|float" pallets/ runtime/` | Any hit = **P0** |
| 5 | No OS-level entropy sources (`/dev/random`, `getrandom` called in runtime) | CHECK | Run `cargo tree --edges features | grep getrandom` in runtime context | Any hit = **P0** |
| 6 | BTC merkle proof computation deterministic (no sort, no order dependence) | PASS (C-009 direction-aware index) | settlement-engine/lib.rs `verify_btc_merkle_proof` | Index-driven, not hash-driven |
| 7 | Cross-VM bridge execution order deterministic (FIFO or explicit ordering) | CHECK | `crates/cross-vm-bridge/src/lib.rs` | Iterator order over `PendingOperations`? |
| 8 | Runtime API results deterministic given same block state | Should be guaranteed by FRAME | `runtime/src/lib.rs` runtime APIs | Verify no side-effect-bearing runtime APIs |

### Commands
```bash
rg -n "SystemTime|Instant|thread_rng|rand::|chrono::Utc|now\(\|timestamp|Duration::from_secs|sleep\(" crates/ runtime/ node/
rg -n "f32|f64|float|NaN|inf" pallets/ runtime/
rg -n "HashMap" pallets/ runtime/ | grep -v "test\|mock\|bench"
```

---

## SECTION 10: PROOF OF INTEGRATION GATE

### Required Deliverable
Run all 12 steps. If any step cannot be run, mark as NOT PROVEN. Verdict is only PASS if all 12 complete.

| # | Step | Command / Method | Expected Result | Actual Result | Verdict |
|---|---|---|---|---|---|
| 1 | Start a real local dev node | `./run-dev-node.sh` | Node starts, produces blocks | ? | ? |
| 2 | Submit a cross-VM intent | `polkadot-js-api tx.x3SettlementEngine.createIntent(...)` | Intent appears in storage | ? | ? |
| 3 | Lock atomic bundle | `tx.x3SettlementEngine.lockAtomicBundle(...)` | Lock appears in `AtomicLocks` | ? | ? |
| 4 | Submit BTC merkle proof | `tx.x3SettlementEngine.submitBtcProof(tx_index, ...)` | Proof accepted, not rejected | ? | ? |
| 5 | Settle atomic bundle | `tx.x3SettlementEngine.settleAtomicBundle(...)` | Settlement event emitted, lock removed | ? | ? |
| 6 | Attempt double-settle | Repeat step 5 | Should fail with error | ? | ? |
| 7 | Attempt refund after settlement | `tx.x3SettlementEngine.refundAtomicLock(...)` | Should fail with error | ? | ? |
| 8 | Wait for intent to expire via `on_initialize` | Advance blocks past deadline | `IntentExpired` event emitted | ? | ? |
| 9 | Submit a cross-VM bridge operation | Bridge extrinsic + `execute_pending_with_dispatcher` | Operation executed, removed from pending | ? | Partial (C-008 test) |
| 10 | Rollback an atomic bundle | `tx.x3AtomicKernel.rollbackAtomicBundle(...)` | `BundleRolledBack` event, status updated | ? | ? |
| 11 | Query runtime API `get_poae_proof` | RPC call | Returns proof or None, no panic | ? | ? |
| 12 | Run all `cargo test --workspace` | `cargo test --workspace` | All tests pass, no warnings | ? | MOSTLY COMPLETE |

### Verdict Rules
- **PASS**: All 12 steps completed with expected results, all tests pass
- **MOSTLY COMPLETE, NOT FULLY PROVEN**: Some steps not runnable (missing integration test, no dev node available) but no known failures
- **FAIL**: Any step produces wrong result or panic

---

## COMPLETION RULE (Non-Negotiable)

> A component is complete only if:
> 1. It is implemented (not a stub, not a TODO)
> 2. It is reachable from a real call path (wired into construct_runtime or real service)
> 3. It handles both the success case and every defined failure case
> 4. It emits events on success and specific errors on failure
> 5. It has tests that prove production behavior (not just that it compiles)
> 6. Those tests are enforced by CI (not just runnable locally)

---

## QUICK SCAN COMMANDS (Run First)

```bash
# Nondeterminism scan
rg -n "SystemTime|Instant|thread_rng|rand::|chrono::Utc|now\(\|timestamp|Duration::from_secs|sleep\(" crates/ runtime/ node/

# Panic / unwrap scan
rg -n "unwrap\(\|expect\(\|panic!|unimplemented!|todo!" crates/ runtime/ node/ pallets/

# Stub / placeholder scan
rg -n "mock|stub|dummy|fake|placeholder|todo!\|unimplemented!\|// TODO\|// FIXME" crates/ runtime/ node/ pallets/

# Cross-VM / atomic paths
rg -n "atomic|cross.?vm|HTLC|merkle|btc_proof|settlement" crates/ runtime/ node/ pallets/

# Runtime wiring check
rg -n "construct_runtime|impl pallet|impl_runtime_apis" runtime/ crates/

# Event emission check
rg -n "deposit_event|Event::|Error::" pallets/ runtime/
```

---

## THE REAL QUESTION (Answer This Before Merging)

> **"If I pushed this to public testnet today and real users sent value through it, what are the top 10 ways it could lose funds, get stuck, falsely finalize, replay, or silently fail?"**

Answer this with reference to specific functions, storage keys, and missing tests — not generalities.

---

## SECTION 11: CANONICAL ENCODING / SERIALIZATION AUDIT

### Required Deliverable
For each type crossing a VM boundary or used in a hash/signature, document encoding method and cross-VM equality test.

| Struct / Type | Encoding Method | Hash Method | Signature Domain Separator | Cross-VM Equality Test |
|---|---|---|---|---|
| `AtomicLock` | SCALE | `BlakeTwo256::hash_of` | None? | No |
| `PoaeProof` | SCALE | ? | ? | No |
| `BundleStatus` | SCALE enum | N/A | N/A | No |
| `CrossVmOperation` | SCALE or bincode? | ? | ? | No |
| EVM transaction hash | RLP + keccak256 | keccak256 | EVM chainId | No |
| BTC txid | SHA256d | SHA256d | N/A | No |
| SVM instruction pubkey | ed25519 | SHA256 | Program ID | No |

### Checks
- [ ] Does every cross-VM type have a canonical byte representation that all VMs agree on?
- [ ] Are SCALE types never compared by value with RLP/JSON/borsh types without explicit conversion?
- [ ] Is there a domain separator per operation type to prevent signature reuse across operation types?

### Commands
```bash
grep -rn "SCALE\|bincode\|RLP\|borsh\|serde_json\|Encode\|Decode" crates/ pallets/
grep -rn "BlakeTwo256\|keccak256\|sha256\|domain_separator" crates/ pallets/
```

---

## SECTION 12: IDEMPOTENCY / RETRY SAFETY AUDIT

### Required Deliverable
For every state-mutating operation: what happens if it is submitted twice or the service restarts mid-operation?

| Operation | Double-Submit Behavior | Service Restart Behavior | Missing Guard | Classification |
|---|---|---|---|---|
| `lock_atomic_bundle` | Second call should fail (lock exists) | Lock is persisted — safe | Verify error on duplicate lock ID | P1 |
| `settle_atomic_bundle` | Second call should fail (lock removed) | Settlement is final — safe | Verify error on already-settled | P1 |
| `refund_atomic_lock` | Second call should fail (lock removed) | Refund is final — safe | Verify error on already-refunded | P1 |
| `submit_btc_proof` | Second call with same proof — fatal if no replay protection | After restart, proof not tracked | **No proof-consumed storage found** | **P0 BLOCKER** |
| `execute_pending_with_dispatcher` | Must be idempotent across restarts | Depends on `PendingOperations` persistence | Is `PendingOperations` substrate storage? Survives restart? | P1 |
| `SwapCoordinator` phase transitions | Must be idempotent | Depends on persistence backend | In-memory = not idempotent across restart | P1 |

---

## SECTION 13: LIVENESS AUDIT

### Required Deliverable
For every user-initiated session: prove there is an eventual settlement path or eventual refund path. No session should be able to get stuck forever.

| Session Type | Settlement Path | Refund Path | Stuck Condition | Recovery Mechanism | Tested? |
|---|---|---|---|---|---|
| Atomic lock | `settle_atomic_bundle` | `refund_atomic_lock` after deadline | Deadline never reached (clock attack) | `IntentDeadlineIndex` + `on_initialize` sweep | Partial (C-006) |
| BTC-backed atomic lock | Submit BTC proof → settle | Refund after deadline | BTC reorg removes proof | No reorg recovery | No |
| Cross-VM bridge operation | `execute_pending_with_dispatcher` | No refund path? | Dispatcher permanently fails | Manual admin intervention only? | No |
| Coordinator 4-phase swap | Phase 4 complete | Timelock-based refund on each chain | Phase 2 complete, phase 3 fails, restart (in-memory) | Restart loses session | No |
| EVM HTLC position | Claim on EVM | Timeout + refund on EVM | EVM node down during claim window | No X3 handling | No |

### Rule: Every session must have a documented, tested liveness guarantee. Missing = **P1 public testnet blocker**

---

## SECTION 14: PERMISSION / ORIGIN AUDIT

### Required Deliverable
Every privileged function: required origin, what storage it can modify, whether it can affect user funds, whether abuse is tested.

| Function | Pallet | Required Origin | Storage Modified | Can Affect User Funds? | Abuse Tested? |
|---|---|---|---|---|---|
| `rollback_atomic_bundle` | x3-atomic-kernel | EnsureRoot or KernelAuthority? | `Bundles<T>` | Indirectly (blocks settlement) | No |
| `settle_atomic_bundle` | x3-settlement-engine | Manager/Oracle? | `AtomicLocks<T>`, balances | Yes — moves funds | No |
| `refund_atomic_lock` | x3-settlement-engine | Signed (owner) | `AtomicLocks<T>`, balances | Yes — returns funds to owner | No |
| `set_finality_cert_anchor` | x3-atomic-kernel (if exists) | EnsureRoot? | `FinalityCertAnchors<T>` | Yes — root of trust for settlement | No |
| `force_refund` (if exists) | any | EnsureRoot | Balances | **CRITICAL** | No |
| Dispatcher admin (`NoOpDispatcher`) | cross-vm-bridge | N/A (testnet only) | None in testnet | No | Partial |

### Commands
```bash
grep -rn "EnsureRoot\|EnsureSigned\|EnsureNever\|T::ManagerOrigin\|T::KernelAuthority" pallets/
grep -rn "fn force_\|fn admin_\|fn emergency_\|fn override_" pallets/
```

### Flag: Any fund-moving function callable by Root without timelock or multisig = **CRITICAL**

---

## SECTION 15: DEPENDENCY / SUPPLY CHAIN AUDIT

### Required Deliverable
Run all commands. List every dependency touching signatures, hashing, randomness, serialization, runtime, or networking.

### Commands
```bash
# Full dependency tree
cargo tree --workspace 2>&1 | head -200

# License and ban check
cargo deny check

# Known vulnerability scan
cargo audit

# Unsafe code in dependencies
cargo geiger --all-features 2>&1 | head -100

# Dependencies touching cryptographic operations
cargo tree --workspace | grep -E "sha2|sha3|blake|curve25519|ed25519|secp256|k256|p256|rand|getrandom|hmac|aes|chacha"

# Dependencies touching networking
cargo tree --workspace | grep -E "tokio|hyper|reqwest|tungstenite|libp2p|jsonrpsee"

# Dependencies touching serialization
cargo tree --workspace | grep -E "serde|bincode|borsh|rlp|parity-scale"
```

### Required Flag: Any crate not from parity/paritytech/substrate ecosystem touching on-chain logic = **SUPPLY CHAIN REVIEW REQUIRED**

---

## SECTION 16: UNSAFE RUST / PANIC BOUNDARY AUDIT

### Required Deliverable
For every `unsafe` block and every `unwrap`/`expect`/`panic!`: is it consensus-critical? Is it reachable from user input?

```bash
# Find all unsafe blocks
rg -n "unsafe \{|unsafe fn|unsafe impl" crates/ pallets/ runtime/ node/

# Find all panic-capable paths
rg -n "\.unwrap\(\|\.expect\(\|panic!\|unreachable!\|unimplemented!\|todo!" crates/ pallets/ runtime/ node/
```

| Location | Type | Consensus-Critical? | Runtime-Reachable? | User-Triggerable? | Classification |
|---|---|---|---|---|---|
| (fill from rg output) | unsafe / unwrap / panic | ? | ? | ? | P0 if user-triggerable in runtime |

### Rule: Any `unwrap()` or `panic!` in pallet code that can be reached from a user-submitted extrinsic = **P0 BLOCKER** (can be used to halt the chain)

---

## SECTION 17: DIFFERENTIAL TEST / REFERENCE MODEL GATE

### Required Deliverable
For every core property listed, is there a property test that covers it?

| Property | Test Framework | Test Name / File | Status |
|---|---|---|---|
| No double-claim: settled lock cannot be settled again | `quickcheck` / `proptest` | ? | NOT PROVEN |
| No refund after claim: refund fails if already settled | proptest | ? | NOT PROVEN |
| No settlement without source lock: settlement requires lock to exist | proptest | ? | NOT PROVEN |
| Settlement amount ≤ lock amount: no value creation on settlement | proptest | ? | NOT PROVEN |
| Refund restores exactly locked amount: no slippage on refund | proptest | ? | NOT PROVEN |
| BTC proof accepted only within confirmation window | proptest | ? | NOT PROVEN |
| Expired intents always reach final state: never stuck in expired-but-not-cleaned | proptest | ? | NOT PROVEN |
| Coordinator eventual termination: every session terminates or refunds within timeout | quickcheck | ? | NOT PROVEN |

### Commands
```bash
grep -rn "proptest\|quickcheck\|arbitrary\|#\[test\]" pallets/ crates/ --include="*.rs" | grep -v "mod tests"
cargo test --workspace -- --test-threads=1 2>&1 | grep -E "FAILED|test result"
```

---

## SECTION 18: CROSS-VERSION COMPATIBILITY AUDIT

### Required Deliverable
Prove that a session started on version N can complete on version N+1 after a runtime upgrade.

| Scenario | Test Method | Expected Result | Tested? |
|---|---|---|---|
| Lock created on v1, settle called on v2 | Integration test with simulated upgrade | Settlement succeeds | No |
| Coordinator session started on v1.0, resumed on v1.1 | Session serialization round-trip test | Session state preserved | No |
| BTC proof submitted on v1, verification logic changed in v2 | Regression test with fixed proof vector | Old proof still valid OR migration documented | No |
| `IntentDeadlineIndex` entry from v1 swept correctly in v2 | Migration test | No stuck intents after upgrade | No |

### Rule: If any upgrade can strand an in-flight session, that upgrade is not safe to deploy on a live chain with value.

---

## SECTION 19: CHAIN-SPECIFIC EDGE CASE AUDIT

### EVM Edges
| Edge Case | Location | Status |
|---|---|---|
| EVM chainId mismatch (replay on wrong chain) | EVM adapter / proof verification | ? |
| ERC-20 tokens that return `false` instead of reverting | EVM settlement path | ? |
| Fee-on-transfer tokens (amount received ≠ amount sent) | EVM HTLC | ? |
| EVM revert mid-HTLC claim (partial state change?) | EVM adapter | ? |

### SVM (Solana) Edges
| Edge Case | Location | Status |
|---|---|---|
| PDA derivation mismatch between X3 and on-chain program | SVM adapter | ? |
| Account rent exhaustion during HTLC creation | SVM adapter | ? |
| Instruction introspection restrictions | SVM bridge | ? |

### BTC Edges
| Edge Case | Location | Status |
|---|---|---|
| RBF (Replace-By-Fee): original tx replaced before confirmation | BTC proof checker | ? |
| Dust output: lock amount too small for BTC transaction | BTC adapter | ? |
| SegWit vs. legacy txid ambiguity | BTC merkle proof | ? |
| Non-standard scripts | BTC SPV verifier | ? |

### X3VM Edges
| Edge Case | Location | Status |
|---|---|---|
| State root diverges from EVM state root at same block | x3-atomic-kernel | ? |
| Gas exhaustion during cross-VM dispatch | x3-vm / cross-vm-bridge | ? |
| Multiple concurrent settlements competing for same state root slot | Finality cert anchors | ? |

---

## SECTION 20: CANARY TESTNET RELEASE GATE

### Stage Progression (Must complete in order)

| Stage | Criteria | Status |
|---|---|---|
| **Stage 1: Local single-node** | All cargo tests pass; node starts; manual E2E walkthrough completes without error | ? |
| **Stage 2: Multi-node local (4 validators)** | Consensus works; atomic swaps complete under normal conditions; no stuck sessions after 100 blocks | ? |
| **Stage 3: Private testnet** | External testers reproduce E2E; reorg simulation; restart recovery; upgrade simulation | ? |
| **Stage 4: Public testnet** | ONLY if: zero P0 blockers; all P1 accepted with documented mitigations; E2E CI passes; runbook written and reviewed | ? |

### Gate: Public testnet requires all of the following
- [ ] No P0 blockers from any section in this document
- [ ] All P1 issues either fixed or explicitly accepted with written mitigation
- [ ] End-to-end test passes on multi-node local setup
- [ ] CI enforces all tests (no manual-only tests)
- [ ] Operator runbook reviewed by at least one person who did not write it

---

## SECTION 21: OPERATOR RUNBOOK AUDIT

### Required Deliverable
Every scenario must have documented commands. Missing = **P1 public testnet blocker**

| # | Scenario | Documented Command | Runbook Location |
|---|---|---|---|
| 1 | Start the node from scratch | ? | ? |
| 2 | Start the node from existing state | ? | ? |
| 3 | Pause all atomic operations (emergency) | ? | ? |
| 4 | Resume after emergency pause | ? | ? |
| 5 | Manually refund a stuck atomic lock | ? | ? |
| 6 | Check all in-progress atomic swaps | ? | ? |
| 7 | Check BTC adapter sync status | ? | ? |
| 8 | Check EVM adapter sync status | ? | ? |
| 9 | Trigger a runtime upgrade safely (with live sessions) | ? | ? |
| 10 | Roll back a botched runtime upgrade | ? | ? |
| 11 | Recover from a validator crash mid-session | ? | ? |
| 12 | Identify why a specific swap failed (from logs) | ? | ? |
| 13 | Verify a specific BTC proof was accepted correctly | ? | ? |

---

## SECTION 22: FINAL "NO BULLSHIT" VERDICT RULE

A component earns the COMPLETE verdict only when ALL of the following are true:

1. **IMPLEMENTED**: The code exists and is not a stub, TODO, or `unimplemented!()`
2. **WIRED**: It is reachable from `construct_runtime!`, a real offchain worker, or a real service binary — not just importable
3. **BIDIRECTIONAL**: It handles success AND failure for every defined error condition
4. **OBSERVABLE**: It emits an event on success and a typed error on failure
5. **TESTED**: There is a test proving its production behavior (not just that it compiles)
6. **CI-ENFORCED**: That test runs in CI automatically
7. **UPGRADE-SAFE**: It survives a runtime upgrade without corrupting user funds
8. **REORG-SAFE**: It handles the relevant chain reorganization scenario
9. **IDEMPOTENT**: Duplicate messages, retries, and restarts do not corrupt state
10. **LIVENESS-PROVEN**: Every session it creates has a documented, tested path to either completion or refund

### Verdict Table

| Verdict | Meaning |
|---|---|
| **COMPLETE** | All 10 conditions above true; CI passing |
| **COMPLETE EXCEPT CI** | All conditions true locally; not yet in CI — merge blocked until CI added |
| **FUNCTIONAL BUT NOT PROVEN** | Code works in happy path; failure cases, upgrade safety, and idempotency not tested |
| **PARTIAL INTEGRATION** | Component exists but not fully wired into runtime or real service path |
| **UNSAFE FOR PUBLIC TESTNET** | Known P0 or P1 unfixed; real users could lose funds |
| **UNSAFE FOR ANY VALUE** | Critical invariants not implemented (e.g., double-claim possible, replay attack possible) |

---

## FINAL CLOSING QUESTION (Fill In Before Every Release)

> **"Give me the shortest honest release statement I could tell another engineer:**
> *'X3 atomic cross-VM is ready for _______ because _______, but it is not ready for _______ until _______.'*
> **Fill in the blanks using only evidence from code and tests, not intentions or plans."**

---

## FINAL STANDARD

> **"If it cannot survive duplicate messages, reorgs, retries, restarts, adapter failure, timeout races, and runtime upgrades — it is not complete."**

---

*This document supersedes all prior checklists for the purpose of public testnet release gating. All prior documents (AUDIT_FINDINGS.md, COMPREHENSIVE_CROSS_VM_AUDIT.md, CRITICAL_ISSUES_VERIFICATION.md, HIGH_MEDIUM_ISSUES_AUDIT.md, DEPLOY_CHECKLIST.md, X3_GOLIVE_CHECKLIST.md) remain valid records of findings and completions but do not replace this gate.*
