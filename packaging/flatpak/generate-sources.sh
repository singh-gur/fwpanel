#!/usr/bin/env bash
# Generate the pinned offline dependency manifests for the Flatpak build:
#   - packaging/flatpak/node-sources.json  (upstream flatpak-node-generator, pnpm)
#   - packaging/flatpak/cargo-sources.json (upstream flatpak-cargo-generator)
#
# Requires: python3 + aiohttp, git, and the service-side Cargo workspace.
# The generators are fetched from the pinned flatpak-builder-tools revision so
# output stays reproducible. No network is touched at build time; downloads
# happen in this source-generation step only.
set -euo pipefail

FBT_REV="1fc32195e3e60fe5c97f0af646dec7a99df5962b"  # pin; update deliberately
FBT_DIR="${FBT_DIR:-/tmp/flatpak-builder-tools}"
OUT="$(cd "$(dirname "$0")" && pwd)"

if [ ! -d "$FBT_DIR" ]; then
  git clone --depth 1 https://github.com/flatpak/flatpak-builder-tools "$FBT_DIR"
fi
git -C "$FBT_DIR" fetch --depth 1 origin "$FBT_REV" 2>/dev/null || true
git -C "$FBT_DIR" checkout -q "$FBT_REV"

ROOT="$(cd "$OUT/../.." && pwd)"

echo "== node sources (pnpm) =="
PYTHONPATH="$FBT_DIR/node" /usr/bin/python3 -m flatpak_node_generator pnpm \
  "$ROOT/pnpm-lock.yaml" -o "$OUT/node-sources.json"

echo "== cargo sources =="
/usr/bin/python3 "$FBT_DIR/cargo/flatpak-cargo-generator.py" \
  "$ROOT/Cargo.lock" -o "$OUT/cargo-sources.json"

echo "Wrote $OUT/node-sources.json and $OUT/cargo-sources.json"
