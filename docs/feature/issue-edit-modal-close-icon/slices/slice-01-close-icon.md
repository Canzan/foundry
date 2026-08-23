# Slice 01 — A visible way out of the edit dialog

## Goal
The issue edit dialog carries a conventional × in its top right; one click (or one
Enter on the focused control) closes it without saving.

## Learning hypothesis
**Disproves if it fails**: that a per-template dialog affordance can reuse the
shipped close semantics — emptying `#modal-root` — without disturbing
`keyboard.js`'s single-owner Esc stack (BR-4). If the × can only be built by
registering a second `Escape`-adjacent listener or by forking the close logic,
then dismissal does not have one mechanism with two triggers, and every future
dialog affordance will pay the same duplication tax.
**Confirms if it succeeds**: dialog affordances can be added template-by-template
against one close mechanism, which is exactly what the new-issue dialog fast-follow
would then reuse.

## IN scope
- The × control in `partials/issue_edit_modal.html`'s header: icon-only visually,
  accessible name "Close", ≥24×24 px target, visible focus indicator.
- Pointer activation closes the dialog: `#modal-root` emptied, board interactive,
  no save request issued, typed-but-unsaved edits discarded.
- Keyboard activation: Tab-reachable inside the dialog, Enter and Space both close.
- Close works from the 4xx error state (`data-error-slot` populated).
- Regression: Esc, Save, and "Open full page" byte-identical to before; `j`/`k`/`c`
  still work after a ×-close.
- Wiring that survives htmx swaps (house idiom: document-delegated listener) and
  adds NO second Escape listener.

## OUT of scope
- The new-issue dialog (recorded fast-follow, D-01).
- Backdrop-click dismiss, unsaved-changes confirmation (D-02).
- Focus restoration to the triggering card (D-06 — DESIGN may add it).
- Any change to `closeTopLayer()` or the layer stack.

## Acceptance criteria
AC-1.1 … AC-1.6 (`feature-delta.md`, US-01), exercised by scenarios S1–S4.

## Dependencies
None. No new crates, no migration, no config, no cross-repo work.

## Effort
≤0.5 day. Reference class: the keyboard-shortcuts feature's per-affordance steps —
each shipped a single delegated listener plus scenarios against the existing
harness.

## Taste-test note
Thin — one template edit, a few CSS lines, one small wiring touch. Its value is
user-visible per the US-01 elevator pitch (click the ×, see the board again), so
it is not an `@infrastructure` slice.

## Dogfood moment
Same day: open any card on the live board intending only to read it, and leave it
with the mouse for the first time.
