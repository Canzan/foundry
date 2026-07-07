# Requirements — issue-change-history

## Context

An issue changes over its life — its status moves across columns, its title and description are edited, its rank
shifts. Today those changes leave **no first-class, readable record**: a member cannot see "what happened to this
issue, by whom, and when." The app already emits some change events into a **durable, append-only outbox**
(`migrations/0003_outbox_notify.sql` — `IssueCreated` / `IssueUpdated` / `CommentAdded|Edited|Deleted`, no GC),
but those rows are shaped for the realtime notify envelope: **new-value-only** (e.g. the new `state`, not
old→new), 8 KB-capped, and not surfaced anywhere a human, a program, or a report can consume.

This feature makes every change to an issue a **durable, well-modeled change event** — who / which field /
old → new / when — surfaced in three forms the user named: a **human** timeline on the issue, a **program**
JSON feed, and a **report** aggregation.

## JTBD (anchor job)

> **When** an issue has been worked on over time, **I want** the full, trustworthy record of what changed, who
> changed it, and when — in a form I can *read*, a program can *consume*, and a report can *aggregate* — **so I
> can** understand how the work got here, integrate it into other tools, and account for it.

Dimensions — **functional**: reconstruct the change history of any issue; **emotional**: trust that nothing is
silently lost or rewritten; **social**: show the team an honest, attributable record of the work.

## Personas
| ID | Persona | Surface they care about | Cares about |
|----|---------|-------------------------|-------------|
| P1 | A member working an issue | **Human** timeline | "What changed on this issue, and who did it?" — read it inline, in plain language. |
| P2 | An integrator / automation | **Program** JSON feed | A structured, stable change feed to sync into other tools / trigger workflows. |
| P3 | A lead / reporter | **Report** aggregation | Roll changes up across issues (who did what, when; status flow) — read or export. |

Three personas → the three surfaces are the defining requirement (one model, three consumers).

## Scope (v1) — user-confirmed 2026-07-06: all issue field changes, all three surfaces

- **In scope**:
  - **Record** a durable change event for **every change to a tracked issue field** — who (actor), which field,
    old value → new value, and when — captured in the SAME transaction as the mutation so the record can never
    drift from the change. Tracked fields = the ones that can change today: **status, title, description, rank
    (position)**. The model is **field-agnostic** so priority/assignee join automatically once they become
    editable.
  - **Human** surface — a readable change timeline on the issue (e.g. "Mei moved this from Todo → In Progress ·
    2h ago"), newest-relevant ordering, plain-language per-field phrasing.
  - **Program** surface — a JSON change feed for an issue via the `/api/v1` surface (stable, paginated-ready
    envelope; actor / field / old / new / timestamp).
  - **Report** surface — an aggregated change report across a project (who changed what, when; status-flow
    counts), viewable and exportable.
- **Out of scope** (v1, deferred): **adding** priority/assignee *editing* (this feature records changes; it does
  not add new editing surfaces — those fields simply have nothing to record until a separate feature makes them
  editable); comments/attachments in the timeline (they already have their own surfaces — v1 is *field* changes,
  though the model may later fold them in); editing/deleting history entries (append-only, immutable); realtime
  push of new history entries to open viewers (converge on reload, mirroring `card-ranking` UC-1); cross-project
  or org-wide audit export; retention/pruning policy (append-only in v1).

## Brownfield grounding (seams — REUSE / EXTEND / MIRROR; DESIGN owns the model)

| Seam | Location | Role |
|------|----------|------|
| Durable outbox (REUSE-or-MIRROR) | `migrations/0003_outbox_notify.sql`; `INSERT INTO outbox …` in `lib.rs` | Append-only event log (no GC) already recording issue events — BUT new-value-only, notify-shaped, 8 KB-capped. DESIGN decides: enrich+reuse as the history source vs a dedicated per-field history model (ODD-1). |
| Issue write paths (EXTEND — capture points) | `insert_issue_with_outbox` (`lib.rs:1169`), `update_issue_state_with_outbox` (`lib.rs:1287`), `reposition_issue_with_outbox` (`lib.rs:1364`), `update_issue_details` (`lib.rs:1524`, emits NOTHING today) | Every mutation must record a change event in-tx. Each write path holds both old + new. The title/desc path must start recording. |
| API sub-router (EXTEND — program surface) | `foundry-api` `routes()` — `/api/v1/.../issues`, `PATCH …/{n}`, `…/comments` | Home for `GET …/issues/{n}/history` (program feed). |
| Issue web view (EXTEND — human surface) | board `issue_card.html`; `IssueEditModal` / `show_edit_form` (issue-edit-dialog); NO full issue-detail page today | The timeline needs a home — the edit dialog vs a new issue-detail view (ODD-3). |
| Comments (MIRROR — precedent) | `migrations/0004_comments.sql`; comment add/edit/delete + outbox | An existing per-issue sub-record + render + API + tombstone pattern to mirror for shape/tenancy/testing. |
| Tenancy / non-enumerability | the shipped `resolve_member_project` + uniform-404 lineage (ADR-003) | History reads scope by acting workspace; a foreign issue → uniform non-enumerable refusal. |

## Constraints

- **In-transaction capture** — a change event is written in the SAME transaction as the mutation it records, so
  history can never phantom (record a change that rolled back) or drop (a change with no record).
- **Append-only / immutable** — history entries are never edited or deleted (audit integrity); v1 has no
  retention/pruning.
- **One model, three surfaces** — the human timeline, the program JSON, and the report all read the SAME stored
  change event; the model is designed once to serve all three (no per-surface divergence).
- **Field-agnostic + extensible** — recording keys off (field, old, new); new tracked fields (priority,
  assignee) require no model change.
- **Tenancy / CSRF** — history reads are workspace-scoped; a foreign/absent issue → uniform non-enumerable
  refusal, never a 500.
- **No regression to the realtime path** — whatever DESIGN chooses, the existing outbox → SSE behaviour
  (card-ranking / issue-status-move) stays green.

## Open decisions (for DESIGN)

- **ODD-1: History store** — enrich + reuse the durable **outbox** as the history source (it is already
  append-only and durable, but new-value-only, notify-shaped, 8 KB-capped) vs a **dedicated `issue_events` /
  `issue_history` table** modeling per-field old→new decoupled from the realtime envelope. Likely a new table;
  reuse must be explicitly weighed.
- **ODD-2: Capture mechanism** — record in each store write method (both old + new are in-tx there), including
  the currently-silent `update_issue_details`; a shared helper vs per-path inserts; one row per changed field vs
  one row per mutation with a field set.
- **ODD-3: Human-timeline home** — render in the existing edit dialog vs introduce a first issue-detail page.
- **ODD-4: Report shape** — an on-page project report vs a CSV/export vs API-side aggregation (and what
  dimensions: per-actor, per-day, status-flow).
- **ODD-5: Old-value capture for pre-existing issues** — issues created before this feature have no prior
  history; the timeline starts from "created" (backfill a single genesis entry, or start empty). DESIGN decides
  the genesis story.
- **ODD-6: Program envelope** — reuse the api-contract JSON conventions + pagination readiness for the history
  GET.
