# Dependabot job failures on `master` — 2026-09-28

Source: `gh run list --branch master --limit 60`. Every failing run on the
default branch at `53efb3a793` is a **Dependabot Updates** job; there are no
failing repository workflows. The jobs come in three shapes, and each one ends
red for a different reason:

| Job (name fragment) | Directories in the job | Error |
| --- | --- | --- |
| `cargo in /.` | `/.` | `ed25519-dalek`: `dependency_file_not_resolvable` |
| `cargo in /X3-contracts/svm, …` | `/X3-contracts/svm`, `/crates/dylint-determinism`, `/crates/gpu-swarm`, `/crates/x3-gulfstream`, `/x3-autonomic-core` | `No Cargo.toml!` at `/crates/gpu-swarm` |
| `npm_and_yarn in /apps/explorer, …` | `/apps/explorer`, `/packages/atomic-swap-sdk`, `/packages/blockchain-connector`, `/packages/polkawallet-bridge-adapter`, `/packages/ts-sdk`, `/tests/wallet-integration`, `/tests_core/wallet-integration`, `/x3-app-store/backend`, `/x3-app-store/frontend` | `elliptic`: `security_update_not_found` |

The directory lists are not read from `.github/dependabot.yml`. They are the
manifests that carry the open **security** alerts, which is why a fix has to be
either "make the alert go away" or "teach the matching config entry to ignore
it".

## 1. `ed25519-dalek` — the `cargo in /.` job

Alert #222 (`GHSA-w5vr-6qhr-36cc`, medium, "Double Public Key Signing Function
Oracle Attack", `< 2.0.0`, patched `2.0.0`), reported against the root
`Cargo.lock`. The vulnerable copy is `ed25519-dalek 1.0.1`, required by
`agave-precompiles 3.0.14` (`Cargo.lock` line 314) — a Solana crate reached
through `crates/svm-integration`. `agave` pins `^1.0`, so the patched line
cannot be selected.

The job reported:

```
error: failed to select a version for `ed25519-dalek`.
    ... required by package `x3-common v0.1.0`
versions that meet the requirements `=3.0.0` are: 3.0.0
package `x3-common` depends on `ed25519-dalek` with feature `std` but
`ed25519-dalek` does not have that feature.
```

Two independent defects are visible in that message:

* `crates/x3-common/Cargo.toml` declared `ed25519-dalek` but **no source file in
  the crate uses it** (`rg 'dalek' crates/x3-common` matched only the manifest).
  It was an unused dependency, and its `std` feature entry was the thing that
  made a 3.x resolution impossible: `ed25519-dalek 3.0.0` dropped `std`
  (`crates.io/api/v1/crates/ed25519-dalek/3.0.0` features), while `2.2.0` still
  has it.
* Even without that feature, `agave-precompiles` cannot take 2.x/3.x, so no
  update exists for the 1.0.1 copy. `cargo tree -i ed25519-dalek@1.0.1` resolves
  nothing, i.e. the copy is not compiled into the node.

Fix: the unused dependency (and its `std` reference) is removed from
`crates/x3-common/Cargo.toml`, and the root `cargo` entry in
`.github/dependabot.yml` ignores `ed25519-dalek` so the job stops trying to
force a version `agave` rejects. Removing the dependency touches the runtime's
dependency graph, so `docs/reports/runtime-wasm-hashes.json` is re-attested in
the same change.

## 2. `lru` — the multi-directory `cargo` job

`No Cargo.toml!` is raised by `dependabot-core`
`cargo/lib/dependabot/cargo/update_checker/file_preparer.rb:397`
(`raise "No Cargo.toml!" if @manifest_files.none?`) when the update checker is
handed a directory's dependency files with no `Cargo.toml` among them. The job
got there because the two extra directories it targets —
`/crates/gpu-swarm` and `/crates/x3-gulfstream` — carry their only alert
(`lru`, #1177 and #398) on `Cargo.toml` rather than a lockfile.

The alert is real: `GHSA-rhfx-m35p-ff5j` ("`IterMut` violates Stacked Borrows
by invalidating internal pointer") affects `>= 0.9.0, < 0.16.3`, patched in
`0.16.3`. Both manifests pinned `lru = "0.12"`, which resolves to `0.12.5`.

Fix: `lru` moves to `0.16.3` in both manifests (resolving `0.16.4`). The only
API in either crate is `LruCache::new(NonZeroUsize)`, `.get`, `.put`,
`.contains` and `.clear`, all unchanged across the bump. Once the alerts close,
those two directories drop out of the job and the remaining three
(`X3-contracts/svm`, `crates/dylint-determinism`, `x3-autonomic-core`) already
process cleanly.

## 3. `elliptic` — the `npm_and_yarn` job

Alert #292 (`GHSA-848j-6mx2-7j84`, low, "Elliptic Uses a Cryptographic
Primitive with a Risky Implementation") is against
`packages/polkawallet-bridge-adapter/package-lock.json`. The advisory covers
`<= 6.6.1` with **no patched version**, and `6.6.1` is the newest published
`elliptic` release (`npm view elliptic versions`), so the job ends in
`security_update_not_found` — there is no version for it to select. The copies
arrive through the legacy `@polkadot/util-crypto` 6.x trees that
`@polkadot/api`'s type-definition packages still carry
(`@digitalnative/type-definitions`, `@polkadot/metadata`,
`pontem-types-bundle`) and through `@ethersproject/signing-key`.

Fix: a `npm` entry for `/packages/polkawallet-bridge-adapter` with
`ignore: elliptic`, so the security job for that directory skips the
dependency instead of inventing an update path that does not exist.

## Verification

```
cargo check -p x3-common                      # ok (dependency removed)
cargo check -p gpu-swarm --no-default-features --features x3-runtime   # ok (lru 0.16.4)
cargo check --manifest-path crates/x3-gulfstream/Cargo.toml            # ok (lru 0.16.4)
SKIP_WASM_BUILD=1 cargo check --workspace     # ok
python3 scripts/check-advisory-scope.py       # ok, 3 records
./scripts/update-runtime-hashes.sh            # runtime record re-attested
```

## Follow-ups

1. `agave-precompiles` pins `ed25519-dalek ^1.0`. Moving `crates/svm-integration`
   to an Agave/Solana line that accepts 2.x would let the `ed25519-dalek`
   ignore in `.github/dependabot.yml` be removed and the alert actually close.
2. `GHSA-848j-6mx2-7j84` and `GHSA-w5vr-6qhr-36cc` are tracked only by this
   `ignore` list today. If the repository wants them enforced the way
   `security/advisory-scope.toml` enforces the yamux/hickory records, they need
   evidence documents and `[[advisory]]` entries.
3. The root `Cargo.lock` still resolves `lru 0.12.5` and `lru 0.7.8` for other
   consumers; only the two workspace members above were in scope here.
