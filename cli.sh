#!/usr/bin/env bash
#
# cli.sh — run the Foundry CLI (`foundry doctor …`) against your local dev DB.
#
# A thin wrapper that spares you the env dance: it loads .env, points
# DATABASE_URL at the same Postgres `run.sh` uses (the `foundry-ui-pg`
# container), supplies a dev SESSION_SECRET when one isn't set, builds the
# `foundry` binary, then forwards every argument straight to it.
#
#   ./cli.sh doctor list-workspaces
#   ./cli.sh doctor grant-super-admin --email you@example.com
#   ./cli.sh doctor provision-workspace --name "Acme" --admin-email a@acme.io --as you@example.com
#   ./cli.sh doctor export-workspace "Dev Workspace" /tmp/dev.tar
#   ./cli.sh doctor verify-export /tmp/dev.tar        # offline, no DB needed
#
# The `doctor` subcommands read (not create) the database — start the app once
# with ./run.sh first so migrations are applied and a workspace exists.
#
# DATABASE_URL is chosen for you (see precedence below) — the running
# `foundry-ui-pg` container wins over any compose-host value in your shell/.env,
# because `.env` points at `postgres:5432` (the docker-network host) which a
# natively-run binary can't resolve.
#
# Overridable via env:
#   FOUNDRY_CLI_DATABASE_URL  force a specific database, used verbatim (wins over
#                             the container). Use this to target staging/prod.
#   FOUNDRY_PG_CONTAINER      Postgres container name to reuse (default foundry-ui-pg)
#   FOUNDRY_RELEASE=1         build --release (default: debug, faster incremental)

set -euo pipefail
cd "$(dirname "$0")"

PG_CONTAINER="${FOUNDRY_PG_CONTAINER:-foundry-ui-pg}"
PROFILE_DIR="debug"
PROFILE_FLAG=""
[ "${FOUNDRY_RELEASE:-0}" = "1" ] && { PROFILE_FLAG="--release"; PROFILE_DIR="release"; }

log()  { printf '\033[1;34m▶ %s\033[0m\n' "$*" >&2; }
ok()   { printf '\033[1;32m✔ %s\033[0m\n' "$*" >&2; }
err()  { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }

# --- help (no args) --------------------------------------------------------
if [ "$#" -eq 0 ]; then
  cat >&2 <<'EOF'
Usage: ./cli.sh <foundry-subcommand> [args…]

Foundry CLI (`foundry doctor …`) subcommands:
  list-workspaces                                     list every workspace (id + name)
  grant-super-admin --email <addr>                    make an existing user an instance super-admin
  provision-workspace --name <n> --admin-email <a>    create a new workspace + first admin
                      --as <super-admin-email>        (prints the first-admin invite link)
  export-workspace <id|name> <out-path>               export one workspace to a tar archive
  verify-export <archive-path>                         verify an export archive (offline, no DB)
  restore-comment <comment-uuid>                      un-delete a soft-deleted comment
  backup-verify <file>                                verify a pg_dump archive
                                                      (set FOUNDRY_DOCTOR_PROBE_URL first)

Examples:
  ./cli.sh doctor list-workspaces
  ./cli.sh doctor grant-super-admin --email you@example.com

To run the server instead, use ./run.sh (it applies migrations and serves the UI).
EOF
  exit 2
fi

