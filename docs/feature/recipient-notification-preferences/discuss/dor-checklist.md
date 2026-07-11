# Definition of Ready — recipient-notification-preferences (v1 = recipient unsubscribe)

9-item hard gate. Each item must PASS with evidence before DESIGN handoff.

## US-01 — Stop a workspace's invite emails with one click (Walking Skeleton)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Sam is an invitee with no account; repeat `workspace_invite` reminders arrive and he can't turn them off — no opt-out exists, and an account-gated screen wouldn't reach him." |
| 2 | User/persona with specific characteristics | PASS | A notification recipient identified only by email (invitee or member), invite email in hand, no account required. |
| 3 | 3+ domain examples with real data | PASS | Sam unsubscribes from Northwind → next Northwind invite suppressed; also invited to Contoso → still delivers; clicks twice → idempotent no-op. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (one-click-stops, one-workspace-untouched, twice-is-no-op). |
| 5 | AC derived from UAT | PASS | AC-01.1..01.6, traced to FR-1/2/3/9 + NFR-1/7 + BR-2/8. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | Token + route + `0014` table + suppression filter + one re-routed emit; 3 scenarios; ~1.5 days (carries the abstraction uncertainty). |
| 7 | Technical notes: constraints/dependencies | PASS | `UnsubscribeToken` (InviteToken model, `foundry-auth/src/lib.rs:354-390`); new `0014` table (`reset_tokens` shape); suppression hook ODD-3; token/expiry ODD-1; GET-safety ODD-2; table/keying ODD-4. |
| 8 | Dependencies resolved or tracked | PASS | Reuses shipped seams; adds one migration. ODD-1/2/3/4 flagged as DESIGN inputs. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (100% of confirmed unsubscribes suppress the next matching notification), baseline 0%. |

**US-01 DoR: PASSED**

## US-02 — Never lose a security-critical notification, even when unsubscribed

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "An opt-out that could swallow a password-reset or removal notice is a safety hazard and would scare recipients away from using it." |
| 2 | User/persona with specific characteristics | PASS | The same recipient who unsubscribed (Sam) + Ops/Compliance Olivia guaranteeing security mail is never withheld. |
| 3 | 3+ domain examples with real data | PASS | Unsubscribed Sam gets a password reset; gets a `member_removed`; gets a `password_changed` — all deliver, none suppressed. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (reset-reaches, removal-reaches, no-mandatory-ever-suppressed) + never-suppress `@property`. |
| 5 | AC derived from UAT | PASS | AC-02.1..02.5, traced to FR-4 + NFR-3 + BR-1/3. |
| 6 | Right-sized | PASS | A bounded allow-list + a litmus; mandatory events skip the check; 3 scenarios; ~0.5 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Enforced at the single suppression point (ODD-3); mandatory events skip the unsubscribe lookup; precedence BR-3. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (the filter it constrains). No new persistence. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-2 (0 mandatory events suppressed — hard guardrail). |

**US-02 DoR: PASSED**

## US-03 — A tampered or unknown link is safely refused without leaking who exists

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "A public token endpoint must not leak existence via differential responses, and must not mutate state on a prefetch." |
| 2 | User/persona with specific characteristics | PASS | Every recipient (protected) + Malicious Mallory (the adversary the design defeats). |
| 3 | 3+ domain examples with real data | PASS | Mallory alters Sam's token → refusal; real vs fake address → identical response; scanner prefetches Sam's real link → no row written. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (tamper-refused, no-existence-leak, prefetch-safe) — the non-enumerable + prefetch `@property`. |
| 5 | AC derived from UAT | PASS | AC-03.1..03.5, traced to FR-5 + NFR-1/2/4 + BR-4. |
| 6 | Right-sized | PASS | Reuses the shipped uniform refusal (`invites_accept.rs:332-339`) + constant-time verify (`foundry-auth/src/lib.rs:260`); 3 scenarios; ~1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Uniform refusal + constant-time verify reuse; GET-safety / RFC 8058 one-click stance ODD-2. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (the token + route it hardens). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-3 (0 differential responses, 0 prefetch state changes — guardrail). |

