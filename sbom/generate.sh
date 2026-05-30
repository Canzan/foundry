#!/usr/bin/env bash
# Regenerate the checked-in Cargo dependency-graph SBOM, deterministically.
#
# syft stamps a fresh `metadata.timestamp` and a random `serialNumber` on every
# run; everything else (components + dependsOn edges) is byte-stable for a given
# Cargo.lock. We strip those two fields so the checked-in file changes ONLY when
# the dependency set changes — keeping PR diffs meaningful.
#
# Requires: syft, jq.  Usage: ./sbom/generate.sh
set -euo pipefail
cd "$(dirname "$0")/.."

syft scan file:Cargo.lock -o cyclonedx-json \
  | jq 'del(.serialNumber) | del(.metadata.timestamp)' \
  > sbom/crates.cdx.json

echo "wrote sbom/crates.cdx.json ($(jq '.components | length' sbom/crates.cdx.json) components)"
