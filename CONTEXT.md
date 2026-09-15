# CONTEXT

## Current Task

`card-drag-drop-feedback` is delivered and closed on branch `card-drag-drop-feedback` (local, **NOT pushed**): `3ee56fa` feature · `90ed631` rustls advisory · `868090f` kb test-flake fix · then the closing-docs commit.
Final gate, attempt #4 on `868090f` (`FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`): **GREEN**. All gates passed; 825/825 scenarios (5757 steps); 2026-09-15, 13:46–14:00Z.
`3ee56fa`'s "NOT YET GATED" paragraph and the earlier "gate is not green" handoff are superseded; the full record is in `deliver/closing-notes.md`.

## Key Decisions

- Counts: 35 scenario declarations = 54 examples (`cdf` 54/54). M1–M9 were 9/9 killed both before and after the refactor.
- Gate reds #1–#3 were not the branch: a `grant_super_admin` PoolTimedOut; the kb focus race, traced and fixed in `868090f`; and a Postgres `SSLRequest 0x48` error on a host at load 95 (`pi-companion`).
- Pushing is the user's gate-then-push call (AGENTS.md). Nothing is pushed.

## Next Steps

- Owed to the user: a real-mouse in-lane reorder, a real Finder drop, and Firefox + Safari checks. The user deletes temp issues GEN-3 and GEN-4.
- Follow-ups: a new issue jumps to the top of its lane on reload; the stale `foundry.52ad52fa.css` comment (`feature_issue_card_delete.rs:1571`); `.lane-drop-indicator` contrast; the dead `cdf_marker_before`; pin the Chrome image; the testcontainers connect flakes.