**US-03 DoR: PASSED**

## US-04 — The same one-click unsubscribe works for member-invite emails

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "An opt-out that covered only `workspace_invite` would still leave `member_invite` reminders arriving after unsubscribe — a half-measure." |
| 2 | User/persona with specific characteristics | PASS | A recipient getting `member_invite` reminders (an admin keeps re-adding/re-inviting them). |
| 3 | 3+ domain examples with real data | PASS | Sam (unsubscribed from Northwind) re-added → `member_invite` suppressed; first contact is a `member_invite` → its link works; `member_removed` still delivers. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (one-opt-out-covers-both, member-invite-carries-link, security-still-delivers). |
| 5 | AC derived from UAT | PASS | AC-04.1..04.5, traced to FR-1/3/4 + NFR-3/7 + BR-1/2. |
| 6 | Right-sized | PASS | Attach the same link to `member_invites.rs:204` + add the event to the allow-list; reuses all of US-01; 3 scenarios; ~0.5 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses US-01 token/table/route/filter; no new token/table/route. Closes the v1 boundary. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 + US-02. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (100% of `member_invite` to an unsubscribed pair suppressed; both events covered by one opt-out). |

**US-04 DoR: PASSED**

## US-05 — See my per-workspace notification status when signed in

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Once a recipient unsubscribes via an email link, there is no way for an account holder to review that state — Maria can't tell what she muted." |
| 2 | User/persona with specific characteristics | PASS | An account-holding member of one or more workspaces, signed in, wants to review her per-workspace status. |
| 3 | 3+ domain examples with real data | PASS | Maria (Northwind muted, Contoso/Initech subscribed) sees accurate status; all-subscribed case; a request naming another user returns only her scope. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (shows-status, own-workspaces-only, cannot-steer-to-another). |
| 5 | AC derived from UAT | PASS | AC-05.1..05.5, traced to FR-6 + NFR-6/8 + BR-6/7. |
| 6 | Right-sized | PASS | A read-only authed page over the unsubscribe table + membership lookups; 3 scenarios; ~1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | New authed route beside `/account/password` (`lib.rs:415-418`); session identity (`SessionUser`); membership + `find_user_by_email`; a11y NFR-8; multi-workspace ODD-7. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (the state it reads). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-4 (100% accurate per-workspace status; 0 cross-recipient rows). |

**US-05 DoR: PASSED**

## US-06 — Resubscribe a workspace I previously muted

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Unsubscribe is currently one-way; an account holder who changes her mind has no path back to subscribed." |
| 2 | User/persona with specific characteristics | PASS | An account-holding member viewing her settings page, wants to undo a previous mute. |
| 3 | 3+ domain examples with real data | PASS | Maria resubscribes Northwind → delivers again; resubscribes already-subscribed Contoso → no-op; forged CSRF POST → 403. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (restores-delivery, idempotent, csrf-rejected). |
| 5 | AC derived from UAT | PASS | AC-06.1..06.5, traced to FR-3/7 + NFR-5/6 + BR-5/6/8. |
| 6 | Right-sized | PASS | One CSRF-protected POST clearing a row + re-render; 3 scenarios; ~0.5–1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | CSRF-protected POST (`csrf.rs:137`); session-scoped identity (NFR-6); account-less resubscribe ODD-6. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-05 + US-01. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-4 (100% of resubscribes restore delivery; 0 cross-recipient/CSRF-forged changes). |

**US-06 DoR: PASSED**

