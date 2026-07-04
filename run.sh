#!/usr/bin/env bash
#
# run.sh — launch Foundry locally and open the UI in a browser.
#
# Mode: native app + Dockerized Postgres (the DEVELOPER.md inner-loop shape).
#   1. Start a script-owned Postgres 16 container with a host port mapping
#      (the compose `postgres` service is docker-network-only, so a native
#      `cargo run` cannot reach it — this container is separate and host-
#      reachable, and won't collide with the acceptance-suite compose stacks).
#   2. Load env from .env (or .env.example on a fresh clone) for the required
#      boot secrets (SESSION_SECRET, MACHINE_TOKEN_PUBLIC_KEYS), then override
#      DATABASE_URL to talk to Postgres on localhost.
#   3. Build + run `foundry`, which runs migrations and serves the UI on :3000.
#   4. Wait for GET /healthz, print the first-run admin-claim URL if the
#      database is unclaimed, then open http://localhost:3000 in a browser.
#
# Overridable via env:
#   FOUNDRY_PORT           app port          (default 3000)
#   FOUNDRY_PG_HOST_PORT   Postgres host port(default 5432; change if taken)
#   FOUNDRY_RELEASE=1      build --release   (default: debug, faster first build)
#
# Ctrl-C stops Foundry; Postgres is left running for the next launch.

set -euo pipefail
cd "$(dirname "$0")"

APP_PORT="${FOUNDRY_PORT:-3000}"
PG_CONTAINER="foundry-ui-pg"
PG_VOLUME="foundry-ui-pg-data"
URL="http://localhost:${APP_PORT}"
PROFILE_FLAG=""
[ "${FOUNDRY_RELEASE:-0}" = "1" ] && PROFILE_FLAG="--release"

