# Wave Decisions — comment-edit-delete (slice 5)

DESIGN-wave decisions. All seven user picks accepted verbatim from the
proposals dialogue (no overrides). This document is the slice-5 handoff
artifact for the DISTILL wave alongside `architecture.md`.

## DDD Decisions (D1 - D7)

| ID  | Question | Pick | Captured in |
|-----|----------|------|-------------|
| D1  | Edit-window policy | **A** — Always editable (no time limit) | ADR-006 |
| D2  | Soft-vs-hard delete | **B** — Soft tombstone (`deleted_at` + `deleted_by`); GC deferred to v0.2 | ADR-007 |
| D3  | SSE event shape | **A** — Two new event_types: `CommentEdited`, `CommentDeleted` | ADR-008 |
| D4  | Edit-history visibility | **A** — "edited" label + timestamp only; no revisions table | `architecture.md` (inline) |
| D5  | htmx edit affordance | **A** — Inline replace via `hx-swap=outerHTML`; no modal | `architecture.md` (inline) |
| D6  | Status code for soft-deleted | **B** — 410 Gone for soft-deleted; 404 for genuinely non-existent | `architecture.md` (inline) |
| D7  | CSRF coverage for PATCH/DELETE | **accept** — Inherits existing middleware; `hx-delete` uses `HX-CSRF` header | ADR-009 |

D1, D2, D3, D7 are promoted to standalone ADRs because they constrain
v0.2 evolution (schema shape, wire shape, security posture).
D4, D5, D6 are documented inline in `architecture.md` because they're
presentation/UX choices that can flip in a v0.2 follow-up without
breaking downstream consumers.

## Reuse Analysis — HARD GATE artefact

This table is the slice's hard gate. Every "create new" is challenged;
every "extend" is justified by reuse over reimplementation per principle 5.

| Action | Target | Why | LOC delta |
|---|---|---|---|
| EXTEND | `crates/foundry-store/migrations/` — add `0006_comments_edit_delete.sql` | New migration is the only way to add columns (`updated_at TIMESTAMPTZ NULL`, `deleted_at TIMESTAMPTZ NULL`, `deleted_by UUID NULL REFERENCES users(id)`) to the existing `comments` table; forward-only migration discipline (ADR-003) forbids editing `0004_comments.sql`. | +~20 SQL |
| EXTEND | `crates/foundry-store/src/lib.rs` § comments | Add `update_comment_with_outbox(...)`, `soft_delete_comment_with_outbox(...)`, `find_comment_by_id(...)`. Mirrors the existing `insert_comment_with_outbox` shape (same tx + outbox pattern) so the realtime trigger fans out automatically. | +~120 |
| EXTEND | `crates/foundry-store/src/lib.rs` § `CommentRow` projection | Add `edited: bool` (derived from `updated_at IS NOT NULL`) + `deleted: bool` (from `deleted_at IS NOT NULL`); list query filters or projects deleted rows depending on Q2 outcome. | +~10 |
| EXTEND | `crates/foundry-app/src/comments.rs` | Add handlers `submit_edit_comment` (PATCH), `submit_delete_comment` (DELETE), `show_edit_form` (GET fragment if Q5 picks inline-replace flow). Reuse `signed_in_user`, `bad_request_fragment`, `internal_error`, `redirect_to`, `is_htmx`, `render_comment_card`, `render_comment_card_oob` verbatim. | +~150 |
| EXTEND | `crates/foundry-app/src/lib.rs` — route table | Two new routes registered next to existing `submit_comment` route; same CSRF middleware applies. | +~12 |
| EXTEND | `crates/foundry-realtime/src/lib.rs` — `EventPayload` | Add `#[serde(default)] pub deleted: Option<bool>` and/or new `event_type` constants (Q3 decides shape). Forward-compatible additions per realtime-roadmap invariant 4 — `schema_version` stays at 1. | +~10 |
| EXTEND | `crates/foundry-app/src/events.rs` (SSE handler) | If Q3 picks polymorphic event_type, no change beyond a new match arm for rendering. If Q3 picks 2 distinct event_types, add 2 new arms. | +~15 to +~25 |
| EXTEND | `crates/foundry-core/src/markdown.rs` | NO change — `render_comment_markdown` is reused verbatim on edit (same sanitizer pipeline, same allowlist). This is the SECURITY argument for re-rendering on edit, not on read. | 0 |
| CREATE NEW | none | All work fits in existing files/crates. The slice-1 ADR-001 explicitly said "slices 2/3/4 add files to existing crates, not new crates" and slice 5 honors the same property. | — |

