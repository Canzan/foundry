# Outcome KPIs — invite-accept-flow

## Feature: invite-accept-flow

### Objective
Within one release, make the provisioned first-admin invite link actually work — so every super-admin
who provisions a workspace produces an admin who can get in, safely, without operator hand-holding.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-1 | Provisioned first-admins | complete the accept flow and land signed-in on their workspace | 90%+ of issued first-admin invites result in a signed-in landing within 7 days | 0% (link is dead — no first-admin can sign in via it) | ratio of invites consumed-with-session to invites issued | Leading (Activation) |
| KPI-2 | Anyone opening a non-live link (legit or hostile) | receives a uniform, non-enumerable refusal | 100% byte-identical refusal body+status across {expired, used, tampered, unknown-id} | undefined today (no route); bootstrap claim flow IS enumerable | refusal-arm byte-identity litmus in the acceptance suite | Guardrail |
| KPI-3 | Any invite | is consumed at most once | 0 successful double-consumes under concurrency | undefined today | single-use @property concurrency test | Guardrail |
| KPI-4 | First-admins who hit a password validation error | recover and complete accept on the SAME invite | 80%+ complete without a re-issue | 0% (no flow) | accept-completion rate among sessions with a recorded password error | Leading (Secondary) |

### Metric Hierarchy
- **North Star**: KPI-1 — first-admin activation rate (issued invite -> signed-in landing). This is the
  feature's whole reason to exist: a provisioned admin who can actually get in.
- **Leading Indicators**: KPI-4 (recovery-from-error completion) predicts KPI-1 by removing a drop-off cause.
- **Guardrail Metrics**: KPI-2 (non-enumerability) and KPI-3 (single-use integrity) must NOT degrade —
  they are security invariants, not optimization targets. A regression in either blocks release.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| KPI-1 | `invites` table (issued vs consumed-with-session) + provisioning telemetry | query: consumed-with-session / issued, trailing 7d | weekly | platform-architect (DEVOPS) |
| KPI-2 | acceptance suite | byte-identity litmus (revert-reds-it) — pass/fail gate | every CI run | DELIVER |
| KPI-3 | acceptance suite | @property concurrency test — pass/fail gate | every CI run | DELIVER |
| KPI-4 | accept-flow request telemetry (error event -> completion event, same invite) | funnel ratio | weekly | platform-architect (DEVOPS) |

> Instrumentation note for DEVOPS: KPI-1 and KPI-4 need lightweight telemetry distinguishing
> "invite issued", "accept page viewed", "password validation error", and "accept completed (session
> established)" — keyed by `invite_id` ONLY (never `sig`, never password — NFR-5). KPI-2/KPI-3 are
> enforced as CI gates, not runtime dashboards.

### Hypothesis
We believe that a working `/invites/accept` flow (verify -> set-password -> atomic single-use consume ->
auto sign-in) for provisioned first-admins will achieve a high first-admin activation rate.
We will know this is true when 90%+ of issued first-admin invites result in a signed-in landing within
7 days, while refusals stay 100% non-enumerable and no invite is ever consumed twice.
