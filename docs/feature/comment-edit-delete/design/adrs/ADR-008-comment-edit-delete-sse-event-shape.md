# ADR-008: Comment Edit/Delete SSE Event Shape

## Status
Accepted — 2026-05-25

## Context

Slice 5 must fan PATCH and DELETE events out to every viewer of the
issue page so the UI updates without a page reload. The existing
slice-2 SSE channel carries:

- `IssueCreated`
- `IssueUpdated`
- `CommentAdded`

The realtime-roadmap (slice 1) and `wave-decisions.md` (slice 2) lock in
the invariant: **single `event:` type per channel** (`issue_events`);
payload-discriminated `event_type`; `schema_version` field for
forward-compatible additions. The question is how the slice-5 edit and
delete events fit this shape.

Three options exist:

- **Two new event_types**: `CommentEdited` + `CommentDeleted`. Mirrors
  the `IssueCreated`/`IssueUpdated` pattern.
- **One polymorphic event_type**: `CommentMutated` with a sub-field
  (`sub_type: "edited" | "deleted"`).
- **Reuse `CommentAdded`** with payload flags (`edited: true` /
  `deleted: true`). No new event_type at all.

Quality attributes: **realtime consistency (HIGH)** — 1s p99 fanout
inherited from slice-2 NFR-PERF-03; **forward-compat (HIGH)** — new
fields/types must not break old listeners; **debuggability (MEDIUM)** —
`event_type` on the wire should make Wireshark / log analysis trivial.

## Decision

**Add two new `event_type` constants: `CommentEdited` and
`CommentDeleted`.** Each rides the existing `issue_events` channel
with the existing payload envelope.

Payload field reuse:

- `CommentEdited`: `comment_id`, `issue_id`, `workspace_id`, `key`,
  `author_id`, `author_email` (all existing fields). No new fields
  required.
- `CommentDeleted`: `comment_id`, `issue_id`, `workspace_id`, `key`
  (existing fields) + new optional field `deleted: Option<bool>` set
  to `Some(true)` so receivers that match on payload structure (rather
  than on `event_type` alone) can detect tombstones.

`EventPayload` gains:

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

`schema_version` stays at **1**. This is a forward-compatible field
addition per realtime-roadmap invariant 4 (never rename a field; bump
schema_version only on incompatible changes). Old listeners that have
not been recompiled ignore the new field and the two new `event_type`
values fall through the default match arm (logged but not fatal).

## Alternatives Considered

### A: Two new event_types (chosen)
See Decision.

### B: Single polymorphic `CommentMutated` with `sub_type`
One new event_type. Sub-field discriminates between edit and delete.

- **Pros**: Fewer new strings to spell-check across server + SSE
  consumer.
- **Cons**: Adds a SECOND discriminator next to `event_type` — the
  existing convention is one discriminator. Receiver still needs a
  two-arm match on `sub_type`. Forward-compat suffers: if slice-6 adds
  `CommentReactionAdded`, does it polymorph under `CommentMutated` too?
  The pattern doesn't compose.
- **Rejected because**: introduces a second discriminator without
  compensating benefit; doesn't compose for future comment-related
  events.

### C: Reuse `CommentAdded` with payload flags
Re-fire `CommentAdded` with `{edited: true}` or `{deleted: true}`.

- **Pros**: No new `event_type` at all.
- **Cons**: Breaks the "event_type tells you what happened" property.
  Receivers that filter by `event_type == "CommentAdded"` would now
  process edits/deletes too — surprising and bug-prone. The wire is no
  longer self-documenting.
- **Rejected on principle**: violates the slice-2 invariant that
  `event_type` carries behavioural intent.

## Consequences

### Positive
- Matches the established `IssueCreated`/`IssueUpdated` pattern. Mei
  reads the `EventPayload` enum and sees a coherent family of
  comment-event types.
- Cleaner receiver dispatch — the existing match in
  `crates/foundry-app/src/events.rs` gets two new arms, not a nested
  switch.
- htmx OOB-swap target differs naturally between events: edit swaps
  `outerHTML` of the comment card with the re-rendered card; delete
  removes the card from the DOM. Two distinct event_types make the
  receiver's render branches obvious.
- `event_type` on the wire makes log-grep and Wireshark debugging
  trivial. "Did the server emit a CommentEdited?" is a textual search.
- `schema_version` stays at 1; no downstream consumer (replicas not yet
  upgraded) breaks.

### Negative
- Two new string constants instead of one. Slightly larger receiver
  match. Both are O(few LOC) changes.
- One new optional `EventPayload` field (`deleted`). Negligible
  serialization cost; `#[serde(default)]` keeps it free for events that
  don't set it.

### Neutral
- Forward-compat behaviour: old listeners encountering the new
  `event_type` values fall through to the default match arm. The
  default arm currently logs and continues; slice 5 confirms this is
  the desired behaviour (not fatal). If slice 5 deploys to a partially
  upgraded fleet during a rolling upgrade, mismatched replicas will
  silently miss edit/delete fanout to the viewers they serve — acceptable
  for a slice-5 rollout because the rolling-upgrade window is short.

## Verification

- The `EventPayload` serde round-trip for both new `event_type` values
  preserves all fields. A unit test in `foundry-realtime` exercises
  serialization symmetry.
- An acceptance scenario asserts that a viewer SSE stream receives a
  payload with `event_type == "CommentEdited"` after the author submits
  a PATCH, and the payload carries the expected `comment_id` matching
  the edited row.
- An acceptance scenario asserts that a viewer SSE stream receives a
  payload with `event_type == "CommentDeleted"` and `deleted ==
  Some(true)` after the author submits a DELETE.
- The forward-compat assertion: a synthetic `EventPayload` JSON with
  the new `deleted` field present deserializes successfully via the
  slice-4 deserializer (`#[serde(default)]` provides the field for old
  binaries; the new field is silently ignored by old listeners). Unit
  test in `foundry-realtime`.