**Total estimated delta**: ~340 LOC of Rust + ~20 LOC of SQL + scenario files.
This is the largest application-level extension since slice 2 itself; size
is consistent with adding two HTTP verbs + their fan-out + their authorization.

## Architecture Summary

- **Pattern**: Layered with strict inward dependency, dependency-inversion
  at the crate boundary (inherited from slice-1 ADR-001).
- **Paradigm**: OOP-flavored Rust with plain async fns; aggregates are
  data + invariants in `foundry-core`. No traits introduced in slice 5
  (no second implementer appears).
- **Key components touched**:
  - `foundry-app` — adds 3 handlers (PATCH, DELETE, GET edit-form) + 2
    route registrations.
  - `foundry-store` — adds 3 methods on the existing `Store` adapter +
    extends `CommentRow` projection.
  - `foundry-realtime` — extends `EventPayload` with `deleted: Option<bool>`
    and 2 new `event_type` string constants.
  - `foundry-core` — unchanged (`render_comment_markdown` reused).
  - `foundry-auth` — unchanged (existing `signed_in_user` extractor + CSRF
    middleware cover the new routes).
- **Communication**: All existing patterns reused. PATCH/DELETE -> store
  -> Postgres outbox -> trigger -> pg_notify -> per-replica listener ->
  broadcast -> SSE viewers. No new wiring.

## Technology Stack

**Zero new dependencies.** Slice 5 is a pure extension of existing
adapters:

- Rust 2021 / axum 0.8 / sqlx — unchanged.
- `pulldown-cmark` + `ammonia` — reused via existing
  `render_comment_markdown`; no version bump.
- `tower-sessions` + `argon2` + HMAC tokens — unchanged.
- `tokio::sync::broadcast` + PgListener — unchanged.
- htmx 2.x — reused for `hx-patch`, `hx-delete`, `hx-swap=outerHTML`,
  `HX-CSRF` header (all already in use in slice 2 + slice 4 surfaces).

`cargo deny check` is expected to pass without changes.

## Constraints Established

These constraints are established by slice-5 decisions and become
invariants downstream waves and future slices must honor:

1. **Soft-delete invariant**: every read path against `comments` MUST
   filter `WHERE deleted_at IS NULL` unless the read is for moderation
   audit. Enforcement is behavioural via the acceptance suite (real
   testcontainers Postgres + real list endpoint asserts only live rows
   render). A v0.2 follow-up MAY introduce a `comments_visible` SQL VIEW
   to make this a schema-level rather than convention-level rule.
2. **SSE event_type additions are additive, not breaking**: new
   `CommentEdited` and `CommentDeleted` ride the existing
   `issue_events` channel with `schema_version` unchanged at 1. Old
   listeners ignore unknown `event_type` values (they fall through the
   default match arm and are logged but not fatal). Future slices follow
   the same forward-compat pattern.
3. **CSRF posture extends uniformly**: PATCH and DELETE inherit the
   slice-1 double-submit middleware. htmx `hx-delete` MUST carry the
   token in the `HX-CSRF` header (empty body has no form field). Future
   verbs (PUT, etc.) inherit the same rule.
4. **Authorization is HTTP-verb-uniform**: the rule
   `comment.author_id == actor.user_id || actor.role == admin` applies
   identically to the GET edit-form, PATCH, and DELETE handlers. A
   non-author hitting the GET edit-form URL directly gets 403, not the
   markdown source. This is the "probe the substrate lie that
   authorization is uniform across HTTP verbs" invariant.
