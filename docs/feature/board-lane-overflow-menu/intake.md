# board-lane-overflow-menu — Intake record

**Created**: 2026-09-02 via `/nw:new` → `/nw:discuss`

## The original ask (verbatim)

> adjust the `crates/foundry-app/src/` board view so it uses `...` instead of X
> to get to an "archive list" menu item rather than a large x to close it.
> Use the attached screenshot for an example of the behavior I'm looking for.

Reference screenshot: a Trello list menu showing *Edit list · Archive cards ·
Insert list before · Insert list after · Archive list · Delete list · Sort by*.

## What the wizard settled

Archive was offered and **declined** — foundry has no archive concept in the
domain, so the destructive verb stays **Delete**. Scope became four menu items:
**Edit list · Insert list before · Insert list after · Delete list**.

- Classification: cross-cutting
- Codebase: brownfield (48 prior features; `board-lane-management` is the
  predecessor, and its **D9** pre-registered this feature by name)
- Starting wave: DISCUSS

## Where the requirements live

**`feature-delta.md` in this directory is the single source of truth** for this
feature — decisions D1–D14, the three user stories with acceptance criteria,
KPIs, DoR validation and inherited commitments. Slice briefs are under
`slices/`. This file records only how the feature was framed at intake; it is
not a second requirements document and should not be read as one.
