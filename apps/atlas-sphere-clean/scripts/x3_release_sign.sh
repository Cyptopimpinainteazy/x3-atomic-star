#!/usr/bin/env bash
# scripts/x3_release_sign.sh — Reproducible build + release signing
#
# Usage:
#   bash scripts/x3_release_sign.sh [--sign-key <gpg-key-id>]
#   bash scripts/x3_release_sign.sh --verify <release-dir>
#
# What it does:
#   1. Sets deterministic build environment (SOURCE_DATE_EPOCH, RUSTFLAGS)
#   2. Builds x3-chain-node in release mode with --locked
#   3. Computes SHA-256 of the binary
#   4. Optionally GPG-signs the hash file
#   5. Writes release manifest: release/RELEASE_MANIFEST.json
#
# Verification (anyone can run):
#   bash scripts/x3_release_sign.sh --verify release/
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ── Defaults ──────────────────────────────────────────────────────────────────
SIGN_KEY=""
VERIFY_DIR=""
RELEASE_DIR="$ROOT/release"
BINARY_NAME="x3-chain-node"
BINARY_PATH="$ROOT/target/release/$BINARY_NAME"

for arg in "$@"; do
  case "$arg" in
    --sign-key) shift; SIGN_KEY="${1:-}" ;;
    --verify)   shift; VERIFY_DIR="${1:-}" ;;
  esac
done

# ── VERIFY mode ───────────────────────────────────────────────────────────────
if [ -n "$VERIFY_DIR" ]; then
  echo "=== X3 Release Verification ==="
  manifest="$VERIFY_DIR/RELEASE_MANIFEST.json"
  hashfile="$VERIFY_DIR/release.sha256"
  binary="$VERIFY_DIR/$BINARY_NAME"

  [ -f "$manifest" ] || { echo "❌ RELEASE_MANIFEST.json not found in $VERIFY_DIR"; exit 1; }
  [ -f "$hashfile" ] || { echo "❌ release.sha256 not found"; exit 1; }
  [ -f "$binary" ]   || { echo "❌ Binary $binary not found"; exit 1; }

  echo "Verifying binary hash..."
  if sha256sum --check "$hashfile"; then
    echo "✅ Binary hash matches"
  else
    echo "❌ Hash mismatch — binary may be tampered"
    exit 1
  fi

  if [ -f "$VERIFY_DIR/release.sha256.asc" ]; then
    echo "Verifying GPG signature..."
    if gpg --verify "$VERIFY_DIR/release.sha256.asc" "$hashfile"; then
      echo "✅ GPG signature valid"
    else
      echo "❌ GPG signature invalid"
      exit 1
    fi
  else
    echo "ℹ️  No GPG signature file found — skipping signature check"
  fi

  echo ""
  echo "=== Manifest ==="
  cat "$manifest"
  exit 0
fi

# ── BUILD mode ────────────────────────────────────────────────────────────────
echo "=== X3 Reproducible Build ==="

# Deterministic build environment
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct 2>/dev/null || echo 1700000000)}"
export CARGO_INCREMENTAL=0
export RUSTFLAGS="${RUSTFLAGS:--C debuginfo=0 -C codegen-units=1 -C opt-level=3}"

echo "SOURCE_DATE_EPOCH: $SOURCE_DATE_EPOCH"
echo "RUSTFLAGS: $RUSTFLAGS"

echo ""
echo "Building $BINARY_NAME (release, locked)..."
cargo build --release --locked -p x3-chain-node 2>&1 | tail -5

[ -f "$BINARY_PATH" ] || { echo "❌ Build failed — binary not found at $BINARY_PATH"; exit 1; }

# ── Hash ──────────────────────────────────────────────────────────────────────
mkdir -p "$RELEASE_DIR"
cp "$BINARY_PATH" "$RELEASE_DIR/$BINARY_NAME"

HASH=$(sha256sum "$RELEASE_DIR/$BINARY_NAME" | awk '{print $1}')
echo "$HASH  $BINARY_NAME" > "$RELEASE_DIR/release.sha256"
echo "SHA-256: $HASH"

# ── Signing ───────────────────────────────────────────────────────────────────
if [ -n "$SIGN_KEY" ]; then
  echo ""
  echo "Signing with GPG key: $SIGN_KEY"
  gpg --armor --detach-sign \
      --default-key "$SIGN_KEY" \
      --output "$RELEASE_DIR/release.sha256.asc" \
      "$RELEASE_DIR/release.sha256"
  echo "✅ Signature written: release/release.sha256.asc"
else
  echo "ℹ️  No --sign-key provided — skipping GPG signing"
  echo "   To sign: bash scripts/x3_release_sign.sh --sign-key <your-gpg-key-id>"
fi

# ── Manifest ─────────────────────────────────────────────────────────────────
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
GIT_TAG=$(git describe --tags --exact-match 2>/dev/null || echo "untagged")
RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

cat > "$RELEASE_DIR/RELEASE_MANIFEST.json" <<EOF
{
  "binary": "$BINARY_NAME",
  "sha256": "$HASH",
  "git_commit": "$GIT_COMMIT",
  "git_tag": "$GIT_TAG",
  "rust_version": "$RUST_VERSION",
  "source_date_epoch": $SOURCE_DATE_EPOCH,
  "build_date": "$BUILD_DATE",
  "build_flags": "$RUSTFLAGS",
  "reproducible": true,
  "signed": $([ -n "$SIGN_KEY" ] && echo "true" || echo "false")
}
EOF

echo ""
echo "=== Release Manifest ==="
cat "$RELEASE_DIR/RELEASE_MANIFEST.json"
echo ""
echo "✅ Release artifacts written to: $RELEASE_DIR/"
echo "   Verify on any machine: bash scripts/x3_release_sign.sh --verify $RELEASE_DIR/"
