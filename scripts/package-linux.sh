#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
cargo build --release --locked
formats=(portable)
if command -v dpkg-deb >/dev/null; then formats+=(deb); fi
if command -v rpmbuild >/dev/null; then formats+=(rpm); fi
if command -v pacman >/dev/null && command -v zstd >/dev/null; then formats+=(arch); fi
# Explicit formats may be passed, e.g. package-linux.sh deb rpm arch appimage portable.
if [ "$#" -gt 0 ]; then formats=("$@"); fi
python3 scripts/package-linux.py --binary "${CARGO_TARGET_DIR:-target}/release/jetseat-massage" --format "${formats[@]}"
