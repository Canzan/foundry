# ADR-EIP-003: The v1 routable event catalogue is a closed enum mapped from the shipped outbox, enriched by three payload fields

## Status

Proposed (2026-10-21, external-integration-plugins DESIGN).

## Context

D-2 lists what Foundry emits today and what each kind carries in v1. DISCUSS found two gaps:
title and description edits write `issue_change_events` but no outbox row (ODD-4, follow-up F-1),
and assignment has no writer at all. It also found that `IssueUpdated` carries the new lane only
(US-EIP-03 needs "from → to"), and the shipped `IssueDeleted` payload carries neither the actor nor
the title, both of which K-8's deleted line and D-11's burst line need after the issue row is gone
(hard delete with `ON DELETE CASCADE`). ADR-005 of notification-delivery-providers requires a closed
catalogue. The `${event_catalogue}` shared artifact (HIGH risk) must have one name per event across
UI, API, webhook, metrics and `openapi.json`.

Code facts: in-lane reorders write no outbox row (`reposition_issue_with_outbox` emits only when the
lane changes), so `IssueUpdated` always means a lane change; `move_cards_to_destination` (lane delete,
MoveCards fate) writes one `IssueUpdated` per card; `delete_issues_with_outbox` writes one
`IssueDeleted` per removed card; `foundry-realtime::EventPayload` tolerates added fields
(`#[serde(default)]`), and the store comments state "adding fields is fine".

## Options considered

1. **Read everything at send time, change no writer.** Fails for deletes: the title and actor are
   gone with the row. The old lane could be read from `issue_change_events`, but that makes every
   move delivery a second lookup keyed on timestamps, kept only as the upgrade-window fallback.
2. **Add an `integration_events` snapshot written by each writer.** Rejected in ADR-EIP-002.
3. **Enrich the outbox payload in the three writers that lack data (chosen),** and keep everything
   else read at send time so renames show current names (D-8).
4. **Also add `IssueEdited` to the outbox now (close F-1).** Rejected for v1: the realtime
   consumer would receive a new event type the board was deliberately kept from broadcasting
   (ODD-4); F-1 stays a follow-up, and the closed enum below admits it as one new variant.

## Decision

1. **`EventKind` (in `foundry-integrations`) is closed:** `IssueCreated`, `IssueMoved`,
   `IssueDeleted`, `CommentAdded`, `CommentEdited`, `CommentDeleted`, each with one wire name
   (`issue.created`, `issue.moved`, `issue.deleted`, `comment.added`, `comment.edited`,
   `comment.deleted`) and one product label (K-8 words). Per-kind eligibility is a method:
   chat and email accept the first four; webhook (deliverable C) accepts all six. There is no
   `issue.reordered` event, because the store never writes one. Issue edited and assignment are
   not in v1.
2. **Mapping from the outbox `event_type`** is one pure function: `IssueCreated` → `issue.created`;
   `IssueUpdated` → `issue.moved`; `IssueDeleted` → `issue.deleted`; `CommentAdded`,
   `CommentEdited`, `CommentDeleted` → the comment kinds. An unknown `event_type` is skipped and
   counted, never guessed.
3. **Payload enrichment, same transaction, no extra query:**
   - `IssueUpdated` gains `from_state` (the lane slug the writer already holds:
     `reposition_issue_with_outbox` has `old_state`; `move_cards_to_destination` has the dying
     slug);
   - `IssueDeleted` gains `author_id` (the deleting actor, passed in through
     `IssueDeleteContext`) and `title` (`DELETE … RETURNING id, number, title`; at most 256
     characters, inside the NOTIFY cap).
4. **Rolling-deploy tolerance:** rows written by a not-yet-replaced replica lack these fields. The
   hydrator reads the from-lane from the issue's latest `issue_change_events` status row whose
   `new_value` is the event's lane and whose `created_at` is not after the outbox row's (written in
   the same transaction by both move writers), so the K-8 line is unchanged. A deleted issue without
   a title renders its key in place of "{key} {title}" and "A Foundry member" as actor (its change
   rows are gone with it). These fallbacks are only reachable during an upgrade window.
5. **Lane filters bind lane ids (`lanes.id`), not slugs or labels.** Lane slugs are immutable but
   can be reused after a delete; ids cannot. An event's lane slug is resolved to the project's
   current lane id at dispatch.
6. **Bursts:** the dispatcher groups a source transaction's rows (`source_txid`, ADR-EIP-002) per
   route and `EventKind`; more than 5 become one delivery whose composer line is the D-11 summary.
7. **The event id** a receiver sees is the outbox `event_id` (uuid); a burst delivery carries the id
   of its first row and lists the rest (webhook schema, deliverable C).

## Consequences

- Positive: one name per event everywhere; the API, webhook schema and metric labels all read
  `EventKind::wire_name`.
- Positive: three small writer edits; no new query on the board path.
- Negative: the realtime NOTIFY payload grows by three fields (the deleted issue's title reaches
  that project's SSE subscribers, who could already see it).
- Negative: a deleted issue's title stays in `outbox` after the issue row is gone, alongside the
  `author_email` the shipped `CommentAdded` payload already carries. The outbox is not exported and
  not shown anywhere. Its personal data is bounded by the retention sweep (ADR-EIP-002 decision 9),
  which deletes dispatched rows older than 30 days that no pending delivery references.
- Follow-ups unchanged: F-1 (issue edited) becomes one enum variant plus one writer row; assignment
  needs a writer first.
