# Failure Modes and Resilience Playbook

## Audience

Operators on-call for Foundry, and reviewers stress-testing the architecture. Every failure listed here has been traced to its root substrate fact and matched with a mitigation that lives in the MVP scope (not vaporware).

## Single Points of Failure inventory

| Component | SPOF? | Mitigation in MVP | Path to remove (deferred) |
|-----------|-------|-------------------|---------------------------|
| Postgres | **YES** | Documented + accepted (ADR-105) | HA Postgres via Patroni / pg_auto_failover (v0.4+) |
| Foundry app replicas | No (multi-replica from Variant 2) | Caddy round-robin removes any failed replica from rotation | N/A — solved |
| Load balancer (Caddy) | Yes for single-LB deploys | Documented; operators with HA needs run 2 Caddy instances behind DNS round-robin or use a cloud LB | Active-passive Caddy pair (post-MVP) |
| `.env` file with `SESSION_SECRET` | Yes (lose it = all bootstrap/invite tokens invalid; sessions remain because they're in Postgres) | Documented backup procedure | K8s `Secret` (no infra change in Foundry) |
| Single VM host (Variant 1) | Yes (single host = everything dies together) | Operator's choice — explicit in `topology.md` | Multi-replica + external Postgres on separate host |

The Postgres SPOF is the largest accepted risk in the MVP. ADR-105 documents why we accept it and what the v0.4 path looks like.

## Per-failure runbook

For each failure mode below: the *trigger*, the *user-visible symptom*, the *detection signal*, the *automatic mitigation* (if any), and the *operator action*.

### Failure 1 — Postgres goes down

- **Trigger**: Postgres crashes, OOMs, disk fills, host loses network.
- **User-visible symptom**: every request returns 503 (after a brief pool-exhaustion period of 5xx); SSE clients see no new events.
- **Detection signal**:
  - `/readyz` flips to 503 within 10 s on every replica (NFR-OBS-02).
  - `db_connections_in_use` drops to 0; `db_connection_wait_seconds` p95 spikes.
  - `realtime_listen_disconnects_total` increments.
- **Automatic mitigation**: LB removes all replicas from rotation; users see Caddy's maintenance response or a connection-refused message. Replicas do NOT crash — they keep `/healthz` at 200 (process is alive) and `/readyz` at 503 (not ready). When Postgres comes back, replicas re-acquire pool connections and the LISTEN task reconnects within seconds; `/readyz` flips back to 200; LB resumes routing.
- **Operator action**: restore Postgres (restart, fix disk, restore from backup per `backup-restore.md`). No Foundry-side intervention needed.
- **Why not auto-failover**: the MVP has no replica Postgres to fail over to. ADR-105 owns the deferral.

### Failure 2 — LISTEN connection drops

- **Trigger**: Postgres restarts; network blip; pgbouncer (misconfigured) closes idle connections.
- **User-visible symptom**: realtime updates stop for clients on the affected replica; non-realtime functionality (page loads, mutations) keeps working.
- **Detection signal**: `realtime_listen_disconnects_total` increments; log line `realtime.listen.disconnected`.
- **Automatic mitigation**: backoff-and-retry loop reconnects (100 ms → 5 s capped; see `realtime-infrastructure.md`). Once reconnected, the LISTEN task re-issues `LISTEN issue_events`. Events that fired during the gap are NOT replayed in MVP; clients see a "Reconnected — refresh for latest" toast (US-09).
- **Operator action**: typically none. If `realtime_listen_disconnects_total` is incrementing more than ~1/hour, investigate (likely a pgbouncer-in-transaction-pool problem detected by `probe.pg.listen_notify` at next startup).

### Failure 3 — Migration fails mid-rollout

- **Trigger**: New release ships a migration that errors (constraint violation on existing data, missing default, syntax error).
- **User-visible symptom**: One replica fails to start; LB removes it. Other replicas still serving on the old version. No user-visible outage if at least one old replica remains.
- **Detection signal**: `/readyz` permanently 503 on the new replica; `migration_apply_duration_seconds` shows the failed migration name; container exits non-zero and Docker `restart: unless-stopped` puts it in a restart loop.
- **Automatic mitigation**: advisory lock auto-releases (session ended on crash) — the next replica startup will try again. If the migration is deterministically broken, every new replica fails the same way. **Old replicas keep serving.**
- **Operator action**: roll back to the previous image tag; fix the migration in a hotfix; re-deploy. See `migrations.md` "Failure modes" section for detail.

### Failure 4 — Disk fills (bytea attachments dominate)

- **Trigger**: Operators didn't set up disk-space alerting; attachment uploads gradually fill the Postgres data volume.
- **User-visible symptom**: writes start failing with Postgres error "no space left on device"; reads keep working. App returns 500 on mutations; SSE events stop firing (because the WAL can't write).
- **Detection signal**:
  - `http_requests_total{status="5xx"}` spikes on POST/PUT routes.
  - Postgres logs `disk full`.
  - The recommended starter alert rule: `node_filesystem_avail_bytes{mountpoint="/var/lib/postgresql/data"} / node_filesystem_size_bytes < 0.10`.
- **Automatic mitigation**: none in MVP. The app cannot magically free disk.
- **Operator action**: free disk (delete old WAL after a `pg_basebackup`, or expand the volume). Foundry resumes accepting writes when Postgres can write. **Prevention**: NFR-PERF-02 caps per-file size; operators are advised to set up disk alerts when attachment count grows.

### Failure 5 — Single replica crashes

- **Trigger**: app OOM (rare; Rust + bounded buffers); a bug; SIGKILL.
- **User-visible symptom**: clients currently connected to that replica see connection-reset; their browsers retry (HTTP) or auto-reconnect (SSE) and land on another replica.
- **Detection signal**: replica's `/healthz` stops responding; LB stops routing.
- **Automatic mitigation**: Docker `restart: unless-stopped` brings the replica back within seconds. Caddy resumes routing once `/readyz` returns 200.
- **Operator action**: if it crashes repeatedly, investigate (logs, metrics). Otherwise none.

### Failure 6 — Network partition between app replicas

- **Trigger**: replicas on different hosts; network between them drops.
- **User-visible symptom**: none. Replicas don't need to talk to each other — they only talk to Postgres. Cross-replica coordination (SSE fan-out) happens via Postgres.
- **Operator action**: none. This is a non-issue by design (the MVP has no replica-to-replica RPC).

### Failure 7 — Caddy crashes

- **Trigger**: Caddy bug; OOM (it's small but possible); config reload typo (Caddy validates first, so this is rare).
- **User-visible symptom**: complete outage (port 443 unreachable).
- **Detection signal**: external uptime monitoring (operator-supplied) catches it.
- **Automatic mitigation**: Docker `restart: unless-stopped` revives Caddy in seconds.
- **Operator action**: investigate if recurring. **Path to remove**: 2 Caddy instances behind DNS RR or a cloud LB (post-MVP).

### Failure 8 — `.env` file lost (operator deletes by accident)

- **Trigger**: file deletion, host rebuild without restoring `.env`.
- **User-visible symptom**: container fails to start (`probe.env.required_set` refuses). No data loss (Postgres data is in a separate volume).
- **Detection signal**: `health.startup.refused` log line naming the missing env var.
- **Operator action**: restore `.env` from backup. If `SESSION_SECRET` was rotated (different value than before), all outstanding HMAC-signed tokens (bootstrap, invite, password-reset) become invalid; existing sessions in Postgres survive because session data is in the table, not the cookie.

### Failure 9 — Bytea attachment corruption during pg_dump/pg_restore

- **Trigger**: cosmic ray, disk error, Postgres bug.
- **User-visible symptom**: a specific attachment download returns the wrong bytes; user sees a broken image / corrupt PDF.
- **Detection signal**: `foundry doctor backup-verify` runs sha256 against the stored sha256 column and reports mismatches.
- **Operator action**: investigate dump integrity; re-take if needed; restore the specific attachment from a known-good backup. This is the **Earned Trust** verification path from Principle 9 — we don't trust `bytea` round-trips silently; the per-attachment sha256 makes the substrate's honesty checkable.

### Failure 10 — Clock skew across replicas

- **Trigger**: NTP failure; container clock drift.
- **User-visible symptom**: subtle ordering bugs in event timestamps; sessions might expire early or late; rate-limit windows misalign.
- **Detection signal**: `probe.clock.monotonic_skew` at startup refuses if >2 s/min drift; runtime alert if `process_clock_skew_seconds` (exposed as a metric) >5 s.
- **Automatic mitigation**: refuse to start on egregious skew.
- **Operator action**: fix NTP / chrony on the host. Foundry refuses to start on a host with broken time.

### Failure 11 — SSE clients exhaust file descriptors

- **Trigger**: more concurrent SSE clients than the container `ulimit -n` allows.
- **User-visible symptom**: new connections fail; existing connections OK.
- **Detection signal**: `sse_subscribers_total` plateaus near `ulimit`; log line `accept failed: too many open files`.
- **Automatic mitigation**: none — the app cannot raise its own ulimit.
- **Operator action**: bump container ulimit (`ulimits: nofile: 65535` in compose). Documented in operator runbook for deployments expecting >500 concurrent SSE clients.

### Failure 12 — Probe lies about itself (Principle 9 recursion)

- **Trigger**: A future Foundry upgrade silently broke a probe; the probe no longer detects what it claims to detect.
- **User-visible symptom**: substrate failures slip past the probe gate; failure mode reappears in production.
- **Detection signal**: `probe_failures_total{probe_name=...}` is suspiciously flat for a probe known to fail historically; CI integration test exercises each probe against a known-bad fixture and asserts the probe rejects.
- **Operator action**: trust the CI integration test; treat its red as a blocker. This is the recursive self-application of Principle 9 — there must be a meta-probe verifying probes still work.

### Failure 13 — Caddy TLS certificate renewal fails silently

- **Trigger**: Caddy's auto-Let's-Encrypt renewal cycle fails repeatedly inside the 30-day pre-expiry window. Realistic causes: ACME API outage during the renewal attempt, Caddy data volume corruption or permission change, port 80 blocked at the network edge after a firewall change, Let's Encrypt rate-limit hit, or the domain's DNS pointed away from this Caddy.
- **User-visible symptom**: nothing for up to 30 days, then a hard browser-level TLS error and full outage on the next morning the operator least expects it. The Foundry app itself stays healthy throughout, so `/readyz` does not catch this.
- **Detection signal**: scrape Caddy's `/metrics` endpoint and alert on `caddy_certificates_renewals_failed_total` increasing, or `caddy_certificate_expiry_seconds` dropping below 30 days × 86400. Operators who skip the observability overlay get the lighter-weight `foundry doctor tls-expiry` CLI check that runs the same check via Caddy's admin API; recommended as a daily cron in the operator runbook.
- **Automatic mitigation**: none. Caddy retries renewal on its own schedule but cannot recover from external causes (network, DNS, rate limit) without operator action.
- **Operator action**: inspect `caddy logs` for the specific failure (Caddy logs ACME errors in detail), fix the underlying cause (open port 80, restore DNS, wait out rate limit), and trigger a manual `caddy reload`. Escape hatch: drop in an operator-managed cert via the manual cert override documented in `loadbalancer-and-tls.md` and disable Caddy auto-TLS for that host.
- **Scope note**: this failure mode applies to Variant 2 (multi-replica with Caddy) and Variant 1 deploys that expose Foundry over public TLS. Variant 1 single-VPS deploys behind an operator-supplied LB inherit that LB's renewal failure mode instead. See ADR-103 for the explicit acknowledgement that Foundry does not own TLS renewal health beyond exposing the signal.

## Alert recommendations (starter set)

Operators using the bundled observability overlay get these as suggestions in the docs, not auto-installed in Prometheus:

| Severity | Condition | Rationale |
|----------|-----------|-----------|
| Critical | `/readyz` returns non-200 on >50% of replicas for >2 min | Cluster degraded; user-visible outage likely |
| Critical | `node_filesystem_avail_bytes{mountpoint="/var/lib/postgresql/data"} / node_filesystem_size_bytes < 0.10` | Postgres disk almost full; writes will fail soon |
| Warning | `realtime_listen_disconnects_total` increase >5 in 1h | Realtime degraded; investigate Postgres or pgbouncer |
| Warning | `migration_apply_duration_seconds_bucket{le="60"} - on() migration_apply_duration_seconds_count < 0.95` | Migrations taking longer than expected; release-notes prediction was off |
| Warning | `db_connection_wait_seconds_bucket{le="0.1"} / db_connection_wait_seconds_count < 0.95` | Pool exhaustion; raise `DATABASE_MAX_CONNECTIONS` or shard load |
| Info | `bootstrap_tokens_unclaimed > 0` for >24h | Admin hasn't completed bootstrap; possibly install abandoned |
| Critical | `probe_failures_total` increases | A startup or runtime probe is failing; substrate lying — investigate before next deploy |
| Critical | `caddy_certificate_expiry_seconds < 7*86400` for any host | TLS cert under 7 days from expiry and Caddy has not renewed — manual intervention needed before outage (see Failure 13) |
| Warning | `caddy_certificates_renewals_failed_total` increase >0 in 1h | Renewal attempt failing; investigate while there's still runway before expiry |

## What we don't try to detect or mitigate

Explicitly out of scope; operators run their own monitoring:

- Distributed-system Byzantine failures (Foundry is single-Postgres; no consensus protocol to lie about).
- Geographic disaster (single region in MVP; no multi-region replication).
- Application-layer DDoS (rate limiting is deferred to post-MVP; until then, the LB and cloud provider are responsible).
- Insider threats / audit log integrity (audit log is post-MVP).

## Cross-references

- `topology.md` — substrate variants where each failure manifests differently.
- `migrations.md` — detail for failures 3 and 12.
- `realtime-infrastructure.md` — detail for failures 2 and 11.
- `backup-restore.md` — detail for failures 1, 4, and 9.
- `observability-infra.md` — where these detection signals are exposed.
- ADR-105 — single-Postgres SPOF accepted; this document is the *what happens* side of that decision.
