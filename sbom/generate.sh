#!/usr/bin/env bash
# Regenerate the checked-in Cargo dependency-graph SBOM, deterministically.
#
# syft stamps a fresh `metadata.timestamp` and a random `serialNumber` on every
# run; everything else (components + dependsOn edges) is byte-stable for a given
# Cargo.lock. We strip those two fields so the checked-in file changes ONLY when
# the dependency set changes — keeping PR diffs meaningful.
#
# Requires: syft, jq.  Usage: ./sbom/generate.sh
#   Override the syft binary with SYFT=/path/to/syft ./sbom/generate.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SYFT="${SYFT:-syft}"
if ! command -v "$SYFT" >/dev/null 2>&1; then
  echo "error: '$SYFT' not found on PATH (set SYFT=/path/to/syft)" >&2
  exit 1
fi

# Write via a temp file + atomic mv so a failed scan never truncates the
# committed SBOM.
tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
"$SYFT" scan file:Cargo.lock -o cyclonedx-json \
  | jq 'del(.serialNumber) | del(.metadata.timestamp)' \
  > "$tmp"
mv "$tmp" sbom/crates.cdx.json

echo "wrote sbom/crates.cdx.json ($(jq '.components | length' sbom/crates.cdx.json) components)"
