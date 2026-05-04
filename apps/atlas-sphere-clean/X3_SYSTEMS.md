# X3 YOLO EXECUTION PACK — OPERATIONAL DIRECTORY

**Status:** PRODUCTION ENFORCEMENT LIVE  
**Date:** 2026-03-12  
**Authority:** X3 Core + DevOps  

---

## EXECUTIVE SUMMARY

The X3 repository now operates under **four-pillar enforcement**:

1. **Checklist as Law** — `X3_COMPLETION.md` is the authoritative source of truth
2. **Automated Auditing** — `scripts/x3_audit.sh` validates structural integrity every run
3. **CI Gate Enforcement** — `.github/workflows/x3-audit.yml` blocks merges on failures
4. **Coverage + Issue Tracking** — Coverage thresholds per subsystem + auto-generated GitHub Issues

---

## PILLAR 1: CHECKLIST AS LAW

### File: `X3_COMPLETION.md`

**Purpose:** Single source of truth for completion status across 12 system categories + 50+ subsystems.

**Format:**
- All items marked as `⬜` (unchecked) or `✅` (checked)
- Mapped to exact files/modules for traceability
- Versioned (currently v1.0.0)

**Enforcement:**
- CI gate **fails PRs** if any `⬜` remains
- Code review requires explicit sign-off on checklist updates
- No "temporary" TODOs allowed

**Current Status:**
```bash
grep -c "⬜" X3_COMPLETION.md   # Count remaining unchecked items
grep -c "✅" X3_COMPLETION.md   # Count completed items
```

**Update Flow:**
1. Complete a subsystem (code + tests)
2. Run `scripts/x3_audit.sh` to validate
3. Update `X3_COMPLETION.md` with `✅`
4. Run full CI before committing
5. CI gate confirms: no `⬜` remain

---

## PILLAR 2: AUTOMATED SELF-AUDIT

### File: `scripts/x3_audit.sh`

**Purpose:** Machine-enforced structural validation. Runs before every human decision.

**Checks (9 categories):**

1. **Repository Structure** — All canonical directories exist
   - `/runtime`, `/node`, `/pallets`, `/crates`, `/apps`, `/scripts`, `/docs`, `/tests`, `/.github`

2. **Orphaned Folders** — No junk directories
   - Scans for `_unused/`, `.tmp/`, `.bak/`, etc.

3. **Cargo Lock Integrity** — Dependencies are locked
   - Validates `cargo metadata --locked` succeeds

4. **Build Safety** — `cargo build --release` passes
   - Enforces release-mode compilation

5. **Test Suite** — `cargo test --all` passes
   - All 200+ tests must pass

6. **Unsafe Code Detection** — `unwrap()/expect()` outside tests
   - Regex scan: production code uses `Result<T, E>` only

7. **Unsafe Block Audit** — Documents SAFETY assumptions
   - Warns if unsafe blocks lack comments

8. **Critical Files** — Infrastructure files exist
   - `X3_COMPLETION.md`, `docs/ARCHITECTURE.md`, `.github/copilot-instructions.md`

9. **Dependency Validation** — `cargo-deny` checks pass
   - Blocks abandoned/insecure crates

**Usage:**

```bash
# Local audit (pre-commit)
bash scripts/x3_audit.sh

# CI mode (stricter, machine-readable)
bash scripts/x3_audit.sh --ci

# Auto-fix where possible
bash scripts/x3_audit.sh --fix
```

**Phase 3 minimal gate command set (v1.1):**
```bash
bash scripts/x3_audit.sh
bash scripts/x3_audit.sh --ci
cargo check --workspace
cargo fmt --all -- --check
npm run build:all-packages --if-present
```

**Note:** Toolchain pinning is documented in `rust-toolchain.toml`. Phase 3 does not add extra pin enforcement beyond that file.

