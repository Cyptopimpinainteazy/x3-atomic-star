# Current release status

This repository is a blockchain source/development candidate. It is not certified production-ready and no readiness percentage is used as a release certificate. XXXSTAR remains active. Draft PR #185 contains the initial extraction; it has not been represented here as merged.

## Established initial extraction evidence

At candidate `b74015420efcded666bdc61d46436d23202801f9`, local checks passed for the node and embedded WASM runtime, 68 canonical-bytecode tests, three compiler-schema tests, source identity, dependency resolution and launcher failure/ownership paths. See [the exact recorded results](../../audit-artifacts/migration/verification.json).

These observations do not establish a release binary, running network, partition/recovery execution, live cleanup signal delivery, GPU benchmark or settlement. The follow-up organization restores supporting modules and maintains separate evidence for its own checks. Restored code is not automatically verified by the earlier extraction tests.

## Required before release

- Exact-head CI and relevant unit/integration gates must pass.
- Build and execute the intended release node/runtime feature set.
- Verify isolated multi-validator finality, restart, snapshot restore and partition/recovery behavior.
- Verify atomic success and failure/refund/replay paths at the actual execution boundary.
- Resolve applicable security findings, review authority/custody and bind artifacts to reviewed source.
- Use an explicit reviewed deployment/upgrade procedure; repository organization grants no chain, key or payment authority.

Source registry scores, archived reports and historical CI claims are supporting history, not proof these requirements have been completed.

## Experimental utilities

The legacy RPC probe and health prototype are not accepted release gates. Their known limitations are recorded in [experimental utilities](../verification/EXPERIMENTAL.md). Compilation and their unit tests do not correct those limitations.

## Organization follow-up checks

The restored support modules passed 399 local tests, and the maintained one-command core check passed, including a fresh node/embedded-runtime check. Documentation checks and source-identity checks also passed. [The follow-up evidence record](../../audit-artifacts/organization/verification.json) separates those results from the initial extraction. These are local checks; hosted CI and live-chain release requirements remain separate.
