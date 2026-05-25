# Coverage Matrix — Slice 3 (operator-grade)

Acceptance-Criterion-to-Scenario trace for US-02, US-03, US-04, US-11.
Companion to `wave-decisions.md` and `step-skeletons.md`. Walking-
skeleton scenarios marked WS. NFR mappings cross-reference
`docs/feature/foundry-backend-mvp/discuss/nfrs.md`.

## US-02 — Operator scales to multiple replicas (7 scenarios)

| Acceptance Criterion (from `stories.md`) | Scenario | Tag(s) | NFR |
|---|---|---|---|
| AC1: No sticky-session requirement: any cookie validates on any replica | "A member's session is recognised by every replica regardless of which one the load balancer routes her to" | WS, `@walking_skeleton @real-io @driving_adapter` | NFR-AVAIL-01 |
| (AC1 extension): cross-replica issue write is visible to a subscriber on a different replica within the realtime budget | "A member files an issue on one replica and another member observes it through a different replica within two seconds" | `@real-io` | NFR-PERF-03 |
| AC3: SSE reconnect resumes user-visible updates within 15 seconds of replica failure | "A member's SSE stream auto-reconnects to a healthy replica when its landing replica is stopped" | `@real-io @nfr-avail-03` | NFR-AVAIL-03 |
| AC2: `/readyz` returns 503 within 10 seconds of losing Postgres connectivity | "All replicas flip /readyz to 503 within ten seconds when Postgres becomes unreachable, and the load balancer removes them from rotation" | `@real-io @error @nfr-obs-02` | NFR-OBS-02 |
| (AC related to NFR-AVAIL-02): graceful shutdown drains in <=15s | "A replica receiving SIGTERM finishes in-flight requests and flips to draining within the grace window" | `@real-io @nfr-avail-02` | NFR-AVAIL-02 |
| (NFR-PERF-04 trace): per-replica pool stays <= 10 connections | "Per-replica connection pool stays below the configured ceiling under sustained traffic across all replicas" | `@real-io @nfr-perf-04` | NFR-PERF-04 |
| AC4: Three replicas + 1 LB is the documented production reference topology | "The production-shaped Caddy + 3-replica docker-compose stack serves the session-survives-replica-switch scenario" | `@docker-compose @us-02 @real-io @manual-trigger` | NFR-AVAIL-01, NFR-PORT-01 |

**Deferred** (recorded in `wave-decisions.md`):
- Rolling-restart preserves SSE subscriptions (US-02 UAT scenario 4) — partially covered by the auto-reconnect scenario above; full multi-replica-restart sequence is `@manual` for slice 3 and may graduate to automation when `@docker-compose` lane stabilises.
- K8s manifest end-to-end via kind/k3d — deferred to a future `@k8s` slice.

## US-03 — Operator backs up and restores (6 scenarios)

| Acceptance Criterion (from `stories.md`) | Scenario | Tag(s) | NFR |
|---|---|---|---|
| AC2: A `pg_restore` on the dump alone, on a fresh Postgres of the same major version, produces an identical functional system | "The operator restores a freshly-dumped backup on a clean Postgres and finds every workspace, issue, and attachment intact" | WS, `@walking_skeleton @real-io @driving_adapter` | NFR-DATA-02 |
| (NFR-DATA-02 trace, attachment round-trip): bytea survives dump+restore byte-identically | "Attachment bytea round-trips through pg_dump and pg_restore byte-identically" | `@real-io` | NFR-DATA-02 |
| (US-03 implicit AC, sequential keys after restore): issue keys continue from source | "Sequential issue keys continue from where the source instance left off after a restore" | `@real-io` | — |
| AC1: No Foundry state lives outside the Postgres database (verifiable by checking the running container has no `/data` volume) | "No Foundry state lives outside Postgres — the dump file alone reproduces the system" | `@real-io @nfr-data-01` | NFR-DATA-01 |
| AC3: `foundry doctor backup-verify` is provided and exits non-zero on integrity violations (happy path) | "The `foundry doctor backup-verify` CLI subcommand reports row counts and exits zero on a healthy dump" | `@real-io @driving_adapter @us-03-cli` | — |
| AC3: same, sad path | "The `foundry doctor backup-verify` CLI subcommand reports failure and exits non-zero on a truncated dump" | `@real-io @driving_adapter @us-03-cli @error` | — |

**Deferred**:
- Cross-major-version restore (PG15 dump on PG16) — operator-responsibility per `backup-restore.md`; not auto-tested.
- Continuous backup / WAL archiving — post-MVP per `backup-restore.md`.

## US-04 — Operator upgrades in place (4 scenarios)

| Acceptance Criterion (from `stories.md`) | Scenario | Tag(s) | NFR |
|---|---|---|---|
| AC2: Multi-replica startup uses Postgres advisory locks to serialize migration runs (concurrent-race happy path) | "Two replicas race to apply a new migration; the advisory lock serialises them and the migration is applied exactly once" | WS, `@walking_skeleton @real-io @driving_adapter @nfr-mig-01` | NFR-MIG-01 |
| AC1: Migrations are forward-only SQL files; idempotent re-application | "A second startup of the same replica is a no-op — the migration is not re-applied" | `@real-io @nfr-mig-01` | NFR-MIG-01 |
| AC3: Failed migrations roll back; no partial schema state | "A migration that fails rolls back and leaves the schema unchanged; the replica exits non-zero" | `@real-io @error @nfr-mig-02` | NFR-MIG-02 |
| (AC2 extension — explicit blocking semantics): the racing replica blocks on the advisory lock and proceeds when released | "A replica racing for the migration lock blocks until the holder releases, then proceeds without error" | `@real-io @nfr-mig-01` | NFR-MIG-01 |

