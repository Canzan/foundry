# Foundry Developer Guide

The nuts and bolts of working on Foundry's code: local setup, the gate you must
pass before pushing, the inner loop, crate boundaries, and CI. If you only want
to fix docs or file an issue, you don't need any of this — start at
[CONTRIBUTING.md](./CONTRIBUTING.md). For what Foundry *is* and how to run it,
see the [README](./README.md).

## Quickstart on a fresh machine

The README's [Quickstart](./README.md#quickstart) walks from `git clone` to a
green test run in five commands. Once those work, these are the contributor-only
follow-ups:

```sh
# The full local gate — fmt + clippy + boundary guard + build + test +
# cargo deny + the acceptance suite. This is the command CI runs; it must be
# green before you push (see "The local gate" below). Auto-detects Docker;
# FOUNDRY_XTASK_INCLUDE_DOCKER=1 forces the @docker-compose group on.
cargo xtask ci

# Inner-loop edit/save/reload for the running app.
cargo install cargo-watch
cargo watch -x 'run --bin foundry'
```

First-time notes:

- **`cargo test` works without `DATABASE_URL`.** The acceptance harness boots its
  own ephemeral Postgres via testcontainers; don't export `DATABASE_URL` in your
  shell unless you're running the app outside the test suite.
- **The Rust toolchain auto-installs.** `rust-toolchain.toml` pins the version;
  `rustup` fetches the exact channel on your first `cargo` invocation — there's no
  `rustup install` step.
- **`cargo watch -x 'run --bin foundry'`** plus a browser refresh at
  `http://localhost:3000` is the documented one-line-edit-to-visible loop. See
  manual Drill B in
  [`docs/feature/foundry-contributor-onboarding/distill/manual-drills.md`](./docs/feature/foundry-contributor-onboarding/distill/manual-drills.md).

## The local gate — `cargo xtask ci`

**`cargo xtask ci` is the single source of truth for "is this change OK to
push?"** CI runs the exact same command, so green locally means green in CI —
they cannot drift. It runs every check in order and stops on the first failure:

