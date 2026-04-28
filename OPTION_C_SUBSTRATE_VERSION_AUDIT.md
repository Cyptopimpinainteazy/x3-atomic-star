# Option C: Phase 1 Substrate Version Audit
**Date**: 2026-04-28  
**Status**: RESEARCH COMPLETE — Decision Gate Pending  
**Sprint**: Next Quarter (Q2 2026, May–June)

---

## 1. Executive Summary

The workspace is pinned to Substrate commit `948fbd2` (a custom Paritytech revision
with X3 wasm integration). Unpinning to a stable Substrate release would allow:
- `trie-db` upgrade: `0.27.1` → `0.28.0` (future-incompat warning resolved)
- `uint` upgrade: `0.4.1` → `0.9.5` (future-incompat warning resolved)
- `sp-trie` upgrade: `v22.0.0` → `v29.0.0` (full crates.io alignment)

**Key Risk**: The wasm integration custom-patched into `948fbd2` is the blocker.
Until a replacement strategy is confirmed, Phase 1 should be deferred.

---

## 2. Inventory of Substrate Pins

### Total Pinned Crates
- **90 entries** in workspace `Cargo.toml` `[patch.crates-io]` block
- **139 entries** in `Cargo.lock` from the git pin

### Pinned Crate Categories

| Category | Count | Examples |
|----------|-------|---------|
| `sp-*` (primitives) | ~35 | sp-core, sp-runtime, sp-io, sp-trie, sp-api |
| `frame-*` (FRAME) | ~10 | frame-support, frame-system, frame-executive |
| `pallet-*` | ~15 | pallet-balances, pallet-aura, pallet-grandpa |
| `substrate-*` | ~5 | substrate-frame-rpc-system |

### Critical Path Crates (must upgrade together)
```
sp-trie v22.0.0 (948fbd2)
  └── trie-db 0.27.1 [FUTURE-INCOMPAT]
  └── hash-db 0.16.0

sp-state-machine (948fbd2)
  └── trie-db 0.27.1 [FUTURE-INCOMPAT]

sp-core (948fbd2 — custom wasm integration)
  └── X3 WASM hostcalls (CRITICAL — only in this rev)

uint 0.4.1 [FUTURE-INCOMPAT]
  └── primitive-types
  └── sp-core (indirect)
```

---

## 3. Dependency Upgrade Matrix

### Current State
| Crate | Pinned Version | Latest (crates.io) | Gap | Future-Incompat? |
|-------|---------------|---------------------|-----|-----------------|
| `trie-db` | 0.27.1 | 0.28.0 | 1 minor | ✅ YES |
| `sp-trie` | 22.0.0 (git) | 29.0.0 | 7 major | — |
| `uint` | 0.4.1 | 0.9.5 | 5 minor | ✅ YES |
| `sp-core` | 21.0.0 (git) | 34.0.0+ | 13 major | — |
| `frame-support` | v4.0.0-dev (git) | 38.0.0+ | — | — |

### Target State After Unpin
| Crate | New Version | Notes |
|-------|-------------|-------|
| `trie-db` | 0.28.0 | Future-incompat warning resolved |
| `sp-trie` | 29.0.0 | Now on crates.io |
| `uint` | 0.9.5 | Future-incompat warning resolved |
| `sp-core` | 34.0.0+ | Requires wasm replacement strategy |

---

## 4. Risk Assessment

### CRITICAL — X3 Wasm Integration
**Risk Level**: 🔴 HIGH  
The custom Substrate commit `948fbd2` contains X3 wasm hostcall integration
not present in any published Substrate release. All 4 wasm-critical crates
depend on this:
- `sp-io` — exposes the custom hostcall interfaces
- `sp-core` — types used by X3 custom wasm
- `sp-wasm-interface` — wasm function bindings
- `sp-runtime` — used by x3-vm wasm execution

**Mitigation Required**: Extract wasm customizations into a standalone crate
(e.g., `x3-sp-io-ext`) that wraps any published `sp-io` version, OR find a
Substrate fork/branch that includes equivalent functionality.

### MODERATE — Mass Version Bumps
**Risk Level**: 🟡 MEDIUM  
139 locked entries must be updated simultaneously. Any single crate API break
can cascade to all 154 Substrate-dependent workspace crates. Historical data
shows Substrate major versions contain many breaking changes.

