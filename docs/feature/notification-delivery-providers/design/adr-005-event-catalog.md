# ADR-005: Notification catalog shape + (non-)alignment with the realtime EventPayload

## Status
Accepted (DESIGN, Propose mode). Resolves **ODD-6**.

## Context
The `event` metric label must stay bounded (NFR-4, BR-7, R6). The notification catalog is the set of event
types call sites can emit. The house has a forward-compatible event envelope precedent — the realtime
`EventPayload` (`crates/foundry-realtime/src/lib.rs:66-105`) — but it is **stringly-typed**
(`event_type: String`, `:68`) for forward-compat over an in-process SSE broadcast bus. ODD-6 asks: model the
notification catalog as a bounded Rust enum or a stringly-typed discriminator, and align it with the realtime
model or keep them distinct? Two new v1 events (`member_removed`, `password_changed`, FR-9) must be modeled.

The two models solve different problems: the SSE bus has **no cardinality constraint** (a new `event_type`
string is free); the notification `event` is a **Prometheus label with a hard cardinality bound** (a new value
is a new time series axis that a fail-closed test guards, ADR-004).

## Decision
**A closed Rust enum `NotificationEvent` — NOT stringly-typed, and NOT aligned with `EventPayload`:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    PasswordReset,      // signin.rs:235
    WorkspaceInvite,    // bootstrap.rs:258
    MemberInvite,       // member_invites.rs:189
    MemberRemoved,      // NEW (US-06)
    PasswordChanged,    // NEW (US-06)
}
impl NotificationEvent {
    pub fn as_str(&self) -> &'static str { /* "password_reset" | "workspace_invite" | "member_invite"
                                              | "member_removed" | "password_changed" */ }
}
```
Adding an event = one variant + one `as_str()` arm — an explicit, reviewable catalog addition (BR-7) that keeps
the `event` metric label **compile-time bounded**. The `as_str()` domain is exactly the label domain the
cardinality test asserts (ADR-004).

We **mirror the forward-compat *discipline*** of `EventPayload` (a struct carrying a discriminator + payload,
never rename a field — add) via the `Notification` envelope (ADR-001), but with a **closed enum** discriminator
instead of an open `String`. We **do not align the two catalogs**: notification delivery and SSE broadcast are
distinct concerns (delivery to external transports with a metric-cardinality bound vs in-process fan-out to
browser subscribers with none). Aligning them would force one of two bad outcomes: adopt `String` (unbounding
the metric label — violates ADR-004/ADR-011, R6) or bound the SSE model (an unrelated constraint on a
different subsystem).

## Alternatives Considered
- **Stringly-typed `event: String`, mirroring `EventPayload.event_type`** — REJECTED. It unbounds the `event`
  metric label — the exact cardinality blow-up R6 warns of — and defeats the fail-closed cardinality test
  (there is no closed domain to assert). The SSE model tolerates open strings because it has no label; the
  notification metric cannot.
- **Share the `EventPayload` type / a single unified event model across realtime + notifications** — REJECTED.
  They are different bounded concerns; `EventPayload` carries SSE-specific fields (`project_id`, `issue_id`,
  `schema_version`, `comment_id`, ...) irrelevant to delivery, and delivery needs a bounded discriminator SSE
  does not. Coupling them would drag SSE-shaped fields into the notification envelope and vice-versa, and a
  change in one subsystem's catalog would ripple into the other. Keep them distinct; reuse only the *pattern*.
- **A catalog registry / config-driven event list** — REJECTED. A runtime-extensible event set cannot be
  bounded at compile time (BR-7) and would need its own validation; the closed enum is the simplest thing that
  keeps the label bounded and makes "add an event" a one-line reviewed change.
- **Model the two new events as free-form now, enum-ify later** — REJECTED. That ships an unbounded label into
  v1 and pays the migration cost twice; the enum is cheap from slice 01.

## Consequences
- Positive: the `event` label is compile-time bounded (BR-7, R6); the cardinality test has a finite domain to
  assert (ADR-004); "add an event" is a reviewed one-liner + an emit call, with zero transport change (US-06,
  AC-06.4).
- Positive: the two subsystems stay decoupled — a realtime `event_type` change never touches the notification
  catalog and vice-versa.
- Negative: a call site emitting a genuinely-new event must add a variant first (cannot pass an arbitrary
  string) — intended friction (BR-7); it is what keeps the label bounded.
- Negative: the two catalogs can drift (an event meaningful to both subsystems must be added in both) —
  accepted; they are distinct concerns and the duplication is a handful of variants.
- Probe (Earned Trust): `member_removed` + `password_changed` flow end-to-end through the notifier and appear
  as bounded `event` label values in the delivery metric; a test emitting an out-of-domain value fails the
  bounded-label @property (AC-06.2/06.5).