log()  { printf '\033[1;34m▶ %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m✔ %s\033[0m\n' "$*"; }
err()  { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }
# True when nothing is listening on 127.0.0.1:$1 (i.e. the port is free).
port_free() { ! (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

# --- preflight -------------------------------------------------------------
for bin in docker cargo curl; do
  command -v "$bin" >/dev/null 2>&1 || { err "'$bin' not found on PATH"; exit 1; }
done
docker info >/dev/null 2>&1 || { err "Docker daemon not reachable — start Docker/OrbStack/colima"; exit 1; }

# --- environment -----------------------------------------------------------
# Load a dotenv file WITHOUT executing it: values here are unquoted and can
# contain spaces (the Ed25519 PEM keys embed "BEGIN PUBLIC KEY"), so `source`
# would try to run words as commands. Parse KEY=VALUE literally instead.
load_dotenv() {
  local line key val
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"                              # strip CRLF
    case "$line" in ''|'#'*) continue ;; esac         # blank / comment
    [ "$line" != "${line#*=}" ] || continue           # no '=' → skip
    key="${line%%=*}"; val="${line#*=}"
    key="${key#"${key%%[![:space:]]*}"}"              # ltrim key
    key="${key%"${key##*[![:space:]]}"}"              # rtrim key
    case "$key" in [A-Za-z_][A-Za-z0-9_]*) ;; *) continue ;; esac
    if   [[ $val == \"*\" ]]; then val="${val#\"}"; val="${val%\"}"
    elif [[ $val == \'*\' ]]; then val="${val#\'}"; val="${val%\'}"
    else
      case "$val" in *" #"*) val="${val%% #*}" ;; esac # strip inline comment
      val="${val%"${val##*[![:space:]]}"}"            # rtrim value
    fi
    export "$key=$val"
  done < "$1"
}
if   [ -f .env ];         then load_dotenv .env;         log "loaded .env"
elif [ -f .env.example ]; then load_dotenv .env.example; log "loaded .env.example (no .env found)"
fi
# Native-mode overrides: reach Postgres on the host, not the compose network.
# (DATABASE_URL is set after the Postgres host port is finalized, below.)
export FOUNDRY_PORT="$APP_PORT"
export FOUNDRY_PUBLIC_URL="$URL"
export SESSION_COOKIE_SECURE="${SESSION_COOKIE_SECURE:-false}"
if [ -z "${SESSION_SECRET:-}" ] || [ "${#SESSION_SECRET}" -lt 32 ]; then
  export SESSION_SECRET="dev-only-secret-change-me-at-least-32-bytes-long-please"
  log "using built-in dev SESSION_SECRET"
fi

# --- Postgres --------------------------------------------------------------
# A container's published port is fixed at creation. So: reuse ours if it's
# already running (reading its actual host port), otherwise drop any stopped
# remnant and create fresh on a free port. 5432 is often taken (another
# Postgres, an SSH tunnel), so auto-advance unless FOUNDRY_PG_HOST_PORT pins it.
if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
  PG_PORT="$(docker port "$PG_CONTAINER" 5432/tcp 2>/dev/null | head -1 | sed 's/.*://')"
  log "reusing running Postgres ($PG_CONTAINER) on :${PG_PORT}"
else
  docker rm -f "$PG_CONTAINER" >/dev/null 2>&1 || true   # clear stopped remnant
  if [ -n "${FOUNDRY_PG_HOST_PORT:-}" ]; then
    PG_PORT="$FOUNDRY_PG_HOST_PORT"
    port_free "$PG_PORT" || { err "port $PG_PORT (FOUNDRY_PG_HOST_PORT) is already in use"; exit 1; }
  else
    PG_PORT=5432
    while ! port_free "$PG_PORT"; do PG_PORT=$((PG_PORT + 1)); done
  fi
  log "creating Postgres on :${PG_PORT} ($PG_CONTAINER)"
  docker run -d --name "$PG_CONTAINER" \
    -e POSTGRES_USER=foundry -e POSTGRES_PASSWORD=foundry -e POSTGRES_DB=foundry \
    -p "${PG_PORT}:5432" -v "${PG_VOLUME}:/var/lib/postgresql/data" \
    postgres:16-alpine >/dev/null
fi
# Now that the host port is known, point the app at it.
export DATABASE_URL="postgres://foundry:foundry@localhost:${PG_PORT}/foundry"

log "waiting for Postgres…"
for _ in $(seq 1 30); do
  docker exec "$PG_CONTAINER" pg_isready -U foundry -d foundry >/dev/null 2>&1 && break
  sleep 1
done
docker exec "$PG_CONTAINER" pg_isready -U foundry -d foundry >/dev/null 2>&1 \
  || { err "Postgres did not become ready"; exit 1; }
ok "Postgres ready on :${PG_PORT}"

# --- build + run -----------------------------------------------------------
log "building foundry (first build can take a few minutes)…"
cargo build $PROFILE_FLAG --bin foundry

APP_LOG="$(mktemp -t foundry-run.XXXXXX)"
cargo run $PROFILE_FLAG --bin foundry >"$APP_LOG" 2>&1 &
APP_PID=$!

cleanup() {
  kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
  log "Foundry stopped. Postgres left running — stop it with: docker stop $PG_CONTAINER"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

log "waiting for Foundry at ${URL}/healthz …"
for _ in $(seq 1 120); do
  kill -0 "$APP_PID" 2>/dev/null || { err "foundry exited during startup:"; tail -20 "$APP_LOG" >&2; exit 1; }
  curl -fsS "${URL}/healthz" >/dev/null 2>&1 && break
  sleep 1
done
curl -fsS "${URL}/healthz" >/dev/null 2>&1 || { err "foundry not healthy; see $APP_LOG"; exit 1; }
ok "Foundry is up at ${URL}"

# First-run admin-claim link (printed by the app when the workspace is unclaimed).
BOOTSTRAP="$(grep -m1 '\[BOOTSTRAP\]' "$APP_LOG" 2>/dev/null || true)"
[ -n "$BOOTSTRAP" ] && printf '\033[1;33m%s\033[0m\n' "$BOOTSTRAP"

if   command -v open     >/dev/null 2>&1; then open "$URL"
elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$URL"
else log "open your browser at $URL"
fi

log "logs → $APP_LOG   ·   Ctrl-C to stop"
wait "$APP_PID"
