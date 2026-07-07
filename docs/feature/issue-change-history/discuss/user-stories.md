# User Stories — issue-change-history

Four stories, one per slice. Each traces to the anchor JTBD and a persona surface. Every field change writes ONE
durable, immutable change event (actor / field / old → new / when) in the same transaction as the mutation; the
three stories US-01…US-04 progressively record more fields and add each consumption surface over the SAME model.

## US-01 — See an issue's status history on a timeline (slice 1)
**As a** member working an issue (P1)
**I want** a readable, attributed timeline of the issue's status changes
**so that** I can see how the work progressed — who moved it, and when — without asking anyone.

### Elevator Pitch
Before: an issue's status moves between columns but leaves no visible record — you can't tell who moved it or when.
After: open an issue → the timeline shows `Mei moved status Todo → In Progress · 2h ago` (newest first), persisted.
Decision enabled: I understand how and when the work advanced, and hold the record without a side channel.

### Acceptance Criteria
- AC-01.1: Every status change (via the dialog, a drag, or the API) writes ONE change event — `actor`, `field=status`,
  `old`, `new`, `timestamp` — in the SAME transaction as the state write (never phantom, never dropped).
- AC-01.2: The issue view renders a timeline of its change events, newest first, each in plain language naming the
  actor, the change (`Todo → In Progress`), and a relative time.
- AC-01.3: The timeline persists — a reload (and any other viewer, on their reload) shows the same history.
- AC-01.4: History is append-only — an entry is never edited or deleted; a subsequent change adds a new entry.
- AC-01.5: Tenancy — the timeline for a foreign/absent issue is refused uniformly (non-enumerable), never a 500.
- AC-01.6: A same-value save records NO event (only real changes are history).

## US-02 — Record every editable field, not just status (slice 2)
**As a** member (P1)
**I want** title, description, and rank changes recorded on the same timeline
**so that** the history is the *whole* story of the issue, not only its status.

### Elevator Pitch
Before: editing the title, description, or reordering a card vanishes without a trace — only status is recorded.
After: rename an issue → the timeline gains `Mei changed title "Login bug" → "Login 500 on submit" · just now`.
Decision enabled: I can trust the timeline as the complete field-level record of the issue.

### Acceptance Criteria
- AC-02.1: A title change writes a `field=title` event (old → new); a description change writes `field=description`;
  a rank change writes `field=rank` — each in-tx with the mutation, via the same model as US-01.
- AC-02.2: The `update_issue_details` path (which emits nothing today) now records title/description changes.
- AC-02.3: A single save that changes multiple fields records one event per changed field (not one blob).
- AC-02.4: Long values are handled without truncating the record (the human view may abbreviate; the stored
  old→new is complete for the program/report surfaces).
- AC-02.5: The model is field-agnostic — adding a new tracked field later needs no history-model change.

## US-03 — Consume the change history as a program (slice 3)
**As an** integrator / automation (P2)
**I want** a stable JSON feed of an issue's change events
**so that** I can sync issue history into another tool or trigger workflows on changes.

### Elevator Pitch
Before: there's no machine-readable change feed — a program can't see what changed on an issue.
After: `GET /api/v1/.../issues/{n}/history` → `[{"actor":"mei@…","field":"status","old":"todo","new":"in_progress","at":"…Z"}, …]`.
Decision enabled: a program syncs or reacts to issue changes off a stable, documented contract.

### Acceptance Criteria
- AC-03.1: `GET …/issues/{n}/history` returns the issue's change events as a JSON array over the `/api/v1` surface,
  each with `actor`, `field`, `old`, `new`, `at` (ISO-8601 UTC), ordered oldest→newest (a stable audit order).
- AC-03.2: The endpoint authenticates + authorizes exactly like the other `/api/v1` issue routes; a foreign/absent
  issue → the API's uniform non-enumerable refusal, never a 500.
- AC-03.3: The envelope is pagination-ready (documented shape; v1 may return all with a stable order + a cursor
  field reserved).
- AC-03.4: The JSON is the SAME stored events the human timeline renders (no second source of truth).

## US-04 — Report change activity across a project (slice 4)
**As a** lead / reporter (P3)
**I want** an aggregated, exportable report of change activity across the project
**so that** I can account for the work and spot where issues stall.

### Elevator Pitch
Before: there's no way to see change activity across issues — only one issue at a time, if at all.
After: open the project change report → a table of who changed what, when (and status-flow counts); Export → CSV.
Decision enabled: I account for the work and see where the flow bottlenecks, across the whole project.

### Acceptance Criteria
- AC-04.1: A project-level report lists change events across the project's issues (issue key, actor, field,
  old → new, when), most recent first, workspace-scoped.
- AC-04.2: The report summarizes at least status-flow (counts of transitions) and per-actor change counts.
- AC-04.3: The report is exportable (CSV) with a stable column contract, from the SAME stored events.
- AC-04.4: Tenancy — the report shows only the acting workspace's data; no cross-tenant leakage.