**Phase 4 gate command set (v1.1):**
```bash
bash scripts/x3_audit.sh
bash scripts/x3_audit.sh --ci
cargo build --release --locked --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --release --locked
cd runtime && cargo build --release --target wasm32-unknown-unknown --no-default-features
cargo run -p x3-launch-validator -- --check pre-launch
cargo run -p x3-launch-validator -- --check failure-conditions
```

**Phase 4 CI toolchain:** pinned nightly `nightly-2024-12-01` with `rustfmt`, `clippy`, `rust-src`, and target `wasm32-unknown-unknown`. `wasm-opt` required in CI.
**Phase 4 CI policy:** warnings are errors (`-D warnings`), flaky tests fail immediately (no retries).
**Launch-validator defaults:** `target/release/x3-chain-node`, `testnet/genesis.json`, and `prometheus.yml` must exist or the gate fails.

**Exit Codes:**
- `0` — All checks passed
- `1` — Hard failure (build, tests, lock file)
- `2` — Warnings only (use `--strict` to fail)

**Output:**
```
╔════════════════════════════════════════════════════════════╗
║           X3 STRUCTURAL SELF-AUDIT RUNNER (v1.0)          ║
╚════════════════════════════════════════════════════════════╝

[1/9] Checking repository structure...
✓ Directory exists: runtime
✓ Directory exists: node
...
[9/9] Running cargo-deny checks...
✓ cargo-deny check passed

╔════════════════════════════════════════════════════════════╗
║               ✓ X3 AUDIT PASSED                          ║
╚════════════════════════════════════════════════════════════╝
```

---

## PILLAR 3: CI GATE ENFORCEMENT

### File: `.github/workflows/x3-audit.yml`

**Purpose:** Organizational enforcement. No merge without passing all gates.

**Trigger:** Every PR + push to `main` / `master`

**Phase 4 CI gate:**

- Runs `bash scripts/x3_audit.sh --ci`
- Enforces Phase 4 build/test/WASM/launch-validator gates
- CI fails on warnings; local runs require `--strict` to fail on warnings

**Result:**
- ✅ All jobs pass → **Merge allowed**
- ❌ Any job fails → **PR blocked** (no override without governance)

**View CI Status:**
```bash
# Check latest CI run
gh run list --workflow x3-audit.yml --limit 5

# View full log
gh run view <run-id>
```

---

## PILLAR 4: COVERAGE + ISSUE TRACKING

### File: `scripts/x3_coverage_gate.sh`

**Purpose:** Per-subsystem coverage enforcement. No regression below thresholds.

**Thresholds (codified in `Cargo.toml`):**

| Subsystem | Threshold | Rationale |
|-----------|-----------|-----------|
| `x3-constitution` | 90% | Core governance engine |
| `x3-proof` | 90% | ZK + formal verification |
| `x3-slash` | 85% | Economic safety critical |
| `x3-verifier` | 85% | Cryptographic validation |
| `x3-agent` | 80% | AI safety bounds |
| `x3-sdk` | 80% | Developer UX |
| `x3-economics` | 75% | Incentive modeling |
| `runtime` | 95% | Consensus critical |
| `pallets` | 90% | State transition critical |

**Usage:**

```bash
# Check coverage for all subsystems
bash scripts/x3_coverage_gate.sh

# Generate HTML report
bash scripts/x3_coverage_gate.sh --report

# Install tarpaulin (one-time)
bash scripts/x3_coverage_gate.sh --install
```

**Output:**
```
x3-constitution:  92.3% ✓ (threshold: 90%)
x3-proof:         88.1% ✗ FAILURE (threshold: 90%)
Exit code: 1
```

**Integration with CI:**
```yaml
# In x3-audit.yml (optional, can be enforced strictly)
- name: Coverage Gates
  run: bash scripts/x3_coverage_gate.sh
```

---

### File: `scripts/x3_generate_issues.py`

**Purpose:** Auto-generate GitHub Issues from unchecked checklist items.

**Behavior:**

