#!/usr/bin/env bash
# Creates the isolated Lima VM that scripts/test-in-lima.sh runs tests in. Run once.
#
# The VM has no host mounts and runs its own Docker, which is what testcontainers
# needs. Nothing on the Mac's Docker is used, so other projects' containers and
# ports cannot reach a test run.
set -euo pipefail

VM="${FOUNDRY_TEST_VM:-foundry-test}"

timeout 900 limactl start --name="$VM" --cpus=8 --memory=24 --disk=200 --vm-type=vz \
  --mount-none --tty=false template:docker-rootful

limactl shell --workdir / "$VM" bash -s <<'SETUP'
set -euo pipefail
sudo apt-get update -qq
sudo apt-get install -y -qq build-essential pkg-config libssl-dev rsync git curl jq lld >/dev/null
[ -x ~/.cargo/bin/rustup ] || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
. ~/.cargo/env
rustup toolchain install 1.91 --profile minimal -c rustfmt -c clippy
cargo +1.91 install --locked cargo-deny
cat >~/foundry-test-env <<'ENV'
. ~/.cargo/env
export DOCKER_HOST=unix:///var/run/docker.sock
ENV
SETUP
