# X3 Chain v1.1 Release

**Release Date:** March 24, 2026  
**Milestone:** v1.1 Release Readiness (Phase 8 Complete)  
**Status:** ✅ Production Ready for Testnet

---

## Release Summary

X3 Chain v1.1 completes the dual-EVM/SVM blockchain runtime with production-hardened operator tooling and comprehensive release gates.

### Key Features

- **Atomic Cross-VM Swaps:** 2-phase commit (prepare → commit/abort) with escrow-backed compensating refunds on SVM failure
- **Dual VM Execution:** EVM via Frontier pallet (Solidity contracts) + SVM kernel (Solana-compatible programs)
- **Operator Tooling:** 4-validator cluster scripts, automated health checks, rollback procedures
- **Production Hardening:** Input validation (128KB calldata limits), 186 pallet permission checks, panic elimination

---

## Release Artifacts

### Checksums

```
08963239d56c0524d97c67c9b779d8bb42b169bfc3317fe6c5d3fe7f2aa31709  x3-chain-v1.1.1.tar.gz
```

### Signature Verification

```bash
# Verify GPG signature (release key: X3 Chain Release <release@x3-chain.io>)
gpg --verify CHECKSUMS.sha256.asc CHECKSUMS.sha256

# Verify distributable tarball
sha256sum -c CHECKSUMS.sha256

# After extraction, verify the extracted bundle contents
sha256sum -c CHECKSUMS.bundle.sha256
```

---

## Downloads

- **Release Package:** `x3-chain-v1.1.1.tar.gz`
  - Binary: `x3-chain-node` (54 MB)
  - Runtime: `x3_chain_runtime.compact.compressed.wasm` (824 KB)
  - Scripts: `run-dev-node.sh`, `run-production-node.sh`, `x3_node_healthcheck.sh`
  - Docs: Complete operator SOP, development guide, node requirements
  - Config: Chain specs for local/testnet networks + `.env.example`

- **Checksums:** `CHECKSUMS.sha256`, `CHECKSUMS.sha256.asc`, `CHECKSUMS.bundle.sha256` (inside the extracted bundle)

---

## Installation & Deployment

### Quick Start (Dev Mode)

```bash
# Extract release
rm -rf /tmp/x3-chain-release && mkdir -p /tmp/x3-chain-release
tar -xzf x3-chain-v1.1.1.tar.gz -C /tmp/x3-chain-release

# Verify tarball + signature
sha256sum -c CHECKSUMS.sha256
gpg --verify CHECKSUMS.sha256.asc CHECKSUMS.sha256

# Verify extracted bundle contents
(cd /tmp/x3-chain-release && sha256sum -c CHECKSUMS.bundle.sha256)

# Run health check
NODE_NAME=my-node bash /tmp/x3-chain-release/scripts/x3_node_healthcheck.sh --mode prod

# Start node
cd /tmp/x3-chain-release
CHAIN=dev ./scripts/run-dev-node.sh
```

### Production Deployment

1. **Verify artifacts:**
   ```bash
   sha256sum -c CHECKSUMS.sha256
   gpg --verify CHECKSUMS.sha256.asc CHECKSUMS.sha256
  (cd /opt/x3-chain && sha256sum -c CHECKSUMS.bundle.sha256)
   ```

2. **Extract and prepare:**
   ```bash
  sudo mkdir -p /opt/x3-chain
  sudo tar -xzf x3-chain-v1.1.1.tar.gz -C /opt/x3-chain
  sudo chmod +x /opt/x3-chain/x3-chain-node /opt/x3-chain/scripts/*.sh
   ```

3. **Run pre-deployment checks:**
   ```bash
  NODE_NAME=validator-1 bash /opt/x3-chain/scripts/x3_node_healthcheck.sh --mode prod
   ```

4. **Start node** (see `docs/X3_OPERATOR_SOP.md` for multi-validator setup):
   ```bash
  NODE_NAME=validator-1 CHAIN=testnet /opt/x3-chain/x3-chain-node
   ```

5. **Monitor:**
   ```bash
   # RPC health
   curl http://localhost:9944/health

   # Prometheus metrics
   curl http://localhost:9615/metrics
   ```

---

## What's New in v1.1

### Core Runtime (Phase 5)

- ✅ **Cross-VM Bridge:** 2PC atomic swap dispatch with rollback on SVM failure
  - Test: `test_atomic_swap_restores_evm_balance_on_svm_prepare_failure` (101/101 x3-bridge tests passing)
  - Reference: `crates/x3-bridge/dispatch.rs`

- ✅ **Integration Test Harness:** Real TestExternalities with event assertion
  - Test: `submit_assign_finalize_transitions_bundle_status_and_emits_events` 
  - WebSocket e2e test available (requires live node)
  - Reference: `integration-tests/cross-vm-pallet-test.rs`

### Reliability (Phase 6)

- ✅ **Panic Elimination:** All critical paths converted from panic! to Result
  - Startup gate, RPC error handling, bridge adapters
  - Reference: `startup_gate.rs`, `x3-rpc/crates/`

- ✅ **RPC Hardening:** Input validation enforced
  - Max calldata: 128 KB
  - Signature count limits
  - Reference: `crates/x3-rpc/src/gas_estimation.rs`

- ✅ **Pallet Security:** 186 origin checks audited
  - 100% permission validation
  - Reference: Phase 6 origin audit report

### APIs & Deprecations (Phase 8)

