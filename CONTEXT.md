# CONTEXT

## Current Task

`issue-card-delete` is COMMITTED and green at `1d91ad8` (branch `issue-card-delete`,
**unpushed**). Latest commit is a bugfix: `#modal-root` is declared in `board.html`
alone, so on the full issue page the confirm's × was dead and the Delete link was
inert (`htmx:targetError`) — the page could not delete at all with scripting on. Both
repaired, plus the scripting-ON browser lane that surface never had. CSS re-hashed
`3d3b9564` → `52ad52fa`. Full lane 771/771; no migration, head still 0015.

## Key Decisions

- **The full page navigates, it does not popup** — Delete is a plain link to the confirm page. Adding `#modal-root` to the shell was rejected: DDD-5's htmx success arm refreshes `#board-columns`, which that page has not got, so confirming would strand her on a deleted issue.
- **`close_href` selects the close ELEMENT** — empty keeps the popup's `[data-action="close-modal"]` button; non-empty renders an anchor back to the issue. One partial, one carrier-specific control.
- **Hard delete, not a tombstone** (carried) — one store primitive serves both the single-card path and the lane fate; ADR-BOARD-LANE-002 amended in one clause.

## Next Steps

- **Push the branch** — `1d91ad8` and the two commits before it are local only.
- **App-handler mutation layer unmeasured** (~13 mutants, ~15 min each); `show_delete_form` just gained a branch. Store+services measured 7/7 killed.
- **The four-reviewer DISTILL gate never ran** — the one Phase 4 reviewer returned "0 defects" while misquoting the oracle it was told to attack.
