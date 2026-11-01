#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
version="$(awk -F '\"' '/^version = / { print $2; exit }' Cargo.toml)"
mkdir -p dist
git archive --format=zip --prefix="JetSeat-Massage-$version/" HEAD > "dist/JetSeat-Massage-$version-source.zip"
git archive --format=tar --prefix="JetSeat-Massage-$version/" HEAD | gzip -n > "dist/JetSeat-Massage-$version-source.tar.gz"
printf 'Source ZIP and tar.gz created in dist/\n'
