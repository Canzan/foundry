# ADR-002: The guard chain — the exact predicate that decides typing vs commanding (ODD-4)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-4** (blocking, slice 02).
**The crux ADR** — Risk R2, NFR-2, KPI-2.

## Context
Every one of the seven advertised shortcuts is a **plain printable character or a bare key** (`c`, `/`,
`j`, `k`, `Enter`, `?`, `Esc`). Those are exactly the characters people type. Bind them naively — the
default outcome of the obvious implementation (`addEventListener('keydown')`, `switch (e.key)`) — and Mei
cannot type the letter `c` into an issue title. **That is strictly worse than shipping nothing**, and it is
why slice 02 is deliberately sequenced ahead of higher-scoring slice 03 (D11).

BR-2 requires the guards evaluated **before** any dispatch, for **all seven**, with **no exemptions**.
BR-7 requires `Shift` **not** to suppress — `?` *is* `Shift+/`. Mei Tanaka uses a Japanese IME, so
composition is concrete, not theoretical. The real text-entry surfaces the predicate must cover are shipped
and enumerable: `new_issue_modal.html:4` (`input[name=title][autofocus]`), `issue_edit_modal.html:4`,
`comment_edit_form.html:1`, and the search box this feature injects (ADR-005).

## Decision
**One chain, in one function, evaluated in this order, before the dispatch table is reachable at all.**
Falling off the end is the *only* path to dispatch — dispatch is not reachable around the chain.

| # | Guard | Verdict |
|---|-------|---------|
| 1 | `event.isComposing === true \|\| event.keyCode === 229` | INERT — IME composition |
| 2 | `event.ctrlKey \|\| event.metaKey \|\| event.altKey` | INERT — `Cmd+C` copies. **`shiftKey` deliberately absent** (BR-7) |
| 3 | `event.defaultPrevented` | INERT — another handler already owns this key |
| 4 | `isTextEntry(event.target)` | INERT — the character is typed |

```
isTextEntry(el):
  el is an element?                                   else false
  TEXTAREA | SELECT                                 → true
  INPUT and type ∉ {button submit reset checkbox
                   radio file image range color
                   hidden}                          → true    // absent/unknown type ⇒ "text"
  el.isContentEditable === true                     → true    // covers INHERITED contenteditable
  role ∈ {textbox, searchbox, combobox, spinbutton}  → true
  otherwise                                          → false
```

Three details carry the design:

- **`isContentEditable`, not `getAttribute("contenteditable")`.** The DOM property is `true` for
  **descendants** of an editable region, so typing inside a nested `<b>` in a rich-text field is guarded
  without an ancestor walk. The attribute is only present on the region root and would miss the child.
- **`keyCode === 229` beside `isComposing`.** 229 is the legacy composition sentinel and remains the
  reliable signal on some IME/browser pairs where `isComposing` is unset on the composition-terminating
  event. It specifically stops an IME-commit `Enter` from being read as "open selected" (FR-9) — the exact
  way this breaks for Mei.
- **`INPUT` is an allow-list of *non*-text types, not a deny-list of text types.** `type=text`, `search`,
  `email`, `password`, `url`, `tel`, `number`, `date`, and **any future/unknown type** are all guarded by
  default. A deny-list fails open (a new input type silently becomes unguarded); this fails **closed**,
  which is the correct direction when the cost of a false negative is an unusable product.

The guard is **contextual, not a toggle**: it reads the live event target, so leaving a field re-enables the
shortcuts on the very next keypress with no state to reset (AC-02.6).

## Alternatives Considered
- **Per-shortcut checks (`if (e.key === 'c' && !typing()) …` × 7)** — REJECTED, and this is the failure
  mode the DISCUSS explicitly names. Seven places to forget; the eighth shortcut forgets by default. BR-2
  is a *structural* rule precisely so correctness does not depend on remembering. The chain-before-dispatch
  shape makes the guard unreachable-around.
- **A `focusin`/`focusout` flag (`let typing = false`)** — REJECTED. It is a **global toggle**, which is the
  AC-02.6 failure mode (K-E4: shortcuts stay dead after leaving the field). It desyncs whenever focus moves
  in ways the listeners miss — an htmx swap removing the focused node, programmatic `.focus()`, or the
  autofocused title arriving inside a fragment. Reading the live target cannot desync.
- **`document.activeElement` instead of `event.target`** — REJECTED as the primary. For `keydown` they
  agree, but `event.target` is the authoritative dispatch target and is correct under retargeting;
  `activeElement` is a second source of truth that can only disagree. (Rejecting it is also what keeps the
  predicate a pure function of the event — trivially unit-testable.)
- **Including `shiftKey` in the modifier guard** — REJECTED, and it would be a *bug*: `?` is `Shift+/` on a
  US layout (BR-7). Called out because "suppress all modifiers" is the tempting simplification and it
  silently kills one of the seven.
- **Excluding `Alt` from the modifier guard (to preserve AltGr layouts)** — REJECTED. AltGr surfaces as
  `Ctrl+Alt` on Windows/Linux and produces *characters*, which land in text fields and are caught by guard
  4 anyway. Suppressing `Alt` costs nothing real and protects `Alt`-chord browser/OS shortcuts (NFR-3).
- **`contenteditable` via `closest('[contenteditable]')`** — REJECTED as redundant and subtly wrong: it
  matches `contenteditable="false"` too (an explicitly *non*-editable island inside an editable region).
  `isContentEditable` already resolves inheritance correctly, including the `false` override.

## Consequences
- **Positive**: **`Enter`-in-a-form submits, and `/`-typed-into-the-search-box inserts a slash, as
  consequences of BR-2 rather than special cases.** US-06's "Enter in a form still submits" and US-04's
  "typing `and/or` works" need *no code at all* — they fall out of guard 4. That is the strongest available
  evidence the guard is structural rather than incidental.
- **Positive**: the predicate is a pure function of `(event)` → `bool`, so it is unit-testable without a
  browser, and the `@property` litmus can enumerate every shortcut × every shipped text surface.
- **Negative / accepted**: `role="textbox"` etc. are honoured but Foundry renders none today — a small
  amount of defence for surfaces that do not yet exist. Accepted: it is three array entries, it fails
  closed, and a future ARIA text widget silently escaping the guard is exactly the R2 regression.
- **Negative / accepted**: guard 4 fires on `SELECT`, so `j`/`k` will not move the board selection while a
  `<select>` is focused. Correct — `j`/`k` are type-ahead keys in a native select.
- **Probe (Earned Trust) — and the honest limit.** The composition clause is the one guard whose real-world
  substrate we **cannot** faithfully drive in CI: W3C WebDriver `send_keys` does not produce IME
  composition. The lane must exercise it with JS-dispatched `CompositionEvent` + a `KeyboardEvent` carrying
  `isComposing: true` via `client.execute()` (ADR-007). That truthfully exercises **our predicate** — the
  listener fires for untrusted events — but it is **not** a real IME, and a real-IME regression could still
  reach Mei. Named here rather than papered over. The mitigation is that guard 1 is two boolean reads with
  no branching subtlety; the risk is in the browser, not in us. **If DELIVER cannot make guard 1 hold across
  real IME input, the honest response is the one DISCUSS already ratified: unbind the character keys and
  shrink `SHORTCUTS` so the help page stays truthful (BR-1) — not ship carve-outs.**
