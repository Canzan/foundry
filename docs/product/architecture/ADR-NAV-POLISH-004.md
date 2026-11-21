# NAV\-POLISH\-004 The rail carries its own paint level above page content
Status: Accepted

## Context

```text
Holding the rail in view moves it out of the normal flow twice over. A scroll container clips its scrollable overflow on BOTH axes, because a declared overflow-y of auto makes overflow-x compute to auto as well, and position: sticky makes the rail a stacking context even at z-index auto. The second one was missed. The folded flyout carries z-index: 30, written when the rail was unpositioned, and inside the rail own stacking context that value orders the word only among its siblings. The board page content - .column and .issue-card, both position: relative at z-index auto and later in the document - then paints over the whole rail, so the revealed word is covered wherever it overlaps a column, which the shipped folded-rail promise forbids. The comment beside the flyout rule still states the premise that a positioned descendant with a z-index paints above the non-positioned content column; the content column is positioned and that premise no longer holds.
```

## Decision

```text
The rail carries an explicit paint level: .sidebar takes z-index: 20, above the positioned page content that declares no z-index of its own and below every overlay the stylesheet already declares - the modal at 50, the keyboard-help overlay at 100 and the card drag ghost at 200 - so the rail and its revealed word paint over a board column while every overlay still covers the rail. The acceptance reading moves with it: the clip-aware centre hit test runs in the folded desk geometry on a board page carrying cards, with the revealed word overlapping a column, and must answer the word itself rather than an ancestor, a column or a card, because a layout-box reading sees neither a clip nor a cover. The crafter rewrites the comment beside the flyout rule to name the rail own level. Everything else of NAV-POLISH-003 stands unchanged.
```

## Alternatives

```text
Raise the flyout own z-index above the page content (refuted: position: sticky already makes the rail a stacking context, so a descendant z-index orders the word only inside the rail). Give the rail a z-index above the overlays (rejected: a modal, the keyboard-help overlay and the card drag ghost must cover the rail). Drop position: sticky to avoid the stacking context (rejected: it is what keeps the rail in view, which is the promise). Take the flyout out of the rail into a fixed layer (rejected: its vertical anchor is a layout-time static position that drifts when the rail scrolls under an open word). Leave the paint order to the browser default (refuted: the board column is later in the document, so the default covers the rail).
```

## Consequences

```text
The rail paint level is a band, not a free number: page content that takes a z-index above 20 covers the rail again, and an overlay that drops below 20 is covered by it. No Rust type and no gate refuses either, so the clip-aware browser row read on a board page where the word overlaps a column is the standing check for both the clip and the cover. ADR-NAV-POLISH-002 refines the control-boundary argument for the two lower-group controls. Every other consequence of NAV-POLISH-003 stands.
```
