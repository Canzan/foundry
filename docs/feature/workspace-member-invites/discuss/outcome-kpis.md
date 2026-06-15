# Outcome KPIs — workspace-member-invites

## Feature: workspace-member-invites

### Objective
Within one release, let any workspace admin bring teammates into their workspace self-serve — so a
person with no Foundry account can go from an invite link to a signed-in member in one step, safely,
without involving the instance operator.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-1 | Invitees with no prior Foundry account | complete the accept flow — account created, joined as member, signed in on the workspace | 90%+ of issued member invites result in a signed-in member landing within 7 days | 0% (no member-accept path that creates an account exists today) | ratio of invites consumed-with-account-and-session to invites issued | Leading (Activation) |
| KPI-2 | Anyone probing the issuance surface or opening a non-live/colliding accept link (legit or hostile) | receives a uniform, non-enumerable refusal | 100% byte-identical refusals across {non-admin issuance, signed-out issuance, expired, used, tampered, unknown-id, email-collision} | undefined today (no route) | refusal-arm byte-identity litmus in the acceptance suite | Guardrail |
| KPI-3 | Any member invite | creates exactly one account and is consumed at most once | 0 successful double-creates / double-consumes under concurrency | undefined today | single-use + single-create @property concurrency test | Guardrail |
| KPI-4 | Invitees who hit a password validation error | recover and complete the join on the SAME invite | 80%+ complete without a re-issue | 0% (no flow) | join-completion rate among sessions with a recorded password error | Leading (Secondary) |
| KPI-5 | Workspace admins | issue member invites self-serve (without an operator/IT ticket) | 95%+ of admin issuance attempts produce a valid emitted link | 0% (no admin issuance surface) | ratio of successful invite-row-creations to admin issuance POSTs | Leading (Activation) |

### Metric Hierarchy
- **North Star**: KPI-1 — member-invite activation rate (issued invite -> signed-in member landing). The
  whole point: an invited teammate who actually becomes a working member.
- **Leading Indicators**: KPI-5 (admins can issue at all) is upstream of KPI-1 — no invites, no
  activations. KPI-4 (recovery-from-error completion) predicts KPI-1 by removing a drop-off cause.
- **Guardrail Metrics**: KPI-2 (non-enumerability, both surfaces) and KPI-3 (single-use + single-create
  integrity) must NOT degrade — they are security invariants, not optimization targets. A regression in
  either blocks release.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| KPI-1 | `invites` table (issued vs consumed-with-account-and-session) + issuance/accept telemetry | query: consumed-with-account-and-session / issued, trailing 7d | weekly | platform-architect (DEVOPS) |
| KPI-2 | acceptance suite | byte-identity litmus (revert-reds-it), both surfaces — pass/fail gate | every CI run | DELIVER |
| KPI-3 | acceptance suite | single-use + single-create @property concurrency test — pass/fail gate | every CI run | DELIVER |
| KPI-4 | accept-flow request telemetry (password-error event -> join-completed event, same invite) | funnel ratio | weekly | platform-architect (DEVOPS) |
| KPI-5 | issuance request telemetry (admin POST -> invite-row created) | success ratio | weekly | platform-architect (DEVOPS) |

> Instrumentation note for DEVOPS: KPI-1/KPI-4/KPI-5 need lightweight telemetry distinguishing "invite
> issued", "accept page viewed", "password validation error", "account created + joined", and "accept
> completed (session established)" — keyed by `invite_id` ONLY (never `sig`, never password — NFR-5).
> KPI-2/KPI-3 are enforced as CI gates, not runtime dashboards.

### Hypothesis
We believe that a self-serve member-invite flow (admin-gated issuance -> verify -> set-password ->
atomic create-user + member-membership + consume -> auto sign-in) will let workspace admins onboard
teammates without operator involvement.
We will know this is true when 90%+ of issued member invites result in a signed-in member landing within
7 days, while refusals stay 100% non-enumerable on both surfaces and no invite ever creates a second
account or is consumed twice.
