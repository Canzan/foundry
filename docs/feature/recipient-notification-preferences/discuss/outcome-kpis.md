# Outcome KPIs — recipient-notification-preferences (v1 = recipient unsubscribe)

## Feature: recipient-notification-preferences

### Objective
Within this feature's slices, let a notification recipient — often an account-less invitee identified only by
email — stop a workspace's suppressible notifications (`workspace_invite`, `member_invite`) with one click from
the email, keyed per `(email_lower, workspace_id)`, **safely** (a tampered/prefetched link is inert and leaks
no existence; security-critical events are never suppressed) and **observably** (opt-out volume is visible with
no PII) — and let account holders review per-workspace status and resubscribe.

### Baseline (today)
**No recipient opt-out of any kind exists.** Every notification — including the two invite reminders — is
delivered to every configured provider; a recipient's only lever is a blunt inbox filter that would also bury a
password-reset. Many recipients are invitees with **no account**, so no account-gated screen would reach them.
There is no unsubscribe link, no unsubscribe state, no suppression, and no suppression metric. This feature adds
the first opt-out mechanism (and the first store migration since the delivery-provider work, `0014`).

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-1 | Recipients getting unwanted workspace invites | stop a specific workspace's invitation emails from the email itself | 100% of confirmed unsubscribes result in the next matching suppressible notification being suppressed (0 leaks); a muted workspace does not affect another | 0% (no opt-out exists today) | suppression count vs post-unsubscribe suppressible emissions to unsubscribed pairs, dogfood run | Leading (Activation) |
| KPI-2 | Recipients who have unsubscribed | still receive 100% of security-critical notifications | 0 mandatory events (`password_reset`, `password_changed`, `member_removed`) suppressed, across every unsubscribe configuration | N/A today (no suppression exists) | count of mandatory events with `outcome=suppressed` (must be 0) + never-suppress `@property` | Guardrail |
| KPI-3 | Every recipient + would-be attacker of the unsubscribe endpoint | are protected from enumeration and prefetch/silent unsubscribes | 0 differential responses between a real and a non-existent address; 0 state changes from a GET prefetch | N/A today (no public unsubscribe endpoint) | response-equality litmus (real vs fake) + prefetch-safety litmus, acceptance suite (revert-reds-it) | Guardrail |
| KPI-4 | Account-holding members | review per-workspace status and self-serve resubscribe | 100% of a member's workspaces show an accurate Subscribed/Muted status; 100% of resubscribes restore delivery; 0 cross-recipient views/mutations | 0 (no status view, no resubscribe today) | page-render correctness vs the unsubscribe table + post-resubscribe delivery check + least-privilege test | Leading (Secondary) |
| KPI-5 | Operators / compliance | observe opt-out volume and confirm suppression is enforced, without seeing who | 100% of suppressed deliveries counted by event; 0 recipient-PII occurrences in metrics or logs | 0 (no suppression metric today) | suppression count vs actual suppressions + no-PII-in-metrics `@property` | Leading (Secondary) |
| KPI-6 | Everyone whose delivery is unaffected by opt-out | sees byte-for-byte unchanged behaviour (subscribed recipients + all mandatory events) | 100% of existing delivery scenarios unchanged with an empty unsubscribe table; the filter only removes a suppressible delivery for an unsubscribed pair | current shipped delivery behaviour | existing delivery acceptance suite passes unchanged with the filter present + empty table | Guardrail |

### Metric Hierarchy
- **North Star**: KPI-1 — **opt-out actually works**: a confirmed unsubscribe suppresses the next matching
  suppressible notification, per workspace, with zero leaks. The whole point: a recipient can silence the noise
  they didn't ask for, from the email, without an account.
- **Leading Indicators**: KPI-4 (account holders self-serve status + resubscribe) and KPI-5 (opt-out volume is
  observable) expand and expose the surface KPI-1 measures.
- **Guardrail Metrics**: KPI-2 (mandatory never suppressed), KPI-3 (non-enumerable + prefetch-safe), and KPI-6
  (backwards-compat) must NOT degrade — they are the safety, security, and regression invariants, not
  optimization targets. A regression in any of the three blocks release.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| KPI-1 | suppression count + delivery logs | suppression count vs suppressible emissions to unsubscribed pairs, dogfood | per-slice dogfood, then weekly | platform-architect (DEVOPS) |
| KPI-2 | acceptance suite + suppression metric | never-suppress `@property` (revert-reds-it) — pass/fail gate; mandatory `outcome=suppressed` must be 0 | every CI run | DELIVER |
| KPI-3 | acceptance suite | response-equality + prefetch-safety `@property` litmuses — pass/fail gate | every CI run | DELIVER |
| KPI-4 | acceptance suite + page render | status correctness vs table + post-resubscribe delivery + least-privilege scope test | per-slice (US-05/06) | DISCUSS/DELIVER |
| KPI-5 | `/metrics` + logs | suppression count vs actual + no-PII `@property` (grep scrape/logs) | every CI run + per-slice dogfood | DELIVER + DEVOPS |
| KPI-6 | existing delivery acceptance suite | run with the filter present + empty table; behaviour unchanged | every CI run | DELIVER |

> Instrumentation note for DEVOPS: KPI-1/KPI-5 are served by extending the shipped delivery counter
> `foundry_notification_deliveries_total` with a `suppressed` outcome (or a sibling
> `foundry_notification_suppressions_total{event}` — ODD-5) on the existing `/metrics` sidecar
> (`metrics_server.rs:66`), bounded-label, **PII-free** (no recipient email/token in any label). No new
> dashboard infra is required — it rides the existing Prometheus scrape. KPI-2/KPI-3/KPI-6 are enforced as CI
> gates (acceptance-suite `@property` litmuses), not runtime dashboards. A guardrail alert on a spike in
> `outcome=suppressed` for a provider/event is a DEVOPS follow-up.

### Hypothesis
We believe that a per-workspace, email-keyed unsubscribe — a signed link in the two suppressible emails, a
non-enumerable/prefetch-safe public route, a bounded suppression allow-list, and a signed-in status/resubscribe
page — will let recipients (including account-less invitees) silence workspace invitation noise without losing
security-critical mail, and let account holders manage their own subscriptions.
We will know this is true when 100% of confirmed unsubscribes suppress the next matching suppressible
notification (KPI-1), while 0 mandatory events are ever suppressed (KPI-2), the public link leaks no existence
and never mutates on a prefetch (KPI-3), and no recipient PII ever appears in the suppression metric (KPI-5).
</content>
