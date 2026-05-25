# Application Architecture — comment-edit-delete (slice 5)

Owner: solution-architect (Morgan). Slice-specific design summary. Inherits
the entire slice-1 architecture by reference; does NOT restate the 5-crate
workspace, dependency direction, CSRF model, SSE topology, or sanitizer
pipeline.

## Inheritance

- **Workspace shape** — unchanged from `docs/feature/foundry-backend-mvp/design/adrs/ADR-001.md`.
  No new crates. All slice-5 code lands in existing files within
  `foundry-app`, `foundry-store`, `foundry-realtime`.
- **CSRF model** — unchanged from slice-1 `csrf::csrf_middleware` (double
  submit; `_csrf` form field for form bodies, `HX-CSRF` header for htmx).
  Layer-wide application in `build_router` covers PATCH/DELETE automatically.
  See ADR-009 for the one-line clarification on `hx-delete`'s empty body.
- **SSE topology** — unchanged from `docs/feature/foundry-backend-mvp/design/realtime-roadmap.md`.
  Single `event:` per channel; payload-discriminated `event_type`;
  `schema_version` stays at 1. Slice 5 adds two new `event_type` values via
  forward-compatible field addition.
- **Sanitizer pipeline** — unchanged. `pulldown-cmark` -> `ammonia` via the
  existing `render_comment_markdown` function in `foundry-core/src/markdown.rs`.
  Edit path re-runs the SAME function on the new markdown source; no separate
  edit-time sanitizer surface.
- **Migration discipline** — unchanged from slice-1 ADR-003. Forward-only.
  Slice 5 adds `0006_comments_edit_delete.sql`; never edits `0004_comments.sql`.

## What this slice changes

| Surface | Change |
|---|---|
| Routes | +2 new routes (`PATCH`, `DELETE`) + 1 GET fragment route for inline edit form |
| Database schema | +1 migration adding 3 nullable columns to `comments` |
| Store methods | +3 methods: `find_comment_by_id`, `update_comment_with_outbox`, `soft_delete_comment_with_outbox` |
| `EventPayload` | +2 new `event_type` constants; +1 new optional field (`deleted: Option<bool>`); existing `comment_id` field carries through |
| htmx fragments | +1 new template: the inline edit form (shaped like the comment card) |
| HTTP status semantics | +1 distinguished status (410 Gone) for soft-deleted rows |

Zero new crates. Zero new dependencies. Zero new external integrations.

## Component Diagram (C4 Level 3) — comment write path with slice-5 verbs

```mermaid
sequenceDiagram
    autonumber
    participant B as Browser (htmx)
    participant R as axum Router
    participant H as comments handler
    participant ST as Store (foundry-store)
    participant PG as Postgres
    participant T as outbox trigger
    participant L as PgListener (per-replica)
    participant BC as broadcast (in-process)
    participant V as Viewer browser (SSE)

    Note over B,V: EXISTING (slice 2) - POST /comments
    B->>R: POST .../comments
    R->>H: submit_comment
    H->>ST: insert_comment_with_outbox (tx)
    ST->>PG: INSERT comments; INSERT outbox; COMMIT
    PG->>T: trigger notify_outbox_event
    T->>PG: pg_notify('issue_events', envelope)
    PG-->>L: notification
    L->>BC: send EventPayload {event_type=CommentAdded}
    BC->>V: SSE event

    Note over B,V: NEW (slice 5) - PATCH /comments/{id}
    B->>R: PATCH .../comments/{id}
    R->>H: submit_edit_comment
    H->>ST: find_comment_by_id (authz)
    H->>ST: update_comment_with_outbox (tx)
    ST->>PG: UPDATE comments SET body_*, updated_at; INSERT outbox CommentEdited; COMMIT
    PG->>T: same trigger
    T->>PG: pg_notify
    PG-->>L: notification
    L->>BC: send EventPayload {event_type=CommentEdited}
    BC->>V: SSE event (htmx swaps card in place)

    Note over B,V: NEW (slice 5) - DELETE /comments/{id}
    B->>R: DELETE .../comments/{id}
    R->>H: submit_delete_comment
    H->>ST: find_comment_by_id (authz: author OR admin)
    H->>ST: soft_delete_comment_with_outbox (tx)
    ST->>PG: UPDATE comments SET deleted_at, deleted_by; INSERT outbox CommentDeleted; COMMIT
    PG->>T: same trigger
    T->>PG: pg_notify
    PG-->>L: notification
    L->>BC: send EventPayload {event_type=CommentDeleted}
    BC->>V: SSE event (htmx removes card from DOM)
```

Property the diagram makes obvious: the slice-1 trigger-based fanout works
unchanged. Slice 5 adds two outbox event_types; no new pg_notify wiring, no
new listener task, no broadcast topology change.

## Route Additions

