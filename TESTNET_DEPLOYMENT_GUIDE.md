# X3 Chain Testnet Deployment Guide

This guide covers the deployment and key management procedures for X3 Chain testnet.

## Table of Contents

- [Overview](#overview)
- [Node Key Generation](#node-key-generation)
- [Key Management](#key-management)
- [Chain Specification](#chain-specification)
- [Deployment Checklist](#deployment-checklist)

## Overview

X3 Chain testnet requires a minimum of 3 validator nodes for proper consensus. Each node needs:

1. A unique ed25519 node key for P2P networking
2. Aura (sr25519) keys for block production
3. Grandpa (ed25519) keys for finality

## Node Key Generation

### Generating Node Keys

Run the key generation script:

```bash
python3 scripts/generate-node-keys/generate_keys.py
```

This will:
- Generate 3 ed25519 keypairs
- Output multiaddrs to `deployment/keys/bootnode-info.txt`
- Output key data to `deployment/keys/bootnode-keys.json`

### Manual Key Generation

If you need to generate keys manually:

```bash
# Using Rust with sp-core
cargo run --release --bin generate-node-keys
```

Or using Python:

```python
from nacl.signing import SigningKey
import hashlib
import base58

# Generate ed25519 keypair
signing_key = SigningKey.generate()
public_key = signing_key.verify_key.encode()
secret_key = signing_key.encode()

# Compute peer ID (base58(sha256(public_key)))
peer_id_hash = hashlib.sha256(public_key).digest()
peer_id = base58.b58encode(peer_id_hash).decode()

print(f"Peer ID: {peer_id}")
print(f"Multiaddr: /ip4/127.0.0.1/tcp/30333/p2p/{peer_id}")
```

## Key Management

### Storage

- **Node keys**: Store in `deployment/keys/` (gitignored)
- **Validator keys**: Store in a secure HSM or encrypted vault
- **Backup keys**: Store in a secure offline location

### Security Best Practices

1. **Never commit keys to version control**
   - The `deployment/keys/` directory is gitignored
   - Use `.gitignore` to exclude sensitive files

2. **Use environment variables for sensitive data**
   ```bash
   export X3_NODE_KEY="base64-encoded-secret-key"
   export X3_AURA_KEY="base64-encoded-secret-key"
   export X3_GRANDPA_KEY="base64-encoded-secret-key"
   ```

3. **Rotate keys periodically**
   - Generate new keys every 90 days
   - Update chain specification with new keys
   - Perform a scheduled upgrade

4. **Backup keys securely**
   - Encrypt backups with AES-256-GCM
   - Store backups in multiple locations
   - Test key recovery procedures

### Key File Structure

```
deployment/keys/
├── bootnode-info.txt      # Multiaddrs for bootnodes (public)
├── bootnode-keys.json     # Full key data (secret, gitignored)
├── validator-keys/        # Validator key storage
│   ├── node1/
│   │   ├── node_key       # ed25519 node key
│   │   ├── aura_key       # sr25519 aura key
│   │   └── grandpa_key    # ed25519 grandpa key
│   └── node2/
│       └── ...
└── backups/               # Encrypted key backups
    ├── node1-backup.enc
    └── node2-backup.enc
```

## Chain Specification

### Updating Bootnodes

After generating keys, update the chain specification:

```json
{
  "bootNodes": [
    "/ip4/127.0.0.1/tcp/30333/p2p/81SQketjqCBnnCb8rQw1dThQS1AJnqMjYZ2RAr2cN8Sc",
    "/ip4/127.0.0.1/tcp/30333/p2p/F31BGgebnyLRvMTerSxnMFSmWtNJFpVS3E34qiKkjNo2",
    "/ip4/127.0.0.1/tcp/30333/p2p/8wwRqSDhQAw44KbFH4x8CJ1cJPZZs9y2U9m3xCHe4rRP"
  ]
}
```

### Building Chain Specification

```bash
# Build raw chain spec
./target/release/x3-chain-node build-spec \
    --chain=dev \
    --raw \
    --disable-default-bootnode \
    > deployment/chain-specs/x3-testnet-raw.json

# Update bootnodes in the chain spec
# (edit deployment/chain-specs/x3-testnet-raw.json)
```

### Loading Bootnodes from Environment

The node can load bootnodes from environment variables:

```bash
# Single bootnode
export TESTNET_BOOTNODES="/ip4/127.0.0.1/tcp/30333/p2p/81SQketjqCBnnCb8rQw1dThQS1AJnqMjYZ2RAr2cN8Sc"

# Multiple bootnodes (comma-separated)
export TESTNET_BOOTNODES="/ip4/127.0.0.1/tcp/30333/p2p/81SQketjqCBnnCb8rQw1dThQS1AJnqMjYZ2RAr2cN8Sc,/ip4/127.0.0.1/tcp/30333/p2p/F31BGgebnyLRvMTerSxnMFSmWtNJFpVS3E34qiKkjNo2"
```

## Deployment Checklist

### Pre-Deployment

- [ ] Generate node keys for all validators
- [ ] Store keys securely (encrypted, backed up)
- [ ] Update chain specification with real peer IDs
- [ ] Configure environment variables
- [ ] Verify network connectivity between nodes

### Deployment

- [ ] Start validator nodes with correct keys
- [ ] Verify nodes are syncing
- [ ] Check P2P connections
- [ ] Verify block production
- [ ] Verify finality

### Post-Deployment

- [ ] Monitor node health
- [ ] Check block production rate
- [ ] Verify finality gadget progress
- [ ] Set up monitoring and alerting

## Troubleshooting

### Node Not Connecting

- Check firewall rules
- Verify bootnode multiaddrs are correct
- Check network connectivity

### Block Production Stalled

- Verify Aura keys are loaded
- Check clock synchronization
- Verify validator set

### Finality Not Progressing

- Verify Grandpa keys are loaded
- Check validator participation
- Verify network connectivity

## References

- [Substrate Node Keys](https://docs.substrate.io/reference/command-line-tools/subkey/)
- [Chain Specification](https://docs.substrate.io/how-to-guides/v3/advanced/chainspec/)
- [P2P Networking](https://docs.substrate.io/reference/architecture/networking/)
