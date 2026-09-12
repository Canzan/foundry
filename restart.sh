#!/usr/bin/env bash
#
# restart.sh — rebuild the latest code and relaunch Foundry, DETACHED.
#
# The loop this exists for: you changed Rust or a template, and you want the
# running app to be that code, without surrendering a terminal to it. Stops
# whatever is running, builds, brings it back up, waits until `/healthz`
# answers, prints where it is, and returns you to the prompt.
#
# IT DOES NOT REIMPLEMENT run.sh, AND MUST NOT. run.sh owns the launch: the
# dotenv parse (values contain PEM keys with spaces, so `source` is wrong), the
# Postgres create-or-reuse with host-port auto-advance, the readiness waits, and
# the first-run bootstrap auto-claim. Copying any of that here would be two
# launchers to keep in step, and they would drift on the first `.env` change.
# So this script is exactly three things run.sh cannot do for itself:
#
#   1. stop the previous instance        -> ./stop.sh
#   2. surface BUILD FAILURES to your terminal, not to a temp log
#   3. leave run.sh supervising in the background and hand the prompt back
#
# ON THE FOREGROUND BUILD (step 2): a detached run.sh writes its output to a
# mktemp file, so a compile error would look identical to a slow start — you
# would watch a health check time out and then go hunting for the log. Building
# HERE first means a broken tree fails in the obvious place, with cargo's own
# diagnostics on your screen, before anything is torn down or backgrounded.
# run.sh then builds again and gets a cache hit, so this costs nothing.
#
# THE APP OUTLIVES THIS SCRIPT, and run.sh keeps supervising it — which is what
# makes `./stop.sh` able to tear the whole thing down cleanly later (TERM to
# run.sh runs run.sh's own `cleanup` trap). Note the consequence: run.sh's
# default teardown stops Postgres too, so FOUNDRY_KEEP_PG is deliberately NOT
# set here.
#
# Overridable via env (passed through to run.sh, which owns their defaults):
#   FOUNDRY_PORT           app port          (default 3000)
#   FOUNDRY_PG_HOST_PORT   Postgres host port
#   FOUNDRY_RELEASE=1      build --release   (default: debug)
#   FOUNDRY_NO_AUTOCLAIM=1 skip the dev-admin auto-claim
#
# Flags:
#   --open       open a browser too (default: do NOT — a restart should not
#                steal focus, and the tab you already have is still correct)
#   -h, --help   this message.

set -euo pipefail
cd "$(dirname "$0")"

APP_PORT="${FOUNDRY_PORT:-3000}"
URL="http://localhost:${APP_PORT}"
PROFILE_FLAG=""
[ "${FOUNDRY_RELEASE:-0}" = "1" ] && PROFILE_FLAG="--release"
OPEN_BROWSER=0

log()  { printf '\033[1;34m▶ %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m✔ %s\033[0m\n' "$*"; }
err()  { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }

while [ $# -gt 0 ]; do
  case "$1" in
    --open) OPEN_BROWSER=1; shift ;;
    -h|--help) sed -n '2,44p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) err "unknown flag: $1 (try --help)"; exit 2 ;;
  esac
done

for bin in docker cargo curl; do
  command -v "$bin" >/dev/null 2>&1 || { err "'$bin' not found on PATH"; exit 1; }
done
[ -x ./run.sh ]  || { err "./run.sh is missing or not executable"; exit 1; }
[ -x ./stop.sh ] || { err "./stop.sh is missing or not executable"; exit 1; }

# --- 1. stop what is running -----------------------------------------------
log "stopping the running stack…"
./stop.sh

# --- 2. build HERE, so failures are visible --------------------------------
# See the header: this is the whole reason the build is not simply left to the
# backgrounded run.sh.
log "building foundry${PROFILE_FLAG:+ (release)} — first build can take a few minutes…"
if ! cargo build $PROFILE_FLAG --bin foundry; then
  err "build failed — nothing was restarted, and the stack is stopped"
  err "fix the errors above, then run ./restart.sh again"
  exit 1
fi
ok "build succeeded"

# --- 3. relaunch detached --------------------------------------------------
# `nohup` + `</dev/null` + `disown` so run.sh survives this shell exiting and
# never competes for stdin. FOUNDRY_NO_OPEN is honoured by run.sh; the `--open`
# flag here opts back in.
RUN_LOG="$(mktemp -t foundry-restart.XXXXXX)"
log "relaunching (detached)…"
if [ "$OPEN_BROWSER" = 1 ]; then
  nohup ./run.sh >"$RUN_LOG" 2>&1 </dev/null &
else
  FOUNDRY_NO_OPEN=1 nohup ./run.sh >"$RUN_LOG" 2>&1 </dev/null &
fi
RUN_PID=$!
disown "$RUN_PID" 2>/dev/null || true

# --- 4. wait for health ----------------------------------------------------
# Bounded, and it watches the SUPERVISOR as well as the endpoint: if run.sh
# dies (port taken, Postgres refused to start, a panic on boot) we say so
# immediately with its output, instead of burning the full timeout on a process
# that is already gone. 600 × 1s matches run.sh's own health budget.
log "waiting for Foundry at ${URL}/healthz …"
for _ in $(seq 1 600); do
  if ! kill -0 "$RUN_PID" 2>/dev/null; then
    err "run.sh exited during startup:"
    tail -30 "$RUN_LOG" >&2
    exit 1
  fi
  curl -fsS "${URL}/healthz" >/dev/null 2>&1 && break
  sleep 1
done
if ! curl -fsS "${URL}/healthz" >/dev/null 2>&1; then
  err "Foundry did not become healthy; see $RUN_LOG"
  tail -30 "$RUN_LOG" >&2
  exit 1
fi

# Surface the dev-login banner / bootstrap link run.sh printed into its log on
# a fresh database — it is the one piece of run.sh's output a detached launch
# would otherwise swallow, and without it a first run is unusable.
if grep -q 'DEV LOGIN\|\[BOOTSTRAP\]' "$RUN_LOG" 2>/dev/null; then
  grep -A3 'DEV LOGIN' "$RUN_LOG" 2>/dev/null || true
  grep -m1 '\[BOOTSTRAP\]' "$RUN_LOG" 2>/dev/null || true
fi

ok "Foundry is up at ${URL}"
log "supervisor pid $RUN_PID   ·   logs → $RUN_LOG"
log "stop it with: ./stop.sh"
