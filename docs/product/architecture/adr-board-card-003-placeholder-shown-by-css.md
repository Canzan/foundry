# ADR-BOARD-CARD-003: An empty lane's placeholder is always rendered and shown by CSS, so no script writes placeholders

## Status

Accepted (2026-09-13; card-drag-drop-feedback DESIGN wave, confirmed by the user).
`:has()` was accepted; the `<template>` alternative below is the documented fallback only.
Covers DDD-5.

## Context

The placeholder `<p class="empty">No issues yet — press <kbd>c</kbd> to file the
first one.</p>` is rendered by the shared partial only for a lane with no cards
(`board_columns.html:39`). Nothing in the browser touches it afterwards
(RCA finding 3):

- A drop into an empty lane leaves the placeholder beside the card.
- A lane emptied by a drag shows nothing.
- A lane emptied by a remote delete (`board-live.js`, `IssueDeleted`) shows nothing.

DISCUSS requires the placeholder to be truthful on both sides of every drop,
including a refused drop's revert, and after a remote delete (D10, D16). The words
and markup must have **one source**, the server template, and no second client copy
(AC-4.2).

Keeping it truthful by script means **two independent writers**, the card drag and
the live board. Each must add and remove the placeholder at the right moments and
restore it exactly on revert, all without holding nodes across a board replace
(ADR-BOARD-CARD-001). That is five transitions spread across two modules, each a
place to drift.

## Decision

**The shared partial renders the placeholder in every lane, and CSS displays it
exactly when the lane holds no card.**

1. `board_columns.html` emits the existing `<p class="empty">…</p>` in every
   `section.column`, before the cards. The markup and words are unchanged, and both
   render paths stay byte-identical because they share the one partial.
2. The stylesheet adds one rule pair, tokens-free:
   `.column > .empty { display: none }` and
   `.column:not(:has(> .issue-card)) > .empty { display: block }`.
3. No script writes, clones or restores a placeholder. The optimistic drop, the
   exact-origin revert and `board-live.js`'s `dropCard` become truthful by
   construction. `board-live.js` is unchanged.

The hide-by-default pair is chosen over a single "hide when it has a card" rule for
its degradation. Where `:has()` is unsupported, the placeholder never shows, which
is the pre-feature behaviour for an emptied lane. The alternative would show "No
issues yet" in lanes full of cards, the contradiction this feature exists to remove.
Lying is worse than silence.

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| A `<template>` in the shared partial, cloned by both writers (the DISCUSS-suggested shape) | Viable: single source, no server change. It still needs two writers and five add/remove/restore transitions, and it makes `board-live.js` the second placeholder writer. **This is the fallback if the user declines `:has()`**, and it keeps slice 04 at 0.75d. |
| The words in a data attribute, rebuilt by the client | An attribute carries text, not markup, so the `<kbd>` would be re-authored in JS. That is a second copy of the markup, which AC-4.2 forbids. |
| A server fragment route returning a lane or its placeholder after each change | A new handler and a round trip per drop, which D9 rules out. The client would also still decide when to ask. |
| JS reconciliation (`syncPlaceholder(lane)` after every mutation) | The same two-writer problem as the template, with an idempotent helper. It is better than imperative add/remove, but it is still code that CSS makes unnecessary. |

## Consequences

- Positive: zero writers, so there is nothing to drift, nothing to restore on revert,
  and nothing for a future board mutation (a live card arriving, a bulk move) to
  forget. "Placeholder if and only if no card" becomes a stylesheet fact.
- Positive: the displayed placeholder is literally the server's node, so AC-4.2
  holds without comparison logic.
- Positive: `neighbourAbove` already skips non-card siblings, `slotFor` queries
  `.issue-card` only, and `keyboard.js` selects `.board .issue-card[data-issue-key]`,
  so the always-present node is invisible to all three.
- Negative: this is the first `:has()` in the stylesheet. It is Baseline widely
  available (Chrome/Edge 105, Safari 15.4, Firefox 121), and the degradation is
  silent rather than wrong (see Decision).
- Negative: without CSS the placeholder shows in every lane. Unstyled rendering is
  not a supported profile, and scripting-disabled rendering, which is supported, is
  unaffected because it uses the stylesheet.
- Obligation for DISTILL: "no placeholder" means **not displayed** (computed
  `display`), not absent from the DOM. No story or AC wording changes. A search of
  `crates/` found no shipped test asserting the placeholder's absence. The one
  presence assertion (`projects.rs:1157`, an empty board shows the "press c to file
  the first" guidance) stays green because the placeholder is still rendered, so
  KPI 8 is unaffected.
- The CSS edit carries the D13 hash rename in slice 04.
