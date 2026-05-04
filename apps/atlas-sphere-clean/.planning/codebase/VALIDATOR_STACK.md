# Deep Dive: Validator Stack (Consensus + Scaling)

**Purpose:** Explain how the node runs consensus, executes blocks, and scales via GPU / parallel proposer components.

---

## Key Components

### Substrate Node & Consensus
- `node/src/service.rs` — sets up the Substrate `Service` (networking, executor, consensus, telemetry)
- `node/src/cli.rs` — command‑line options, chain spec, node configuration

### Runtime Composition
- `runtime/src/lib.rs` — runtime definition (pallets, types, weights)
- `runtime/src/weights.rs` — weight functions used for fee and block limits

### Consensus Layers
- **Aura / Grandpa** — standard Substrate consensus in use for block production + finality.
- **Flash Finality** — custom consensus layer in `crates/flash-finality/`.
- **Proof-of-History / GPU Validator** — in `crates/poh-generator/` + `crates/x3-gpu-validator-swarm/`.

---

## Key Files & Directories

| Area | File/Dir | Purpose |
|------|----------|---------|
| Consensus | `crates/flash-finality/` | Flash finality logic (faster finality mode) |
| PoH | `crates/poh-generator/` | Proof‑of‑history generator used for GPU validator ordering |
| GPU Validator | `crates/x3-gpu-validator-swarm/` | Orchestration for GPU-based validators |
| Node | `node/src/` | Service wiring, execution, RPC, telemetry |
| Runtime | `runtime/src/` | Pallet composition + weight math |

---

## Workflow

### Validator Startup
1. Node reads `chainSpec` (from `deployment/chain-specs/*` or CLI)
2. Consensus is configured (Aura + Grandpa, or Flash Finality if enabled)
3. Block authorship is handled by Aura (or Flash Finality proposer) and executes runtime in WASM

### GPU Validator (Emerging)
- The GPU validator stack aims to offload execution to GPU for high throughput.
- Needs deterministic execution to ensure block replayable by CPU validators.
- Heavily tied to `crates/poh-generator` and `crates/x3-gpu-validator-swarm` coordination.

---

## Risks & Gaps

### 1) Flash Finality integration
- Flash Finality does not appear fully integrated with standard Grandpa finality.
- Risk: split view of finality if nodes disagree on finalization method.

### 2) Deterministic GPU Execution
- GPU execution must yield bit-for-bit identical results as CPU execution.
- Risk: Floating point / non-deterministic kernel behavior causes consensus divergence.

### 3) Validator Network / Testnet Hardening
- There is no clear multi-node test harness.
- `tests/` and `integration-tests/` may not include full validator clusters.

---

## Suggested Deep Audit Activities

1. **Run a multi-node local network** (use existing scripts or compose file) to validate consensus under churn.
2. **Verify block replay**: produce blocks with GPU validator (if enabled), replay them on CPU-only node.
3. **Load test**: measure TPS with `crates/tps-tracker/` and identify bottlenecks in proposer / block execution.

---

## References
- `docs/ARCHITECTURE.md` (high-level system)
- `X3_GAPS_REPORT.md` (validator scaling gaps)
- `deployment/` scripts (deployment patterns for nodes)
