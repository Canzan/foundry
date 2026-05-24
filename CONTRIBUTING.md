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
