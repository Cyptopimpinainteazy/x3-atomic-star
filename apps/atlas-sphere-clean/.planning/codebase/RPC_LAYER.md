# Deep Dive: RPC Layer (eth_*, svm_*, atlasKernel_*)

**Purpose:** Provide a unified JSON-RPC API for interacting with the X3 node, including Ethereum-compatible endpoints, Solana-style VM endpoints, and X3 Kernel-specific controls.

---

## Key Entry Points

### Node RPC Wiring (Substrate / jsonrpsee)
- `node/src/rpc.rs` — Main RPC service setup (JSON-RPC server, WebSockets, HTTP, `jsonrpsee` integration)
- `node/src/service.rs` — RPC server launched with the node service

### Frontier (Ethereum) RPC
- `node/src/rpc_frontier.rs` — implements `eth_*` endpoints (e.g., `eth_sendRawTransaction`, `eth_getBalance`)
- `crates/evm-integration/` — EVM integration providing the backend execution engine and state mapping

### SVM RPC
- Search for `svm_` handlers in `node/src/` (likely `node/src/rpc_svm.rs` or similar).
- `crates/svm-integration/` — SVM execution engine (solana-rbpf) and transaction encoding.

### Kernel / Atlas APIs
- RPC methods prefixed with `atlasKernel_` likely live in `node/src/rpc_atlas.rs` or similar.
- Provides canonical ledger introspection (balances, nonces), and submission points for Comits.

---

## Key Files to Review

| File | Purpose |
|------|---------|
| `node/src/rpc.rs` | Endpoint registration, server config, middleware, rate limiting hooks (if any) |
| `node/src/rpc_frontier.rs` | Ethereum JSON-RPC implementation: mapping between `eth_` calls and runtime state |
| `node/src/rpc_*svm*` | SVM JSON-RPC implementation (RPC methods + encoding/decoding) |
| `node/src/rpc_atlas.rs` | Kernel control API, Comits submission & status |
| `packages/ts-sdk/` | Client SDK implementations calling RPC endpoints |

---

## Known Gaps & Risks

### WebSocket Support (Missing/Incomplete)
- `X3_GAPS_REPORT.md` indicates WebSocket server support is missing, yet most dApps expect `eth_subscribe` and pub/sub.
- Identify whether `jsonrpsee` WebSocket server is enabled and if `rpc_ws` is configured in `node/src/service.rs`.

### Rate Limiting / DDoS Protection
- No obvious rate limiting layer; this is a high-risk security gap.
- Look for `jsonrpsee` middleware or reverse-proxy patterns in `deployment/` scripts.

### Unsafe RPC Exposure (Production Risk)
- Scripts indicate `--rpc-methods Unsafe` is commonly used (e.g., `run-everything.sh`, `deployment/*`).
- Confirm production config avoids unsafe methods and uses `rpc-cors`/`rpc-host` restrictions.

### API Consistency & Versioning
- Ensure RPC endpoints are stable and versioned (esp. `atlasKernel_*`).
- A change in runtime should not silently break SDKs; add tests to lock API shapes.

---

## Suggested Deep Audit Activities

1. **Enumerate all RPC methods** exposed by the node (HTTP & WS) and compare against expected lists (eth_*, svm_*, atlasKernel_*).
2. **Integration test**: run `node` locally, call `eth_sendRawTransaction`, `svm_sendTransaction`, and `atlasKernel_submit_comit` and ensure correct behavior.
3. **Security review**: ensure WebSocket endpoints are not open to the world in deployment, or add optional auth layer.

---

## References
- `docs/ARCHITECTURE.md` (RPC surface overview)
- `X3_GAPS_REPORT.md` (RPC gaps list)
- `scripts/x3_audit.sh` (checks for key files, but not RPC correctness)
