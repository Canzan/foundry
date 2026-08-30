#!/usr/bin/env sh
# derive-fonts.sh — re-derive foundry's three self-hosted webfonts from their
# pinned upstreams, and print the provenance block VENDOR.md records.
#
# THIS IS NOT A BUILD STEP. It is never invoked by `cargo build`, by
# `cargo xtask ci`, or by CI (VENDOR.md:3-4, assets.md DB6). A maintainer runs
# it when a font is added or bumped, and an auditor runs it to answer ADR-002's
# Tier-2 question: "is this blob really Bricolage Grotesque, and only that?"
#
#   ./tools/fonts/derive-fonts.sh [WORKDIR]
#
# It lives OUTSIDE the cargo workspace and OUTSIDE static/ — a served directory
# must not hold a shell script.
#
# THE AUDIT IT SUPPORTS (ADR-CANZAN-THEME-002, three steps):
#   1. input sha256   — matches VENDOR.md item 3, else the upstream moved.
#   2. INTERMEDIATE sha256 (instanced + subset TTF, no --flavor) — the
#      brotli-independent STABLE ANCHOR. A match proves the font CONTENT is what
#      was recorded: same glyphs, same axes, same tables.
#   3. output woff2 sha256 — full byte-level provenance when it matches. A
#      MISMATCH HERE AFTER STEP 2 PASSED IS A COMPRESSOR DIFFERENCE, NOT A
#      PROVENANCE FAILURE, and is recorded as such rather than treated as
#      tampering. A varying step 2 is a different matter entirely: it would mean
#      the intermediate is not the stable anchor the model assumes.
#
# EXPECTED, BENIGN: `varLib.instancer` emits an `OTLOffsetOverflowError` warning
# on range instancing and repairs it internally. The outputs are valid woff2.
# Recorded here so it is not mistaken for a failure.
#
# WHY `SOURCE_DATE_EPOCH` IS PINNED, AND WHY THE ANCHOR WOULD BE WORTHLESS
# WITHOUT IT (measured at 04-01, not assumed). `varLib.instancer` stamps
# `head.modified` with the WALL CLOCK. Two runs on the SAME machine seconds
# apart therefore produce intermediates whose sha256 differ — measured: 5 bytes,
# `head.modified` plus the `checkSumAdjustment` derived from it, with EVERY
# OTHER TABLE byte-identical. An anchor that changes when nothing changed proves
# nothing about content, and would have trained an auditor to shrug at step 2 —
# the exact failure ADR-002 alternative C rejects. fontTools honours
# `SOURCE_DATE_EPOCH` for that field, so pinning it makes the recipe hermetic
# with respect to the clock and turns the intermediate hash back into the
# content claim ADR-002 says it is. The value is arbitrary but FIXED, recorded
# in VENDOR.md, and must never be "refreshed": changing it changes every hash
# below while changing no glyph.
SOURCE_DATE_EPOCH=1787961600   # 2026-08-29T00:00:00Z
export SOURCE_DATE_EPOCH
set -eu

WORK="${1:-$(mktemp -d)}"
HERE=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$HERE/../.." && pwd)
VENV="${FOUNDRY_FONT_VENV:-$ROOT/venv}"

mkdir -p "$WORK"

if [ ! -x "$VENV/bin/pyftsubset" ]; then
  echo "creating $VENV from tools/fonts/requirements.txt (fontTools is NOT installed system-wide)" >&2
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install --quiet --disable-pip-version-check -r "$HERE/requirements.txt"
fi

# The Google "latin" range, verbatim. It is part of the recipe, so it is
# transcribed into VENDOR.md in full rather than summarised as "latin".
LATIN='U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD'

# curl where it exists, python otherwise — `python:3.14-slim`, the container
# ADR-002 names as the expected second environment for the reproducibility
# probe, ships no curl and a probe that needs apt is a probe that gets skipped.
fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSLo "$2" "$1"
  else
    python3 -c 'import sys,urllib.request;urllib.request.urlretrieve(sys.argv[1],sys.argv[2])' "$1" "$2"
  fi
}

sha() { python3 -c 'import hashlib,sys;print(hashlib.sha256(open(sys.argv[1],"rb").read()).hexdigest())' "$1"; }
bytes_of() { python3 -c 'import os,sys;print(os.path.getsize(sys.argv[1]))' "$1"; }

