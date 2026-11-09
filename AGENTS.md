# Development Workflow

Trunk-based: commit and push on `main`, open no pull requests, and branch only
when the user asks.

- Commit only after `cargo xtask smoke` prints `xtask smoke :: all gates green`.
- Push only after `cargo xtask ci` prints `xtask ci :: all gates green`.
- Run tests and both gates in the isolated Lima VM through `scripts/test-in-lima.sh`,
  never on Docker on the Mac. `scripts/test-vm-setup.sh` creates the VM once.

## Deployment independence

Foundry must always run on its own, with no dependency on Canzan Dev, Canzan Prod, the
homelab repo, or any other project. Anyone must be able to deploy it with docker compose,
plain Kubernetes, or the single binary.

- Every feature works on a standalone deploy: no setting, secret, service, URL, or DNS name
  that only those environments supply.
- Code, defaults, tests, and docs never name Canzan Dev or Prod hosts, clusters, or repos.
  Environment-specific values come from documented `FOUNDRY_*` settings or the UI.
- Changes to the homelab config only configure Foundry from the outside. They never add
  something Foundry needs that a standalone deploy cannot provide.

Writing or changing code or tests → [`CODING_STANDARDS.md`](./CODING_STANDARDS.md).

Adding a CI check, or a gate fails on a missing prerequisite →
[`DEVELOPER.md`, "The local gate"](./DEVELOPER.md#the-local-gate--cargo-xtask-ci).

Adding or changing a compose acceptance harness or the image it builds →
`crates/foundry-acceptance/src/support/compose_harness.rs` module docs.
