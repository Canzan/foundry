# ADR-003: Suppression hook location + workspace threading + fail stance (ODD-3)

## Status
Accepted — 2026-07-11 (Morgan, DESIGN wave). Feature-local. **The crux ADR.**

## Context
FR-3 requires a suppressible notification to an unsubscribed `(email, workspace)` to be **suppressed before
`deliver()`**, for **every** provider. FR-4/NFR-3 require the three mandatory events to be **always**
delivered. The shipped `Notifier::notify` (`notify.rs:237`) is **INFALLIBLE** and **await-bounded** (a
provider error/timeout/panic is contained; the request is never failed or stalled). The shipped
`Notification` (`notify.rs:117-122`) carries **no `workspace_id`**. Both suppressible emit sites already have
workspace context in scope (`bootstrap.rs:226` `user.workspace_id`; `member_invites.rs` admin workspace).
R5 warns that a store lookup inside `notify()` could stall or fail the infallible path.

## Decision
1. Add `workspace_id: Option<Uuid>` to `Notification`. Suppressible emits set `Some(ws)`; mandatory emits set
   `None`.
2. Add `NotificationEvent::is_suppressible()` → true **only** for `{WorkspaceInvite, MemberInvite}`.
3. Introduce a **driven port** `SuppressionPolicy { async fn is_suppressed(&self, email_lower: &str, ws:
   Uuid) -> Result<bool, SuppressionError>; }`. `Notifier` holds `Arc<dyn SuppressionPolicy>`.
4. Add ONE guard at the **top of `notify()`**, before the shipped fan-out:
   - `!is_suppressible()` → skip the check entirely (mandatory events never reach a lookup).
   - `Some(ws)` → `is_suppressed(recipient.to_ascii_lowercase(), ws)` **bounded by a short timeout**:
     `Ok(true)` ⇒ increment `foundry_notification_suppressions_total{event}` and **early-return** (no
     provider is invoked); `Ok(false)` ⇒ fall through; `Err`/timeout ⇒ **fail-open** (log `warn`, deliver).
5. Composition-root default is `AllowAllSuppression` (`Ok(false)`); production wires `StoreSuppression`
   (an indexed point-read on the `0014` PK).

## Alternatives Considered
- **Filter at each emit site** (option ii) — rejected: the rule would be duplicated across sites and a new
  suppressible emit site could forget it. The DISCUSS alternative analysis chose a single enforcement point
  "so the rule is enforced once for all providers and can't be forgotten."
- **Filter inside each provider adapter** — rejected: N places to forget, and it wastes the fan-out; a
  suppression is provider-independent (suppress = deliver to nobody), so it belongs above the fan-out.
- **Fail-closed on a lookup error** (suppress when the store errors) — rejected: it would silently drop
  invites during a store blip — a worse regression than an occasional un-honored opt-out, and worse than
  today's behaviour (R5: "no worse than today"). Fail-open is chosen. Note fail-open **cannot** endanger a
  mandatory event: mandatory events never reach the lookup, so the safety invariant is independent of the
  fail stance. (Contrast `member_invites.rs:255` `find_user_by_email` which fails **closed** — there the
  concern is enumeration, a different direction.)
- **Notifier depends on `Store` directly** — rejected: violates dependency inversion and couples the domain
  dispatcher to persistence; the `SuppressionPolicy` port keeps `notify()` testable with a fake and inert by
  default (NFR-7).
- **Spawn-detached lookup** — rejected: `notify()` must remain await-bounded so the shipped synchronous
  delivery assertions hold; the bounded-timeout lookup preserves that.

## Consequences
- **Positive**: one enforcement point; dependency inversion preserved; **NFR-3 structural** (the
  `is_suppressible()` gate means mandatory events — the allow-list complement — are never checked, provable
  by an allow-list unit test + a never-suppress `@property`); **infallible + await-bounded preserved**
  (error swallowed, timeout-bounded); **NFR-7 exact** (inert `AllowAllSuppression` + early-return-only-on-true
  ⇒ byte-for-byte unchanged fan-out with an empty table).
- **Negative / accepted**: `Notification` gains one optional field (every construction site sets it — a
  trivial mechanical change at 5 emit sites + tests); the suppression lookup adds one indexed point-read to
  the emit path for suppressible events only (bounded, fail-open).