# derive <slug> <url> <axes...>
derive() {
  slug=$1
  url=$2
  shift 2

  echo "=== $slug"
  fetch "$url" "$WORK/$slug.source.ttf"
  input_sha=$(sha "$WORK/$slug.source.ttf")

  # `--no-optimize` and `--no-recalc-timestamp` are LOAD-BEARING FOR THE AUDIT,
  # not tuning. Measured at 04-01 across macOS/arm64 and a Debian container:
  #
  #   * WITHOUT `--no-optimize` the intermediate anchor DIVERGES between
  #     environments (JetBrains Mono: 2c13f344… vs b608c69a…). The cause is the
  #     IUP optimiser — on 4 of 414 glyphs it stores a delta explicitly on one
  #     machine and leaves it to be interpolated on the other. Both encodings
  #     render identically (instancing each at wght=400 and wght=500 gives
  #     byte-identical fonts), but the SHA does not, and a step-2 hash that
  #     varies while the font does not is an anchor that proves nothing.
  #     With the flag: 446cb992… on both.
  #   * `--no-recalc-timestamp` holds `head.modified` alongside the
  #     SOURCE_DATE_EPOCH pin above.
  #
  # The cost is ~200 B on the shipped woff2 (gvar is left unoptimised). DO NOT
  # remove either flag to reclaim it: doing so silently breaks ADR-002's Tier-2
  # audit while every test stays green.
  "$VENV/bin/fonttools" varLib.instancer "$WORK/$slug.source.ttf" "$@" \
    --no-optimize --no-recalc-timestamp \
    -o "$WORK/$slug.instanced.ttf" >/dev/null

  # NO --flavor here: this intermediate is the brotli-independent anchor.
  "$VENV/bin/pyftsubset" "$WORK/$slug.instanced.ttf" \
    --unicodes="$LATIN" \
    --layout-features="kern,liga,clig,calt" \
    --output-file="$WORK/$slug.subset.ttf"
  intermediate_sha=$(sha "$WORK/$slug.subset.ttf")

  "$VENV/bin/pyftsubset" "$WORK/$slug.subset.ttf" \
    --unicodes="*" --layout-features="*" --flavor=woff2 \
    --output-file="$WORK/$slug.woff2"
  output_sha=$(sha "$WORK/$slug.woff2")

  short=$(printf '%s' "$output_sha" | cut -c1-8)
  mv "$WORK/$slug.woff2" "$WORK/$slug.$short.woff2"

  echo "  input url          : $url"
  echo "  input sha256       : $input_sha"
  echo "  axes               : $*"
  echo "  intermediate sha256: $intermediate_sha  (ANCHOR — step 2 of the audit)"
  echo "  output sha256      : $output_sha"
  echo "  output bytes       : $(bytes_of "$WORK/$slug.$short.woff2")"
  echo "  committed as       : crates/foundry-app/static/fonts/$slug.$short.woff2"
}

echo "workdir: $WORK"
echo "SOURCE_DATE_EPOCH: $SOURCE_DATE_EPOCH"
"$VENV/bin/pip" freeze | grep -Ei '^(fonttools|brotli)==' || true
python3 --version

derive bricolage-grotesque \
  'https://raw.githubusercontent.com/ateliertriay/bricolage/84745e5b96261ae5f8c6c856e262fe78d1d6efdd/fonts/variable/BricolageGrotesque%5Bopsz%2Cwdth%2Cwght%5D.ttf' \
  opsz=24 wdth=100 wght=600:700

derive public-sans \
  'https://raw.githubusercontent.com/uswds/public-sans/v2.001/fonts/variable/PublicSans%5Bwght%5D.ttf' \
  wght=400:700

derive jetbrains-mono \
  'https://raw.githubusercontent.com/JetBrains/JetBrainsMono/v2.304/fonts/variable/JetBrainsMono%5Bwght%5D.ttf' \
  wght=400:500

echo "=== licences"
fetch 'https://raw.githubusercontent.com/ateliertriay/bricolage/84745e5b96261ae5f8c6c856e262fe78d1d6efdd/OFL.txt' "$WORK/OFL-bricolage-grotesque.txt"
fetch 'https://raw.githubusercontent.com/uswds/public-sans/v2.001/LICENSE.md' "$WORK/OFL-public-sans.txt"
fetch 'https://raw.githubusercontent.com/JetBrains/JetBrainsMono/v2.304/OFL.txt' "$WORK/OFL-jetbrains-mono.txt"
echo "  three OFL-1.1 texts written to $WORK (ship as static/fonts/OFL-<family>.txt — clause 2)"
