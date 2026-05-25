# Contributing to Foundry

Foundry is built outside-in. The contributor experience matches that
shape: open a Gherkin scenario, watch the acceptance test go red,
then make it green crate by crate.

## Inner loop

```sh
cargo build --all
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Cargo gates that MUST be green on every PR:

| Gate                | Command                                          |
|---------------------|--------------------------------------------------|
| Build               | `cargo build --all`                              |
| Unit + integration  | `cargo test --workspace`                         |
| Fast acceptance     | `cargo test -p foundry-acceptance`               |
| Lints               | `cargo clippy --all-targets -- -D warnings`      |
| Format              | `cargo fmt --all -- --check`                     |
| Licenses + advisory | `cargo deny check`                               |

The `@docker-compose` and `@manual` cucumber tag groups are slow and
human-driven respectively; they are excluded from the default run
and exercised via tag filter or in the post-merge job.

To run the `@docker-compose` group locally:

```sh
FOUNDRY_ACCEPTANCE_TAGS=docker-compose cargo test -p foundry-acceptance
```

### System prereqs for the acceptance suite

Some slice-3 acceptance lanes drive system tooling other than the Rust
toolchain. Install these once:

| Tool | Lane | Install (macOS) | Install (Debian/Ubuntu) |
|------|------|------------------|--------------------------|
| `docker` (or `colima`, `orbstack`, `lima`) | every acceptance run | Docker Desktop, OrbStack, or `brew install colima` | `apt-get install docker.io` or follow docker.com docs |
| `pg_dump` + `pg_restore` | `@us-03 @backup-restore` | `brew install libpq && brew link --force libpq` | `apt-get install postgresql-client-16` |
| `psql` | `foundry doctor backup-verify` row counts | same as above (ships with libpq) | same as above |

The US-03 lane probes for `pg_dump` and `pg_restore` at suite startup
and `panic!`s with a clear message if either is missing — silent skips
would let backup regressions ship undetected (F-004 anti-flake).

Use the **same major version of the Postgres client tooling as the
running `foundry-db` container** when planning a restore drill. The
test harness uses `postgres:11-alpine` (testcontainers default); the
production runbook targets PG 16. `pg_dump` from a newer client
against an older server works (pg_dump 14 happily dumps PG 11); the
reverse does not.

### `foundry doctor backup-verify` (operator CLI)

The `foundry` binary doubles as an operator CLI. Today the only
subcommand is `doctor backup-verify`, which validates a `pg_dump -Fc`
custom-format archive and reports row counts:

```sh
# Boot a throwaway Postgres the verifier can restore into.
docker run --rm -d --name verify-db -p 5544:5432 \
    -e POSTGRES_PASSWORD=postgres postgres:11-alpine
export FOUNDRY_DOCTOR_PROBE_URL=postgres://postgres:postgres@127.0.0.1:5544/postgres

# Run the verification.
foundry doctor backup-verify /backups/foundry-2026-05-22.dump
# backup-file: /backups/foundry-2026-05-22.dump
# backup-format: pg_dump custom
# backup-size-bytes: 5421366784
# schema: public
# row-counts:
#   workspaces: 1
#   users: 12
#   teams: 3
#   projects: 8
#   issues: 4823
#   comments: 19311
#   issue_attachments: 1142
# status: OK
```

Exit code 0 on a healthy backup; non-zero (`2`–`7`) on missing args,
unreadable / truncated archive, or restore-probe failure. Pipe the
output into `grep -q 'status: OK'` from cron to fail loudly on
corruption.

### Cleaning up leaked testcontainers

The acceptance harness uses testcontainers-rs's shared-container pattern
(one Postgres container per `cargo test` invocation, kept alive via a
`OnceCell` static so per-scenario schemas can reuse it). Rust does not
guarantee that statics run their `Drop` impl at process exit, so each
`cargo test` invocation leaks ~3-4 Postgres containers on the docker
daemon. They accumulate across runs.

Symptoms: tests start flaking, Postgres testcontainers OOM-kill, or
`docker ps -a` shows many anonymously-named `postgres:11-alpine`
containers older than a few minutes.

Quick cleanup (safe IF you don't have another project using
`postgres:11-alpine` on the same daemon):

```sh
docker ps -aq --filter "ancestor=postgres:11-alpine" | xargs -r docker rm -f
```

A `cargo xtask docker-prune-leaked` subcommand is a planned future
polish — until then, the one-liner above is the supported cleanup.

### Docker on macOS (Colima / OrbStack / Lima)

The acceptance harness (testcontainers-rs + the `@docker-compose`
group) drives a real Docker daemon. Docker Desktop works out of the
box. On Colima / OrbStack / Lima, export the daemon socket once so
both `docker` and testcontainers find it:

```sh
# Colima
export DOCKER_HOST="unix://$HOME/.colima/default/docker.sock"

