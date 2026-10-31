#!/usr/bin/env bash
# Records real Slack answers into crates/foundry-acceptance/tests/fixtures/slack
# (DEVOPS H-3, `eip-slack-live`). Manual, never in CI. See the test's module docs
# in crates/foundry-acceptance/tests/eip_slack_live.rs.
#
#     scripts/slack-record.sh
#
# Reads EIP_SLACK_TEST_CLIENT_ID, EIP_SLACK_TEST_CLIENT_SECRET and
# EIP_SLACK_TEST_SIGNING_SECRET from `.env`. They reach the VM on stdin, in a
# mode 600 file the recorder deletes on read, never on a command line.
#
# Slack must reach the recorder, so a Cloudflare quick tunnel gives the run a
# throwaway https address. It prints the manifest to paste into the Slack app and
# the link to approve, then records once someone clicks Allow.
set -euo pipefail

VM="${FOUNDRY_TEST_VM:-foundry-test}"
TUNNEL_IMAGE="cloudflare/cloudflared:2025.10.1"
TUNNEL=foundry-slack-record
SECRETS=/tmp/foundry-slack-record-secrets
OUT=slack-fixtures
FIXTURES=crates/foundry-acceptance/tests/fixtures/slack
TUNNEL_WAIT_SECONDS=60

cd "$(git rev-parse --show-toplevel)"

in_vm() { limactl shell --workdir / "$VM" "$@"; }

remove_tunnel() { in_vm docker rm -f "$TUNNEL" >/dev/null 2>&1 || true; }

trap remove_tunnel EXIT

grep -E '^EIP_SLACK_TEST_(CLIENT_ID|CLIENT_SECRET|SIGNING_SECRET)=' .env |
  in_vm bash -c "umask 077; cat >$SECRETS"

remove_tunnel
in_vm bash -c "rm -rf ~/$OUT"
in_vm docker run -d --name "$TUNNEL" --network host "$TUNNEL_IMAGE" \
  tunnel --no-autoupdate --url http://127.0.0.1:8765 >/dev/null

public_url=""
for _ in $(seq "$TUNNEL_WAIT_SECONDS"); do
  public_url=$(in_vm docker logs "$TUNNEL" 2>&1 |
    grep -oE 'https://[a-z0-9-]+\.trycloudflare\.com' | head -1 || true)
  if [ -n "$public_url" ]; then break; fi
  sleep 1
done
if [ -z "$public_url" ]; then
  echo "the tunnel gave no address within ${TUNNEL_WAIT_SECONDS}s" >&2
  exit 1
fi

scripts/test-in-lima.sh env \
  EIP_SLACK_RECORD_SECRETS="$SECRETS" \
  EIP_SLACK_RECORD_PUBLIC_URL="$public_url" \
  EIP_SLACK_RECORD_OUT="$(in_vm bash -c 'echo "$HOME"')/$OUT" \
  timeout 60m cargo test -p foundry-acceptance --release --test eip_slack_live -- \
  --ignored --nocapture --exact eip_slack_live

mkdir -p "$FIXTURES"
ssh -F "$HOME/.lima/$VM/ssh.config" "lima-$VM" "tar -C $OUT -cf - ." | tar -C "$FIXTURES" -xf -
echo "fixtures in $FIXTURES"
