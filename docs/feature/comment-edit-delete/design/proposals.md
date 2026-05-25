# Design proposals — comment-edit-delete (slice 5)

**Mode**: propose
**Owner (this wave)**: solution-architect (Morgan)
**Status**: AWAITING USER DECISION on Q1–Q6; Q7 has a default-accept recommendation.
**Predecessor design**: `docs/feature/foundry-backend-mvp/design/` (slice 1) +
`docs/feature/foundry-realtime-collab/distill/wave-decisions.md` (slice 2) +
`docs/feature/foundry-operator-grade/distill/` (slice 3) +
`docs/feature/foundry-contributor-onboarding/distill/` (slice 4).
**Layout convention**: legacy per-wave (no `docs/product/`, no `feature-delta.md`)
— see slice-4 `wave-decisions.md` line 204.

---

## 0. What this slice is

A brownfield extension of US-10 (comments) that finally lights up the three
sub-ACs slice 2 deferred:

| Capability | Actor | HTTP shape |
|---|---|---|
| Edit own comment | comment.author | `PATCH /team/{team}/project/{project}/issues/{n}/comments/{comment_id}` |
| Delete own comment | comment.author | `DELETE …/comments/{comment_id}` |
| Delete any comment | workspace admin | same `DELETE …/comments/{comment_id}` |
| Show "edited" indicator | viewer | rendered into the comment card |
| Fan out PATCH/DELETE to viewers | server → all viewers of issue | extends existing SSE channel |
| Block non-author edit | server | 403 HTML fragment |

The requirements ARE pinned in `docs/feature/foundry-backend-mvp/discuss/stories.md`
§ US-10 lines 1057–1175 (4 ACs + 6 UAT scenarios). The open questions are
purely about HOW.

---

## 1. Reuse Analysis — HARD GATE

Slice-5 footprint should be heavily skewed to EXTEND. Every CREATE NEW is
challenged. "Existing class too coupled" is not a valid reason — the
slice-1 architecture already enforces dependency inversion at the crate
boundary; new files within existing crates are the right answer.

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

---

## 2. Quality attribute drivers (drives the decisions below)

| Attribute | Priority | Why |
|---|---|---|
| Auditability | HIGH | "Admin deletes Mei's comment" is a moderation action; the system must remember it happened. Drives Q2. |
| Realtime consistency | HIGH | 1s p99 fanout is inherited from slice 2 NFR-PERF-03; edit/delete must hit the same SLA. Drives Q3 and Q2. |
| Security (authorization) | HIGH | Non-author edit attempt = explicit 403 contract (UAT scenario). CSRF must apply. Drives Q7. |
| Simplicity | HIGH | Slice 1 taste filter — "approachability". A modal-edit fragment with diff history is overkill for a 2-day slice. Drives Q4 and Q5. |
| Recoverability | MEDIUM | If admin accidentally deletes the wrong comment, can we restore? Drives Q2. |
| Forward-compat with future search (US-12+) | LOW | Slice 3 ILIKE search hits issue text, not comment text — comment search is not on the roadmap. Drives Q2 (search index NOT a blocker). |

---

## Q1 — Edit-window policy

