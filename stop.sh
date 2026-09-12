#!/usr/bin/env bash
#
# stop.sh — stop everything `./run.sh` or `./restart.sh` started.
#
# The exact inverse of run.sh's own teardown, reachable from a DIFFERENT
# terminal. run.sh already stops the app and its Postgres on exit (`cleanup()`,
# run.sh:174) — but only for whoever is holding its Ctrl-C. This script is for
# the other cases: a `FOUNDRY_KEEP_PG=1` run that deliberately left Postgres up,
# a detached `./restart.sh`, a run.sh whose terminal is gone, or a bare
# `cargo run` that still owns :3000.
#
# Stops, in order:
#   1. the run.sh supervisor, if one is alive — TERM makes its OWN trap tear
#      down the app and Postgres, which is strictly better than us racing it.
#   2. any remaining Foundry app process (the bare-`cargo run` case).
#   3. the script-owned Postgres container (`foundry-ui-pg`).
#   4. leaked acceptance-suite containers (see LEAKED, below).
#
# DOES NOT touch the compose stacks — `docker-compose.yml` (postgres + foundry)
# and `docker-compose.observability.yml` (prometheus/loki/promtail/grafana) are
# long-lived operator stacks, not dev-loop state. Stop those deliberately with
# `docker compose down`.
#
# THE DATABASE SURVIVES. `foundry-ui-pg` is created with `--rm`, so stopping it
# also removes the container — but the data lives in the named volume
# `foundry-ui-pg-data`, which outlives both. The next `./run.sh` picks up
# exactly where you left off. To discard the data you must remove the volume
# yourself: `docker volume rm foundry-ui-pg-data`.
#
# Overridable via env (same names run.sh uses, so both read one config):
#   FOUNDRY_PORT           app port          (default 3000)
#   FOUNDRY_PG_CONTAINER   pg container name (default foundry-ui-pg)
#
# Flags:
#   --testcontainers   ALSO remove testcontainers-managed containers. Opt-in
#                      and never the default; see LEAKED below for why.
#   -h, --help         this message.

set -euo pipefail
cd "$(dirname "$0")"

APP_PORT="${FOUNDRY_PORT:-3000}"
PG_CONTAINER="${FOUNDRY_PG_CONTAINER:-foundry-ui-pg}"
PG_VOLUME="${FOUNDRY_PG_VOLUME:-foundry-ui-pg-data}"
WITH_TESTCONTAINERS=0