| Check | Command it runs |
|-------|-----------------|
| Format | `cargo fmt --all -- --check` |
| Lints | `cargo clippy --all-targets --release -- -D warnings` |
| Boundary guard | `cargo xtask check-arch` (architecture rules — see [Crate boundaries](#crate-boundaries)) |
| Build | `cargo build --all --release` |
| Tests | `cargo test --workspace` (excludes the heavy acceptance crate, run separately below) |
| Licenses + advisories | `cargo deny check` |
| Acceptance | `cargo test -p foundry-acceptance` with `FOUNDRY_ACCEPTANCE_TAGS=all` |

**Do not push unless `cargo xtask ci` prints `all gates green`** — this is a
hard project rule ([AGENTS.md](./AGENTS.md)). If a check ever runs for the first
time in CI rather than locally, that's a process bug: add it to `xtask::run_ci`,
not just to the workflow.

```sh
cargo xtask ci
# Force the slow @docker-compose acceptance group on (CI sets this):
FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci
```

`cargo xtask ci` checks for the tools it needs and prints an install hint if one
is missing. Install these once:

- **`cargo-deny`** — `cargo install --locked cargo-deny`.
- **A PostgreSQL 16+ client** (`pg_dump`/`pg_restore` on PATH) for the US-03
  backup lane — macOS `brew install postgresql@16` (then add its `bin` to PATH),
  Debian/Ubuntu `apt-get install -y postgresql-client-16`.
- **A reachable Docker daemon** for the acceptance suite (see
  [Docker on macOS](#docker-on-macos-colima--orbstack--lima)).
- A `.env` is auto-seeded from `.env.example` when missing.

## Inner loop

For fast iteration you can run the underlying checks directly:

```sh
cargo build --all
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

The `@docker-compose` and `@manual` cucumber tag groups are slow and
human-driven respectively; they're excluded from the default run and exercised
via tag filter:

```sh
FOUNDRY_ACCEPTANCE_TAGS=docker-compose cargo test -p foundry-acceptance
```

### System prereqs for the acceptance suite

Some acceptance lanes drive system tooling beyond the Rust toolchain. Install
these once:

| Tool | Lane | Install (macOS) | Install (Debian/Ubuntu) |
|------|------|------------------|--------------------------|
| `docker` (or `colima`, `orbstack`, `lima`) | every acceptance run | Docker Desktop, OrbStack, or `brew install colima` | `apt-get install docker.io` or docker.com docs |
| `pg_dump` + `pg_restore` (v16+) | `@needs-pgclient` backup/restore | `brew install postgresql@16` | `apt-get install -y postgresql-client-16` |

The backup lane probes for `pg_dump`/`pg_restore` at startup and fails with a
clear message if either is missing — silent skips would let backup regressions
ship undetected. Use a client **>= 16**: the test database is `postgres:16-alpine`,
and `pg_dump` refuses to dump a server newer than itself.

### Docker on macOS (Colima / OrbStack / Lima)

The acceptance harness (testcontainers-rs + the `@docker-compose` group) drives
a real Docker daemon. Docker Desktop works out of the box. On Colima / OrbStack
/ Lima, export the daemon socket once so both the `docker` CLI and
testcontainers find it:

```sh
# Colima
export DOCKER_HOST="unix://$HOME/.colima/default/docker.sock"

# OrbStack
export DOCKER_HOST="unix://$HOME/.orbstack/run/docker.sock"
```

`docker context use <name>` sets the value for the `docker` CLI but **does not**
propagate to testcontainers — the `DOCKER_HOST` environment variable does.
(`cargo xtask ci` mirrors your current docker context into `DOCKER_HOST`
automatically.)

### Cleaning up leaked testcontainers

The acceptance harness keeps one Postgres container per `cargo test` invocation
alive via a static so per-scenario schemas can reuse it. Rust doesn't guarantee
statics run `Drop` at process exit, so each run can leak a few
`postgres:16-alpine` containers; they accumulate across runs.

Symptoms: tests start flaking, Postgres testcontainers OOM-kill, or `docker ps -a`
shows many anonymously-named `postgres:16-alpine` containers older than a few
minutes. Quick cleanup (safe if no other project uses `postgres:16-alpine` on the
same daemon):

```sh
docker ps -aq --filter "ancestor=postgres:16-alpine" | xargs -r docker rm -f
```

## The operator CLI (`foundry doctor`)

The `foundry` binary doubles as an operator CLI under `foundry doctor`:

| Subcommand | Purpose |
|------------|---------|
| `backup-verify <archive>` | Validate a `pg_dump -Fc` custom-format archive and report row counts. Exit 0 on a healthy backup; non-zero on missing args, an unreadable/truncated archive, or a restore-probe failure. |
| `list-workspaces` | List workspaces with the identity/selector used by `export-workspace`. |
| `export-workspace <id\|name> <path>` | Export one workspace's tenant tables to a verifiable, isolation-scoped archive (per-workspace logical backup). |
| `verify-export <path>` | Verify an exported archive's completeness and per-tenant isolation from the path alone. |

The per-workspace export/verify surface is documented in
[`docs/evolution/2026-06-16-per-workspace-backup.md`](./docs/evolution/2026-06-16-per-workspace-backup.md);
`backup-verify` covers whole-instance `pg_dump` archives:

```sh
# Boot a throwaway Postgres the verifier can restore into.
docker run --rm -d --name verify-db -p 5544:5432 \
    -e POSTGRES_PASSWORD=postgres postgres:16-alpine
export FOUNDRY_DOCTOR_PROBE_URL=postgres://postgres:postgres@127.0.0.1:5544/postgres

foundry doctor backup-verify /backups/foundry-2026-05-22.dump
# ... row-counts per table ...
# status: OK
```

Pipe the output into `grep -q 'status: OK'` from cron to fail loudly on corruption.

### Manual onboarding drills (release-candidate cuts)

Two human-run drills measure the contributor experience the onboarding promise
relies on (≤10-min time-to-green-tests, one-line-edit visible within 30s). Scripts
live at
[`docs/feature/foundry-contributor-onboarding/distill/manual-drills.md`](./docs/feature/foundry-contributor-onboarding/distill/manual-drills.md).
Run them at release-candidate cuts and whenever you want a fresh onboarding-survey
baseline.

## Crate boundaries

The workspace is layered, and the layering is **enforced** by
`cargo xtask check-arch` (part of `cargo xtask ci`) plus `cargo deny`'s `bans`
section — a change that crosses a boundary goes red before it can merge:

- **`foundry-core`** has no I/O dependencies (no `sqlx`, no `axum`, no `tokio`).
- **`foundry-store`** owns persistence (Postgres via `sqlx`).
- **`foundry-services`** is the application/use-case layer over the store.
- **`foundry-api`** (data API tier) and **`foundry-app`** (web + CLI) are
  adapters. An adapter reaches `foundry-store` **only through `foundry-services`**,
  and the data-API tier renders no HTML.
- **`foundry-auth`** and **`foundry-realtime`** are cross-cutting runtime concerns.
- **`foundry-acceptance`** is the cucumber harness; it depends on `foundry-app`
  through its `test-support` feature.

If you're unsure whether a dependency edge is allowed, run `cargo xtask check-arch`
— it names any offending edge.

## How a slice is built (nWave)

Foundry follows the nWave methodology; feature wave artefacts live under
`docs/feature/<feature>/` (e.g. the MVP under `docs/feature/foundry-backend-mvp/`):

- `distill/features/` — Gherkin scenarios (the contract).
- `distill/driver.md` + `step-skeletons.md` — harness shape and step signatures.
- `design/` — architecture, data access, auth, observability.

A typical contribution:

1. Pick one scenario in a feature's `distill/features/`.
2. Remove its `@skip` if quarantined, run the acceptance test, watch it fail for
   the *right reason*.
3. Write the smallest production code (in one of the library crates or
   `foundry-app`) that turns the test green.
4. Refactor under green, lint clean, `cargo xtask ci`, commit.

Land **one slice at a time** — a PR that turns five scenarios green in one swing
is harder to review than five PRs that each turn one green.

## Continuous Integration

`.github/workflows/ci.yml` runs a **single job** on every push to `main` and
every PR: `cargo xtask ci` — the same gate you run locally. Nothing runs in CI
that you can't run locally, which is the whole point (see [AGENTS.md](./AGENTS.md)).
A Forgejo mirror runs a near-identical pipeline in `.forgejo/workflows/ci.yml`.

To add a check to CI, add it to `xtask::run_ci` (so it also runs locally) — never
to the workflow alone.

### Releases

See [`RELEASING.md`](./RELEASING.md). Cutting a release is
`git tag vX.Y.Z && git push origin vX.Y.Z`; the release workflow
(`.github/workflows/release.yml`) handles multi-arch image builds, cosign keyless
signing, and SBOM generation.

### Dependabot

`.github/dependabot.yml` opens daily Cargo PRs and weekly Actions / Docker PRs.
Minor + patch bumps are grouped into one PR per ecosystem per week; major bumps
land individually for review.

To enable auto-merge for patch-level dependabot PRs when CI is green:

1. Repo Settings → General → "Allow auto-merge".
2. Repo Settings → Branches → protect `main`, require the **`cargo xtask ci`**
   check as a required status check.
3. Add a workflow that runs `gh pr merge --auto --squash` on dependabot PRs
   labeled `patch` (a one-line script; not currently bundled).

Without those pieces, dependabot PRs wait for a human merge button — the correct
default for an early project.