5. **404-vs-410 disambiguation**: PATCH and DELETE on a row that exists
   but is soft-deleted return 410 Gone (RFC 7231 semantics). PATCH and
   DELETE on a non-existent UUID return 404 Not Found. The handler does
   a row-existence check first via `find_comment_by_id`, which returns
   tombstones.
6. **Migration `0006` is the schema floor for slice 5+**: the three new
   nullable columns (`updated_at`, `deleted_at`, `deleted_by`) are a
   strict subset of the schema needed by the deferred 90-day GC task
   (ADR-007 alternative C). No further migration is needed when v0.2
   adopts the GC task.

## Open Questions for DISTILL

These are intentionally small and bounded; DISTILL resolves them with
the acceptance-designer:

1. **Exact `@nfr-*` tag set for slice-5 scenarios**: the slice-2 suite
   uses `@nfr-perf-03` (1s p99 fanout), `@nfr-sec-05` (XSS sanitizer).
   Does slice 5 need a fresh tag for "edited indicator render
   correctness" or does it ride existing tags? DISTILL decides.
2. **Walking-skeleton coverage for `GET …/comments/{id}/edit`**: is the
   inline edit-form fragment its own `@walking_skeleton` (real
   spawn_app + real Postgres + real htmx fragment response), or does it
   ride the PATCH walking skeleton (which exercises the form
   round-trip end-to-end)? DISTILL decides; recommendation is to
   bundle it under the PATCH skeleton to keep suite wall-clock down.
3. **Cancel-edit handler URL shape**: the cancel path is a server
   round-trip (`GET …/comments/{id}` returns the un-edited card
   fragment). Does this route reuse an existing handler (the "show a
   single comment" path may not exist yet) or get a thin new handler?
   DISTILL + software-crafter decide during RED — both options satisfy
   the AC.
4. **410-Gone htmx UX wording**: the precise text for the "this
   comment was deleted, refresh to see the latest state" fragment
   returned by the 410 handler. UX copy; not architecturally
   constrained.
5. **Admin-undelete operator path**: slice 5 does NOT ship a UI to
   undelete a tombstoned comment, but the schema supports it
   (`UPDATE comments SET deleted_at = NULL, deleted_by = NULL`). Is
   the operator runbook addition (a one-line `psql` recipe) a slice-5
   deliverable or a v0.2 follow-up? Recommendation: v0.2; DISTILL
   confirms.

## Decision-driven invented detail (FLAGGED for user override)

The following specifics were chosen to make the design concrete, but
they're under the user's authority to override before DISTILL picks
them up:

1. **Migration column types**: `updated_at TIMESTAMPTZ NULL`,
   `deleted_at TIMESTAMPTZ NULL`, `deleted_by UUID NULL REFERENCES users(id)`.
   These mirror the existing `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`
   convention. If you want `ON DELETE SET NULL` semantics for `deleted_by`
   when the admin user is later deleted, say so.
2. **Partial index**: `CREATE INDEX idx_comments_issue_live ON comments
   (issue_id, created_at) WHERE deleted_at IS NULL`. Added to keep the
   "live comments for issue" hot path narrow. If you'd rather the existing
   `idx_comments_issue_created` suffice (it covers all rows including
   tombstones, with a small cost), drop the partial index.
3. **`EventPayload` new field name**: `deleted: Option<bool>`. Bool feels
   right because the only state for the receiver is "is this a tombstone
   notification?". A more elaborate enum (`tombstone_reason`) is
   over-design at this slice.
4. **GET edit-form URL suffix**: `/comments/{id}/edit`. RFC-style hint
   that this is a representation, not a sub-resource. Alternative
   `/comments/{id}?action=edit` was considered and rejected as less
   htmx-idiomatic.
5. **htmx swap target convention**: `id="comment-{uuid}"` on the comment
   card root element. Inherits from the slice-2 OOB-swap pattern; if
   slice 2 used a different id shape (e.g., `comment_{uuid}`), the
   software-crafter aligns to the existing convention during GREEN.