# Identical helpers to run.sh:41-44 — deliberately duplicated rather than
# sourced. These four lines are trivial and stable; the logic worth
# single-sourcing (Postgres lifecycle, dotenv parsing, the bootstrap claim)
# is NOT duplicated anywhere — restart.sh delegates all of it to run.sh.
log()  { printf '\033[1;34m▶ %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m✔ %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m! %s\033[0m\n' "$*"; }
err()  { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }
# True when nothing is listening on 127.0.0.1:$1 (i.e. the port is free).
port_free() { ! (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

while [ $# -gt 0 ]; do
  case "$1" in
    --testcontainers) WITH_TESTCONTAINERS=1; shift ;;
    -h|--help) sed -n '2,37p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) err "unknown flag: $1 (try --help)"; exit 2 ;;
  esac
done

command -v docker >/dev/null 2>&1 || { err "'docker' not found on PATH"; exit 1; }
DOCKER_UP=1
docker info >/dev/null 2>&1 || DOCKER_UP=0
[ "$DOCKER_UP" = 1 ] || warn "Docker daemon not reachable — stopping processes only"

# --- 1. the supervisor ------------------------------------------------------
# Kill run.sh FIRST and let its EXIT trap do the teardown. Killing the app out
# from under a live run.sh would just leave the supervisor sitting in its
# `wait`, and on a watch-mode run cargo-watch would cheerfully restart the very
# process we just killed.
#
# `-f run.sh` would also match THIS script's own argv on some platforms, and
# matching `restart.sh` would kill a rebuild in progress, so the pattern is
# anchored to how the shell actually invokes the launcher.
stop_supervisor() {
  local pids
  pids="$(pgrep -f '(^|/)(bash|sh|zsh) .*(^|/)run\.sh' 2>/dev/null || true)"
  pids="$(printf '%s\n' "$pids" | grep -vx "$$" | grep -v '^$' || true)"
  if [ -z "$pids" ]; then
    log "no run.sh supervisor alive"
    return
  fi
  log "stopping run.sh supervisor(s): $(echo "$pids" | tr '\n' ' ')"
  # shellcheck disable=SC2086
  kill $pids 2>/dev/null || true
  # Give its trap a moment to stop the app and the container it owns.
  for _ in $(seq 1 25); do
    pgrep -f '(^|/)(bash|sh|zsh) .*(^|/)run\.sh' >/dev/null 2>&1 || break
    sleep 0.2
  done
  ok "supervisor stopped"
}

# --- 2. remaining app processes ---------------------------------------------
# The SAME four patterns run.sh's `reap_foundry` uses (run.sh:59-73), so "what
# counts as a Foundry app process" is defined once in shape even though the
# code is in two files. A bare `cargo run --bin foundry` has no supervisor to
# TERM, so this is the only thing that catches it.
reap_app() {
  local self=$$ found=0 pids pat
  for pat in "target/debug/foundry" "target/release/foundry" \
             "cargo-watch .*--bin foundry" "run .*--bin foundry"; do
    pids="$(pgrep -f "$pat" 2>/dev/null | grep -vx "$self" || true)"
    [ -n "$pids" ] || continue
    found=1
    log "reaping Foundry process(es) [$pat]: $(echo "$pids" | tr '\n' ' ')"
    # shellcheck disable=SC2086
    kill $pids 2>/dev/null || true
  done
  for _ in $(seq 1 25); do port_free "$APP_PORT" && break; sleep 0.2; done
  if [ "$found" != 1 ]; then
    log "no Foundry app process found"
  elif port_free "$APP_PORT"; then
    ok "app stopped; :$APP_PORT released"
  else
    warn "app killed but :$APP_PORT is still held by something else"
  fi
}

# --- 3. the script-owned Postgres ------------------------------------------
stop_pg() {
  [ "$DOCKER_UP" = 1 ] || return 0
  if docker ps --format '{{.Names}}' | grep -qx "$PG_CONTAINER"; then
    log "stopping Postgres ($PG_CONTAINER)"
    docker stop "$PG_CONTAINER" >/dev/null 2>&1 || true
    ok "Postgres stopped; data kept in volume $PG_VOLUME"
  else
    # `--rm` means a stopped one is already gone, so "not running" is the
    # normal clean state here, not a surprise.
    log "Postgres ($PG_CONTAINER) not running"
  fi
}

# --- 4. LEAKED acceptance containers ---------------------------------------
# Two prefixes, both unambiguously this repo's test harness:
#   foundry-acceptance-chrome-<pid>   browser_harness.rs:200 (`docker run --rm`)
#   foundry-at-<8hex>-*              compose_harness.rs:65 (`-p foundry-at-…`)
# Neither can collide with `foundry-ui-pg`, nor with the real compose stack's
# `foundry-postgres-1` / `foundry-foundry-1`. Both are meant to be reaped on a
# clean exit — `browser_harness::shutdown_chromedriver()` and
# `harness::shutdown_postgres()` — but an INTERRUPTED run (Ctrl-C, a panic
# before the reaper, a killed test binary) strands them, and nothing else ever
# collects them. `std::process::Child` does not kill on drop and a `static`'s
# `Drop` never runs at process exit, as browser_harness.rs itself records.
#
# WHY testcontainers IS OPT-IN: testcontainers-managed containers carry
# `org.testcontainers.managed-by=testcontainers` and NOTHING that names the
# project that started them. On this machine that label matches canzan-lift's
# live `canzan-lift-test-pg` and `canzan-lift-test-mailpit` alongside foundry's
# anonymous `postgres:16-alpine` leftovers. Removing by label alone would take
# out another project's running fixtures, so the default is to COUNT them and
# say so. `--testcontainers` opts in, and still skips anything named for
# another project.
clean_leaked() {
  [ "$DOCKER_UP" = 1 ] || return 0
  local -a leaked=() tc=()
  # Read names into an ARRAY rather than splitting a command substitution: a
  # container name can in principle carry anything the shell would glob on, and
  # `docker rm -f "${arr[@]}"` is both safe and empty-safe.
  while IFS= read -r n; do [ -n "$n" ] && leaked+=("$n"); done < <(
    docker ps -a --format '{{.Names}}' \
      | grep -E '^(foundry-acceptance-|foundry-at-)' || true
  )
  if [ "${#leaked[@]}" -gt 0 ]; then
    log "removing ${#leaked[@]} leaked acceptance container(s)"
    docker rm -f "${leaked[@]}" >/dev/null 2>&1 || true
    ok "leaked acceptance containers removed"
  else
    log "no leaked acceptance containers"
  fi

  while IFS= read -r n; do [ -n "$n" ] && tc+=("$n"); done < <(
    docker ps -a --format '{{.Names}}' \
      --filter 'label=org.testcontainers.managed-by=testcontainers' \
      | grep -v '^canzan-lift-' || true
  )
  [ "${#tc[@]}" -gt 0 ] || return 0
  if [ "$WITH_TESTCONTAINERS" = 1 ]; then
    log "removing ${#tc[@]} testcontainers-managed container(s)"
    docker rm -f "${tc[@]}" >/dev/null 2>&1 || true
    ok "testcontainers-managed containers removed"
  else
    warn "${#tc[@]} testcontainers-managed container(s) still up — ownership cannot be"
    warn "proven from Docker metadata, so they are left alone. Remove with:"
    warn "  ./stop.sh --testcontainers"
  fi
}

stop_supervisor
reap_app
stop_pg
clean_leaked
ok "Foundry dev stack stopped"
