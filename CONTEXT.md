# CONTEXT

## Current Task

**`issue-card-delete` is CODE-COMPLETE and UNCOMMITTED** (HEAD still `d3c87ca`). All
waves ran; DELIVER finished 12 steps + phases 3-7. Delete works from the edit popup
and the full page, with or without JS; every authz refusal in the HTML adapter is now
the uniform non-enumerable 404; a second open board drops the card live via
`board-live.js`, foundry's first browser-side live-update surface. **No migration —
head still 0015.** Full lane 632/632.

## Key Decisions

- **Hard delete, not the requested tombstone** — issues already hard-deleted via the lane fate, and D1 of `board-lane-overflow-menu` had declined archive. One primitive now serves both callers; ADR-BOARD-LANE-002 amended in one clause.
- **Refusals converged upward** (user call): 20 `non_member_page` sites → uniform 404, leaking helper deleted. CSRF and author-only 403s untouched.
- **DISCUSS assumed a capability that never existed** — no browser had ever consumed SSE. Built as step 03-02, scoped to `IssueDeleted` only.

## Next Steps

- **Commit needs explicit `git add` of 11 untracked paths** — `git commit -a` misses the whole `@icd` lane and leaves the tree green but hollow.
- **The four-reviewer DISTILL gate never ran**; the one Phase 4 reviewer returned "0 defects" while misquoting the oracle it was told to attack. A real vacuous-assertion defect (D8's redirect target) was found by hand and fixed in Phase 3.
- **App-handler mutation layer unmeasured** (13 mutants, ~15 min each). Store+services measured 7/7 killed.
