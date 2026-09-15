# CONTEXT

## Current Task

`card-drag-drop-feedback` is delivered, closed, and **on `main`**. `origin/main` was
fast-forwarded `aa8a6f6` → `0beb1d5` on 2026-09-15 at the user's instruction ("No PR,
just commit to main"): no merge commit, 4 commits — `3ee56fa` feature · `90ed631` rustls
advisory · `868090f` kb test-flake fix · `0beb1d5` closing docs; then `181a65a` reconciled
the records. The merged `card-drag-drop-feedback` branch is deleted, locally and on origin.
Final gate, attempt #4 on `868090f` (`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`):
**GREEN** — 825/825 scenarios, 5757 steps, 2026-09-15 13:46–14:00Z. `0beb1d5` is
docs-only, so what is on `main` is byte-identical in code to the gated commit.
`3ee56fa`'s "NOT YET GATED" paragraph is superseded; full record in
`deliver/closing-notes.md`.

## Key Decisions

- **Supersede, never rewrite.** No commit is amended and no historical record edited: `3ee56fa`'s message and the evolution doc's finalize-time "Not pushed" both stand, corrected by dated addenda. Only live status fields (this file, `kpi-contracts.yaml`, the gate table) were updated in place.
- Counts: 35 scenario declarations = 54 examples (`cdf` 54/54). M1–M9 were 9/9 killed both before and after the refactor.
- Gate reds #1–#3 were not the branch: a `grant_super_admin` PoolTimedOut; the kb focus race, fixed in `868090f`; and a Postgres `SSLRequest 0x48` on a host at load 95 (`pi-companion`).

## Next Steps

- Owed to the user: a real-mouse in-lane reorder, a real Finder drop, and Firefox + Safari checks. The user deletes temp issues GEN-3 and GEN-4.
- `.nwave/des/des-task-active*` (untracked) are left for the user.
- Follow-ups: a new issue jumps to the top of its lane on reload; the stale `foundry.52ad52fa.css` comment (`feature_issue_card_delete.rs:1571`); `.lane-drop-indicator` contrast; the dead `cdf_marker_before`; pin the Chrome image; the testcontainers connect flakes.