1. Parses `X3_COMPLETION.md` for all `⬜` entries
2. Extracts: item name, section, files/modules
3. Creates GitHub issue with auto-labels: `x3`, `audit`, `blocking`
4. **Idempotent:** Skips if issue with same title already exists
5. Dry-run mode (`--dry-run`) previews without creating

**Usage:**

```bash
# Install gh (one time)
# https://cli.github.com/

# Authenticate
gh auth login

# Generate issues from unchecked items
python3 scripts/x3_generate_issues.py

# Preview without creating
python3 scripts/x3_generate_issues.py --dry-run
```

**Example Issue Created:**

```
Title: [X3] REPO STRUCTURE & HYGIENE: Canonical directories finalized

Body:
**Checklist Item:** Canonical directories finalized

**Subsystem:** REPO STRUCTURE & HYGIENE

**Files / Modules:**
/runtime /node /pallets /vm /daemon /ai /sdk /cli /ui /docs

**Completion Criteria:**
- Implement / Fix
- Write tests
- Run: bash scripts/x3_audit.sh
- Update X3_COMPLETION.md to ✅
- Run full CI before merge

**Labels:** x3, audit, blocking
```

---

## OPERATIONAL WORKFLOWS

### Daily Workflow: Before Committing

```bash
# 1. Run self-audit (2-3 minutes)
bash scripts/x3_audit.sh

# 2. Run full test suite (included in audit, but explicit check)
cargo test --all --locked

# 3. Check code coverage if touching critical subsystems
bash scripts/x3_coverage_gate.sh

# 4. Update X3_COMPLETION.md with progress
# (Change ⬜ to ✅ for completed items only)

# 5. Push and let CI validate
git push origin feature-branch
```

### When PR Fails CI

```bash
# View which check failed
gh run view <run-id> --log

# Common failures:
# [FAIL] Found N unwrap() in production code
#   → Replace with Result/Option handling
#
# [FAIL] cargo build --release failed
#   → Check compilation errors above
#
# [FAIL] X3 COMPLETION CHECKLIST NOT FULLY GREEN (N items remaining)
#   → Update X3_COMPLETION.md with ✅ for completed items
#
# [FAIL] Coverage for x3-constitution: 88% < 90%
#   → Add more tests: crates/x3-constitution/src/tests.rs

# Once fixed, re-run CI
```

### Quarterly: Full Completion Audit

```bash
# Check current status
grep "⬜" X3_COMPLETION.md | wc -l    # Count remaining items
grep "✅" X3_COMPLETION.md | wc -l    # Count completed items

# Generate fresh issues from any unchecked items
python3 scripts/x3_generate_issues.py

# Review priority blocking issues
gh issue list --label blocking --state open

# Target: Reduce ⬜ count by 10% each quarter
```

---

## FILE TRACEABILITY MAP

Every checklist item maps to exact code locations:

| Checklist Category | Authority File | Critical Module |
|-------------------|-----------------|-----------------|
| Node Determinism | `/node/src/main.rs` | `node::service::Service` |
| Consensus (Aura) | `/runtime/src/lib.rs` | `pallet_aura::Config` |
| Consensus (GRANDPA) | `/node/src/service.rs` | `sc_finality_grandpa` |
| Atlas Kernel Tests | `/pallets/atlas-kernel/src/tests.rs` | `tests::*` (70 tests) |
| EVM Isolation | `/crates/x3-vm/src/sandbox.rs` | `EVM::Sandbox` |
| SVM Determinism | `/crates/svm-integration/src/replay.rs` | `replay::verify` |
| Agent Lifecycle | `/pallets/agent-accounts/src/lib.rs` | `Agent::spawn` / `::kill` |
| MEV Protection | `/crates/private-mempool/src/lib.rs` | `Mempool::is_private` |
| SDK Tests | `/packages/sdk/test/` | all `.test.ts` files |
| CLI Bootstrap | `/crates/x3-cli/src/main.rs` | `cli::bootstrap` |
| Governance | `/pallets/governance/src/lib.rs` | `Pallet::propose` → `::enact` |
| RPC Fuzz | `/node/src/rpc/fuzz/` | all `fuzz_target!` |
| Emergency Halt | `/crates/x3-constitution/src/engine.rs` | `Engine::halt()` |
| Documentation | `/docs/ARCHITECTURE.md` | system design diagrams |
| Disaster Recovery | `/docs/disaster-recovery.md` | backup + restore procedures |

