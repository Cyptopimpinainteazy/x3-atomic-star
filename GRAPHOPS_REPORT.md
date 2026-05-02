# GraphOps Report

Status: initialized and validated

## Graph Build Evidence

- Command: `python3 .scripts/x3_graph_builder.py`
- Result: `nodes=45638 edges=52453 unreadable=0 skipped_large=9`
- Outputs:
  - `.x3/graph/nodes.json`
  - `.x3/graph/edges.json`
  - `.x3/graph/index.md`
  - `.x3/graph/unreadable.json`
  - `.x3/graph/skipped_large.json`

## Invariant Dashboard Evidence

- Command: `python3 .scripts/x3_invariant_dashboard.py`
- Output: `.x3/dashboards/INVARIANT_COVERAGE.md`

## Highest-Risk Feature Buckets

- Universal Asset Kernel: needs review; 541 mapped code/config files, 96 test files, 372 risky files.
- Bridge / Router: needs review; 265 mapped code/config files, 44 test files, 182 risky files.
- EVM Integration: needs review; 178 mapped code/config files, 25 test files, 90 risky files.
- SVM Integration: needs review; 41 mapped code/config files, 7 test files, 17 risky files.
- X3 DEX: needs review; 307 mapped code/config files, 21 test files, 125 risky files.

## Missing Feature Buckets

- Liquidity Locks: no mapped code/test surface found by GraphOps-lite.
- Anti-Rug Mechanics: no mapped code/test surface found by GraphOps-lite.

## Limits

- This is GraphOps-lite, not a formal AST/callgraph proof.
- Feature classification is heuristic and must be verified against code before patching.
- Large files over 1 MB are recorded in `.x3/graph/skipped_large.json`, not silently treated as processed.

## Next Graph Improvements

- Add Rust module import/call edges.
- Add Solidity import/inheritance edges.
- Add test-to-feature edges from file names and manifests.
- Add generated-artifact exclusion tests for the graph builder.
