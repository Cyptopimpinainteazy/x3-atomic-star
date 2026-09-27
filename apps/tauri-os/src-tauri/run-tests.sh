#!/usr/bin/env bash
# Unit + integration tests for the X3 OS operator console backend.
#
# This is the layer `scripts/local-ci.sh` runs in the fast set. It wraps
# `cargo test` for two environment reasons that are properties of this box, not
# of the code:
#
#   1. pkg-config's default search path here is Homebrew-only
#      (`/home/linuxbrew/.../pkgconfig`), so GTK/WebKit are invisible and every
#      gtk-sys/gdk-pixbuf-sys build script fails. The system .pc files are added
#      back explicitly.
#   2. Debian's `shared-mime-info` ships no `.pc` file although
#      `gdk-pixbuf-2.0.pc` lists it in `Requires.private`; pkg-config validates
#      that even for a plain probe and reports the failure against the
#      *transitive* libraries (`gdk-3.0`, `gtk+-3.0`). `build-pkgconfig/` carries
#      the minimal declaration. See the comment in that file.
#
# The nested workspace gets its own target dir; the shared `target/` is written
# by the pinned toolchain too, but nothing here should depend on which compiler
# ran last. `-p tauri-os-backend` also names the package for
# `scripts/swarm/x3_repo_scan.py`, which treats a crate as gated when a
# gate-invoked wrapper names its package.
#
# Usage: bash apps/tauri-os/src-tauri/run-tests.sh [extra cargo test args]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

pc_paths=()
for candidate in /usr/lib/x86_64-linux-gnu/pkgconfig "$SCRIPT_DIR/build-pkgconfig"; do
  [ -d "$candidate" ] && pc_paths+=("$candidate")
done
[ -n "${PKG_CONFIG_PATH:-}" ] && pc_paths+=("$PKG_CONFIG_PATH")
if [ "${#pc_paths[@]}" -gt 0 ]; then
  PKG_CONFIG_PATH="$(IFS=:; printf '%s' "${pc_paths[*]}")"
  export PKG_CONFIG_PATH
fi

export CARGO_TARGET_DIR="${X3_TAURI_OS_TARGET_DIR:-$SCRIPT_DIR/target}"

export RUSTC="${RUSTC:-$(command -v rustc)}"

exec cargo test --locked --manifest-path "$SCRIPT_DIR/Cargo.toml" -p tauri-os-backend "$@"
