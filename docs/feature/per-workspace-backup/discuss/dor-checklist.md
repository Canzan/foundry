# Definition of Ready — per-workspace-backup

9-item hard gate. Each story passes all items with evidence before DESIGN handoff.

## US-PWB-01: Export one workspace's data

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "Devansh cannot lift one tenant's data out; the whole-instance pg_dump mixes every workspace." No tech prescription. |
| 2. User/persona with specific characteristics | PASS | Self-hosting operator with host shell access; motivated to archive/migrate/hand-off one tenant. |
| 3. 3+ domain examples with real data | PASS | Globex (7 members, 412 issues) by slug; Acme by id; single-tenant pre-migration snapshot. |
| 4. UAT in Given/When/Then (3-7) | PASS | 5 scenarios. |
| 5. AC derived from UAT | PASS | 6 AC tracing to the 5 scenarios. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 2-3 days, 5 scenarios; reuses shipped scaffold. |
| 7. Technical notes: constraints/dependencies | PASS | Reuses `admin_cli.rs` scaffold + Store seam; depends on shipped scoping seam + TENANT_TABLES. |
| 8. Dependencies resolved or tracked | PASS | Depends on multi-workspace-tenancy slices 1-5 (all SHIPPED). |
| 9. Outcome KPIs with measurable targets | PASS | KPI-1: one-command export demonstrable in a single session. |

### DoR Status: PASSED

## US-PWB-02: Isolation + verification (the crux)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "A cross-tenant leak in an export is a data-breach incident; Devansh has no way to confirm isolation today." |
| 2. User/persona with specific characteristics | PASS | Operator about to release a workspace archive to a third party. |
| 3. 3+ domain examples with real data | PASS | Clean Globex archive verifies; transitive Globex comment scope; planted Acme row caught. |
| 4. UAT in Given/When/Then (3-7) | PASS | 5 scenarios (incl. 1 @property). |
| 5. AC derived from UAT | PASS | 6 AC incl. the falsifiability bite. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 2-3 days, 5 scenarios. |
| 7. Technical notes: constraints/dependencies | PASS | Selection predicate == isolation predicate; falsifiability mirrors slice-05; OD-PWB-1 users rule flagged. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-PWB-01; OD-PWB-1 tracked as open decision with recommendation. |
| 9. Outcome KPIs with measurable targets | PASS | KPI-2 (north star): 0 cross-tenant leaks pass verification; planted leak always reds. |

### DoR Status: PASSED

## US-PWB-03: Failure paths & safety

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "A partial archive that passes for complete is worse than an obvious failure." |
| 2. User/persona with specific characteristics | PASS | Operator under real conditions: fat-fingered args, full disks, cron exit-code branching. |
| 3. 3+ domain examples with real data | PASS | Typo'd `globx`; disk fills mid Globex export; single-tenant + sensitivity note. |
| 4. UAT in Given/When/Then (3-7) | PASS | 6 scenarios. |
| 5. AC derived from UAT | PASS | 6 AC tracing to the 6 scenarios. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 2 days, 6 scenarios; decorates existing commands. |
| 7. Technical notes: constraints/dependencies | PASS | Exit-code contract mirrors admin_cli.rs; atomic `.partial`->rename; sensitivity disclosure. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-PWB-01, US-PWB-02. |
| 9. Outcome KPIs with measurable targets | PASS | KPI-3: 100% documented failure paths exit specified code; 0 partial archives pass verify. |

### DoR Status: PASSED

## Feature-level gate

| Check | Status |
|-------|--------|
| All 3 stories PASS the 9-item DoR | PASS |
| Every story has a job_id | PASS (`od-5-per-workspace-export` on all 3) |
| Every non-@infrastructure story has an Elevator Pitch | PASS (all 3 have Before/After/Decision) |
| Walking skeleton identified | PASS (US-PWB-01 + US-PWB-02) |
| Security NFRs captured (isolation crux) | PASS (NFR-PWB-ISO-01 + KPI-2 north star) |
| Open decisions carry a recommended option | PASS (OD-PWB-1/2/3) |

### Feature DoR Status: PASSED — ready for reviewer gate
