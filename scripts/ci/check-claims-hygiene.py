#!/usr/bin/env python3
"""Does the repository still make claims it cannot back with a command?

The audit registry (`feature-matrix/*.toml`, `docs/audit/*`, `audit-artifacts/*`) *records*
claims — that is its job — so it is out of scope here. This scans the surfaces a person
actually reads or copies: the operator status docs, the release notes, and the outbound
message templates in the desktop CRM. Those are the places a claim becomes a public
statement, and they are the places nothing used to check.

Measured 2026-09-26: `apps/x3-desktop/src-tauri/src/crm/outreach_system.rs` carried
"300ms cross-chain finality (vs 12s on Solana)", "Compute revenue per GPU: $1200/month
(current pilot)", "5,000 TPS baseline", "Live in 3 production networks" and
"Deterministic execution guarantees (no MEV/reorg risk)" — none of it produced by anything
in this tree (`X3-CLAIM-001` established there is no GPU or chain-level TPS benchmark at
all), and all of it text a human would paste into an outbound email. `X3-CLAIM-002`'s
"MEV-proof marketing claim" had been removed from `CURRENT_MAINNET_STATUS.md`, which is why
the row read as half-closed: the claim had moved here.

Three rule sets, because the kinds of claim behave differently:

  * ABSOLUTE  — phrases that are false regardless of context ("MEV-proof", "no MEV",
                "no front-running", "guaranteed finality"). Scanned across the whole tree.
  * NUMERIC   — performance and traction figures. Only a claim when they are asserted as
                results, so these are scanned on the declared claim surfaces and skipped
                when the line itself qualifies the number (target, research, unverified,
                not measured, placeholder, ...).
  * LIVENESS  — statements about *deployment state* ("Now live on testnet", "Testnet is
                live"). Added 2026-09-26 (TICKET-141): every page under `production/public/`
                carried the hero tag `Now live on testnet` while the ledger records that the
                public testnet is not deployed and all local evidence is loopback. A liveness
                tag with nothing behind it is the same class of claim as the retracted MEV
                one (`X3-CLAIM-002`), so it is scanned on the claim surfaces and, like
                NUMERIC, it is skipped when the line qualifies itself (`not live`, `planned`,
                a plan checkbox).

Widened 2026-09-26 (TICKET-139). The surface list used to be root markdown, `docs/testnet-config`
and the CRM; the ledger measured ~150 unqualified figures outside it. Each of those paths is now
decided, one way or the other, and the decision is in this file:

  * claim surface  — a document or page a person reads *as a statement of what X3 does*:
                     `docs/**`, `production/public/**` (the public site HTML), `.planning/**`.
  * evidence record — a path whose job is to *quote* claims or measurements, so scanning it is
                     scanning the registry rather than the surface: see EVIDENCE_RECORDS below,
                     each with the reason written down. They are still declared as surfaces, so
                     deleting an entry re-enables scanning and the gate can fail — the skip list
                     is load-bearing, not decorative.

The qualification rules grew with it, because a requirement or a plan is not a claim:
an unchecked markdown box (`- [ ]`) is a plan item, and `acceptance`/`requirement`/`criteria`/
`threshold`/`goal`/`objective`/`must`/`versus` mark a line as a bound to meet or a comparison
rather than a result to report. A *checked* box (`- [x]`) is still scanned — that is what makes
a proposal's completed checklist a claim.

    scripts/ci/check-claims-hygiene.py            # check
    scripts/ci/check-claims-hygiene.py --list     # print every claim surface scanned

Exit 0 -> no unqualified claim found. Exit 1 -> at least one line asserts something the
repository cannot show a command for.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Never descend into build output, vendored sources, or the audit registry/reports that
# exist to quote claims and measurements.
SKIP_DIRS = {
    ".git",
    "target",
    "node_modules",
    "tauri-vendor",
    "dist",
    "build",
    ".srtool",
    ".ai",
    "audit-artifacts",
    "feature-matrix",
    "reports",
    "docs/audit",
    "docs/superpowers",
    "launch-gates",
    ".wt-lang",
}

# The surfaces a human reads or copies. Segment-anchored: `*` is one path component, `**`
# is the rest of the path, so `("*.md",)` means root-level markdown only and does not
# quietly sweep every document in the repository.
SURFACES = [
    ("*.md",),
    # The whole documentation tree a person reads. `docs/testnet-config` used to be named
    # here on its own; `docs/**` covers it and the operator/roadmap docs beside it.
    ("docs", "**"),
    # The published site. These are the pages a stranger reads before deciding we are real,
    # which makes them the most expensive place in the repository to be wrong.
    ("production", "public", "**"),
    # Roadmap and sprint plans: they carry dated "Complete"/"Live" rows, which are claims.
    (".planning", "**"),
    # Declared so that EVIDENCE_RECORDS is load-bearing: these paths *are* walked, and every
    # one of them is excluded by name below with its reason. Delete a reason and the gate
    # starts scanning the path again.
    ("benchmarks", "**"),
    (".audit", "**"),
    ("infra-structure", "**"),
    ("tests_phase4", "**"),
    ("tests_core", "**"),
    ("tools", "**"),
    ("apps", "*", "src-tauri", "src", "crm", "**"),
]

# Paths that are evidence records rather than claim surfaces. Every entry needs a reason,
# and `--list` prints them, because "a surface list nobody revisits" is how the gap that
# produced this list happened the first time.
EVIDENCE_RECORDS = {
    "benchmarks/": "an archive of measured TPS runs: the figures are the measurement, not a claim",
    ".audit/": "a claims inventory that exists to quote claims (same class as docs/audit)",
    "infra-structure/": "a standalone validator/dashboard package copied in for reference; its UI copy and benchmark docs are not X3 claim surfaces",
    "tests_phase4/": "test code and fixtures, not a surface a person reads a claim from",
    "tests_core/": "test code and fixtures, not a surface a person reads a claim from",
    "tools/": "developer tooling and its own test fixtures, not a claim surface",
}

# Phrases that are false in every context: the capability does not exist, so no qualifier
# rescues them (`feature-matrix/mev-privacy.toml` scores the whole MEV family at 2-52%).
ABSOLUTE = [
    (re.compile(r"\bMEV[-\s]?proof\b", re.I), "MEV protection is not implemented (X3-MEV-001..008)"),
    (re.compile(r"\bno\s+MEV\b", re.I), "no MEV protection is implemented"),
    (re.compile(r"\bMEV[-\s]?free\b", re.I), "no MEV protection is implemented"),
    (re.compile(r"\bno\s+front[-\s]?running\b", re.I), "no fair-ordering protocol is implemented (X3-MEV-008)"),
    (re.compile(r"\bfront[-\s]?running[-\s]?proof\b", re.I), "no fair-ordering protocol is implemented (X3-MEV-008)"),
    (re.compile(r"\bguaranteed\s+(order|ordering|execution order)\b", re.I), "no ordering guarantee is implemented (X3-MEV-008)"),
    (re.compile(r"\breorg[-\s]?proof\b", re.I), "reorg resistance is a GRANDPA finality property, not a proof"),
    (
        re.compile(r"\bdeterministic\s+execution\s+guarantees?\b", re.I),
        "the compiler/VM verifies a subset of programs, not all execution",
    ),
]

# Figures that are only a claim when asserted as a current result. Deliberately narrow:
# a plain "guarantee" or a price list is ordinary engineering prose and is not scanned, or
# the linter would be noise and the next person would switch it off.
NUMERIC = [
    (re.compile(r"\b\d[\d,._]*\s*(?:k|m|thousand|million)?\s*TPS\b", re.I), "throughput figure"),
    (re.compile(r"\bsub[-\s]?\d+(?:\.\d+)?\s*ms\b", re.I), "latency figure"),
    (re.compile(r"\b\d+(?:\.\d+)?\s*ms\s*(?:p99|latency|finality|settlement|block)", re.I), "latency figure"),
    (re.compile(r"\b\d+\s*[x×]\s*(?:faster|speedup|improvement|better)", re.I), "speedup figure"),
    (re.compile(r"\$\s?\d[\d,.]*\s*(?:/|per\s+)\s*gpu\b", re.I), "per-GPU revenue figure"),
    (re.compile(r"\bzero\s+(?:risk|downtime|capex|cap\s?ex)\b", re.I), "absolute claim"),
    (re.compile(r"\blive\s+in\s+\d+\s+production\b", re.I), "traction claim"),
    (re.compile(r"\b\d+\s+production\s+networks?\b", re.I), "traction claim"),
    (re.compile(r"\bcurrent\s+pilot\b", re.I), "traction claim"),
    (re.compile(r"\bpartners?\s+already\s+live\b", re.I), "traction claim"),
    (re.compile(r"\bacross\s+\d+\s+(?:countries|continents|regions)\b", re.I), "traction claim"),
]

# Deployment-state phrases (TICKET-141). Narrow on purpose: `LIVE_TESTNET` is a registry
# lifecycle label, "live network data" is a panel heading, and "live tests" are gates — none
# of those is a statement that a network is up, and matching them would make the rule noise
# that the next person switches off. Each pattern here names an actual deployment claim.
# Scanned on the declared claim surfaces only, with the same self-qualification escape as
# NUMERIC, so "not live", "planned", "will be live" and `- [ ] go live on testnet` are not
# claims. (TICKET-141 closed 2026-09-26: the four sites this found were the two public
# `production/public/x3-ecosystem*` pages and one line each of `docs/root/README.md`.)
LIVENESS = [
    (re.compile(r"\bnow\s+live\b", re.I), "deployment-state claim"),
    (re.compile(r"\blive\s+on\s+(?:testnet|mainnet|devnet)\b", re.I), "deployment-state claim"),
    (re.compile(r"\b(?:testnet|mainnet|network|chain)\s+is\s+(?:now\s+)?live\b", re.I), "deployment-state claim"),
    (re.compile(r"\bwe(?:'re|\s+are)\s+live\b", re.I), "deployment-state claim"),
]

# A number is a claim only when nothing on the line marks it as not-yet-true. `claim` is in
# this list so that a sentence *about* a claim (the registry, a ticket, a report) is not one.
# The second group covers what the widened surfaces turned up: a bound to meet
# (`acceptance`, `requirement`, `criteria`, `threshold`, `must`) or a comparison against
# another system (`versus`, `vs`) is not a statement that X3 achieved the number.
QUALIFIER = re.compile(
    r"\b(?:not|no|never|without|avoid|do not|don't|none|target|targets|targeted|planned|planning|"
    r"roadmap|under development|research|experimental|unverified|unproven|unreachable|dead|disabled|"
    r"blocked|ticket|todo|placeholder|aspirational|intended|proposed|proposal|aim|would|could|"
    r"claim|claims|claiming|report|reports|matrix|row|score|measured|verify|verification|test|tests|"
    r"gate|check|scan|fail|fails|refuses|refused|removed|removes|renamed|rename|baseline|honest|"
    r"inject|injects|injected|injection|simulate|simulated|withdrawn|dropped)\b"
    r"|\b(?:acceptance|accept|require|requires|required|requirement|requirements|criteria|"
    r"criterion|threshold|must|goal|goals|objective|objectives|versus|vs|slo|budget|parity|"
    r"expect|expected|projected|projection|estimate|estimated)\b",
    re.I,
)

# An unchecked markdown box is a plan item — "- [ ] 1,000 TPS demonstrated" states an
# intention, not a result. A *checked* box is the opposite and stays scanned, which is what
# makes a proposal's finished checklist a claim (`P4_IMPLEMENTATION_GUIDE.md`).
UNCHECKED_TASK = re.compile(r"^\s*[-*+]\s+\[\s*\]")

TEXT_SUFFIXES = {".md", ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".sh", ".toml", ".json", ".txt", ".html"}


def _in_skip_dirs(rel: str) -> bool:
    """Is this tracked path inside a skipped directory?

    SKIP_DIRS holds single path components, but a few entries name a whole prefix
    (`docs/audit`). Comparing only components silently ignored those: `docs/audit/*` was
    scanned even though the docstring says the audit registry is out of scope — the same
    class of hand-maintained-list bug as the surface list TICKET-139 widened. Prefix entries
    are matched against the path as well.
    """
    dirs = rel.split("/")[:-1]
    if any(part in SKIP_DIRS for part in dirs):
        return True
    return any("/" in entry and rel.startswith(entry + "/") for entry in SKIP_DIRS)


def tracked_files() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True
    ).stdout
    files = []
    for raw in out.split(b"\0"):
        if not raw:
            continue
        rel = raw.decode("utf-8", "replace")
        if _in_skip_dirs(rel):
            continue
        p = ROOT / rel
        if p.suffix in TEXT_SUFFIXES and p.is_file():
            files.append(Path(rel))
    return files


def _matches(parts: tuple[str, ...], pattern: tuple[str, ...]) -> bool:
    if not pattern:
        return not parts
    head, rest = pattern[0], pattern[1:]
    if head == "**":
        return True
    if not parts:
        return False
    # A segment may be a glob, but it only ever matches *one* component: `parts[0]` never
    # contains "/", so `fnmatch` cannot let a pattern cross a directory boundary.
    from fnmatch import fnmatchcase

    return fnmatchcase(parts[0], head) and _matches(parts[1:], rest)


def is_surface(rel: str) -> bool:
    parts = tuple(rel.split("/"))
    return any(_matches(parts, pattern) for pattern in SURFACES)


def is_evidence_record(rel: str) -> bool:
    """A declared record of claims/measurements, excluded from scanning by name."""
    return any(rel.startswith(prefix) for prefix in EVIDENCE_RECORDS)


def scan_line(rel: str, lineno: int, line: str, surface: bool, absolute_only: bool) -> list[str]:
    hits = []
    qualified = bool(QUALIFIER.search(line))
    # An unchecked plan item suppresses the numeric rule only: a phrase that is false in
    # every context is still false when it is written in a plan.
    numeric_qualified = qualified or bool(UNCHECKED_TASK.match(line))
    for pattern, why in ABSOLUTE:
        if pattern.search(line) and not qualified:
            hits.append(f"{rel}:{lineno}: ABSOLUTE ({why}): {line.strip()[:160]}")
    if surface and not absolute_only and not numeric_qualified:
        for pattern, why in NUMERIC:
            if pattern.search(line):
                hits.append(f"{rel}:{lineno}: unqualified {why}: {line.strip()[:160]}")
                break
        for pattern, why in LIVENESS:
            if pattern.search(line):
                hits.append(f"{rel}:{lineno}: unqualified {why}: {line.strip()[:160]}")
                break
    return hits


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--list", action="store_true", help="print the claim surfaces")
    args = parser.parse_args()

    files = tracked_files()
    surfaces = [str(f) for f in files if is_surface(str(f))]
    declared = [s for s in surfaces if not is_evidence_record(s)]
    records = [s for s in surfaces if is_evidence_record(s)]

    if args.list:
        print(f"claim surfaces ({len(declared)}):")
        for s in declared:
            print(f"  {s}")
        print()
        print(f"evidence records, declared and excluded ({len(records)} file(s)):")
        for prefix, reason in sorted(EVIDENCE_RECORDS.items()):
            matched = sum(1 for s in records if s.startswith(prefix))
            print(f"  {prefix}  ({matched} file(s)) - {reason}")
        return 0

    scanned = 0
    hits: list[str] = []
    for rel in files:
        rel_s = str(rel)
        # A declared evidence record is a place that quotes claims rather than makes them;
        # it is named in EVIDENCE_RECORDS with a reason, and `--list` prints the list.
        if is_evidence_record(rel_s):
            continue
        surface = is_surface(rel_s)
        # ABSOLUTE patterns are cheap and unambiguous, so they are checked tree-wide;
        # NUMERIC patterns only on a declared surface.
        try:
            text = (ROOT / rel).read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        scanned += 1
        if not surface and not any(p.search(text) for p, _ in ABSOLUTE):
            continue
        for i, line in enumerate(text.splitlines(), start=1):
            hits.extend(scan_line(rel_s, i, line, surface, absolute_only=not surface))

    if hits:
        print("claims-hygiene: FAIL")
        for h in hits:
            print(f"  {h}")
        print()
        print(f"{len(hits)} unqualified claim(s) across {scanned} scanned file(s).")
        print("Rewrite the line so it states what is real, or qualify it (target/planned/")
        print("unverified/research), or record it in the audit registry instead of the surface.")
        return 1

    print(
        f"claims-hygiene: OK - {scanned} file(s) scanned, {len(declared)} claim surface(s), "
        f"{len(records)} declared evidence record(s) excluded, no unqualified claim"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
