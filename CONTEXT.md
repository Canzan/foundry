# CONTEXT

## Current Task

`card-drag-drop-feedback` DELIVER is **COMPLETE — 8/8 steps** — and now committed on
branch `card-drag-drop-feedback` (off `main` at `aa8a6f6`). All four slices landed:
drops surviving an in-place board replace, lane activation, the insertion marker, and
the empty-lane placeholder shown by CSS `:has()` with no script writer. Per-slice
mutation gates: **9/9 faults killed** (3/3, 1/1, 3/3, 2/2) against the ≥80% bar.
Stylesheet is `foundry.f7c36a08.css` (filename hash verified against content).

**NOT PUSHED — the gate is not green.** `cargo xtask ci` passes fmt, clippy,
check-arch and `build --release`, then fails 5 tests in
`foundry-services/tests/delete_lane_use_case.rs` with
`start postgres container: WaitContainer(StartupTimeout)`. **Environmental, not the
feature**: the tree touches no `foundry-services` file, and these passed at `921a409`.
A plain `postgres:16-alpine` now takes 13s to be ready (normally 1–3s).

## Key Decisions

- **Fault survivors are test gaps fixed in the slice, never argued away** — M6 (own-slot marker) and M7 (remote-delete placeholder) were both closed by strengthening the scenario; no assertion was weakened.
- **Placeholder is shown by CSS `:has()`** (DDD-5) — rendered in every lane, zero script writers. Slice 04-02 proved the hypothesis: all 8 US-CDF-04 scenarios green with **no production edit**.
- **The whole wave ran in no-commit mode** (user's choice) — all 8 COMMIT phases logged `APPROVED_SKIP`, then landed as one commit.

## Next Steps

- **Re-run `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` after the Docker cleanup, then push.** Docker is carrying 11 Postgres containers, 79.7 GB of images (99% reclaimable) and 20.9 GB of build cache.
- **Owed to the user:** real-mouse reorder within a lane, a real Finder file drop, Firefox + Safari checks; delete temp issues GEN-3/GEN-4 in Sandbox.
- **Follow-ups (pre-existing):** a new issue shows at the bottom of its lane but jumps to the top on reload (`position DEFAULT 0` + `ORDER BY position ASC, number DESC`); and `feature_issue_card_delete.rs:1571` still names the long-superseded `foundry.52ad52fa.css` in a comment.
