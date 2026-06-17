# Contributing to Foundry

**Foundry** is a self-hostable issue tracker for small product teams who want
Linear-style ergonomics without handing their data to a SaaS — one Postgres, one
binary, one `docker compose up`, AGPL-3.0. The [README](./README.md) is the
two-minute tour of what it is and how to run it; start there if you're new.

Thanks for pitching in. There's room for every kind of contribution, and **not
all of them need the Rust toolchain** — pick the lane that fits what you want to
do.

## Ways to contribute

- **Docs & wording** — fix a typo, clarify a step, improve an example. Edit the
  Markdown and open a PR. You do **not** need Rust, Docker, or Postgres for this.
- **Bug reports & ideas** — open an issue describing what you saw, what you
  expected, and how to reproduce it. Small, specific reports are the most useful.
- **Code** — Foundry is built outside-in, one thin slice at a time. The
  [Developer Guide](./DEVELOPER.md) has everything you need: local setup, the
  gate to pass before pushing, crate boundaries, and CI.

## Before you push code

Run the local gate and make sure it's green:

```sh
cargo xtask ci
```

This runs the **exact** checks CI runs — formatting, lints, the architecture
boundary guard, build, tests, license/advisory checks, and the full acceptance
suite. **Do not push red.** Because CI runs this same command, green locally
means green in CI. See [AGENTS.md](./AGENTS.md) for the policy and the
[Developer Guide](./DEVELOPER.md) for one-time setup (Docker, a Postgres 16
client, `cargo-deny`). Docs-only changes don't need this.

## How a code change is shaped

Foundry grows one end-to-end slice at a time:

1. Pick one Gherkin scenario under a feature's `docs/feature/<feature>/distill/features/`.
2. Run its acceptance test and watch it fail for the *right reason*.
3. Write the smallest production code that turns it green.
4. Refactor under green, run `cargo xtask ci`, commit.

Please land **one slice per PR** — five focused PRs are easier to review than one
that turns five scenarios green at once. The [Developer Guide](./DEVELOPER.md#how-a-slice-is-built-nwave)
covers the methodology in more depth.

## Where to look next

- [README](./README.md) — what Foundry is, and the quickstart.
- [Developer Guide](./DEVELOPER.md) — local setup, the `cargo xtask ci` gate,
  the operator CLI, crate boundaries, CI, and releases.
- [RELEASING.md](./RELEASING.md) — cutting a release.

Be kind, assume good faith, and when in doubt open an issue to discuss before a
large change. Welcome aboard.