All routes register next to existing `submit_comment` in
`crates/foundry-app/src/lib.rs` and follow the established URL convention
(singular `team`, singular `project`, plural `issues`):

| Method | Path | Handler | Purpose |
|---|---|---|---|
| `GET` | `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}/edit` | `show_edit_form` | Returns the inline edit-form fragment (htmx target is the comment card). Enforces 403 for non-author. |
| `PATCH` | `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}` | `submit_edit_comment` | Updates `body_markdown` + `body_html` + `updated_at`. Enforces 403 for non-author. Returns 410 for soft-deleted row; 404 for missing row. |
| `DELETE` | `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}` | `submit_delete_comment` | Soft-deletes the comment. Allows author OR workspace admin. Returns 410 for already-soft-deleted row; 404 for missing row. |

CSRF middleware applies layer-wide per slice-1 `build_router`. PATCH form
bodies carry `_csrf` like POST; DELETE rides `HX-CSRF` header (htmx
`hx-delete` sends an empty body — see ADR-009).

## Migration Shape — `0006_comments_edit_delete.sql`

```sql
-- crates/foundry-store/migrations/0006_comments_edit_delete.sql
-- Slice 5: comment edit + delete (US-10 deferred ACs).
--
-- Adds three nullable columns to support:
--   * "edited" indicator           (updated_at IS NOT NULL)
--   * soft-delete tombstone        (deleted_at IS NOT NULL, ADR-007)
--   * admin moderation audit trail (deleted_by points to the admin user)
--
-- All columns are nullable so the migration is non-destructive for
-- existing rows. The 90-day GC task (ADR-007 follow-up "C") is a strict
-- superset of this schema; no further migration is needed when v0.2 adds it.

ALTER TABLE comments
    ADD COLUMN updated_at TIMESTAMPTZ NULL,
    ADD COLUMN deleted_at TIMESTAMPTZ NULL,
    ADD COLUMN deleted_by UUID NULL REFERENCES users(id);

-- Partial index to keep the "live comments for issue" hot path narrow.
-- The existing idx_comments_issue_created covers all rows; this partial
-- index skips tombstones, which is the access shape for the issue page.
CREATE INDEX idx_comments_issue_live ON comments (issue_id, created_at)
    WHERE deleted_at IS NULL;
```

Forward-only per ADR-003. `0004_comments.sql` is NOT edited.

## Store Method Additions

Three new methods on the existing `Store` adapter (added to
`crates/foundry-store/src/lib.rs`, mirroring the `insert_comment_with_outbox`
shape):

| Method | Signature shape | Notes |
|---|---|---|
| `find_comment_by_id` | `(workspace_id, comment_id) -> Option<CommentRow>` | Returns rows including soft-deleted ones; caller distinguishes via `CommentRow::deleted`. Used for authorization checks and 404-vs-410 disambiguation. |
| `update_comment_with_outbox` | `(workspace_id, comment_id, new_markdown, new_html, actor_user_id) -> Result<()>` | Single transaction: UPDATE `comments` SET `body_markdown`, `body_html`, `updated_at = now()` + INSERT outbox row with `event_type = "CommentEdited"`. Trigger fans out. |
| `soft_delete_comment_with_outbox` | `(workspace_id, comment_id, actor_user_id) -> Result<()>` | Single transaction: UPDATE `comments` SET `deleted_at = now()`, `deleted_by = actor` + INSERT outbox row with `event_type = "CommentDeleted"`. |

`CommentRow` projection extends with `edited: bool` (derived from
`updated_at IS NOT NULL`) and `deleted: bool` (derived from
`deleted_at IS NOT NULL`). The existing list query gains
`WHERE deleted_at IS NULL` to hide tombstones from viewers.

## `EventPayload` Additions

Per ADR-008, slice 5 introduces two new `event_type` constants:

- `"CommentEdited"` — emitted on successful PATCH. Payload reuses
  `comment_id`, `issue_id`, `workspace_id`, `key`, `author_id`,
  `author_email`. Carries no new fields beyond the existing ones.
- `"CommentDeleted"` — emitted on successful DELETE (soft). Payload reuses
  `comment_id`, `issue_id`, `workspace_id`, `key`. Sets the new
  `deleted: Some(true)` field so receivers can detect tombstones without
  parsing `event_type`.

