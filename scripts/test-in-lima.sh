#!/usr/bin/env bash
# Runs a test command inside the isolated Lima VM, never Docker on the Mac.
#
#     scripts/test-in-lima.sh cargo test -p foundry-store
#     scripts/test-in-lima.sh bash -c 'source scripts/eip-lanes.sh; exec cargo xtask ci'
#
# Copies what git tracks or would track, so `.env` and everything else ignored
# stays on the Mac, plus the history a few tests read. The VM has no mounts, and
# files deleted on the Mac are deleted there too, so a removed migration cannot
# linger. One command at a time: a second run waits for the first.
#
# Sources are copied by checksum and never keep the Mac's times, so cargo reuses
# the VM's own `target/` and rebuilds only what changed.
set -euo pipefail

VM="${FOUNDRY_TEST_VM:-foundry-test}"
SSH_CONFIG="$HOME/.lima/$VM/ssh.config"

if [ "$#" -eq 0 ]; then
  echo "usage: scripts/test-in-lima.sh <command> [args...]" >&2
  exit 2
fi

cd "$(git rev-parse --show-toplevel)"

files=$(git ls-files -co --exclude-standard | while IFS= read -r path; do
  if [ -e "$path" ]; then printf '%s\n' "$path"; fi
done)

printf '%s\n' "$files" |
  rsync -rlpc --files-from=- -e "ssh -F $SSH_CONFIG" ./ "lima-$VM:foundry/"

printf '%s\n' "$files" | limactl shell --workdir / "$VM" bash -c \
  'cd ~/foundry && sort >~/foundry-files && find . -path ./target -prune -o -path ./.git -prune -o -type f -print |
     sed "s|^\./||" | sort | comm -23 - ~/foundry-files | xargs -r -d "\n" rm -f'

rsync -a --delete --exclude=config --exclude=hooks --exclude=index --exclude='*.lock' \
  -e "ssh -F $SSH_CONFIG" .git/ "lima-$VM:foundry/.git/"

quoted=$(printf '%q ' "$@")
exec limactl shell --workdir / "$VM" bash -c \
  ". ~/foundry-test-env; cd ~/foundry; ulimit -n 8192; exec flock ~/test.lock $quoted"