**Question**: how long after posting may a comment author edit their own comment?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Always editable** | No time limit. Author may edit at any point until the comment is deleted. | Simplest rule (no clock dependency). Mirrors GitHub/GitLab behaviour. No new domain invariant. No clock injection into auth check. | Late edits can re-write history under existing reply threads — but slice-1 explicitly punted on threaded replies, so this risk is theoretical until US-?? adds them. |
| **B. 15-min window** | Edit allowed only within 15 minutes of `created_at`. After that, the Edit affordance vanishes; server returns 403 on attempt. | Limits revisionism; matches Slack default. Forces "edit while you remember" UX. | Introduces a clock dependency in the authorization check (the `Clock` port already exists on `AppState`, so the cost is small but real). New @error scenario needed: "edit window expired → 403". User confusion when admins/peers reference now-vanished Edit button. |
| **C. Until-first-reply** | Edit allowed until any subsequent comment exists on the same issue. After the first reply, edits are locked. | Captures the "don't revise after the conversation moved on" semantic. No wall-clock needed. | Slice-1 has no concept of replies-to-a-comment; "first reply" must mean "any later comment on the issue" — counter-intuitive. Requires a `COUNT(*) WHERE created_at > $self_created_at` on every authorization check (cheap with the `idx_comments_issue_created` index, but it's a second query). Cannot be expressed as a static authorization rule. |

**Recommendation: A (always editable)**. Rationale: (a) matches the
slice-1/2 "ship the smallest thing that satisfies the AC" discipline; the
US-10 AC explicitly says "Author can edit/delete own comments" with no
time qualifier; (b) zero new dependencies — no clock, no count query; (c)
reversible — adding a window in v0.2 is a one-line authorization check
addition; removing one is a breaking UX change. The "edited" indicator
(Q4) carries enough revision-awareness to handle the audit concern
without a time wall.

**Earned-Trust note**: the authorization check is `comment.author_id ==
session.user_id || session.role == admin`. No external dependency means
no probe needed for the policy itself.

---

## Q2 — Soft-vs-hard delete

**Question**: when a comment is deleted, does the row stay in the database?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Hard DELETE** | `DELETE FROM comments WHERE id = $1`. Row is gone. Outbox carries `CommentDeleted {comment_id}` for fanout. | Smallest schema delta (no new columns). Backup/restore is symmetric — deleted comment is not in `pg_dump`, period. No risk of "deleted but visible" bugs. | Lost-forever moderation history. Admin auditability impossible at the data layer. If Mei reports "Devansh deleted my comment unjustly", there's no row to point at. SSE event must be self-describing (no follow-up "fetch the row" path possible). |
| **B. Soft (tombstone)** | Add `deleted_at TIMESTAMPTZ NULL` + `deleted_by UUID NULL`. UI hides rows with `deleted_at IS NOT NULL`. `body_markdown`/`body_html` kept verbatim. List query filters `WHERE deleted_at IS NULL`. | Full moderation audit trail — admin queries the deleted rows on demand. `pg_dump` captures everything (slice-3 backup story holds). Undelete is trivial (UPDATE … SET deleted_at = NULL). SSE event can carry `{deleted: true, comment_id}` and the receiver re-renders. | Storage grows monotonically. Privacy concern: deleted comment body lives forever in backups (matters for GDPR-ish workspaces). UI logic must consistently apply the filter (one missed `WHERE` and deleted comments leak — needs ArchUnit-style query enforcement OR a `comments_visible` VIEW). |
| **C. Hybrid (soft now, GC later)** | Soft-delete on the write path (B above). A background task in `foundry-app` (the existing cleanup-task pattern from `architecture.md` § coord-question-6 — Postgres advisory lock for single-replica execution) hard-deletes rows where `deleted_at < now() - interval '90 days'`. | Audit trail for the 90-day window when moderation disputes happen. Storage stays bounded. GDPR-friendly — privacy stewards can document "deleted comments are unrecoverable after 90 days". | One more background task (cron-style cleanup) — a 5th cleanup pass alongside expired sessions, expired bootstrap tokens, expired reset tokens, expired invites. Operationally cheap but adds surface area. The 90-day knob becomes a config value or a hardcoded constant. |

**Recommendation: B (soft tombstone), with C as a v0.2 follow-up**.
Rationale: (a) the US-10 ACs require admin-delete; without a tombstone,
the admin's action is unauditable — fails NFR audit posture established
in slice 3's backup/restore work; (b) the storage cost for a 2-day-slice
MVP is negligible (a 1000-comment instance with 5% deletion is ~50 rows
of ~few-hundred bytes each); (c) C is a clean follow-up because the
B schema is a strict subset of C — adding a GC task later requires zero
schema migration; (d) the SSE event shape (Q3) is cleaner with B: the
receiver just re-renders the comment card with `deleted=true` styling.

**Earned-Trust note**: B requires a probe that verifies the `WHERE
deleted_at IS NULL` filter actually applies to the list query. The
existing acceptance suite catches this end-to-end (testcontainers + real
SQL), but I recommend adding a tiny gold-test that inserts one deleted +
one live comment and asserts only the live one appears in `GET /issues/.../`.
This is the "probe the substrate lie that we won't forget the WHERE
clause" pattern per principle 12.

**Search-index impact**: slice-3 ILIKE search hits `issues.title`, NOT
`comments.body_markdown` (verified by grep). Comment search is not on the
roadmap; even US-12 (slice 2) keyboard nav is issue-key + title only.
Therefore option B does NOT need a "filter deleted from search index"
follow-up.

---

## Q3 — SSE event shape for edit/delete

**Question**: how do edit and delete events ride the existing
`issue_events` SSE channel?

Constraint inherited from slice-2 ADR (locked in
`docs/feature/foundry-realtime-collab/distill/wave-decisions.md`): a
single `event:` per channel + payload-discriminated `event_type` +
`schema_version: 1`. The existing channel carries `IssueCreated`,
`IssueUpdated`, `CommentAdded`. The question is what the slice-5
additions look like.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Two new event_types: `CommentEdited`, `CommentDeleted`** | Add two distinct `event_type` string values. Receiver matches on `event_type` and renders accordingly. Payload for `CommentEdited` carries `{comment_id, body_html, edited_at}`. Payload for `CommentDeleted` carries `{comment_id}`. | Self-documenting on the wire (`event_type` tells you what to do). Mirrors the existing pattern (`IssueCreated` and `IssueUpdated` are already two distinct types). htmx OOB-swap target differs (replace-in-place for edit vs remove-from-DOM for delete) — distinct event_types make the receiver's logic obvious. No risk of "is this an edit with empty body or a delete?" ambiguity. | Two new constants instead of one. Slightly larger receiver match. (Both are O(few LOC) changes.) |
| **B. Single polymorphic `CommentMutated` with `sub_type: "edited"\|"deleted"`** | One new event_type. Sub-field discriminates. | Fewer new strings to spell-check across server + SSE consumer. | Adds a SECOND discriminator next to `event_type` — the existing convention is one discriminator. Receiver still needs a two-arm match on sub_type. Forward-compat suffers: if slice-6 adds `CommentReactionAdded`, do we polymorph it under `CommentMutated` too? Pattern doesn't compose. |
| **C. Reuse `CommentAdded`, signal edit/delete via payload flags** | Re-fire `CommentAdded` with `{edited: true}` or `{deleted: true}`. | No new event_type at all. | Breaks the "event_type tells you what happened" property. Receivers that filter by `event_type=CommentAdded` would now process edits/deletes too — surprising and bug-prone. Rejected on principle. |

**Recommendation: A (two new event_types)**. Rationale: (a) matches the
established `IssueCreated`/`IssueUpdated` pattern — Mei reads the
`EventPayload` enum and sees a coherent family of types; (b) cleaner
receiver dispatch — the existing match in `crates/foundry-app/src/events.rs`
gets two new arms, not a nested switch; (c) `event_type` carries
behavioural intent on the wire, which makes Wireshark-style debugging
trivial. The schema_version stays at 1 (forward-compatible field
addition per realtime-roadmap invariant 4).

**Earned-Trust note**: the `EventPayload` struct adds `#[serde(default)]
pub edited_at: Option<String>` so old listeners that haven't been updated
ignore the new field rather than failing to deserialize. This is the
slice-2 forward-compat pattern, applied verbatim.

---

## Q4 — Edit-history visibility

**Question**: how much edit history do viewers see?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. "edited" label + timestamp only** | Comment card shows `(edited 2 minutes ago)` next to the author byline when `updated_at IS NOT NULL`. No diff, no history. | Smallest storage delta (no history table). Matches UAT scenario verbatim ("an 'edited' indicator appears next to the comment timestamp"). Privacy-friendly — old content is replaced, not exposed. | No way to recover the pre-edit text if a dispute arises. Cannot answer "what did Mei originally say?" |
| **B. Full diff visible to all viewers** | Each edit appends a row to a new `comment_revisions` table. Comment card has a "show history" affordance that expands to a diff view. | Maximum auditability for everyone. Useful when discussing how a comment evolved. | New table + new query + new UI. Privacy regression — every typo correction is now public. Significant scope expansion (probably +2 days alone). Overkill for the AC. |
| **C. "edited" label for all + diff to admins only** | Same as A on the public surface. New `comment_revisions` table exists but is queried only via an admin-only `/admin/comments/{id}/history` route. | Audit for moderation without exposing typo-corrections to peers. Balances B's audit power with A's privacy posture. | Two surfaces to build (the "edited" label AND the admin history view). Schema is the same as B (revisions table). Test surface doubles. |

**Recommendation: A ("edited" label + timestamp only)**. Rationale:
(a) verbatim match to the UAT scenario ("an 'edited' indicator appears
next to the comment timestamp"); (b) the audit need is satisfied at the
adjacent layer — soft-delete (Q2.B) handles the moderation-history case;
edits are typically typo corrections, not history rewrites; (c)
revisions table is a strict additive change in v0.2 if telemetry says
disputes arise; (d) the "show diff to admins only" surface (C) adds a
test matrix that doubles slice size for marginal benefit.

**Earned-Trust note**: the "edited" label is computed in the store
projection (`edited: updated_at.is_some()`) — pure derivation, no
external dependency.

---

## Q5 — htmx edit-affordance shape

**Question**: how does the Edit button visually transform the comment
card into an editable form?

Reference: slice-2 keyboard-nav `c` shortcut already uses the
`hx-target=#modal` pattern for the new-issue flow (see
`crates/foundry-app/src/keyboard.rs::show_new_issue_modal`).

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Inline replace (hx-swap=outerHTML)** | Author clicks "Edit". htmx fetches `GET …/comments/{id}/edit` returning a `<form>` fragment shaped like the comment card. `hx-target` is the comment card itself; `hx-swap=outerHTML` replaces it in place. Submit (PATCH) returns the re-rendered comment card; cancel returns the original card. | Keeps the user in flow — no modal stack, no scroll loss. Mobile-friendly (no overlay layer). Familiar pattern (GitHub does this). Reuses `render_comment_card` for both the form view and the re-rendered view by switching one template branch. | Two GET endpoints (edit-form, cancel-revert) in addition to the PATCH. The cancel path is a server round-trip rather than a client-side flip; minor extra latency but no JS needed. |
| **B. Modal edit (hx-target=#modal)** | Author clicks "Edit". htmx opens a modal fragment matching slice-2's `c` shortcut shape. Submit replaces the comment card via `hx-swap-oob`. Cancel closes the modal. | Reuses the existing modal infrastructure verbatim. Visually distinct — clear "you're editing now" affordance. Easier to wire keyboard `Esc=cancel` (already implemented for the `c` modal). | Modal stack is awkward for "edit something in a scrollable list" — user loses position. Mobile UX requires extra care. Two templates to maintain (the modal form and the inline card). |
| **C. Author-only visible Edit button + inline contenteditable** | The card has a hidden Edit button that alpine.js reveals when `data-author == current_user_email`. On click, alpine.js makes the body `contenteditable=true`; on blur or save it fires the PATCH. | Zero round-trips to enter edit mode. Maximum perceived speed. | Loses the markdown editing surface (contenteditable shows rendered HTML, not the markdown source). Re-rendering on save creates a paste-vs-formatting mismatch. Author surprise: "I typed `**bold**` and now I see **bold**" — editing the rendered form is not what they want. Rejected on UX. |

**Recommendation: A (inline replace)**. Rationale: (a) preserves the
markdown source (the form's textarea contains the raw `body_markdown` from
the store — author edits the same characters they originally typed);
(b) no modal stack, no scroll loss in a long comment thread; (c) the
"author-only visible Edit button" concern from C is satisfied by
server-side rendering: the existing `render_comment_card` already
receives the actor's user_id at render time — slice-5 conditionally
emits the Edit affordance only when `comment.author_id == actor.user_id
|| actor.role == admin`. No alpine.js authorship check needed, which
also closes the trivial XSS surface (hiding the button via JS is
security theatre; the server enforces 403 anyway).

**Earned-Trust note**: the GET `…/comments/{id}/edit` endpoint MUST
enforce the same 403 rule as the PATCH endpoint — a non-author who hits
the form URL directly must not see the markdown source. Reuses the same
authorization helper. This is the "probe the substrate lie that
authorization is uniform across HTTP verbs" pattern.

---

## Q6 — Edit/delete on an already-deleted comment

**Question**: what status does the server return when a user tries to
PATCH or DELETE a comment whose row is soft-deleted (or non-existent)?

Note: this question only matters because Q2.B (soft-delete) introduces
the "row exists but is logically gone" state. If Q2 lands on A (hard
delete), this question collapses to "404 always" and there's no choice
to make.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. 404 Not Found** | Treat soft-deleted same as non-existent. PATCH/DELETE both return 404. | Simplest semantics — the row "is gone" from the client's perspective. Avoids leaking the existence of deleted rows to non-admins. | Loses the distinction between "never existed" and "was deleted". Admin attempting to audit a deleted comment via direct PATCH hits 404, which is technically correct but unhelpful. |
| **B. 410 Gone** | PATCH/DELETE on a soft-deleted row returns 410. | Semantically precise — RFC 7231 § 6.5.9 reserves 410 for "resource intentionally removed, permanent". Existing slice-1 bootstrap-used path returns 410 for a precedent. | Leaks the fact that the row existed — but for admins this is fine, and for non-author members the 403 fires first (the comment authorship was authoritative before the row was deleted). |
| **C. Soft-error htmx fragment (200 OK with explanatory body)** | Return an htmx fragment saying "this comment has been deleted, refresh to see the latest state". | UX-friendly inline message. Matches the existing `bad_request_fragment` shape. | Status code lies (200 OK for a failure semantically). Breaks REST-style client tooling. Discouraged. |

**Recommendation: B (410 Gone)**. Rationale: (a) precedent — slice-1
bootstrap returns 410 for a used token, this is the same shape; (b)
semantically precise — the row IS intentionally removed; (c) admins
auditing get a meaningful distinguished code; (d) the htmx receiver can
translate 410 into an inline "this comment was deleted" notice via the
existing `hx-target-410` swap mechanism. For genuinely non-existent
comment ids (random UUID), 404 still applies — the handler does a row
existence check first.

**Earned-Trust note**: distinguishing 404 vs 410 requires a SELECT
including deleted rows on the authorization path, then a separate filter
in the list query. The store layer's `find_comment_by_id` returns
`Option<CommentRow>` including `deleted: bool`. Handler logic: `None →
404`, `Some(c) if c.deleted → 410`, `Some(c) → check author/admin →
proceed`.

---

## Q7 — CSRF posture (default-accept recommendation, not 3-option proposal)

**Question**: do PATCH and DELETE inherit the slice-1 CSRF double-submit
middleware?

**Recommendation: YES**, with a one-line ADR (proposed ADR-009 below).

PATCH and DELETE are state-mutating verbs subject to the same
cross-origin-form abuse vector as POST. The existing `csrf::csrf_middleware`
already applies layer-wide in `build_router`; the new routes inherit it
automatically once registered. The middleware checks the `_csrf` form
field for `application/x-www-form-urlencoded` bodies and the `HX-CSRF`
header for htmx requests. Both apply unchanged to PATCH/DELETE.

The one delta worth calling out in the ADR: htmx's default form encoding
for `hx-delete` sends an empty body, so the `_csrf` token MUST ride in
the `HX-CSRF` header for DELETE requests. This is already the alpine.js
hook's behaviour in slice 2 — no change needed, just documented.

---

## 3. Proposed ADRs to write once decisions land

Continuing the slice-1 numbering (slice 1 used ADR-001..005; slices 2/3/4
didn't add new system-level ADRs). Slice 5 proposes:

| ADR | Title | Captures | Decision required from |
|---|---|---|---|
| ADR-006 | Comment edit-window policy | Q1 outcome | User |
| ADR-007 | Comment soft-delete with tombstone | Q2 outcome | User |
| ADR-008 | Comment edit/delete SSE event shape | Q3 outcome | User |
| ADR-009 | CSRF coverage extends to PATCH/DELETE | Q7 default-accept | One-liner — no user pick needed |

Q4 (history visibility), Q5 (htmx affordance shape), Q6 (status code for
deleted comment) are settled in this slice without standalone ADRs —
they're documented in `architecture.md` because they're presentation/UX
choices that don't constrain v0.2 evolution. Promote to ADRs if user
disagrees.

---

## 4. Optional C4 component diagram (write-path with new verbs highlighted)

Slice 1 already documents the system context (L1) and container (L2)
diagrams. Slice 5 doesn't change either — same single binary, same
single Postgres. The only diagram worth adding to `architecture.md` is
an L3 component diagram showing the comment write path with the new
PATCH/DELETE arcs highlighted.

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

Key property the diagram makes obvious: **the slice-1 trigger-based
fanout works unchanged**. Slice 5 adds two outbox event_types, period.
No new pg_notify wiring, no new listener task, no broadcast topology
change.

---

## 5. External integration check (principle 10)

This slice introduces NO new external integrations. Comment edit/delete
is fully internal — same Postgres, same SMTP-not-touched, same
sanitizer. No contract test annotation needed for the platform-architect
handoff. (The existing SMTP contract test annotation from slice 1
remains; no new annotation.)

---

## 6. Architecture enforcement (principle 11)

Existing enforcement holds:

- `cargo xtask check-arch` (slice 1 ADR-001) — no changes to crate boundaries.
- `cargo deny check` — no new dependencies introduced. (Re-uses
  `pulldown-cmark` + `ammonia` for the edit re-render path.)
- `cargo sqlx prepare --check` — new queries added to the offline cache
  in the migration PR.

No new tooling needed. The "list query MUST filter `WHERE deleted_at
IS NULL`" rule is enforced behaviourally by the acceptance suite (real
testcontainers Postgres + real list endpoint).

---

## 7. Earned Trust (principle 12) — adapter probes

Slice 5 adds NO new adapters. The existing `Store::probe()` already
validates Postgres reachability + migration version + LISTEN/NOTIFY
round-trip. The new `update_comment_with_outbox` and
`soft_delete_comment_with_outbox` are new methods on the existing
`Store` adapter; they ride the existing probe.

**New probe scenario to add to the existing Store probe** (one extra
assertion, not a new probe): after the existing `pg_notify` round-trip,
also assert that the `comments.updated_at` and `comments.deleted_at`
columns exist (i.e., migration `0006` ran). This is the "probe the
substrate lie that the migration applied but we didn't notice"
fault-injection pattern.

---

## 8. Quality-gate self-check before user decisions

- [x] Requirements traced to components — US-10 ACs → comments.rs +
      store updates + EventPayload extensions
- [x] Component boundaries respected — no new crates; all changes land
      in existing files (per ADR-001)
- [x] Technology choices justified — zero new deps
- [x] Quality attributes addressed — auditability (Q2), realtime
      consistency (Q3), security (Q7), simplicity (Q1/Q4)
- [x] Dependency-inversion compliance — handler → store → PG, no
      reverse dependencies
- [x] C4 diagrams — L1/L2 inherited from slice 1; L3 sequence diagram
      provided above for the new verbs
- [x] Integration patterns specified — extends existing SSE channel
      with two new event_types (Q3)
- [x] OSS preference validated — no new deps
- [x] AC behavioural, not implementation-coupled — all options framed
      around WHAT the system does
- [x] External integrations — none new (principle 10)
- [x] Architectural enforcement tooling — existing cargo-xtask + cargo
      sqlx prepare + cargo deny suffice (principle 11)
- [ ] Peer review — DEFERRED until user decisions on Q1-Q6 land and
      `architecture.md` is finalized

---

## 9. Constraint contradictions found

**None blocking.** Two minor notes:

1. The task brief says "plural URL form `/issues/{n}/comments/{id}` —
   slice-3 polish unified this." Verified against the actual codebase
   (`crates/foundry-app/src/lib.rs` route table): the convention is
   `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/...`
   (singular `team`, singular `project`, plural `issues`). The "plural"
   claim is correct for the `issues` segment specifically. New routes
   in slice 5 follow this verbatim:
   `/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}`.

2. The slice-2 outbox payload from the existing
   `insert_comment_with_outbox` includes `key: "{project_key_prefix}-{issue_number}"`.
   Slice-5 edit/delete events should include the same `key` field for
   consistency, so SSE receivers can correlate fan-outs to issue-cards
   by stable user-visible identifier. Documented in the recommended
   Q3.A implementation but worth flagging here.

---

## 10. Under-specified inherited docs (flagged for honesty per task brief)

None. Every Q has enough inherited context to propose options. The
slice-2 `wave-decisions.md` "Comment edit + delete → slice 3" line
combined with `evolution/2026-05-25-foundry-realtime-collab.md` lines
74-79 and 208-211 gave a complete picture of what was deferred and why,
and the slice-1 `data-access.md` + `auth.md` + `realtime-roadmap.md`
constrained the design space sufficiently. No new file-line citations
needed.

---

## Next-step instruction for the orchestrator

Collect user picks on Q1-Q6 (Q7 default-accepted). For each picked
option, dispatch back to this agent with `execute --finalize` and the
selected options. The finalize pass will:

1. Write `architecture.md` (slice-specific design summary referencing
   slice-1 by inheritance)
2. Write `wave-decisions.md` (DDD-numbered decision list)
3. Write `adrs/ADR-006`..`ADR-009` (one per decision needing record)
4. Invoke `solution-architect-reviewer` for peer-review approval
5. Produce DISTILL handoff package