Forward-compatible addition pattern (slice-2 invariant):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    // ... existing fields unchanged ...
    /// Set on CommentDeleted events; None otherwise. Forward-compatible
    /// field addition; schema_version stays at 1.
    #[serde(default)]
    pub deleted: Option<bool>,
}
```

`schema_version` stays at 1. Old listeners that haven't been recompiled
ignore the new field and the two new `event_type` values fall through
their existing match arms (logged as "unknown event_type" but not
fatal).

## htmx Fragment Shape — inline edit form

The Edit affordance follows the inline-replace pattern (ADR per Q5 = A;
documented here rather than as a standalone ADR because it's a presentation
choice that doesn't constrain v0.2 evolution):

1. Author clicks "Edit" on a comment card. Browser fires
   `GET .../comments/{id}/edit` with `hx-target="#comment-{id}"` and
   `hx-swap="outerHTML"`.
2. Server returns a `<form>` fragment shaped like the comment card, with
   a `<textarea name="body_markdown">` pre-populated with the raw markdown
   source (NOT the rendered HTML — author edits the same characters they
   originally typed) and Save / Cancel buttons.
3. Save fires `PATCH .../comments/{id}` with `_csrf` + `body_markdown`.
   Server returns the re-rendered comment card (with the "edited"
   indicator) which htmx swaps back in place via `outerHTML`.
4. Cancel fires `GET .../comments/{id}` (a thin handler that returns the
   un-edited comment card fragment) which htmx swaps back in place.

The Edit and Delete buttons render only when
`comment.author_id == actor.user_id || actor.role == admin`. Server-side
gating; no JS authorship check. The GET edit-form endpoint enforces the
SAME 403 rule as the PATCH endpoint — a non-author who hits the form URL
directly gets a 403 fragment, not the markdown source.

## 404-vs-410 Handler Logic

Per ADR-008 and Q6 = B, the PATCH and DELETE handlers distinguish between
"row never existed" and "row was deleted":

```text
let row = store.find_comment_by_id(workspace_id, comment_id).await?;
match row {
    None                              -> 404 Not Found  (random UUID, or wrong workspace)
    Some(c) if c.deleted              -> 410 Gone        (intentionally removed)
    Some(c) if !is_authorized(c, actor) -> 403 Forbidden (not author, not admin)
    Some(c)                           -> proceed with PATCH / DELETE
}
```

Precedent: slice-1 bootstrap-used token returns 410 — slice 5 reuses the
same semantic. htmx receivers translate 410 into an inline "this comment
was deleted, refresh to see the latest state" notice via the existing
`hx-target-410` swap mechanism.

## Quality Attributes Addressed

| Attribute | Mechanism |
|---|---|
| Auditability (HIGH) | Soft-delete tombstone (ADR-007) preserves the row for moderation review. `deleted_by` records the admin actor. |
| Realtime consistency (HIGH) | Reuses slice-2 SSE fanout verbatim. Two new `event_type` values ride the existing channel; 1s p99 NFR inherited. |
| Security / authorization (HIGH) | Server-side gating on both the edit-form GET and the PATCH/DELETE. CSRF middleware applies layer-wide; ADR-009 documents the `hx-delete` header detail. |
| Simplicity (HIGH) | "edited" label only (no revisions table); inline-replace form (no modal); zero new deps; zero new crates. |
| Recoverability (MEDIUM) | Soft-delete is trivially reversible (`UPDATE ... SET deleted_at = NULL`). Operator-only path; no UI affordance in slice 5. |

## External Integration Check (principle 10)

NONE new. Comment edit/delete is fully internal — same Postgres, same SMTP
(untouched), same sanitizer. No new contract test annotation for the
platform-architect handoff. The existing SMTP annotation from slice 1
remains.

## Architecture Enforcement (principle 11)

Existing tooling suffices; no additions needed:

- `cargo xtask check-arch` — no crate-boundary changes.
- `cargo deny check` — zero new dependencies.
- `cargo sqlx prepare --check` — new queries added to the offline cache
  in the migration PR.

Behavioural enforcement of the "list query MUST filter `WHERE deleted_at
IS NULL`" rule lives in the acceptance suite (testcontainers Postgres +
real list endpoint asserts only live rows render).

## Earned Trust (principle 12)

No new adapters. The existing `Store::probe()` validates Postgres
reachability + migration version + LISTEN/NOTIFY round-trip. Slice 5 adds
one assertion to the existing probe: after the LISTEN/NOTIFY round-trip,
also assert that `comments.updated_at` and `comments.deleted_at` columns
exist (i.e., migration `0006` ran). This is the "probe the substrate lie
that the migration applied but we didn't notice" pattern.

## ADRs Created

- `adrs/ADR-006-comment-edit-window-policy.md` — Q1 outcome (always editable)
- `adrs/ADR-007-comment-soft-delete-tombstone.md` — Q2 outcome (soft tombstone)
- `adrs/ADR-008-comment-edit-delete-sse-event-shape.md` — Q3 outcome (two new event_types)
- `adrs/ADR-009-csrf-coverage-patch-delete.md` — Q7 outcome (CSRF inherits)

Q4 (history visibility = A), Q5 (htmx affordance = A), Q6 (status code = B)
are settled inline in this document — they're presentation/UX choices that
don't constrain v0.2 evolution.
