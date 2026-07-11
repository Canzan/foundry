# Upstream Changes — recipient-notification-preferences (DESIGN wave)

> Adjustments DESIGN makes to DISCUSS assumptions. DISCUSS artifacts are **not** modified; this file records
> deltas + rationale (verbatim quote → resolution).

## None to a DISCUSS decision — one honest reaffirmation + two clarifications

DISCUSS deliberately left the genuine architecture choices open as ODD-1..7 and made no claim DESIGN
contradicts. No DISCUSS FR/NFR/BR is weakened. The three notes below sharpen, not overturn.

### 1. Reaffirmed: `Notification` carries no `workspace_id` — resolved by threading, not by refuting
DISCUSS (requirements.md, ODD-3): *"the `Notification` struct carries no `workspace_id` today
(`notify.rs:117-122`), so a notifier-side suppression hook requires threading workspace context."* Verified
true. **Resolution (ADR-003):** add `workspace_id: Option<Uuid>` to `Notification`; both suppressible emit
sites already have it in scope (verified: `bootstrap.rs:226` `user.workspace_id`; `member_invites.rs` admin
workspace), so the threading is additive and mechanical. No DISCUSS assumption changes.

### 2. Clarification: the token reuses the `sign`/`verify` PRIMITIVES, not the `InviteToken` STRUCT
DISCUSS (requirements.md brownfield table + ODD-1) cites *"`InviteToken` … The exact model for
`UnsubscribeToken`."* On verification, `InviteToken` (`foundry-auth/src/lib.rs:354-390`) binds a database
`invite_id` PK + `expires_at` and uses the DB row as the single-use control. The unsubscribe token has no
pre-issued row and (by ADR-001) no expiry. **Clarification:** "model" is honored at the primitive level —
the constant-time `foundry_auth::sign`/`verify` (`:251,260`) are reused directly; a sibling
`UnsubscribeToken` is added beside `InviteToken`. This is a sharpening of "model," not a contradiction.

### 3. Clarification: NFR-4 permits "at most `workspace`" on the suppression metric; DESIGN omits it
DISCUSS (NFR-4): *"Labels carry `event` … and at most `workspace`."* **Clarification (ADR-005):** DESIGN uses
`event` only and **omits** `workspace` — `workspace_id` is unbounded cardinality + semi-identifying, in
tension with the shipped bounded-label discipline. This stays strictly within the NFR ("at most" — zero
workspace labels satisfies it) while honoring the PII-free + bounded intent more strictly.

## Predecessor lineage (unchanged)
This feature is the named successor `notification-delivery-providers` carved recipient preferences out to
(`docs/evolution/2026-07-11-notification-delivery-providers.md`). It **extends** that pipeline additively:
one new `Notification` field, one dispatch gate, one sibling counter, one migration — the shipped
`NotificationProvider` port, the infallible fan-out, the delivery counter, and all four adapters are
untouched. No change is owed to the predecessor's design or its platform-architect handoff.
