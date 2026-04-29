#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
TIMESTAMP="$(date +%Y%m%d-%H%M%S)"
OUT_DIR="${REPO_ROOT}/launch-gates/evidence/ci/startup-gate-${TIMESTAMP}"
# Use shared startup-gate targets by default so reruns reuse prior build
# artifacts. Each job gets a dedicated subdirectory to avoid lock contention.
TARGET_MODE="${STARTUP_GATE_TARGET_MODE:-shared}"
SHARED_TARGET_DIR="${REPO_ROOT}/launch-gates/evidence/ci/startup-gate-target"
if [[ -n "${STARTUP_GATE_TARGET_DIR:-}" ]]; then
  TARGET_DIR="${STARTUP_GATE_TARGET_DIR}"
elif [[ "${TARGET_MODE}" == "shared" ]]; then
  TARGET_DIR="${SHARED_TARGET_DIR}"
else
  TARGET_DIR="${REPO_ROOT}/launch-gates/evidence/ci/startup-gate-target-${TIMESTAMP}"
fi

if [[ "${TARGET_MODE}" == "shared" ]] && pgrep -f -- "--target-dir ${TARGET_DIR}" >/dev/null 2>&1; then
  echo "startup-gate-run: shared target busy; falling back to isolated target for this run"
  TARGET_MODE="isolated-fallback"
  TARGET_DIR="${REPO_ROOT}/launch-gates/evidence/ci/startup-gate-target-${TIMESTAMP}"
fi
LOCK_FILE="${STARTUP_GATE_LOCK_FILE:-${REPO_ROOT}/launch-gates/evidence/ci/startup-gate.lock}"
TMP_DIR="${STARTUP_GATE_TMPDIR:-${REPO_ROOT}/launch-gates/evidence/ci/startup-gate-tmp}"
RUSTFLAGS_VALUE="${STARTUP_GATE_RUSTFLAGS:--C debuginfo=0 -C link-arg=-fuse-ld=lld}"

mkdir -p "$(dirname "${LOCK_FILE}")"
exec 9>"${LOCK_FILE}"
if ! flock -n 9; then
  echo "Another startup-gate run is already active (lock: ${LOCK_FILE})."
  exit 75
fi

mkdir -p "${OUT_DIR}"
mkdir -p "${TMP_DIR}"
mkdir -p "${TARGET_DIR}"

echo "startup-gate-run: out_dir=${OUT_DIR}"
echo "startup-gate-run: target_dir=${TARGET_DIR}"
echo "startup-gate-run: target_mode=${TARGET_MODE}"
echo "startup-gate-run: rustflags=${RUSTFLAGS_VALUE}"
echo "startup-gate-run: tmp_dir=${TMP_DIR}"

{
  echo "matrix_id=${TIMESTAMP}"
  echo "started_at=$(date -Is)"
  echo "workspace=${REPO_ROOT}"
  echo "target_mode=${TARGET_MODE}"
  echo "target_dir=${TARGET_DIR}"
  echo "rustflags=${RUSTFLAGS_VALUE}"
} > "${OUT_DIR}/summary.env"

echo "job,result,exit_code,duration_sec,log_file" > "${OUT_DIR}/matrix.csv"

run_job() {
  local name="$1"
  shift
  local log_file="${OUT_DIR}/${name}.log"
  local status_file="${OUT_DIR}/${name}.status"
  local heartbeat_sec="${STARTUP_GATE_HEARTBEAT_SECONDS:-30}"
  local start_ts
  start_ts=$(date +%s)

  {
    echo "=== JOB: ${name} ==="
    echo "pwd=${REPO_ROOT}"
    echo "command: $*"
  } > "${log_file}"

  set +e
  (
    cd "${REPO_ROOT}"
    "$@"
  ) >> "${log_file}" 2>&1 &
  local cmd_pid=$!

  while kill -0 "${cmd_pid}" >/dev/null 2>&1; do
    echo "${name} => RUNNING (pid=${cmd_pid}, elapsed=$(( $(date +%s) - start_ts ))s)"
    sleep "${heartbeat_sec}"
  done

  wait "${cmd_pid}"
  local ec=$?
  set -e
  local end_ts
  end_ts=$(date +%s)
  local dur=$((end_ts - start_ts))
  local result="FAIL"
  if [[ ${ec} -eq 0 ]]; then
    result="PASS"
  fi

  {
    echo "result=${result}"
    echo "exit_code=${ec}"
    echo "duration_sec=${dur}"
    echo "log_file=$(basename "${log_file}")"
  } > "${status_file}"

  echo "${name},${result},${ec},${dur},$(basename "${log_file}")" >> "${OUT_DIR}/matrix.csv"
  echo "${name} => ${result} (exit=${ec}, ${dur}s)"

  return ${ec}
}

COMMON_ENV=(env CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 RUSTFLAGS="${RUSTFLAGS_VALUE}" TMPDIR="${TMP_DIR}" CXXFLAGS=-pipe)

run_job runtime_gate_reference "${COMMON_ENV[@]}" cargo test -p x3-chain-runtime fraud_proofs::startup_gate::tests::gate_passes_with_reference_scheduler --lib --target-dir "${TARGET_DIR}/runtime" -- --nocapture || true
run_job runtime_gate_deterministic "${COMMON_ENV[@]}" cargo test -p x3-chain-runtime fraud_proofs::startup_gate::tests::gate_is_deterministic --lib --target-dir "${TARGET_DIR}/runtime" -- --nocapture || true
run_job node_gate_reference "${COMMON_ENV[@]}" cargo test -p x3-chain-node startup_gate_passes_for_reference_authority_build --lib --target-dir "${TARGET_DIR}/node" -- --nocapture || true

FAILS=$(awk -F, 'NR>1 && $2=="FAIL" {c++} END {print c+0}' "${OUT_DIR}/matrix.csv")
PASSES=$(awk -F, 'NR>1 && $2=="PASS" {c++} END {print c+0}' "${OUT_DIR}/matrix.csv")

{
  echo "finished_at=$(date -Is)"
  echo "passes=${PASSES}"
  echo "fails=${FAILS}"
} >> "${OUT_DIR}/summary.env"

echo "OUT_DIR=${OUT_DIR}"
cat "${OUT_DIR}/matrix.csv"

if [[ "${FAILS}" -ne 0 ]]; then
  exit 1
fi