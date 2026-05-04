# Phase 07 Release Artifacts

Date: 2026-03-20

## Generated tarballs

Location: `.artifacts/phase7-packages/`

- `x3-chain-ts-sdk-0.1.0.tgz`
- `x3-chain-blockchain-connector-0.1.0.tgz`
- `x3-chain-x3-wallet-0.1.0.tgz`
- `x3-chain-polkawallet-bridge-adapter-0.1.0.tgz`

## Validation commands

- `npm pack --dry-run --workspace packages/ts-sdk`
- `npm pack --dry-run --workspace packages/blockchain-connector`
- `npm pack --dry-run --workspace packages/polkawallet-plugin`
- `npm pack --dry-run --workspace packages/polkawallet-bridge-adapter`

- `npm pack --workspace packages/ts-sdk --pack-destination .artifacts/phase7-packages`
- `npm pack --workspace packages/blockchain-connector --pack-destination .artifacts/phase7-packages`
- `npm pack --workspace packages/polkawallet-plugin --pack-destination .artifacts/phase7-packages`
- `npm pack --workspace packages/polkawallet-bridge-adapter --pack-destination .artifacts/phase7-packages`

## Packaged usage docs

- `packages/ts-sdk/docs/root/README.md`
- `packages/blockchain-connector/docs/root/README.md`
- `packages/polkawallet-plugin/docs/root/README.md`
- `packages/polkawallet-bridge-adapter/docs/root/README.md`
