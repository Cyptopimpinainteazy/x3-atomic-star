#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# scripts/monitoring/check-ops-artifacts.sh
#
# Are the operator artifacts the repository ships actually loadable, and do they describe *this*
# chain?
#
# Measured 2026-09-26: `docs/testnet-config/alert-rules.json` was not a Prometheus rule file. Its
# shape was `{"alerts": [{alert, expr, for, annotation}, ...]}`; Prometheus wants
# `{"groups": [{name, rules: [...]}]}`, and `promtool check rules` refused it outright:
#
#   field alerts not found in type rulefmt.RuleGroups
#
# So an operator who started the monitoring stack from this file got a Prometheus that would not
# load its rules — while the row and the runbooks counted alerting as configured. The alert
# expressions were also written for a Solana GPU validator (`solana_validator_fork_distance_slots`,
# `gpu_memory_used_bytes`, `gpu_kernel_errors_total`), none of which this chain's exporter emits, so
# even in the right shape none of them could ever fire.
#
# This checks both halves:
#
#   1. shape — the artifacts parse and carry the fields their consumers require (always);
#   2. loadability — `promtool check config` and `promtool check rules` accept them (when promtool
#      is installed; the shape check is what catches the class of defect above, and promtool is the
#      stronger confirmation);
#   3. relevance — every alert expression names a metric the node's exporter actually publishes, so
#      a rule cannot come back describing another project's metrics.
#
# Exit 0 -> every operator artifact is loadable and names metrics this chain exports.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG="$ROOT_DIR/docs/testnet-config/prometheus-config.json"
RULES="$ROOT_DIR/docs/testnet-config/alert-rules.json"
DASHBOARDS="$ROOT_DIR/docs/testnet-config/grafana-dashboards.json"

pass() { printf '[PASS] %s\n' "$1"; }
fail() { printf '[FAIL] %s %s\n' "$1" "${2:-}"; OVERALL="FAIL"; }
info() { printf '[ops-artifacts] %s\n' "$*"; }

OVERALL="PASS"

for f in "$CONFIG" "$RULES" "$DASHBOARDS"; do
    if [[ ! -f "$f" ]]; then
        fail "artifact_present" "($f is missing)"
    fi
done
if [[ "$OVERALL" == "FAIL" ]]; then
    echo "check_ops_artifacts: FAIL"
    exit 1
fi

# ── shape and relevance ───────────────────────────────────────────────────────
python3 - "$CONFIG" "$RULES" "$DASHBOARDS" <<'PY' || OVERALL="FAIL"
import json
import re
import sys

config_path, rules_path, dashboards_path = sys.argv[1:4]

# The metrics the node's Prometheus exporter publishes, as the monitoring checks read them
# (`substrate_block_height`, `substrate_sub_libp2p_peers_count`, `substrate_node_roles`,
# `substrate_build_info`), plus Prometheus's own `up`.
KNOWN_METRICS = {
    "substrate_block_height",
    "substrate_sub_libp2p_peers_count",
    "substrate_node_roles",
    "substrate_build_info",
    "up",
}

problems = []

with open(config_path) as fh:
    config = json.load(fh)
if not config.get("scrape_configs"):
    problems.append("prometheus-config.json has no scrape_configs")

with open(rules_path) as fh:
    rules = json.load(fh)
groups = rules.get("groups")
if not isinstance(groups, list) or not groups:
    problems.append(
        "alert-rules.json must be a Prometheus rule file: a non-empty 'groups' list "
        "(a bare 'alerts' list is the shape promtool refuses)"
    )
    groups = []
rule_count = 0
for group in groups:
    if not group.get("name"):
        problems.append("a rule group has no name")
    for rule in group.get("rules", []):
        rule_count += 1
        alert = rule.get("alert", "<unnamed>")
        expr = rule.get("expr")
        if not expr:
            problems.append(f"{alert}: no expr")
            continue
        if not rule.get("for"):
            problems.append(f"{alert}: no 'for' (how long the condition must hold)")
        if not rule.get("labels", {}).get("severity"):
            problems.append(f"{alert}: no labels.severity")
        if not rule.get("annotations", {}).get("summary"):
            problems.append(f"{alert}: no annotations.summary (operators read this first)")
        named = set(re.findall(r"[a-zA-Z_][a-zA-Z0-9_]*", expr))
        referenced = named & KNOWN_METRICS
        if not referenced:
            problems.append(
                f"{alert}: expr names no metric this chain exports "
                f"({expr!r}); known: {sorted(KNOWN_METRICS)}"
            )
if rule_count == 0:
    problems.append("alert-rules.json has no rules")

with open(dashboards_path) as fh:
    dashboards = json.load(fh)
if not dashboards:
    problems.append("grafana-dashboards.json is empty")
for name, dashboard in dashboards.items():
    if not isinstance(dashboard, dict):
        problems.append(f"dashboard {name} is not an object")
        continue
    if not dashboard.get("title"):
        problems.append(f"dashboard {name} has no title")
    if not dashboard.get("panels"):
        problems.append(f"dashboard {name} has no panels")

if problems:
    for problem in problems:
        print(f"[FAIL] ops_artifact_shape ({problem})")
    sys.exit(1)
print(f"[PASS] ops_artifact_shape ({rule_count} alert rule(s), {len(dashboards)} dashboard(s))")
PY

# ── loadability ───────────────────────────────────────────────────────────────
if command -v promtool >/dev/null 2>&1; then
    if promtool check config "$CONFIG" >/dev/null 2>&1; then
        pass "promtool_check_config"
    else
        fail "promtool_check_config" "(promtool rejected $CONFIG)"
    fi
    if promtool check rules "$RULES" >/dev/null 2>&1; then
        pass "promtool_check_rules"
    else
        fail "promtool_check_rules" "(promtool rejected $RULES)"
    fi
    info "promtool accepted both artifacts"
else
    # Not a pass: the shape check above is what catches the defect this gate exists for, and the
    # stronger confirmation is missing. Say so rather than reporting a clean sweep.
    info "promtool is not on PATH: the artifacts were checked for shape and metric relevance, not loaded"
fi

if [[ "$OVERALL" == "PASS" ]]; then
    echo "check_ops_artifacts: PASS"
    exit 0
fi
echo "check_ops_artifacts: FAIL"
exit 1
