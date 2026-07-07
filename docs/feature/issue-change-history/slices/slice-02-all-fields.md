# Slice 02 — Record every editable field

**Goal**: title, description, and rank changes land on the same timeline as status — the whole field-level story.
**Story**: US-02.

**IN scope**
- Extend in-tx capture to the remaining write paths using the slice-01 model:
  - `update_issue_details` (`lib.rs:1524`) — records `field=title` and/or `field=description` (it emits NOTHING
    today — this is the notable change).
  - `reposition_issue_with_outbox` — records `field=rank` on a position change.
- One event **per changed field** on a multi-field save (not one blob); an unchanged field records nothing.
- Complete old→new capture for long values (the human view may abbreviate; the stored record is whole).
- The timeline (slice 01) renders these new event kinds with per-field plain-language phrasing.
- Acceptance: per-field record contract + multi-field decomposition + the timeline showing title/desc/rank events.

**OUT of scope**: the program feed (03); the report (04); comments/attachments in the timeline; priority/assignee
(not editable yet).

**Learning hypothesis**: disproves "the remaining write paths emit the SAME change-event uniformly" if the
currently-silent title/desc path or the rank path resists a uniform in-tx record, or multi-field saves don't
decompose cleanly.

**Seams**: `update_issue_details` (`lib.rs:1524`); `reposition_issue_with_outbox` (`lib.rs:1364`); the slice-01
model + timeline render.
**Dependencies**: slice 01 (model + timeline). **Effort**: ~0.5–1 day (capture-point breadth over a proven model).
