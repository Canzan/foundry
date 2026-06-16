# Prioritization: per-workspace-backup

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|---------------|-----|-----------|
| 1 | Walking Skeleton (R1) | Operator produces a complete, isolation-clean single-tenant archive end-to-end | KPI-1, KPI-2 | Validates the riskiest assumption: a transitive-FK scoped export can be both complete AND provably leak-free. Isolation is the security reason the feature exists, so it ships in slice 1. |
| 2 | Failure-path hardening (R2) | Every failure exits with a documented code + actionable message; no partial archive masquerades as complete; sensitive contents flagged | KPI-3 | Turns a demo into a dependable operator tool. Strictly depends on R1's commands existing. |
| — | Per-workspace RESTORE (Won't-Have v1) | — | — | Deferred by DD-MWT-09 (sibling-clobber risk). Tracked as the follow-up feature, not in scope. |

## Backlog (scored: Value x Urgency / Effort, 1-5 scale)

| Story | Release | Value | Urgency | Effort | Score | Priority | Outcome Link | Dependencies |
|-------|---------|-------|---------|--------|-------|----------|--------------|--------------|
| US-PWB-01 Scoped export happy-path (list + export + per-table counts) | WS/R1 | 5 | 5 | 3 | 8.3 | P1 | KPI-1 | None (reuses shipped `foundry doctor` scaffold + Store seam) |
| US-PWB-02 Isolation: only this tenant + verify-export completeness & isolation | WS/R1 | 5 | 5 | 3 | 8.3 | P1 (tie; crux) | KPI-2 | US-PWB-01 |
| US-PWB-03 Failure-path & safety hardening (unknown ws, output errors, atomic partial-write, sole-ws, truncation, sensitivity note) | R2 | 4 | 3 | 2 | 6.0 | P2 | KPI-3 | US-PWB-01, US-PWB-02 |

Tie-break (per user-story-mapping skill: Walking Skeleton > Riskiest Assumption > Highest Value):
US-PWB-01 and US-PWB-02 are both in the skeleton; US-PWB-02 carries the riskiest assumption
(provable isolation across transitive FKs), so within the skeleton it is the one to prove cannot be
faked. They are sequenced 01 then 02 because 02's verify needs 01's export to exist.

## MoSCoW

| Story | MoSCoW | Note |
|-------|--------|------|
| US-PWB-01 | Must | No feature value without a scoped export. |
| US-PWB-02 | Must | A non-isolated export is a security liability — must ship with the export. |
| US-PWB-03 | Should | High operator value; the happy path is demonstrable without it, so it is a fast-follow within v1. |
| Per-workspace restore | Won't (v1) | Deferred — DD-MWT-09. |

## Risk register (surfaced, not managed — handed to DESIGN)

| Risk | Type | Probability | Impact | Mitigation |
|------|------|-------------|--------|------------|
| Sibling-data leak in an export (isolation breach) | Technical/Security | Low | High | The crux invariant: export selection predicate == verify isolation predicate; falsifiability test plants a sibling row and asserts verify REDS (mirrors slice-05 discipline). |
| Incomplete export (a tenant table silently omitted) | Technical | Medium | High | Single `tenant_tables_set` constant shared by export + verify; completeness test plants a row per table. |
| `users` scoping ambiguity (multi-membership users belong to >1 workspace) | Technical | Medium | Medium | OPEN DECISION OD-PWB-1 — recommend exporting membership edges + users who are members of THIS workspace; DESIGN ratifies the exact rule. |
| At-rest exposure of password hashes / token rows in the archive | Security | Medium | Medium | Operator-trust artifact; CLI prints a sensitivity note; at-rest protection is the operator's responsibility (same posture as the whole-instance dump). |
| Tenant-table set drifts as schema evolves | Technical | Medium | Medium | OD-PWB-2 — DESIGN decides derive-from-schema vs pin-and-test; pin+test mirrors the shipped gold-test discipline. |

> In Phase 2.5 the scores were estimated from discovery; refined in Phase 4 after `outcome-kpis.md`.
