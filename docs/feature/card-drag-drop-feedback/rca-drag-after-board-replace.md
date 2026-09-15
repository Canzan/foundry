# RCA — card drops refused after the board is replaced in place

Input to the `card-drag-drop-feedback` DISCUSS wave (2026-09-13). Investigated by
nw-troubleshooter; reproduced in the containerised `selenium/standalone-chrome` the
acceptance lane uses. No source changed.

## Symptom

On `/team/general/project/sandbox` (Chrome, fresh server) the card lifts but every lane
refuses the drop. A full page reload (the user restarted the server) makes it work again.

| Case | HEAD `board-dnd.js` | Delegated fix |
|---|---|---|
| A: fresh load, card to another lane | drop accepted | same |
| B: after `#board-columns` replaced, card to another lane | drop refused, no POST | accepted |
| C: after replace, card to an empty lane | drop refused, no POST | accepted |

## Root cause chain

1. No drop lands: `dragover` is never `preventDefault()`-ed on the lanes on screen.
2. `board-dnd.js:86-146` binds `dragover`/`drop` per lane **once at load**; only `dragstart`
   (line 67) is on `document`.
3. `partials/oob/board_columns_oob.html:6` replaces `#board-columns` (`hx-swap-oob="true"`),
   returned by `issues.rs` `submit_delete` (`87282d3`, card delete from the popup) and by
   `lanes.rs` delete/edit/move/insert; client-side by `board-lane-dnd.js:144-152` `applyBoard`.
4. `board-dnd.js` predates board replacement (unchanged since `b8e3ae6`); later features
   documented "re-query the live document" (`board-live.js:28-33`, ADR-BOARD-LANE-005) but
   never audited this script. `87282d3` made the trigger an everyday action.
5. Nothing enforces or tests "every board script survives a board replace".

Ruled out: stale cached JS (served file byte-identical to HEAD), card markup change,
lane-drag stealing card events, server rejecting the lane, fresh-load failure.

## Test gap

- Browser card-drag scenarios (`board-lane-reorder.feature:229-235`,
  `keyboard_shortcut_bindings.rs:3013`) always navigate first via `open_board_in_browser`,
  so listeners are fresh. `drag_a_card` (`feature_board_lane_reorder.rs:1137`) reloads.
- `issue-status-move` / `card-ranking-within-status` are HTTP-only.
- `@needs-browser` is excluded from the default lane (`tests/acceptance.rs`).
- `issue-card-delete.feature:304` performs the replacing delete but never drags afterwards.

## Fix direction

Delegate `dragover`/`drop` to `document`, resolve the lane with
`event.target.closest("[data-column]")` at event time, guard on `dragged !== null` (so
external file/text drags are not accepted), reset state on `dragend`. POST contract
unchanged. Regression scenario: replace the board (lane menu Move, and a popup delete),
then drag without reloading; assert the synthetic `dragover` was `defaultPrevented`.

## Findings for the feature asks

1. **Lane activation colour** — nothing exists (no class, no `dragenter`/`dragleave`). Must
   be delegated too. CSS may use colour tokens only (check-arch S1); a CSS edit forces the
   content-hash rename across `base.html`, `lib.rs` tests and `VENDOR.md` (as `1d91ad8`).
2. **Drop before/after any card** — already supported end to end: `insertBeforeTarget`
   (`board-dnd.js:36-48`) → `after=<key>` → `ChangeStateForm.after` (`issues.rs:177-188`) →
   `reposition_issue_with_outbox` (`foundry-store/src/lib.rs`, gap-free `0..N-1`,
   `issues.position`, migration 0012). Missing: a visible insertion marker during the drag
   (could mirror lane reorder's `.lane-drop-indicator`).
3. **Drop into an empty lane** — valid target on a fresh load (`board_columns.html:39`
   `<p class="empty">`). Gaps: placeholder remains after a drop; a lane emptied by dragging
   out gets no placeholder until reload.

Repro harness: session scratchpad `dnd/` (`repro-head.html`, `repro-fix.html`,
`board-dnd.fix.js`) — not committed.
