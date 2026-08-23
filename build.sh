#!/usr/bin/env bash
#
# build.sh — build the Foundry binary (and optionally run the quality gates).
#
# The sibling of run.sh (launch) and cli.sh (doctor commands): one obvious
# entry point that compiles the `foundry` binary the README documents
# (`cargo build --release --bin foundry` — the same binary the @real-io
# acceptance scenarios spawn) and prints where it landed.
#
#   ./build.sh              release build (default) -> target/release/foundry
#   ./build.sh debug        debug build (faster)    -> target/debug/foundry
#   ./build.sh check        quick quality pass: fmt --check, clippy, check-arch
#   ./build.sh ci           the FULL gate (cargo xtask ci) — what pre-push runs
#   ./build.sh help         this text
#
# Anything after the mode is forwarded to cargo build, e.g.:
#   ./build.sh release --timings
#
# No database, no env vars needed — this only compiles. Use ./run.sh to
# launch the app and ./cli.sh for `foundry doctor …`.

set -euo pipefail
cd "$(dirname "$0")"

mode="${1:-release}"
[ $# -gt 0 ] && shift

case "$mode" in
release)
    echo "▶ building foundry (release)…"
    cargo build --release --bin foundry "$@"
    echo "✔ binary: $(pwd)/target/release/foundry"
    ;;
debug)
    echo "▶ building foundry (debug)…"
    cargo build --bin foundry "$@"
    echo "✔ binary: $(pwd)/target/debug/foundry"
    ;;
check)
    echo "▶ cargo fmt --check"
    cargo fmt --all --check
    echo "▶ cargo clippy (all targets)"
    cargo clippy --workspace --all-targets
    echo "▶ cargo xtask check-arch"
    cargo xtask check-arch
    echo "✔ quick quality pass green"
    ;;
ci)
    echo "▶ cargo xtask ci (full gate — compiles everything, runs every lane)"
    exec cargo xtask ci "$@"
    ;;
help | --help | -h)
    sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
    ;;
*)
    echo "build.sh: unknown mode '$mode'. Modes: release (default), debug, check, ci, help" >&2
    exit 2
    ;;
esac
