# Slice 03 — Insert list before / after

**Story**: US-BLO-03 | **Estimate**: 1.5 days (incl. spike) | **Depends on**: slice 01; independent of slice 02

## Goal

`⋯ → Insert list before|after` opens a dialog; a named lane appears at exactly
that position, empty, with every existing card untouched — making lane deletion
no longer one-way.

## IN scope

- Lane insert write port: mint slug, insert at position, shuffle later positions — **one transaction** (D8).
- Lane-slug mint: underscore-separated, letter-anchored, satisfying `^[a-z][a-z0-9_]*$`, unique per project. **Not** `foundry_core::slugify` (it emits hyphens — D6). Lives below `crates/foundry-app/src` (check-arch).
- Slug-collision and empty-normalisation refusals, inline, no auto-suffixing (D7).
- Insert dialog template, declarative close, `_csrf` confirm, 422 → `[data-error-slot]`.
- OOB `#board-columns` refresh on success.
- Proof the inserted lane is first-class on every lane-consuming surface (AC-3.7).

## OUT of scope

- Reordering existing lanes (deferred successor).
- Moving cards into the new lane as part of the insert — it lands empty.
- Any issue-row write; no 0013 change event (AC-3.3).

## Learning hypothesis

**Disproves, if it fails:** that `UNIQUE (project_id, position) DEFERRABLE
INITIALLY IMMEDIATE` (`0015:22`) can absorb a mid-board insert inside one
transaction via `SET CONSTRAINTS ... DEFERRED`, with no schema change. A failure
means this slice grows migration 0016 and its estimate moves — and it must be
reported as a premise break (AC-3.9), never a silent migration.

**Confirms, if it succeeds:** the 0015 author's `DEFERRABLE` declaration was
deliberate headroom, and lane reordering (the deferred successor) inherits the
same machinery for free.

## Acceptance criteria

AC-3.1 … AC-3.9 (see `feature-delta.md` US-BLO-03).

## Production data

Insert into a seeded project holding real issues across real lanes; assert by
SQL that positions stay contiguous and unique, that zero `issues` rows changed,
and that zero `issue_change_events` rows were written.

## Dogfood moment

Same day: insert a real lane into the live "Homelab Ops"-shaped board between
two occupied lanes, then drag a real card into it.

## Pre-slice SPIKE (REQUIRED — uncertainty is high)

**Timebox: 1 hour, before any insert code is written.**

Question: does a transaction doing `SET CONSTRAINTS ALL DEFERRED` then
`UPDATE lanes SET position = position + 1 WHERE project_id = $1 AND position >= $2`
followed by the insert commit cleanly against a live-shaped `lanes` table?

Run it against a database with live-shaped data (several projects, 3–5 lanes
each, issues referencing lanes through `fk_issues_lane`). Record the outcome in
DESIGN. If it fails, stop and design the migration rather than working around
it in application code.
