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
