# Slice 03 — A deleted card leaves every open board

**Story**: US-ICD-03 | **Estimate**: 1 day | **Depends on**: slice 01's primitive

> Estimate raised 0.5 → 1 day by DESIGN. DDD-2 folds lane delete onto the same
> primitive, which changes shipped behaviour and owes the regression coverage
> below. See `feature-delta.md` §DESIGN *Changed Assumptions*.

## Goal

Deleting an issue announces itself once, in the same transaction that deletes
it, and every other board open on that project drops the card without a reload.

## IN scope

- One `outbox` row per committed delete, written **inside the delete's transaction**: `event_type = "IssueDeleted"`, payload carrying `project_id`, `issue_id`, `number`, `key` and `deleted: true` (D9).
- **Rewiring `lanes.rs::delete_cards_permanently` onto the shared primitive** (DDD-2). The lane fate's `DeleteCards` arm stops being silent and emits one `IssueDeleted` per destroyed card, on its own existing transaction. ADR-BOARD-LANE-002's transaction shape, last-lane gate, confirm-time membership binding, FK strand-guard and ≤3 bounded retry must all come through **unchanged** — the amendment to that ADR is one clause, and this slice is what proves it.
- Deleting `delete_cards_permanently` once its body is the primitive call.
- The browser-side removal of `article.issue-card[data-issue-key="…"]` on receipt.
- Acceptance assertions: exactly one event per committed delete, **zero** per refused delete (404, CSRF, race), correct `schema_version = 1`, and **no new `EventPayload` field**.
- Tenancy assertion: a subscriber on a different project receives nothing.
- A `@needs-browser` two-tab scenario: delete in tab A, watch tab B drop the card while its other cards stay put.

## OUT of scope

- Any new `EventPayload` field, any `schema_version` bump, any change to the SSE topology or channel.
- Any change to `CommentAdded` / `CommentEdited` / `CommentDeleted` or issue-state events beyond proving they still work (AC-3.6).
- Any change to the lane fate's transaction shape, locking, retry or refusal behaviour. The rewiring is a call-site substitution; if it turns out to require restructuring `delete_lane_with_fate`, that is a finding to raise, not work to absorb.
- A `check-arch` rule pinning that every issue delete routes through the primitive (deferred — DESIGN open question 2).

## Learning hypothesis

**Disproves, if it fails:** that the `CommentDeleted` fan-out idiom transfers to
an issue delete unchanged. The specific risk is transactional, not
architectural: `soft_delete_comment_with_outbox` emits alongside an `UPDATE`
that leaves the row present, whereas this emits alongside a `DELETE` that
removes the row the payload describes — and the payload's `key` must be composed
from `key_prefix` + `number` **before or during** the delete, because after it
the row is gone. If that ordering turns out to be awkward inside one
transaction, the fallback is composing the payload in the service layer from
the pre-delete read the dialog port already performs.

**Confirms, if it succeeds:** any destructive issue-level write can announce
itself through the shipped topology with zero schema and zero wire change; the
"no write lies to a second viewer" property holds for deletes as it already does
for edits; and one primitive genuinely serves two transaction owners — which is
what makes "one meaning of deleted" structural rather than conventional.

## Acceptance criteria

AC-3.1 … AC-3.7 (see `feature-delta.md` US-ICD-03), **plus the three DESIGN
amendments** AC-3.8 … AC-3.10 (see `feature-delta.md` §DESIGN *Amendments to
US-ICD-03*):

- **AC-3.8** lane delete with `fate=delete` on N cards emits exactly N `IssueDeleted` events, in the lane-delete transaction.
- **AC-3.9** lane delete with `fate=move` still emits exactly N `IssueUpdated` and **zero** `IssueDeleted`, and still writes one 0013 status event per card.
- **AC-3.10** the shipped lane-delete scenarios, the last-lane refusal, the `DestinationNotFound` refusal and the bounded retry are green and unmodified; a second open board drops all N cards on a `fate=delete`.

## Production data

Real outbox rows through the real `notify_outbox_event` trigger (migration
0003), a real `PgListener`, and two real browser sessions on a real seeded
board. Event assertions read the `outbox` table directly as well as the SSE
stream, so a delivered-but-unwritten (or written-but-undelivered) event is
distinguishable.

## Dogfood moment

Same day: two tabs on the same board, one delete, no reload. Then the lane path —
delete a lane holding three cards with `fate=delete` and watch the second board
drop all three. Also the honest negative check: refuse a delete and confirm the
`outbox` table did not grow.

## Dependencies

Slice 01's primitive (`issue_delete::delete_issues_with_outbox`) and its
`&mut Transaction` signature. The outbox trigger, `spawn_pg_listener`,
`EventPayload`, `SseSubscription` and the `@needs-browser` lane are all shipped.
Slice 02 is **not** a dependency — 03 can land before or after it.
