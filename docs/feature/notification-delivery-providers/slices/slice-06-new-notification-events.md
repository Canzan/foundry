# Slice 06 — two new notification event types as first consumers

**Goal**: add a couple of new notification event types to the bounded catalog and emit them through the
abstraction → a developer emits `notify(Notification::member_removed("maria.santos@acme.example", "Northwind"))`
from the remove-member handler and Maria is notified through every configured channel, with `event=member_removed`
counted per provider — and no transport code in the handler.
**Story**: US-06.

**IN scope**
- Add **`member_removed`** (tell a person they were removed from a workspace) and **`password_changed`** (tell
  a user their password changed) to the **bounded notification catalog** (BR-7).
- Emit each from its relevant handler via a single `state.notifier.notify(...)` call — no transport plumbing at
  the call site (JOB-2).
- Each new event **fans out to all active providers** and is **counted with its own bounded `event` label**
  exactly like the existing notifications (NFR-3, NFR-4); the `event` label domain stays bounded (a cardinality
  test fails closed on an unbounded value).
- Acceptance: member_removed-through-channels, password_changed-to-owner, new-event-fans-out-and-isolates;
  dogfooded by triggering a member removal + a password change and watching the delivery counter + log lines.

**OUT of scope**: a large event catalog (only TWO new events); recipient preferences for these events (successor
feature); rich templating of the event content; realtime SSE alignment beyond what ODD-6 decides.

**Learning hypothesis**: disproves "adding a person-facing notification is just a bounded-catalog entry + one
emit call, with the delivery pipeline (fan-out, isolation, bounded metric) already carrying it" if defining a
new event type forces changes to the fan-out/metric machinery (e.g. the `event` label can't stay bounded, or
the catalog can't be extended without touching every provider), or if the catalog can't mirror the house
`EventPayload` forward-compat envelope cleanly (ODD-6).

**Seams**: the fan-out + isolation + metric machinery (slice 03); the `NotificationProvider` port + notifier
(slice 01); the realtime `EventPayload` forward-compat envelope pattern to mirror
(`crates/foundry-realtime/src/lib.rs:66-105`, `event_type` `:68`) — note this is a distinct concern from the
SSE handler (`events.rs`); the relevant handlers where the events originate (member-removal + password-change).
**Dependencies**: US-03 (fan-out delivers whatever event flows through). DESIGN ODD-6 (catalog shape /
realtime alignment).
**Effort**: <1 day (two catalog entries + two emit calls; no transport work — new content over a proven pipeline).
