# board-lane-reorder — Intake record

**Created**: 2026-09-03 via `/nw:new` → `/nw:discuss`

## The original ask (verbatim)

> add a list left/ right position drag and drop allowing a user to change the
> kanban board lane order.

## What the wizard settled

The wizard surfaced one fact that reshaped the ask before any requirement was
written: **native HTML5 drag-and-drop fires no events on touch devices.** The
board's shipped card drag (`board-dnd.js`) is built that way, so copying its
mechanism would have produced a lane reorder that does nothing on a phone —
the same phone `fix-lane-menu-clipped-mobile` had just been fixed for, hours
earlier.

Three choices followed:

- **Pointer Events**, not HTML5 DnD, so the drag works on touch (D3). This
  diverges from how cards are dragged; DESIGN owes an ADR on whether the two
  converge later.
- The **column header** is the drag surface, gated by a movement threshold, so
  the `⋯` button living inside that header stays clickable (D2).
- **Move list left / Move list right** join the `⋯` menu, because Pointer
  Events cover mouse and touch but not keyboard or assistive technology (D4).

Other framing:

- Classification: cross-cutting
- Codebase: brownfield (50 prior features; `board-lane-overflow-menu` is the
  predecessor, and its *Out of Scope* named this feature as "the natural
  successor")
- UX research depth: lightweight
- Starting wave: DISCUSS

## Where the requirements live

**`feature-delta.md` in this directory is the single source of truth** for this
feature — decisions D1–D16, the three user stories with acceptance criteria,
KPIs, DoR validation and inherited commitments. Slice briefs are under
`slices/`. This file records only how the feature was framed at intake; it is
not a second requirements document and should not be read as one.
