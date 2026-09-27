#!/usr/bin/env python3
"""X3 Northern Swarm + Reactor mainnet hard gate.

This gate is intentionally stricter than "the crate compiles".  It blocks a
mainnet-ready claim until the on-chain compute path is real end-to-end:

* Northern Swarm is part of the workspace and runtime.
* The pallet has real tests and benchmark-derived weights.
* Task/result settlement is not single-executor auto-finalization.
* The off-chain watcher reads the storage/events the pallet actually exposes.
* Result submission is signed by the registered executor; no hard-coded pallet
  index or unsigned-call shortcut is accepted.
* AI/GPU work has a typed backend with a CPU fallback; accelerators must not be
  a consensus dependency.
* Both the pallet and executor test suites pass.

The current repository is expected to FAIL this gate until those requirements
are implemented.  That is the point: missing swarm/reac­tor work must be visible
in the same release bar as consensus, storage, bridges, and genesis.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
FAILURES: list[str] = []


def fail(message: str) -> None:
    FAILURES.append(message)
    print(f"  ✗ {message}")


def ok(message: str) -> None:
    print(f"  ✓ {message}")


def read(rel: str) -> str:
    path = ROOT / rel
    if not path.exists():
        fail(f"required path missing: {rel}")
        return ""
    return path.read_text(encoding="utf-8", errors="ignore")


def require_text(rel: str, text: str, message: str | None = None) -> None:
    body = read(rel)
    if text not in body:
        fail(message or f"{rel} missing required marker: {text}")
    else:
        ok(message or f"{rel} contains {text}")


def forbid_text(rel: str, text: str, message: str) -> None:
    body = read(rel)
    if text in body:
        fail(message)
    else:
        ok(message)


def run_tests(package: str) -> None:
    print(f"  running {package} tests...")
    result = subprocess.run(
        ["cargo", "test", "-p", package, "--all-targets", "--no-fail-fast", "-q"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        fail(f"{package} tests failed")
        for line in (result.stdout + result.stderr).splitlines()[-20:]:
            print(f"    {line}")
    else:
        ok(f"{package} tests passed")


def check_runtime_wiring() -> None:
    print("\n── Swarm 1. Workspace/runtime wiring ──")
    require_text(
        "Cargo.toml",
        '"crates/northern-swarm"',
        "off-chain northern-swarm executor is a workspace member",
    )
    require_text(
        "Cargo.toml",
        '"pallets/northern-swarm"',
        "on-chain northern-swarm pallet is a workspace member",
    )
    require_text(
        "runtime/Cargo.toml",
        'pallet-northern-swarm = { path = "../pallets/northern-swarm"',
        "runtime depends on pallet-northern-swarm",
    )
    require_text(
        "runtime/Cargo.toml",
        '"pallet-northern-swarm/std"',
        "runtime propagates pallet-northern-swarm/std",
    )
    require_text(
        "runtime/src/lib.rs",
        "impl pallet_northern_swarm::Config for Runtime",
        "runtime implements pallet_northern_swarm::Config",
    )
    require_text(
        "runtime/src/lib.rs",
        "NorthernSwarm: pallet_northern_swarm",
        "NorthernSwarm is present in construct_runtime!",
    )


def check_pallet_contract() -> None:
    print("\n── Swarm 2. On-chain pallet safety contract ──")
    pallet = read("pallets/northern-swarm/src/lib.rs")

    tests = ROOT / "pallets/northern-swarm/src/tests.rs"
    mock = ROOT / "pallets/northern-swarm/src/mock.rs"
    if not tests.exists() or not mock.exists():
        fail("pallet-northern-swarm needs real mock.rs + tests.rs coverage")
    else:
        ok("pallet-northern-swarm mock/test files exist")

    if "#[cfg(test)]" not in pallet or "mod tests" not in pallet:
        fail("pallet-northern-swarm tests are not wired into the crate")
    else:
        ok("pallet-northern-swarm tests are wired")

    if re.search(r"#\[pallet::weight\(Weight::from_parts\(", pallet):
        fail(
            "pallet-northern-swarm still uses hand-written Weight::from_parts "
            "constants; benchmark-generated weights are required"
        )
    else:
        ok("pallet weights are not hard-coded Weight::from_parts constants")

    if "type QuorumThreshold" not in pallet:
        fail("no on-chain quorum threshold exists for result finalization")
    else:
        ok("on-chain quorum threshold is configured")

    if "task.status == TaskStatus::ResultCommitted" in pallet:
        fail(
            "on_finalize still auto-finalizes a single committed result; "
            "M-of-N verification is required"
        )
    else:
        ok("single-result auto-finalization path is absent")

    required_pallet_tests = (
        "quorum_requires_matching_results",
        "task_reward_moves_reserved_balance_to_winner",
        "task_reward_preserves_total_issuance",
    )
    test_sources = ""
    for rel in ("pallets/northern-swarm/src/tests.rs", "pallets/northern-swarm/src/lib.rs"):
        path = ROOT / rel
        if path.exists():
            test_sources += path.read_text(encoding="utf-8", errors="ignore")
    for test_name in required_pallet_tests:
        if not re.search(rf"fn\s+{re.escape(test_name)}\s*\(", test_sources):
            fail(f"missing required pallet behavior test: {test_name}")
        else:
            ok(f"pallet behavior test exists: {test_name}")


def check_chain_executor_contract() -> None:
    print("\n── Swarm 3. Off-chain executor ↔ chain contract ──")
    watcher = read("crates/northern-swarm/src/chain_watcher.rs")
    submitter = read("crates/northern-swarm/src/result_submitter.rs")

    if "PendingTasks" in watcher or "PENDING_TASKS_KEY" in watcher:
        fail(
            "chain watcher targets PendingTasks, but the pallet exposes Tasks; "
            "use runtime metadata/events or the real Tasks storage layout"
        )
    else:
        ok("chain watcher does not target the obsolete/nonexistent PendingTasks storage")

    if "TaskSubmitted" not in watcher and "Tasks" not in watcher:
        fail("chain watcher has no visible TaskSubmitted event or Tasks storage path")
    else:
        ok("chain watcher follows the pallet's actual task surface")

    if "unsigned extrinsic" in submitter.lower():
        fail(
            "result submitter still uses an unsigned extrinsic while submit_result "
            "requires ensure_signed"
        )
    else:
        ok("result submitter does not describe/use the unsigned shortcut")

    if re.search(r"pallet_index\s*:\s*82u8", submitter):
        fail("result submitter hard-codes pallet index 82; runtime metadata must drive call encoding")
    else:
        ok("result submitter does not hard-code pallet index 82")

    signing_markers = ("subxt", "Pair::sign", ".sign(", "SignedPayload", "signing")
    if not any(marker in submitter for marker in signing_markers):
        fail("no executor signing path is visible in result submission")
    else:
        ok("executor result submission contains a signing path")


def check_gpu_backend_contract() -> None:
    print("\n── Swarm 4. GPU/backend contract ──")
    types = read("crates/northern-swarm/src/types.rs")
    executor_dir = ROOT / "crates/northern-swarm/src"
    combined = "\n".join(
        p.read_text(encoding="utf-8", errors="ignore")
        for p in executor_dir.glob("*.rs")
        if p.is_file()
    )

    if "pub kind: TaskKind" not in types:
        fail(
            "TaskPayload drops TaskKind before execution; AiInference cannot be "
            "routed to a GPU/NPU backend"
        )
    else:
        ok("TaskPayload preserves TaskKind for backend routing")

    if "trait ComputeBackend" not in combined:
        fail("no ComputeBackend abstraction exists for CPU/GPU interchangeable execution")
    else:
        ok("ComputeBackend abstraction exists")

    gpu_markers = ("GpuBackend", "CudaBackend", "RocmBackend", "gpu_backend")
    if not any(marker in combined for marker in gpu_markers):
        fail("no GPU backend is implemented in northern-swarm")
    else:
        ok("GPU backend is implemented")

    cpu_markers = ("CpuBackend", "CpuFallback", "cpu_backend")
    if not any(marker in combined for marker in cpu_markers):
        fail("no explicit CPU fallback backend exists")
    else:
        ok("CPU fallback backend exists")


def check_tests() -> None:
    print("\n── Swarm 5. Executable tests ──")
    run_tests("pallet-northern-swarm")
    run_tests("northern-swarm")


def main() -> int:
    print("═" * 64)
    print("  X3 Northern Swarm + Reactor On-Chain V1 Release Gate")
    print("═" * 64)

    check_runtime_wiring()
    check_pallet_contract()
    check_chain_executor_contract()
    check_gpu_backend_contract()
    check_tests()

    print("\n" + "═" * 64)
    if FAILURES:
        print(f"  ❌ SWARM/REACTOR GATE FAILED — {len(FAILURES)} failure(s)")
        for item in FAILURES:
            print(f"    • {item}")
        print("═" * 64)
        return 1

    print("  ✅ swarm_reactor_gate: PASS")
    print("═" * 64)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
