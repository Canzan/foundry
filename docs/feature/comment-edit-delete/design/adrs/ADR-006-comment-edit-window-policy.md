# ADR-006: Comment Edit-Window Policy

## Status
Accepted — 2026-05-25

## Context

Slice 5 lights up the deferred US-10 ACs for comment edit and delete. The
US-10 AC text reads "Author can edit/delete own comments" with no time
qualifier. The implementation needs an explicit policy answer to a
question the AC leaves open: how long after posting may a comment author
edit their own comment?

Three families of policy exist in the wild:

- "Always editable" (GitHub, GitLab) — no clock dependency in
  authorization.
- "Time-windowed" (Slack default: 15 minutes) — a clock dependency in
  authorization; an `@error` scenario when the window expires; UI
  affordance must vanish when the clock crosses the threshold.
- "Until-first-reply" (forum convention) — no wall-clock dependency but
  requires a count query on every authorization check; counter-intuitive
  in a system without nested replies (slice 1 explicitly punted on
  threaded replies, so "first reply" would mean "any later comment on
  the issue").

Quality attributes driving this decision: **simplicity (HIGH)** —
slice 1 taste filter; **auditability (HIGH)** — moderation needs are
covered at the adjacent layer (Q2 soft-delete tombstone + Q4 "edited"
indicator); **reversibility (MEDIUM)** — adding a window in v0.2 is a
one-line authorization check; removing one is a breaking UX change.

## Decision

**Always editable. No time limit.** A comment author MAY edit their own
comment at any point until the comment is deleted. Workspace admins
inherit edit rights via the existing role check (though slice 5 wires
admin-edit only as a follow-on; the slice-5 PR ships author-edit and
admin-delete).

The authorization check reduces to a pure data comparison:
`comment.author_id == session.user_id`. No clock injected. No count
query. No `@error` scenario for "edit window expired".

## Alternatives Considered

### A: Always editable (chosen)
See Decision.

### B: 15-minute window
Edit allowed only within 15 minutes of `created_at`. After that, the Edit
affordance vanishes from the UI; server returns 403 on attempt.

- **Pros**: Limits revisionism; matches Slack default; forces "edit while
  you remember" UX.
- **Cons**: Introduces a clock dependency in the authorization check (the
  `Clock` port already exists on `AppState`, so the marginal cost is
  small but real). Adds an `@error` scenario: "edit window expired ->
  403". User confusion when admins/peers reference now-vanished Edit
  button. The "edited" indicator (Q4 = A) already carries revision
  awareness; the time wall is redundant audit.
- **Rejected because**: the US-10 AC carries no time qualifier; option A
  ships the smallest thing that satisfies it. B is a one-line addition
  in v0.2 if telemetry shows revisionism abuse.

### C: Until-first-reply
Edit allowed until any later comment exists on the same issue. After the
first subsequent comment, edits are locked.

- **Pros**: Captures the "don't revise after the conversation moved on"
  semantic. No wall-clock dependency.
- **Cons**: Slice 1 has no concept of replies-to-a-comment; "first reply"
  must mean "any later comment on the issue" — counter-intuitive.
  Requires a `COUNT(*) WHERE created_at > $self_created_at` on every
  authorization check (cheap with `idx_comments_issue_created` but it's
  a second query). Cannot be expressed as a static authorization rule.
- **Rejected because**: counter-intuitive semantics for a system without
  threading; adds query cost on every authz check.

## Consequences

### Positive
- Zero new dependencies (no clock injection, no count query, no
  cache-the-current-time pitfalls).
- Simplest possible authorization rule — a single `==` comparison.
- Author UX is predictable; the Edit button never silently disappears.
- The "edited" indicator (Q4 = A) carries enough revision awareness
  for honest audit without a time wall.

### Negative
- Late edits can re-write history under existing reply threads when
  threading is added in a future slice. Theoretical risk until US-??
  ships threaded replies; the "edited" indicator partially mitigates.
- No automatic friction against revisionism abuse.

### Neutral
- Adding a time window in v0.2 is a one-line authorization check
  addition + one new `@error` scenario; the schema requires no change.
- Removing a time window once shipped is a breaking UX change (users
  notice button reappearing); A avoids this trap by default.

## Verification

- An author can edit their own comment 1 hour, 1 day, 1 week after
  creation; server returns 200 with the re-rendered comment card. This
  is exercised by the slice-5 acceptance scenario for US-10 edit AC.
- A non-author cannot edit at any time; server returns 403. This is the
  existing UAT scenario.
- No `Clock` port mock or wall-clock manipulation appears in the
  slice-5 acceptance suite for the edit path (confirms zero clock
  dependency).