# OrbStack
export DOCKER_HOST="unix://$HOME/.orbstack/run/docker.sock"
```

`docker context use <name>` sets the right value for the `docker` CLI
but **does not** propagate to testcontainers — the `DOCKER_HOST`
environment variable does.

## How a slice is built (nWave)

This project follows the nWave methodology. The wave artefacts for
this MVP live under `docs/feature/foundry-backend-mvp/`:

- `distill/features/` — Gherkin scenarios (the contract).
- `distill/driver.md` + `step-skeletons.md` — harness shape and
  step signatures (the test scaffold).
- `design/` — architecture, data access, auth, observability.

A typical contribution:

1. Pick one scenario in `distill/features/`.
2. Remove its `@skip` (if it is currently quarantined), run the
   acceptance test, watch it fail for the *right reason*.
3. Write the smallest production code in one of the four library
   crates (or `foundry-app`) that turns the test green.
4. Refactor under green, lint clean, commit.

PRs are expected to land *one slice at a time*. A PR that turns
five scenarios green in one swing is harder to review than five
PRs that turn one scenario green each.

## Crate boundaries (do not cross)

- `foundry-core` has no I/O dependencies (no sqlx, no axum, no tokio).
- `foundry-auth`, `foundry-store`, `foundry-realtime` may depend on
  `foundry-core` and the runtime crates they wrap; **they do not
  depend on each other**.
- `foundry-app` is the only crate that depends on all of the above.
- Acceptance tests live in `foundry-acceptance` and depend on
  `foundry-app` through the `test-support` feature.

`cargo deny`'s `bans` section enforces this in CI.

## Continuous Integration

Every push to `main` and every PR runs four parallel jobs in
`.github/workflows/ci.yml`:

| Job              | What it runs                                                    |
|------------------|-----------------------------------------------------------------|
| `lint + format`  | `cargo fmt --all -- --check` + `cargo clippy ... -D warnings`   |
| `build + test`   | `cargo build --all --release` + `cargo test --workspace ...` against a Postgres service |
| `acceptance`     | `cargo test -p foundry-acceptance` with `FOUNDRY_ACCEPTANCE_TAGS=all` (default + `@docker-compose`) |
| `cargo deny`     | License + advisory + bans + sources                             |

A typical PR completes in well under 15 minutes thanks to
`Swatinem/rust-cache` keyed on `Cargo.lock`. Re-runs without code
changes are usually under 5 minutes.

Workflow logs live under the repo's **Actions** tab. Forgejo mirrors
get a near-identical pipeline in `.forgejo/workflows/ci.yml` —
documented compatibility deltas (no OIDC by default, slightly
different runner labels) are inline in that file.

### Local CI replication

`cargo xtask ci` runs the same gates as remote CI in the same order
and exits non-zero on the first failure:

```sh
cargo xtask ci
# include the slow @docker-compose acceptance group:
FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci
```

You'll need `cargo-deny` installed once: `cargo install --locked cargo-deny`.

### Releases

See [`RELEASING.md`](./RELEASING.md). Cutting a release is a
`git tag vX.Y.Z && git push origin vX.Y.Z`; the release workflow
(`.github/workflows/release.yml`) handles multi-arch image builds,
cosign keyless signing, and SBOM generation.

### Dependabot

`.github/dependabot.yml` opens daily Cargo PRs and weekly Actions /
Docker PRs. Minor + patch bumps are grouped into one PR per ecosystem
per week; major bumps land individually for individual review.

To enable auto-merge for patch-level dependabot PRs when CI is green:

1. Repo Settings -> General -> "Allow auto-merge".
2. Repo Settings -> Branches -> protect `main`, require the four CI
   jobs above as required status checks.
3. Add a workflow that runs `gh pr merge --auto --squash` on
   dependabot PRs labeled `patch` (one-line script; not currently
   bundled).

Without those three pieces, dependabot PRs sit waiting for a human
merge button — which is the correct default for an early project.
