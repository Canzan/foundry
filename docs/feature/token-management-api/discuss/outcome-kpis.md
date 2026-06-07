# Outcome KPIs — Token-Management API

## Feature: token-management-api

### Objective
Within one quarter of release, integrators and automation manage machine-token credentials
PROGRAMMATICALLY (inventory + revoke + rotate) over `/api/v1` — with zero browser/DB access and a
bounded escalation surface — replacing the human-UI-only lifecycle for machine callers.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | security-automation / integrator pipelines | pull the token registry as JSON instead of scraping HTML or querying the DB | 100% of inventory pulls succeed via `/api/v1`, 0 browser/DB access | 0 (no JSON surface) | token-list API success rate + audit-pull count | Leading |
| 2 | rotation jobs / incident runbooks | revoke a credential programmatically and confirm it dead | revoke-to-refusal latency = 1 request; 100% verifiable in the automation log | 0 (human-UI only) | revoke API success + "next call 401" assertion + revoke→refusal latency | Leading |
| 3 | scheduled rotation jobs | complete a rotation hands-free, retiring the old credential themselves | 100% of rotations leave zero live leftover credentials, no human retire step | 0 (human retire step) | rotation-job success rate + "old credential refused after self-revoke" | Leading |
| 4 | integrators building SDKs/automation | branch on stable `error.code`s instead of parsing prose | 100% of token-route refusals carry a stable code + conventional status; integrator breakage = 0 | N/A (new routes) | per-route contract assertion + reported breakage | Leading (secondary) |
| 5 | hostile caller (negative) / security reviewer (assurance) | fail to enumerate / escalate / run away | 100% adversarial calls refuse non-enumerably; 0 bearer mint routes; bursts throttled | N/A | adversarial suite + route-surface assertion + rate-guardrail metric | Guardrail |

### Metric Hierarchy
- **North Star**: programmatic management adoption — share of machine-token lifecycle actions
  (list/revoke/rotate) performed via `/api/v1` rather than the human UI or DB.
- **Leading Indicators**: token-list API pulls (KPI 1); programmatic revokes incl. self (KPI 2, 3).
- **Guardrail Metrics (must NOT degrade)**:
  - **Escalation surface**: 0 bearer-reachable mint routes in v1 (KPI 5 / NFR-TMA-SEC-08).
  - **Non-enumerability**: 100% of cross-workspace/non-management refusals reveal no existence.
  - **Verify-path latency**: the SHIPPED per-request bearer verify path unchanged (NFR-TMA-PERF-01).
  - **Management-mutation rate**: per-principal revoke rate within the SEC-07 cap (no revoke storm).
  - **No secret ever on a read path / in a log** (NFR-TMA-SEC-01/02).

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| 1 | `/api/v1/.../tokens` GET metrics | request-tracking layer (`http_requests_total{path,method,status}`, SHIPPED) | weekly | platform-architect |
| 2 | revoke route + verify-path metrics | revoke success counter + "next-call-401" acceptance assertion | per release + weekly | platform-architect |
| 3 | rotation-job logs / acceptance | "old credential refused after self-revoke" assertion | per release | acceptance-designer |
| 4 | per-route contract assertions | acceptance suite | per release | acceptance-designer |
| 5 | adversarial suite + rate-guardrail metric | route-surface assertion + per-principal mutation-rate metric | per release + continuous | platform-architect |

### Hypothesis
We believe that a JSON LIST + REVOKE (incl. revoke-self) surface over `/api/v1`, with MINT held
human-session-only, for integrators and automation will achieve hands-free inventory and rotation
with a bounded escalation surface. We will know this is true when security-automation and rotation
jobs perform 100% of inventory + revoke + rotate actions via `/api/v1` with zero browser/DB access,
while 0 bearer-reachable mint routes exist and 100% of adversarial calls refuse non-enumerably.

### Handoff to DEVOPS (platform-architect)
1. **Data collection**: instrument token-route requests (reuse the SHIPPED request-tracking layer);
   add a per-principal management-mutation-rate metric.
2. **Dashboards**: programmatic-management adoption (list/revoke/rotate via API vs UI); revoke→refusal
   latency.
3. **Alerting thresholds**: the SEC-07 per-principal mutation-rate cap (revoke storm); any
   bearer-reachable mint route appearing (must stay 0 in v1).
4. **Baselines**: KPI 1-3 baselines are 0 (no programmatic surface today) — no pre-collection needed.