# --- environment -----------------------------------------------------------
# Load .env / .env.example WITHOUT executing it (values may contain spaces —
# the Ed25519 PEM keys embed "BEGIN PUBLIC KEY"). Real env wins over .env.
load_dotenv() {
  local line key val
  while IFS= read -r line || [ -n "$line" ]; do
    line="${line%$'\r'}"
    case "$line" in ''|'#'*) continue ;; esac
    [ "$line" != "${line#*=}" ] || continue
    key="${line%%=*}"; val="${line#*=}"
    key="${key#"${key%%[![:space:]]*}"}"
    key="${key%"${key##*[![:space:]]}"}"
    case "$key" in [A-Za-z_][A-Za-z0-9_]*) ;; *) continue ;; esac
    if   [[ $val == \"*\" ]]; then val="${val#\"}"; val="${val%\"}"
    elif [[ $val == \'*\' ]]; then val="${val#\'}"; val="${val%\'}"
    else
      case "$val" in *" #"*) val="${val%% #*}" ;; esac
      val="${val%"${val##*[![:space:]]}"}"
    fi
    [ -n "${!key+x}" ] || export "$key=$val"
  done < "$1"
}
if   [ -f .env ];         then load_dotenv .env
elif [ -f .env.example ]; then load_dotenv .env.example
fi

# DATABASE_URL precedence (native execution, mirroring run.sh):
#   1. FOUNDRY_CLI_DATABASE_URL — explicit override, used verbatim.
#   2. the running run.sh Postgres container — its localhost-mapped port, which
#      OVERRIDES any DATABASE_URL from your shell or .env. Those point at the
#      compose-network host `postgres:5432`, which a natively-run binary cannot
#      resolve ("nodename nor servname"); the container's published port on
#      localhost is what's actually reachable — and it's the DB run.sh uses, so
#      it's where your workspace and account live.
#   3. an existing DATABASE_URL (env/.env) — fallback when no container is up.
# We read the container's published host port rather than assume 5432 (the port
# is fixed at creation, and run.sh auto-advances off 5432 when it's taken).
if [ -n "${FOUNDRY_CLI_DATABASE_URL:-}" ]; then
  export DATABASE_URL="$FOUNDRY_CLI_DATABASE_URL"
  log "using FOUNDRY_CLI_DATABASE_URL override"
elif command -v docker >/dev/null 2>&1 && docker ps --format '{{.Names}}' 2>/dev/null | grep -qx "$PG_CONTAINER"; then
  PG_PORT="$(docker port "$PG_CONTAINER" 5432/tcp 2>/dev/null | head -1 | sed 's/.*://')"
  if [ -n "${PG_PORT:-}" ]; then
    export DATABASE_URL="postgres://foundry:foundry@localhost:${PG_PORT}/foundry"
    log "using Postgres from container '$PG_CONTAINER' on :${PG_PORT}"
  else
    err "container '$PG_CONTAINER' is running but has no published 5432 port — can't reach it natively."
  fi
elif [ -n "${DATABASE_URL:-}" ]; then
  log "no '$PG_CONTAINER' container running — using DATABASE_URL as-is ($DATABASE_URL)"
fi
if [ -z "${DATABASE_URL:-}" ]; then
  err "No database: the '$PG_CONTAINER' container isn't running and no DATABASE_URL/FOUNDRY_CLI_DATABASE_URL is set."
  err "Start the app once with ./run.sh (creates the DB + applies migrations), or set FOUNDRY_CLI_DATABASE_URL."
  err "(Offline subcommands like 'doctor verify-export' don't need a DB and will still run.)"
fi

# SESSION_SECRET: provision-workspace signs the invite link with it (>= 32 bytes).
# Match run.sh's built-in dev default so the CLI works out of the box.
if [ -z "${SESSION_SECRET:-}" ] || [ "${#SESSION_SECRET}" -lt 32 ]; then
  export SESSION_SECRET="dev-only-secret-change-me-at-least-32-bytes-long-please"
fi
export FOUNDRY_PUBLIC_URL="${FOUNDRY_PUBLIC_URL:-http://localhost:${FOUNDRY_PORT:-3000}}"

# --- build + run -----------------------------------------------------------
# Build quietly (incremental — near-instant when nothing changed) so the CLI's
# `key: value` output on stdout stays clean, then exec so the binary's exit
# code (the doctor commands' meaningful 0/2/3/4/…) propagates unchanged.
command -v cargo >/dev/null 2>&1 || { err "'cargo' not found on PATH"; exit 1; }
log "building foundry${PROFILE_FLAG:+ (release)}…"
cargo build $PROFILE_FLAG --bin foundry >&2
BIN="target/${PROFILE_DIR}/foundry"
[ -x "$BIN" ] || { err "built binary not found at $BIN"; exit 1; }

exec "$BIN" "$@"