**Mitigation**: Use feature-flagged incremental upgrades + CI gating per crate.

### LOW — trie-db / uint Only
**Risk Level**: 🟢 LOW  
If the goal is specifically to resolve the 2 future-incompat warnings:
- Both `trie-db` and `uint` are *indirect* dependencies of pinned Substrate
- They cannot be independently upgraded without unpinning `sp-trie`/`sp-core`
- The 12-24 month deprecation runway makes deferral acceptable

---

## 5. Substrate Version Candidates

### Candidate 1: v0.9.x Polkadot SDK Era (Near-term)
- Corresponds to sp-core ~v21-v26
- May include wasm interface improvements
- Still pre-Polkadot SDK split (easier to work with for X3 fork)

### Candidate 2: Polkadot SDK v1.x (Medium-term)  
- Major architecture change: substrate/polkadot/cumulus merged into one repo
- sp-core now at v34+, frame-support at v38+
- Requires rewriting X3 wasm hostcalls for new interface
- Estimated effort: 4-8 developer-weeks just for wasm port

### Candidate 3: Stay at 948fbd2 + Cherry-pick (Recommended Short-term)
- Cherry-pick `trie-db` 0.28.0 compatibility patches from sp-trie v29
- Apply only to the pinned revision via a local fork at `x3-substrate-fork`
- Resolves future-incompat warnings without full unpin
- Estimated effort: 1-2 developer-weeks
- Risk: LOW (targeted patches, no API changes)

---

## 6. Research PoC Validation Plan (Week 4)

### Task: Test upgrade on 5 non-critical crates

**Phase A: Zero-risk crates (no wasm dependency)**
1. `pallet-balances` — pure storage/logic, no wasm
2. `pallet-scheduler` — pure pallet, no custom wasm
3. `frame-benchmarking` — dev tooling, not runtime-critical

**Phase B: Medium-risk crates**
4. `sp-arithmetic` — math only, minimal wasm surface
5. `sp-trie` (v22 → v29) — pull in trie-db 0.28 via registry

**Success Criteria**:
- Phase A: All 3 crates compile against latest Substrate in isolated workspace
- Phase B: sp-trie upgrade compiles with trie-db 0.28 without error
- No changes to X3 pallet code required

---

## 7. Decision Gate Criteria

After 4 weeks of research, decide:

| Scenario | Probability | Recommended Action |
|----------|-------------|-------------------|
| **Optimistic**: Wasm hostcalls portable to Polkadot SDK v1 | 30% | Proceed with full Phase 1 |
| **Realistic**: Cherry-pick patches resolve future-incompat | 50% | Cherry-pick only; defer full unpin |
| **Pessimistic**: Wasm requires full rewrite | 20% | Defer Phase 1 indefinitely; track Substrate evolution |

**Recommendation (April 2026)**: Pursue Option 3 (cherry-pick strategy) as
the immediate path. Full Substrate unpin should wait for Polkadot SDK v1
stabilization and a dedicated wasm porting sprint (estimated Q4 2026).

---

## 8. Next Actions

| Priority | Action | Owner | Timeline |
|----------|--------|-------|----------|
| P0 | Create `x3-substrate-fork` at 948fbd2 | Core team | Week 1 |
| P0 | Cherry-pick trie-db 0.28 compat from sp-trie v29 | Core team | Week 1-2 |
| P1 | Run Phase A PoC: 3 non-wasm pallets | Research | Week 2 |
| P1 | Run Phase B PoC: sp-trie upgrade | Research | Week 3-4 |
| P2 | Evaluate Polkadot SDK v1 wasm interface | Architecture | Week 3-4 |
| P3 | Decision gate: cherry-pick vs full unpin | Lead | Week 4 |

---

## 9. Summary

- **91 pins** to Substrate rev `948fbd2`
- **139 lock entries** from the pin
- **2 future-incompat warnings** (trie-db 0.27, uint 0.4)
- **12-24 month deprecation runway** — not urgent
- **Recommended path**: Cherry-pick trie-db 0.28 patches (1-2 weeks, LOW risk)
- **Full unpin**: Blocked on X3 wasm hostcall porting to Polkadot SDK v1 (Q4 2026)