---

## PRODUCTION ENFORCEMENT RULES

### Rule 1: No Soft Failures
- If `x3_audit.sh` returns non-zero, **code does not merge**
- No "we'll fix it later"
- Deadline: Fix before EOD, re-run CI

### Rule 2: Checklist Verbosity
- Every unchecked box is visible
- Every PR shows count of remaining items
- Dashboard (future): Progress trending

### Rule 3: Coverage Ratchet
- Coverage **cannot decrease** in any subsystem
- Only increases allowed
- Regression = auto-block merge

### Rule 4: Issue Discipline
- Unchecked items = GitHub Issues with `blocking` label
- Issues cannot be closed until corresponding checklist item is `✅`
- Prevents "forgotten" work

### Rule 5: CI Authority
- **CI is the arbiter**
- No human can override CI gate for unchecked items
- Only governance can amend the rules

---

## DEPLOYMENT CHECKLIST (GO/NO-GO)

Before shipping a release:

```bash
# 1. Verify checklist completeness
[ "$(grep -c '⬜' X3_COMPLETION.md)" -eq 0 ] || { echo "FAIL: items remain"; exit 1; }

# 2. Run full audit
bash scripts/x3_audit.sh --ci

# 3. Check coverage
bash scripts/x3_coverage_gate.sh

# 4. Verify CI status on main
gh run list --workflow x3-audit.yml --limit 1 | grep -i success

# 5. Generate release artifacts
bash scripts/x3_release_sign.sh

# 6. Tag and ship
git tag -a v1.0.0 -m "Release: v1.0.0"
git push origin v1.0.0
```

---

## NEXT ESCALATION LAYERS (Optional)

If desired, these can be added without changing existing infrastructure:

### Layer 1: Formal Verification
- K Framework specs for VM bytecode semantics
- Coq proofs for invariants
- CI integrates proof checker

### Layer 2: Deterministic Replay Auditor
- Every block records: input, seed, pre-state hash, post-state hash
- Replay logic: `hash(execute(block)) == post_state`
- Detects nondeterminism

### Layer 3: ZK Invariant Proofs
- Halo2 circuits for state validity
- Light clients verify proofs without re-execution
- On-chain verifier pallet

### Layer 4: Self-Certifying Releases
- Release = { source_hash, build_hash, proof }
- Reproducible builds with deterministic flags
- Supply-chain tamper detection

---

## CONTACT & GOVERNANCE

| Role | Contact | Authority |
|------|---------|-----------|
| Checklist Authority | X3 Core | Amends X3_COMPLETION.md |
| CI Gate Authority | DevOps | Configures `.github/workflows/` |
| Release Authority | Project Lead | Signs + publishes releases |
| Coverage Authority | QA Lead | Sets threshold targets |
| Issue Authority | Engineering Lead | Wrangles GitHub issues |

---

## FINAL STATEMENT

**This is not a suggestion. This is law.**

The four pillars work together:

```
Checklist (Truth)
    ↓
    Audit (Validation)
    ↓
    CI Gate (Enforcement)
    ↓
    Issues (Visibility)
    ↓
    SHIP OR DIE TRYING
```

Every line of code is measured against `X3_COMPLETION.md`.  
Every PR is measured against `scripts/x3_audit.sh`.  
Every merge is measured against `.github/workflows/x3-audit.yml`.  
Every incomplete item is tracked as a GitHub Issue.

**There is no middle ground.**

---

**Last Updated:** 2026-03-12  
**Next Review:** 2026-06-12 (Quarterly)  
**Status:** LIVE IN PRODUCTION
