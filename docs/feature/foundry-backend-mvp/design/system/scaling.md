# Scaling and Capacity Estimation — Foundry Backend MVP

## Audience

Operators sizing their first VPS and platform-architects choosing the v0.2 Helm-chart defaults. The goal is to put real numbers behind "fits comfortably on a small box and scales to a 200-person org without re-architecting."

## Reference workload

We size against two workloads. The 20-person team is the MVP target (per the JTBD and recommendation.md's "5-50 person teams"); the 200-person team is the v1.0 ceiling we design *not to break*. Anything beyond 200 is explicitly out of scope.

| Workload axis | 20-person team | 200-person team |
|---------------|----------------|-----------------|
| Total members | 20 | 200 |
| Peak concurrent active users | 10 (50% online) | 80 (40% online) |
| Active SSE connections | 10 | 80 |
| Page views per active user per active hour | 30 | 30 |
| Issue mutations per active user per active hour | 6 | 6 |
| Attachments uploaded per day | 30 | 300 |
| Median attachment size | 500 KB | 500 KB |
| Issue corpus after 1 year | 5,000 | 50,000 |
| Comments per issue (avg, 1-year average) | 4 | 4 |

These numbers come from extrapolating Linear and GitHub usage data for similar team sizes. They are intentionally generous on the active-user fraction (50%/40%) so the resulting capacity has headroom.

## Derived QPS

Both write and read QPS derive directly from the workload table.

### 20-person team

```
Page-view QPS  = 10 concurrent users * 30 pages/hr / 3600
              ~= 0.083 requests/sec averaged, ~5 req/min averaged
Peak (3x burst) ~= 15 req/min == 0.25 req/sec
                   = "noise" by web-server standards

Issue mutation QPS
              = 10 * 6 / 3600 ~= 0.017 mutations/sec
Peak (3x)      = 0.05 mutations/sec
                = "~3 mutations/min at peak"

SSE event QPS at fan-out:
   pg_notify rate from writes ~= 0.05/sec (same as mutation rate)
   per-replica fan-out: each replica multiplies by its local SSE subscriber count.
   With 10 SSE subscribers split across 3 replicas (~3-4 each),
   each LISTEN-wake delivers to ~3-4 subscribers.
   Total work: ~0.15 SSE messages/sec across the fleet.
   = "negligible"
```

### 200-person team

```
Page-view QPS  = 80 * 30 / 3600 ~= 0.67 req/sec averaged
Peak (3x)      ~= 2 req/sec
                = "still well below 1000 RPS server budget"

Issue mutation QPS = 80 * 6 / 3600 ~= 0.13/sec
Peak (3x)          ~= 0.4/sec

SSE event QPS at fan-out:
   ~0.4 pg_notify/sec * 80 subscribers
   = 32 SSE messages/sec across the fleet
   (each LISTEN wake on each replica delivers to its share)
   = "easy"
```

**Conclusion**: Foundry's working set is tiny by web-server standards. CPU is not the constraint; per-request DB latency and the SSE LISTEN connection are. A single replica handles the 200-person workload with comfortable headroom; multi-replica is for HA, not load.

## Storage estimation

### 20-person team after 1 year

```
Issues:        5,000 rows * ~2 KB each (title + description avg)   = 10 MB
Comments:      20,000 rows * ~500 B each                            = 10 MB
Attachments:   ~10,000 attachments (30/day * 365)
              * 500 KB avg                                          = 5 GB
Sessions:      ~50 active rows * 1 KB                              = trivial
Outbox / event log: capped at 100k rows * 200 B                    = 20 MB

Total per year: ~5 GB (attachments dominate by 500x)
5-year projection: ~25 GB
```

### 200-person team after 1 year

```
Issues:        50,000 * 2 KB                                       = 100 MB
Comments:      200,000 * 500 B                                     = 100 MB
Attachments:   100,000 * 500 KB                                    = 50 GB
Outbox + sessions                                                  = ~50 MB

Total per year: ~50 GB
5-year projection: ~250 GB
```

This number is the headline planning constraint. The bytea-in-Postgres decision (NFR-DATA-01) makes attachments dominate storage and *also dominate `pg_dump` size*. A 200-person 5-year deployment produces a 250 GB dump — slow to take, slow to restore, but still mechanically tractable. See `backup-restore.md` for the RTO/RPO implications.

The 100-MB-per-attachment hard cap (NFR-PERF-02) is what keeps Postgres bytea + TOAST honest. Above that we'd recommend an S3 backend (deferred to post-MVP).

## RAM, CPU, and connection-pool sizing

### Per-replica steady-state baseline

| Resource | Per-replica estimate | Where the number comes from |
|----------|---------------------|------------------------------|
| Resident RAM | 80-150 MB | Rust binaries are small; tokio runtime + sqlx pool + askama templates compiled in |
| Peak RAM (under load + 1 attachment streaming) | +50 MB headroom | bytea reads stream, but multipart parsing buffers up to `FILE_UPLOAD_MAX_MB` per concurrent upload |
| CPU steady-state | 1-3% of one core | At 0.25 req/sec the runtime is mostly idle |
| CPU peak (200-person workload) | 5-15% of one core | Driven by template rendering + markdown -> HTML |
| Postgres pool: app requests | 10 connections | `DATABASE_MAX_CONNECTIONS=10` (NFR-PERF-04) |
| Postgres pool: LISTEN connection | 1 dedicated connection | Kept outside the request pool, never returned (see `realtime-infrastructure.md`) |
| File descriptors | ~50 base + 1 per SSE client | Bump the container `ulimit` to 4096 if more than ~500 concurrent SSE clients |

A single replica fits in 256 MB RAM and 1 vCPU with room. Three replicas + Postgres + Caddy fit in 2 GB RAM total, comfortably below a $10/month VPS.

### Postgres sizing

| Resource | 20-person | 200-person |
|----------|-----------|------------|
| `shared_buffers` (Postgres) | 256 MB | 1 GB |
| `effective_cache_size` | 512 MB | 2 GB |
| `max_connections` | 50 (5 replicas * 11 each = 55, room for psql + tools) | 100 (default) |
| Disk IOPS sustained | trivial (<100) | <500 |
| Disk space (5 yr) | 25 GB | 250 GB |
| RAM total | 1-2 GB | 4-8 GB |

The 200-person workload sits comfortably inside a single Postgres on a $40/month VPS. The next bottleneck — and it is firmly post-MVP — is the single-Postgres SPOF (see ADR-105), not capacity.

## Multi-replica scaling thresholds

Operators should switch from Variant 1 (single replica) to Variant 2 (multi-replica) for one of three reasons, not for raw CPU/RAM:

1. **Zero-downtime upgrades become a real requirement** (US-04, NFR-AVAIL-02) — the single-replica variant can't roll without dropping all SSE clients.
2. **Single-replica failure is unacceptable** — the 20-person workload survives a 5-minute restart; the 200-person workload doesn't.
3. **Postgres CPU saturated during scheduled backups** — `pg_dump -Fc` of a 250 GB DB stresses the single Postgres for 15-30 minutes; the app replica is unaffected but Postgres responsiveness drops. Multi-replica doesn't help (single Postgres still the bottleneck), but it lets some replicas keep serving cached data while others wait on slow queries.

For all three triggers, 2-3 replicas is the right starting size. 5+ replicas indicates either (a) the Postgres SPOF needs to be addressed (HA Postgres, see ADR-105) or (b) you've exceeded the 200-person design ceiling and need a re-architecture pass.

## Scaling ladder (Foundry-specific)

Following the canonical "scaling ladder" pattern but pruned to what this workload actually needs:

| Rung | Trigger | Action | Cost |
|------|---------|--------|------|
| 0 — single replica + Postgres | <20 users, no HA requirement | Variant 1 | $10/mo VPS |
| 1 — multi-replica + Postgres | HA + rolling upgrades | Variant 2 with Caddy LB | +$0 (same host) or +$10/mo (separate LB host) |
| 2 — Postgres tuning | Postgres CPU >50% sustained | bump `shared_buffers`, add SSD, raise `max_connections` | $0 (config change) |
| 3 — HA Postgres | Postgres SPOF unacceptable for business continuity | Patroni or `pg_auto_failover` (ADR-105) | +1 Postgres instance host |
| 4 — Read replicas | Read QPS >1000/sec sustained | Postgres streaming replication; route GETs to replica | +1 read host per replica |
| 5 — Object store for attachments | Storage >500 GB or upload-cap >100 MB | Add S3/minio backend, deprecate bytea | object-store cost |

**Critical**: rungs 3-5 are explicitly out of MVP scope and not pre-built. The architecture allows each step but does not implement it. Building rung-3 features now would break the "boring monolith" promise (recommendation.md).

## Cross-references

- Deploy topology: `topology.md`.
- Postgres pool details and LISTEN connection: `realtime-infrastructure.md`.
- Backup size implications: `backup-restore.md`.
- HA Postgres path: ADR-105.
- Failure modes per scaling rung: `failure-modes.md`.
