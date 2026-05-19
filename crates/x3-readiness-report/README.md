# x3-readiness-report

Readiness reporting crate for X3 Atomic Star v0.4.

## Scope

This crate provides an evidence-driven readiness snapshot for launch gates. It is intentionally conservative:

- `Unknown` means not proven.
- `Unknown` never counts as pass.
- Overall readiness is true only when every gate is `Pass`.

## Public API

- `get_readiness() -> ReadinessReport`
- `Collector::collect_offline()`
- `Collector::collect_live(rpc_override)`

## Modules

- `kernel_checks.rs` - kernel health checks
- `gateway_checks.rs` - gateway launch-gate aggregation
- `consensus_checks.rs` - consensus visibility checks
- `invariants.rs` - critical invariant checks
- `collector.rs` - report collection entrypoints
- `formatter.rs` - text/json formatting

## CLI

Run from workspace root:

```bash
cargo run -p x3-readiness-report --bin readiness-cli -- --offline --text
cargo run -p x3-readiness-report --bin readiness-cli -- --json --rpc http://127.0.0.1:9944
```

## Exit semantics

The CLI currently prints a report and exits 0. It is designed for pipeline consumption and can be wrapped by launch-gate scripts.
