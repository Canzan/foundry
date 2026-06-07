# Outcome KPIs — Machine-Token Admin UX

## Feature: machine-token-admin-ux

### Objective
By the end of this feature, a workspace admin can grant, see, and revoke programmatic API
access entirely from the product — safely (the secret is shown once, never persisted) and
accountably (every grant is attributed) — with no operator env change, no redeploy, and no
Postgres console.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Workspace admins (issuer deployments) | Issue a working machine token from the product and use it against `/api/v1` | Time-to-first-working-token < 2 min; 0 token values persisted/logged | Impossible without operator env/test-key access | Mint→first-successful-API-call timing in acceptance + log/DB scan for token-value absence | Leading (Activation) |
| 2 | Workspace admins + security reviewers | Enumerate the workspace's programmatic credentials without DB access | 100% of issued (non-GC'd) tokens visible; DB-access-for-audit → 0 | 0 (no list surface) | Acceptance: listed set == issued set | Leading |
| 3 | Workspace admins + security reviewers | Shut down a credential and have it refused on its next use | Revoke→refusal within one API request | 0 (no revoke surface) | Acceptance: revoke then assert next call refused | Leading (Activation) |
| 4 | Workspace admins issuing tokens | Grant scoped, time-bounded credentials instead of workspace-wide forever-keys | Share of tokens with narrower scope or shorter expiry > 0; 0 tokens exceed the cap | 0 (defaults only at US-MT01) | Distribution of `scope_team_id`/`expires_at`; cap-enforcement scenario | Leading (Secondary) |
| 5 | Security reviewers | Attribute every credential to its issuer and judge staleness | 100% of post-feature tokens show a named issuer; reviewers identify stale tokens without DB access | 0 (no issuer column; last_used unsurfaced) | Row-level `created_by` check + list-shows-issuer/last-used scenario | Leading (Secondary) |

### Metric Hierarchy
- **North Star**: a workspace admin issues a working, listable, revocable machine token from
  the product without operator intervention (KPI 1 + 2 + 3 together — the end-to-end loop).
- **Leading Indicators**: time-to-first-working-token (KPI 1); revoke-to-refusal latency
  (KPI 3); credentials visible without DB access (KPI 2).
- **Guardrail Metrics** (must NOT degrade):
  - 0 token values ever persisted or logged (NFR-MT-SEC-01/02).
  - 0 non-admin or cross-workspace mint/list/revoke successes (NFR-MT-SEC-03).
  - SHIPPED verify-path latency unchanged (NFR-MT-PERF-01).
  - `foundry-acceptance` suite stays green; browser auth/CSRF/session contract unchanged
    (NFR-MT-SEC-07).

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| 1 Time-to-first-token | acceptance timing + structured logs (no token value) | scenario timer + log scan | per release | DEVOPS (instrumentation), QA |
| 2 Tokens visible | `machine_tokens` vs list surface | acceptance set-equality assertion | per release | QA |
| 3 Revoke→refusal | acceptance scenario | revoke then next-call assertion | per release | QA |
| 4 Scope/expiry distribution | `machine_tokens.scope_team_id`/`expires_at` | periodic aggregate query | monthly | DEVOPS |
| 5 Issuer attribution | `machine_tokens.created_by` | row-level check + list scenario | per release | QA |

### Hypothesis
We believe that giving workspace admins an in-product mint/list/revoke surface (over the
shipped signing + registry + denylist primitives) will let admins grant programmatic access
in under 2 minutes and revoke it within one request, without operator help.
We will know this is true when admins issue working tokens (KPI 1), enumerate them (KPI 2),
and revoke them with next-request effect (KPI 3) — while 0 token values are ever persisted or
logged and 0 non-admin attempts succeed.

### Handoff to DEVOPS
- **Data to instrument**: mint events (admin, workspace, scope, expiry — NEVER the value),
  revoke events, mint→first-use timing, revoke→refusal observation; `created_by` and
  `last_used_at` are already columns.
- **Guardrail alerts**: any log line or DB column carrying a token value (must be 0); any
  non-admin/cross-workspace mint-or-revoke success (must be 0); verify-path latency regression.
- **Baselines to collect**: none pre-exist (feature is net-new surface); establish KPI 1/3
  timings from the first issuer-configured deployment.