- ✅ **GasEstimationRPC Deprecation:** Marked `#[deprecated]`, private gas_price()
  - Direct routing: Use `node::rpc_frontier::create_frontier_stub` for production RPC
  - Reference: `crates/x3-rpc/src/gas_estimation.rs`

- ✅ **Admin Billing API:** New endpoints for account management
  - Tier changes, quota reset, API key rotation
  - Reference: `packages/blockchain-connector/src/server/index.ts`

---

## Test Results

### Rust Test Suite

```
x3-bridge:              101/101 passing ✓
x3-atomic-trade:         24/24 passing ✓
pallet-x3-coin:          30/30 passing ✓
x3-rpc:                  14/14 passing ✓
─────────────────────────────────────
Total:                  169/169 passing ✓
```

### TypeScript Test Suite

```
blockchain-connector:    43/43 passing ✓
```

### System Tests

- ✅ 4-validator local cluster (consensus, finality progression)
- ✅ Multi-validator peering (bootnode wiring)
- ✅ Database rollback procedures
- ✅ Graceful shutdown (SIGTERM)
- ✅ Health check script (all modes)

---

## Operator Documentation

Complete operator runbook included:

- **X3_OPERATOR_SOP.md** (777 lines)
  - Single-validator startup
  - Multi-validator cluster setup
  - Monitoring and health checks
  - Troubleshooting guide (10+ common issues)
  - Database, binary, and config rollback procedures
  - Emergency incident response

- **DEVELOPMENT.md** (expanded +120 lines)
  - Node startup health check
  - CLI flags reference
  - Configuration separation (dev/local/staging/production)
  - Telemetry metrics
  - Graceful shutdown procedures

- **NODE_REQUIREMENTS.md**
  - Hardware requirements
  - Software dependencies
  - Port management

---

## Upgrade Path (v1.0 → v1.1)

**No storage migrations required.** Direct binary replacement is safe.

```bash
# 1. Stop running v1.0 node
systemctl stop x3-chain-node

# 2. Backup database (optional but recommended)
cp -r /var/lib/x3-chain /var/lib/x3-chain.v1.0.backup

# 3. Replace binary
cp /tmp/x3-chain-release/x3-chain-node /opt/x3-chain/
chmod +x /opt/x3-chain/x3-chain-node

# 4. Restart node
systemctl start x3-chain-node

# 5. Monitor upgrade
journalctl -u x3-chain-node -f
```

---

## Verification Checklist

Before deploying to production:

- [ ] Extract and verify checksums: `sha256sum -c CHECKSUMS.sha256`
- [ ] Verify GPG signature: `gpg --verify CHECKSUMS.sha256.asc CHECKSUMS.sha256`
- [ ] Run health check: `NODE_NAME=test bash scripts/x3_node_healthcheck.sh --mode prod`
- [ ] Test startup: `./scripts/run-dev-node.sh` and verify RPC responds
- [ ] Read operator SOP: `docs/X3_OPERATOR_SOP.md`
- [ ] Configure multi-validator cluster per SOP (if applicable)
- [ ] Set up monitoring (Prometheus scraping port 9615)

---

## Known Limitations & Future Work

### Current Limitations

1. **SDK Publication (SDK-007):** TypeScript SDK not yet published to npm
   - Decision: Defer to post-deployment (not a blocking release issue)
   - Path: Publish `packages/blockchain-connector` after testnet stabilizes

2. **WebSocket E2E Tests:** Requires live testnet node
   - Decision: Will be validated during testnet deployment
   - Reference: `integration-tests/cross-vm-pallet-test.rs` (test marked `#[ignore]`)

3. **Native Gas Metering:** GasEstimationRPC is simulation-only
   - Decision: Use Frontier RPC for production gas estimation
   - Migration path: See deprecation note in gas_estimation.rs

### Future Improvements (Post-v1.1)

- SDK npm package publication (SDK-007)
- Native gas metering in Frontier
- Cross-chain bridge to Cosmos/Ethereum (Phase 9)
- GPU acceleration for EVM (Phase 10)

---

## Support & Feedback

### Resources

- **Operator SOP:** See `docs/X3_OPERATOR_SOP.md` in the release package
- **Development Guide:** See `docs/DEVELOPMENT.md`
- **Source Code:** https://github.com/x3-chain/x3-chain-master

### Reporting Issues

For deployment or operational issues:

1. Check troubleshooting section in `X3_OPERATOR_SOP.md`
2. Run health check script: `bash scripts/x3_node_healthcheck.sh --mode prod`
3. Capture logs: `journalctl -u x3-chain-node -n 1000`
4. File issue with logs and reproduction steps

---

## Release Commits

Phase 8 included commits:
- c1afa7835: Health check script + DEVELOPMENT.md
- e69a0a88c: X3_OPERATOR_SOP.md
- ef7506d84: progress.txt updates
- (Phase 6-7 commits): See git log for full history

---

## Signed By

**Release Manager:** X3 Chain Core Engineering  
**GPG Key:** C1ACCB82467C41F9 (X3 Chain Release <release@x3-chain.io>)  
**Date:** 2026-03-24  
**Confidence:** Production-ready for testnet deployment

---

## License

X3 Chain is released under the Apache License 2.0.
See LICENSE file in the repository for details.

---

*This release represents the culmination of Phase 8 (Testnet Proving and Go/No-Go). All gates passed. Ready for operator deployment and testnet validation.*
