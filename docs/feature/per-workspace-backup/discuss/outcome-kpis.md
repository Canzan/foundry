# Outcome KPIs: per-workspace-backup

## Feature: per-workspace-backup (per-workspace data export, OD-5 / DD-MWT-09)

### Objective

Give self-hosting operators a trustworthy way to lift exactly one tenant's data out of a
multi-tenant Foundry instance — complete, isolation-clean, and self-verifiable — so they can archive,
migrate, hand off, or pre-deletion-snapshot a single workspace with confidence.

> Note: this is operator infrastructure tooling for a self-hosted product. KPIs are correctness- and
> capability-shaped (achievability, verifiability, leak rate) rather than consumer-funnel metrics —
> the actionable "behavior change" is the operator going from *cannot do this safely* to *does it in
> one verified session*. Per the KPI framework's granularity rule (1-3 stories -> one table suffices),
> all stories share this table.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-1 | Self-hosting operator | Extracts a single tenant's full data set end-to-end via one command | From 0% safely-achievable (manual surgery on a combined dump) to a one-command export demonstrable in a single session | No per-workspace export exists | Acceptance suite green + single-session demo (list -> export) | Leading (Outcome) |
| KPI-2 (NORTH STAR) | Operator releasing a workspace archive | Machine-confirms the export is complete AND isolation-clean before release | 100% of exports verifiable without manual inspection; 0 cross-tenant leaks pass verification (a planted sibling row always reds) | No isolation verification exists | `verify-export` exit code + the falsifiability test (plant sibling row -> non-zero) | Leading (Outcome) |
| KPI-3 | Operator under real failure conditions | Recovers from a failed export using the printed exit code + message, and never acts on a partial archive | 100% of documented failure paths exit with the specified code + actionable message; 0 partial archives pass `verify-export` | n/a (no export today) | Per-failure-path acceptance scenarios + the atomicity test | Leading (Secondary) |

### Metric Hierarchy

- **North Star**: KPI-2 — *zero cross-tenant leaks pass verification*. This is the security-critical
  reason the feature exists; a leaky export is worse than no export. Everything else is in service of
  making this provable.
- **Leading Indicators**: KPI-1 (operators can actually produce an export) predicts that KPI-2 has
  something to verify; KPI-3 (failures are recoverable) predicts operators trust and re-use the tool.
- **Guardrail Metrics** (must NOT degrade):
  - Whole-instance backup (`pg_dump` + `backup-verify`) behavior — unchanged by this feature.
  - The export is read-only: source-instance row counts before == after every export (no mutation).
  - Off-bearer surface: no `/api/v1` or web route exposes export (boundary guard stays green).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-1 | acceptance suite (`foundry-acceptance`) | green list -> export scenarios; manual single-session demo at handoff | per release / per CI run | platform-architect (DEVOPS) |
| KPI-2 | acceptance suite + falsifiability test | `verify-export` exit code on clean vs planted-leak archives | per CI run (the leak test is a permanent regression guard) | platform-architect |
| KPI-3 | acceptance suite | per-failure-path exit-code assertions + atomic-write (killed-export) test | per CI run | platform-architect |

### Hypothesis

We believe that an isolation-scoped, self-verifiable per-workspace export for self-hosting operators
will achieve the data-portability objective. We will know this is true when **operators export a
single tenant's data in one verified session (KPI-1) and `verify-export` confirms zero sibling rows
on every export while always reddening on a planted leak (KPI-2)**.

### Handoff to DEVOPS (instrumentation needs)

1. **Data collection**: acceptance-suite pass/fail for the export, completeness, isolation, and
   falsifiability scenarios; the killed-export atomicity test.
2. **Dashboards/monitoring**: none runtime (this is operator CLI tooling, not a served surface); CI
   green is the signal.
3. **Alerting thresholds**: the isolation falsifiability test is a hard CI gate — if it ever stops
   reddening on a planted leak, the isolation guarantee is broken (treat as a build break).
4. **Baseline**: none to pre-collect — the capability does not exist today; baseline is "0%
   safely-achievable".
