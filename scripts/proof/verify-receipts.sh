#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RECEIPTS_DIR="$ROOT_DIR/proof/receipts/claims"
SCHEMA_FILE="$ROOT_DIR/proof/schema/receipt-v2.schema.json"

if ! command -v jq >/dev/null 2>&1; then
  echo "ERROR: jq is required to validate receipts." >&2
  exit 2
fi

if [[ ! -d "$RECEIPTS_DIR" ]]; then
  echo "No receipts directory found at $RECEIPTS_DIR"
  exit 0
fi

if [[ ! -f "$SCHEMA_FILE" ]]; then
  echo "ERROR: Missing schema file: $SCHEMA_FILE" >&2
  exit 2
fi

legacy_count=0
invalid_count=0
ok_count=0

validate_required_shape() {
  local file="$1"
  jq -e '
    has("repo_commit_hash") and
    has("command_run") and
    has("artifact_hash") and
    has("policy_hash") and
    has("relevant_files") and
    has("timestamp") and
    has("result") and
    has("limitations") and
    has("binding_hash") and
    (.repo_commit_hash | type == "string") and
    (.command_run | type == "string") and
    (.artifact_hash | type == "string") and
    (.policy_hash | type == "string") and
    (.relevant_files | type == "array") and
    (.timestamp | type == "string") and
    (.result | type == "object") and
    (.limitations | type == "array") and
    (.binding_hash | type == "string")
  ' "$file" >/dev/null
}

is_legacy_receipt() {
  local file="$1"
  jq -e '
    has("claim_id") and has("status") and has("date") and has("verifier") and has("hash") and
    (has("repo_commit_hash") | not)
  ' "$file" >/dev/null
}

echo "Validating receipts in $RECEIPTS_DIR"
while IFS= read -r -d '' file; do
  rel="${file#"$ROOT_DIR"/}"

  if is_legacy_receipt "$file"; then
    echo "LEGACY  $rel"
    legacy_count=$((legacy_count + 1))
    continue
  fi

  if validate_required_shape "$file"; then
    echo "OK      $rel"
    ok_count=$((ok_count + 1))
  else
    echo "INVALID $rel"
    invalid_count=$((invalid_count + 1))
  fi
done < <(find "$RECEIPTS_DIR" -type f -name '*.json' -print0 | sort -z)

echo
echo "Summary:"
echo "  ok:      $ok_count"
echo "  legacy:  $legacy_count"
echo "  invalid: $invalid_count"

if (( legacy_count > 0 || invalid_count > 0 )); then
  echo "Receipt validation FAILED: migrate legacy receipts and fix invalid shapes." >&2
  exit 1
fi

echo "Receipt validation PASSED"
