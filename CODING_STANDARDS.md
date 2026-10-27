# Coding Standards

Read before writing or changing code or tests in this repo.

## Design

Foundry is object-oriented: model behaviour as structs with `impl` blocks, and
inject every effect through a trait object (`Arc<dyn Clock>`, `Notifier`,
`Store`) so tests can substitute it. Wire the concrete implementations in one
place, the composition root in `foundry-app/src/main.rs`.

Crate layering is enforced by `cargo xtask check-arch`; run it to see whether
a dependency edge is allowed. The layers and their reasons are in
[`DEVELOPER.md`, "Crate boundaries"](./DEVELOPER.md#crate-boundaries).

## Dead code

This project has not reached a stable release, so it carries no
backward-compatibility obligation. When a route, function, guard, or path is
superseded, delete it in the same change; git history recovers anything
removed. Revisit this policy once a stable version ships, when
deprecate-then-remove may fit better.

## Tests

- Write example-based tests by default. Add property-based tests where an
  invariant is genuinely quantifiable, as in `board-lane-reorder`'s
  position-permutation suite.
- Mutation-test per feature: after refactoring in each delivery, run mutation
  testing scoped to the files that delivery modified. The kill rate must reach
  80% or more.
