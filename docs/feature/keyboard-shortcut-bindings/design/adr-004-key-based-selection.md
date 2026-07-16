# ADR-004: Selection is a key, not an index or a node (ODD-5)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-5** (selection half; the
`Esc`-vs-swap half is ADR-003) and **Risk R4**.

## Context
The locked model (D-4) says `j`/`k` walk **visible cards in DOM order**, the selected card takes a **ring**
and is **scrolled into view**, and selection **resets on navigation** and is never persisted (BR-5). Two
shipped forces move the DOM underneath that selection:

- **htmx** replaces cards and `#modal-root` content (`board.html:6`, `issue_card.html:1`,
  `hx-swap="innerHTML"`). A card that is re-rendered is a **new node**.
- **`board-dnd.js`** reorders the DOM on drop — `insertBefore`/`appendChild` within and across columns
  (`board-dnd.js:97-146`), and reverts to the exact origin slot on a non-2xx or network error
  (`:135-144`). It moves the **same node** to a **different index**.

ODD-5 asks: survive by issue-key, by index, or reset? The choice is not cosmetic — it decides whether the
ring the user sees is the issue `Enter` opens (AC-06.1), which is the difference between a working feature
and one that acts on the wrong issue.

## Decision
**`selectedKey: string | null`** — the issue key (`AUTH-2`), read from the `data-issue-key` attribute that
**every** selectable element already carries (`issue_card.html:1`, `search_results.html:4`). It is the
single source of truth for selection. Nothing else is stored.

- **The ring is derived, never stored.** A projection step applies `aria-selected="true"` + the ring class
  to `[data-issue-key=selectedKey]` **within the active surface** (ADR-005) and clears it elsewhere. It
  re-runs on `htmx:afterSwap` (document-delegated, consistent with NFR-6).
- **After a swap**: the key still resolves → re-ringed, selection preserved. The key is gone (the issue
  left the board) → `selectedKey = null`, selection **clears coherently**. No stale ring, no dangling
  index, no orphan node.
- **Drag needs no code and no change to `board-dnd.js`.** A drag moves the *same node*; its
  `data-issue-key` is unchanged and the ring class rides along on the node. The optimistic move, the
  cross-column move, and the revert-to-origin path are all invisible to selection.
- **`j`/`k`** re-read the active surface's items at each press (`querySelectorAll`), find `selectedKey`'s
  position, and move ±1. Bounded — no wrap past first/last (FR-8); empty surface → no-op. First `j` with no
  selection selects the first item.
- **`scrollIntoView({block: "nearest"})`** on the newly selected element — `nearest` so a selection already
  in view does not jolt the page.

## Alternatives Considered
- **By index (`selectedIndex: number`)** — REJECTED, **disqualifying**. A drag reorders the DOM under a
  fixed index, so the ring silently re-points at a **different issue** while the user is looking away —
  and `Enter` then opens the wrong one (AC-06.1, AC-05.8). An htmx re-render that adds or removes a card
  shifts every index after it. The bug is silent, data-losing (Mei edits the wrong issue), and untestable
  by inspection. This is the alternative that looks simplest and is the most dangerous.
- **By node reference (`selectedEl: Element`)** — REJECTED. It survives a drag (the node moves, the
  reference holds) but **not** an htmx re-render: the node is detached, so the ring renders on an orphan
  the user cannot see, `scrollIntoView` does nothing, and `Enter` fires the detached node's `hx-get` — a
  request for a card that is no longer on the board. It fails exactly where FR-10/NFR-6 say it must not.
- **Reset on every swap** — REJECTED as the primary, though it is the *safe* option and is what we fall
  back to when the key genuinely vanishes. As a blanket rule it breaks the feature's own headline scenario:
  filing an issue via `c` swaps the board, so `j`/`k`/`Enter` would lose the selection every time —
  precisely the `@htmx-swap` property (AC-X.5) that must pass. Reset is the *failure* branch, not the rule.
- **Persist selection (sessionStorage / a server round-trip)** — REJECTED. BR-5 is explicit: client-only,
  ephemeral, never persisted, never sent to the server, resets on navigation. Persisting would also
  resurrect a ring on a card the user never selected in this visit.
- **A `MutationObserver` instead of `htmx:afterSwap`** — REJECTED. It would fire on `board-dnd.js`'s
  optimistic moves too, where nothing needs to happen (the ring already rode along), and it is a broad
  instrument for a narrow, already-announced event. htmx tells us exactly when it swapped; we listen to
  that.

## Consequences
- **Positive — three requirements cost nothing.** (a) **NFR-8 drag coexistence** is free: the ring is a
  class on a node the drag moves. `board-dnd.js` is untouched, so every shipped drag scenario passes
  unchanged. (b) **BR-5 "resets on navigation"** is free: a real navigation reloads the page and the
  variable ceases to exist — it is a property of the representation, not code, and it is why selection
  *cannot* reach the server. (c) **AC-07.3 "`Esc` never clears selection"** is free: `Esc` clears
  containers (ADR-003); `selectedKey` is a detached string it never touches.
- **Positive**: selection identity is a **domain** identity, so it transfers across surfaces for free —
  select `AUTH-2` in the search results, `Esc`, and the ring is on `AUTH-2`'s board card (ADR-005).
- **Positive**: `selectedKey` is a string, so the whole selection model is testable without a DOM, and the
  browser lane asserts the *observable* consequence (`[aria-selected=true][data-issue-key=AUTH-2]`) rather
  than internal state.
- **Negative / accepted**: the projection is O(n) over the active surface on each move and each swap. n is
  the number of cards on one board; this is irrelevant at Foundry's scale and buys a model that cannot
  desync.
- **Negative / accepted**: duplicate `data-issue-key` values within one surface would make the projection
  ambiguous. Not possible today (keys are unique per project and the board renders one project), but it is
  the assumption this design rests on — worth a first-match rule rather than an error.
- **Probe (Earned Trust)**: the two scenarios that would red if anyone switched to an index are pinned
  explicitly — *"Dragging a card leaves selection coherent"* (drag the selected card to another column,
  assert the ring is still on **that key**, not on whatever now occupies the old slot) and the
  `@htmx-swap` property (file via `c`, then `j`/`Enter` still work with no reload). Together they are the
  revert-reds-it guard for this ADR.
