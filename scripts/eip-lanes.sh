# shellcheck shell=bash
# external-integration-plugins DELIVER lanes. Source it, then call a function:
#
#   source scripts/eip-lanes.sh
#   eip_pre                       # refuse to run beside another session or a foreign listener
#   eip_warm                      # release foundry binary built and answering --help fast
#   eip_lane eip-01               # one acceptance tag, bounded
#   eip_mut crates/foundry-integrations/src/route.rs ...   # cargo-mutants, prints the kill rate
#   lane_common foundry-integrations foundry-services     # phase-end regression on touched crates
#
# The KPI-4 bench (manual, never in CI):
#   eip_warm && EIP_KPI4_MODE=baseline timeout 90m cargo test -p foundry-acceptance \
#     --release --test eip_kpi4_bench -- --ignored --nocapture
#
# Every command is bounded by `timeout`. Bounds override: EIP_LANE_TIMEOUT,
# EIP_MUT_TIMEOUT, EIP_BUILD_TIMEOUT (seconds).

export FOUNDRY_XTASK_INCLUDE_DOCKER=1
export RUST_TEST_THREADS=1
export CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false

EIP_ROOT="${EIP_ROOT:-$(git rev-parse --show-toplevel)}"
EIP_LANE_TIMEOUT="${EIP_LANE_TIMEOUT:-2700}"
EIP_MUT_TIMEOUT="${EIP_MUT_TIMEOUT:-3600}"
EIP_BUILD_TIMEOUT="${EIP_BUILD_TIMEOUT:-1800}"
# Listener commands eip_pre accepts (our own and the Docker VM's port forwarders).
EIP_PRE_IGNORE="${EIP_PRE_IGNORE:-^(foundry|com\.docke|docker|limactl)}"

eip__fail() {
  echo "eip-lanes: $*" >&2
  return 1
}

# Other cargo/foundry runs (not this shell's) and listeners on 5xxxx ports that
# are not ours make Docker-backed lanes flaky; say so loudly instead of running.
eip_pre() {
  local others listeners
  others="$(timeout 10 ps -axo pid=,ppid=,command= |
    awk -v me="$$" '$1 != me && $2 != me { n = split($3, p, "/") } $1 != me && $2 != me && p[n] ~ /^(cargo|rustc|foundry)$/')"
  listeners="$(timeout 10 lsof -nP -iTCP -sTCP:LISTEN 2>/dev/null |
    awk -v ok="$EIP_PRE_IGNORE" 'NR > 1 && $9 ~ /:5[0-9][0-9][0-9][0-9]$/ && $1 !~ ok')"
  if [[ -n "$others" ]]; then
    eip__fail "other cargo/foundry processes are running:"$'\n'"$others" || return
  fi
  if [[ -n "$listeners" ]]; then
    eip__fail "foreign listeners on 5xxxx ports:"$'\n'"$listeners" || return
  fi
  echo "eip-lanes: machine quiet ($(timeout 5 uptime))"
}

# Build the release binary the subprocess scenarios and the bench spawn, then run
# --help until it answers within a second (a fresh binary stalls in syspolicyd).
eip_warm() {
  (cd "$EIP_ROOT" &&
    timeout "$EIP_BUILD_TIMEOUT" cargo build --release -p foundry-app --bin foundry) || return
  local attempt start elapsed
  for attempt in 1 2 3 4 5 6; do
    start=$(date +%s)
    # --help is not a subcommand (non-zero exit); only the time to answer matters.
    timeout 120 "$EIP_ROOT/target/release/foundry" --help >/dev/null 2>&1
    [[ $? -ne 124 ]] || eip__fail "foundry --help hung for 120s" || return
    elapsed=$(($(date +%s) - start))
    echo "eip-lanes: warm attempt $attempt took ${elapsed}s"
    [[ "$elapsed" -lt 1 ]] && return 0
  done
  eip__fail "foundry --help still slow after 6 attempts"
}

# One acceptance tag selection, bounded.
eip_lane() {
  local tag="${1:?usage: eip_lane <tag>}"
  (cd "$EIP_ROOT" &&
    FOUNDRY_ACCEPTANCE_TAGS="$tag" timeout "$EIP_LANE_TIMEOUT" \
      cargo test -p foundry-acceptance --release --test acceptance)
}

# cargo-mutants on the given files only; prints caught/(caught+missed).
eip_mut() {
  [[ $# -gt 0 ]] || eip__fail "usage: eip_mut <file>..." || return
  local out="$EIP_ROOT/target/eip-mutants" args=() file
  for file in "$@"; do args+=(--file "$file"); done
  (cd "$EIP_ROOT" &&
    timeout "$EIP_MUT_TIMEOUT" cargo mutants --no-shuffle --output "$out" "${args[@]}")
  local rc=$? caught missed
  caught=$(wc -l <"$out/mutants.out/caught.txt" 2>/dev/null || echo 0)
  missed=$(wc -l <"$out/mutants.out/missed.txt" 2>/dev/null || echo 0)
  if [[ $((caught + missed)) -gt 0 ]]; then
    echo "eip-lanes: kill rate $((100 * caught / (caught + missed)))% ($caught caught, $missed missed)"
  fi
  return "$rc"
}

# Phase-end regression for the crates a phase touched (default foundry-acceptance).
lane_common() {
  local crates=("$@") crate clippy_args=() test_args=()
  [[ ${#crates[@]} -gt 0 ]] || crates=(foundry-acceptance)
  for crate in "${crates[@]}"; do
    clippy_args+=(-p "$crate")
    [[ "$crate" == foundry-acceptance ]] || test_args+=(-p "$crate")
  done
  (cd "$EIP_ROOT" &&
    timeout 300 cargo fmt --all -- --check &&
    timeout "$EIP_BUILD_TIMEOUT" cargo clippy "${clippy_args[@]}" --all-targets -- -D warnings &&
    timeout 600 cargo xtask check-arch &&
    { [[ ${#test_args[@]} -eq 0 ]] || timeout "$EIP_LANE_TIMEOUT" cargo test "${test_args[@]}"; }) ||
    return
  eip_lane external-integration-plugins
}