**Deferred** (recorded in `wave-decisions.md` §US-04 approach decision):
- AC4: Release notes contain a migration impact summary for every release — process discipline, not acceptance-testable; covered by release-notes review checklist (NFR-MIG-03).
- "Old replica keeps serving with old SQL while new schema is being applied" — expand-only discipline enforced by per-migration header comments + code review, not black-box test.
- `CREATE INDEX CONCURRENTLY` non-transactional path — covered by `migrations.md` runbook with manual-recovery procedure.

## US-11 — User attaches a file to an issue (7 scenarios)

| Acceptance Criterion (from `stories.md`) | Scenario | Tag(s) | NFR |
|---|---|---|---|
| AC1: Attachments stored as bytea in `issue_attachments` table (happy path upload + download round-trip) | "A member attaches a screenshot to an issue and a teammate downloads it with a matching sha256" | WS, `@walking_skeleton @real-io @driving_adapter` | — |
| AC1 / AC3 (size approaching cap): under-cap large file still succeeds | "A 9-megabyte attachment under the configured cap uploads successfully" | `@real-io` | NFR-PERF-02 |
| AC2: `FILE_UPLOAD_MAX_MB` env var controls per-file cap (oversize is rejected) | "An attachment above the configured cap is refused with HTTP 413 and no row is created" | `@real-io @error @nfr-perf-02` | NFR-PERF-02 |
| (NFR-SEC-06 trace): non-member cannot upload | "A workspace member outside the team cannot attach files to that team's issues" | `@real-io @error @nfr-sec-06` | NFR-SEC-06 |
| (NFR-SEC-06 trace): non-member cannot download | "A workspace member outside the team cannot download attachments from that team's issues" | `@real-io @error @nfr-sec-06` | NFR-SEC-06 |
| (Auth contract): unauthenticated upload refused | "An unauthenticated request to upload an attachment is refused" | `@real-io @error` | NFR-SEC-06 |
| AC4: CASCADE delete with parent issue | "Deleting the parent issue cascades to delete its attachments" | `@real-io` | — |

**Deferred / covered by implicit slice-1+2 wiring**:
- AC5 (Content-Type sniffed and stored; sent on download) — covered by the WS scenario's Content-Type header assertion + the round-trip preservation assertion; no dedicated scenario to avoid duplication.
- AC3 (Download streams from Postgres bytea; no separate object store in MVP) — process-level invariant, not acceptance-testable beyond what the WS already proves.

## Slice 3 totals

| Story | Scenarios | WS | Error/NFR | Story AC coverage |
|---|---:|---:|---:|---|
| US-02 | 7 | 1 | 3 | 4/4 ACs + 2 NFR scenarios |
| US-03 | 6 | 1 | 1 | 3/3 ACs + 3 supporting scenarios |
| US-04 | 4 | 1 | 1 | 3/4 ACs (AC4 process-only, deferred) |
| US-11 | 7 | 1 | 4 | 5/5 ACs |
| **Total** | **24** | **4** | **9 (37.5%)** | — |

## NFR coverage check (slice 3 only — slice 1 + 2 NFRs unchanged)

| NFR | Covered by slice-3 scenario? | Scenario |
|---|---|---|
| NFR-PERF-02 (10 MB default cap) | YES | US-11 oversize-rejection + 9-MB-under-cap |
| NFR-PERF-04 (per-replica pool size) | YES | US-02 pool-ceiling |
| NFR-OBS-02 (/healthz vs /readyz; 503 on DB outage) | YES | US-02 readyz-on-DB-outage |
| NFR-AVAIL-01 (multi-replica + no sticky) | YES | US-02 session-survives-switch (WS) + Caddy variant |
| NFR-AVAIL-02 (graceful shutdown) | YES | US-02 SIGTERM scenario |
| NFR-AVAIL-03 (SSE reconnect) | YES | US-02 SSE reconnect scenario |
| NFR-MIG-01 (forward-only + advisory lock + idempotent) | YES | All 4 US-04 scenarios |
| NFR-MIG-02 (failure rollback) | YES | US-04 failed-migration scenario |
| NFR-DATA-01 (all state in Postgres) | YES | US-03 no-state-outside-pg scenario |
| NFR-DATA-02 (pg_dump completeness) | YES | US-03 WS + bytea-round-trip |
| NFR-SEC-06 (authorization checks at every endpoint) | YES | US-11 non-member upload + non-member download |
| NFR-PORT-01 (no host-only assumptions) | PARTIAL | US-02 Caddy variant; full K8s coverage is `@k8s` deferred |
| NFR-MIG-03 (migration impact in release notes) | OUT — process discipline | — |
