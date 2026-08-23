# ADR-modal-close-001: Dialog close affordances are declarative triggers of one close mechanism

## Status

Accepted (2026-08-22, issue-edit-modal-close-icon DESIGN wave)

## Context

Foundry's dialogs are `div.modal` fragments htmx-swaps into `#modal-root`; the
closed state is DOM-derived — "the host is empty" (`keyboard.js` ADR-003 §2).
Today the only close mechanism is `keyboard.js::closeModal()`, reached solely by
Esc through `closeTopLayer()`, which is Esc's single owner (BR-4: a second
`Escape` listener would race it and peel two layers per press — the shipped
`@layered` scenario reds on exactly that).

The issue-edit dialog gains a pointer close (×). Every future dialog affordance
(the new-issue dialog is the recorded fast-follow) faces the same question: how
does a per-template control reach the close mechanism without forking it or
adding listeners? Constraints: CSP-safe (no inline handlers — keyboard-shortcut
ADR-001), wiring must survive htmx swaps (delegation house idiom), no new
dependencies, no server round-trip for a client-state change.

## Decision

Close affordances are **attributes, not code**. Any element inside `#modal-root`
carrying `data-action="close-modal"` is a close trigger. One document-delegated
`click` listener in `keyboard.js` resolves the attribute (via `closest()`) and
calls the existing `closeModal()`. One mechanism, N declarative triggers; Esc
remains one of them via `closeTopLayer()`, untouched.

`keyboard.js`'s charter widens from "keyboard dispatch" to "interaction-layer
owner" — accepted, because it already owns the layer stack (help/modal/search)
and the close mechanism already lived there; the alternative is forking that
mechanism to preserve a filename.

## Alternatives Considered

1. **New static file (`modal-close.js`) with its own `#modal-root`-emptying
   listener** — rejected: forks the close mechanism into a second
   implementation. The slice hypothesis names this the duplication tax; any
   later change to close semantics (e.g. focus restore) would have to land
   twice or the two triggers drift — the D-04 failure by another route.
2. **`hx-on:click` / inline handler in the template** — rejected: inline
   handlers violate the CSP-safe posture (external same-origin scripts only,
   all wiring via `addEventListener` — keyboard-shortcut ADR-001), and each
   template would restate the close logic.
3. **Server round-trip (`hx-get` returning an empty fragment into
   `#modal-root`)** — rejected: a network request and a route addition to
   discard client-only state; fails offline mid-session; scope pins "no new
   route".
4. **Synthetic `Escape` dispatch from a click handler** — rejected: launders a
   click through the keyboard path, entangles it with the guard chain
   (`isInert`), and makes the × close the TOP layer rather than the modal —
   semantics the button does not have.

## Consequences

- Positive: one close mechanism with N triggers, held structurally — a new
  dialog gets a close control by adding an attribute to its template, with zero
  JS. BR-4 cannot be violated by a new affordance, because affordances no
  longer register listeners at all.
- Positive: Enter/Space activation and shortcut-guard compatibility fall out of
  using a native `<button type="button">` (it is in `NON_TEXT_INPUT_TYPES`).
- Negative: `keyboard.js` is now misnamed by one increment; renaming it is a
  cache-hash and template touch deliberately not spent here.
- Negative: the trigger contract (`data-action="close-modal"`) is held by the
  acceptance scenarios, not by construction — the same standing as BR-1's
  bound-equals-advertised invariant, and the same remedy if it ever drifts.
