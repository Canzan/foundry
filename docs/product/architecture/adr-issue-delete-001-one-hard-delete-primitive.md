# ADR-ISSUE-DELETE-001: An issue has one delete — a hard delete through one primitive that always announces itself

## Status

Accepted (issue-card-delete DESIGN wave, 2026-09-05)

Supersedes one clause of `adr-board-lane-002-two-fate-delete-transaction.md`
(the `DeleteCards` arm's "No events, no outbox, no tombstone"). Everything else
in that ADR — the transaction shape, the last-lane gate, confirm-time membership
binding, the FK strand-guard and the ≤3 bounded retry — is unchanged.

## Context

Deleting a *card* had no path at any granularity below a lane. The only in-app
removal was `delete_lane_with_fate(…, DeleteCards)`, whose blast radius is the
whole lane. Adding a card-level delete raised two questions the codebase had
already half-answered in incompatible directions.

**Semantics.** The intake proposed following `comment-edit-delete`: a soft
tombstone (`deleted_at`/`deleted_by`, `410 Gone`, GC deferred). But issues
already hard-delete in shipped code — `delete_cards_permanently`
(`lanes.rs:349`) runs `DELETE FROM issues WHERE id = ANY($1)`, recorded in
`registry.yaml` OUT-4 as "the hard `delete_issue_cascade` shape". A tombstone on
the card surfaces would give one entity two incompatible delete meanings,
reachable from two places in the same UI. Independently, `jobs.yaml` records
`board-lane-overflow-menu` D1: archive was offered and **declined**, because
"adding one would create a second way for a card to be invisible — the exact
failure board-lane-management D1(b) removed". A tombstoned issue is precisely
that. And the comment tombstone's own reason does not transfer: it preserves a
comment's *position in a thread*, and an issue holds no such position.

**Announcement.** `lanes.rs:336-340` states, verbatim: "No events, no outbox, no
tombstone (D7 — **parity with `delete_issue_cascade`, which emits nothing**)."
That parity was correct when written. It also means that today, deleting a lane
with `fate=delete` destroys N cards and every other open board keeps rendering
them until someone reloads — the one write in foundry that lies to a second
viewer. Meanwhile `Store::delete_issue_cascade` itself has **zero production
callers**: it is dead code, misfiled in `attachments.rs`, cited only by three
comments. This feature is its first real caller.

Store runs Postgres at READ COMMITTED; homelab scale (single node,
single-digit concurrent writers). `comments`, `issue_attachments` and
`issue_change_events` all reference `issues(id) ON DELETE CASCADE` (migrations
0004, 0005, 0013).

## Decision

**An issue delete is a hard delete, and there is exactly one primitive that
performs it.**

`crates/foundry-store/src/issue_delete.rs` holds
`delete_issues_with_outbox(tx, ctx, cards)`: one
`DELETE FROM issues WHERE id = ANY($1)` (the schema's `ON DELETE CASCADE`
carries comments, attachments and change events), followed by one `IssueDeleted`
outbox row per card **actually deleted**, all on the caller's transaction. A
thin `Store::delete_issue_with_outbox` owns a transaction for the single-card
case; `lanes.rs`'s `DeleteCards` arm rides its own existing transaction. Both
callers route through the primitive, so the lane fate now announces its
destroyed cards too.

The primitive performs **zero lookups**. It receives a fully-resolved
`IssueDeleteContext { workspace_id, project_id, key_prefix }` — both callers
already hold it, so no read is added on either path. The emit binds to
`rows_affected`, never to the requested card list, so a card that vanished
between resolution and delete is silently absent rather than falsely announced.

No lifecycle column is added to `issues`. No `410 Gone`. No GC. **No migration —
the schema head remains `0015`.** The `IssueDeleted` payload uses only fields
`EventPayload` already declares (`project_id`, `workspace_id`, `issue_id`,
`number`, `key`, `deleted`), so `schema_version` stays `1`.

The dead `Store::delete_issue_cascade` is removed from `attachments.rs`.

## Alternatives Considered

- **A. Soft tombstone on `issues`, mirroring `comment-edit-delete` ADR-007.**
  Rejected on three independent grounds, any one of which would be sufficient:
  it creates a second delete meaning for an entity that already hard-deletes
  through a shipped path; it re-creates the invisible-card state
  `board-lane-overflow-menu` D1 explicitly declined and
  `board-lane-management` D1(b) removed; and the precedent's own rationale
  (preserving a gap in a thread) has no analogue for an issue. It would also
  have required a migration, a `410` status, GC scheduling, and a rule for what
  the *lane* fate does with tombstoned cards — the last being unanswerable
  without reopening D1.
- **B. Hard delete, but keep two paths — a `_with_outbox` sibling for the card,
  leaving the lane fate's bare `DELETE` silent.** Rejected. Smallest diff, but
  it leaves two hard-delete paths with different announce behaviour, which is
  how drift starts, and it knowingly preserves a latent bug (a second viewer's
  board keeps N destroyed cards) that this work is one line from fixing. One
  path cannot drift from itself.
- **C. Hard delete now, fix the lane fan-out as a separate feature.** Rejected
  as scope hygiene that costs more than it saves: the fix is a call-site change
  inside the very refactor that creates the primitive, and deferring it means
  shipping a primitive whose two callers deliberately behave differently, then
  changing it again.
- **D. Emit from the application layer after the transaction commits.**
  Rejected. A committed delete could then fail to announce itself (process
  death between commit and emit), and a rolled-back one could announce. The
  outbox pattern exists precisely to make the announcement atomic with the
  write, and `soft_delete_comment_with_outbox` and `move_cards_to_destination`
  both already emit in-transaction.
- **E. Keep the primitive in `lanes.rs` beside `delete_cards_permanently`.**
  Rejected. An issue-lifecycle operation in the lane module inverts the
  relationship — the lane fate is a *caller* of issue deletion, not its owner.
  `lib.rs` was also rejected (already ~3000 lines) in favour of a new focused
  module.

## Consequences

- **Positive.** One meaning of "deleted" for an issue, enforced structurally
  rather than by convention: there is one function, so there is nothing to
  drift. A latent bug is fixed — lane deletes stop lying to second viewers. A
  dead, misfiled method is relocated at zero blast-radius cost (it will never be
  cheaper). No migration, no schema change, no wire change: the schema head
  stays `0015` and `schema_version` stays `1`. A future `/api/v1` DELETE is a
  handler-only change, because the use case (not the handler) owns the write.
- **Negative.** Deleting a lane with `fate=delete` now writes N outbox rows in
  one transaction where it previously wrote none. This is the cost
  ADR-BOARD-LANE-002's own *Consequences* already accepted for the `move` arm
  ("N cards produce N events + N outbox rows in one tx — fine for homelab card
  counts"), now paid symmetrically. A bulk-events optimisation would remain a
  store-internal change behind the same port.
- **Negative.** A behaviour change lands in shipped code this feature was not
  asked to touch. It is stated in the feature's *Changed Assumptions* and
  covered by three new acceptance criteria (AC-3.8…3.10) rather than inherited
  quietly, and slice 03's estimate moved 0.5 → 1 day to pay for that coverage.
- **Irreversibility is the feature.** There is no undo, no trash and no archive.
  The confirm dialog is the entire safety net, which is why it is mandatory
  (ADR-ISSUE-DELETE-002) and why it counts consequences rather than merely
  warning about them.
- **Deferred.** No `check-arch` rule yet pins that both callers route through
  the primitive; a second bare `DELETE FROM issues` would currently compile. The
  equivalent lane rules were added only once a second feature needed them, and
  this follows that precedent — recorded as an open question, not an oversight.