## US-07 — See how much notification opt-out is happening, without exposing who

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Unsubscribe that can't be observed can't be trusted/reported on — but observability that leaks who unsubscribed is itself a privacy problem." |
| 2 | User/persona with specific characteristics | PASS | Ops/Compliance Olivia, watches `/metrics`, needs opt-out volume + proof of enforcement, must never see PII. |
| 3 | 3+ domain examples with real data | PASS | 34 `workspace_invite` + 8 `member_invite` suppressions counted by event; grep `/metrics`+logs → no email/token; mandatory suppressed series always 0. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (visible-count, no-PII, mandatory-never-suppressed) — the no-PII `@property`. |
| 5 | AC derived from UAT | PASS | AC-07.1..07.5, traced to FR-8 + NFR-3/4. |
| 6 | Right-sized | PASS | Extend the shipped delivery counter with a `suppressed` outcome (or sibling), bounded-label; 3 scenarios; ~0.5 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Extends `foundry_notification_deliveries_total` (`notify.rs:39,291-297`); `suppressed` outcome vs sibling counter ODD-5; bounded-label discipline. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (the suppression it counts). No new persistence. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-5 (100% of suppressions counted; 0 recipient-PII occurrences). |

**US-07 DoR: PASSED**

---

## Overall DoR: PASSED (pending peer-review gate)

All seven stories pass all 9 items. The open decisions below are **DESIGN-wave inputs**, not DoR blockers —
requirements are written solution-neutrally and each decision is explicitly tracked in `wave-decisions.md`:

- **ODD-1** `UnsubscribeToken` shape + **expiry stance** (an unsubscribe link arguably should not expire, or
  expire long — unlike `InviteToken`'s short expiry).
- **ODD-2** **GET-safety / one-click stance** — RFC 8058 `List-Unsubscribe-Post` one-click POST vs a
  GET→confirm→POST; and whether to emit `List-Unsubscribe` / `List-Unsubscribe-Post` email headers.
- **ODD-3** **Where the suppression filter hooks** — inside `Notifier::notify` (which needs `workspace_id`
  added to `Notification`, it carries none today, `notify.rs:117-122`) vs at the emit sites; and the fail
  stance for a suppression-lookup error in the INFALLIBLE `notify()`.
- **ODD-4** **State table shape + email keying** — `(email_lower, workspace_id, unsubscribed_at)` uniqueness,
  case-normalisation matching `find_user_by_email(email_lower)`, and how account-less invitee emails reconcile
  with account emails.
- **ODD-5** **Suppression metric contract** — a `suppressed` outcome on `foundry_notification_deliveries_total`
  vs a sibling `foundry_notification_suppressions_total{event}`; label set (no recipient PII).
- **ODD-6** **Resubscribe UX for account-less recipients** — a token-based resubscribe / an undo on the
  confirmation page for recipients who can't sign in.
- **ODD-7** **Multi-workspace interaction** — how per-workspace unsubscribe presents for an email that belongs
  to several workspaces (the settings page lists all the member's workspaces; muting one doesn't affect
  another).

### Peer review (nw-product-owner-reviewer) — gate

**Result (2026-07-11, iteration 1/2): approved** — `critical_issues_count: 0`, `high_issues_count: 0`. All
four hard gates PASS (DoR 9/9 × 7 stories; JTBD traceability; Dimension 0 Elevator Pitch on all 7; every slice
user-visible); zero LeanUX anti-patterns; security NFRs measurable + regression-guarded. **Handoff to DESIGN:
CLEARED.**

Run via Task before `*handoff-design`. Verdict also recorded in `wave-decisions.md`.
Dimension 0 (Elevator Pitch) self-check: every story has an `### Elevator Pitch` with Before/After/Decision,
each "After" anchored to a **real user-invocable entry point** (the unsubscribe link + `/unsubscribe` route,
`GET /account/notifications`, `POST /account/notifications/resubscribe`, `GET /metrics`) plus a **concrete
observable output** (a confirmation page, a suppressed next-invite, a uniform refusal page, a status list, a
`suppressed` metric series) — no internal-only entry points. JTBD traceability: every story carries a `job_id`
(`stop-unwanted-workspace-notifications`, `manage-my-notification-subscription`, or
`honor-unsubscribe-and-preserve-security`); none `infrastructure-only`. No LeanUX anti-patterns (real personas
+ data — Sam Okafor / Maria Santos / Olivia / Mallory; outcome-focused AC; right-sized slices). **DoR gate:
PASSED pending the peer-review pass.**
</content>
