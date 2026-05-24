# Deployment Topology — Foundry Backend MVP

## Audience

Operator personas defined in `journey.md` and `stories.md`: Devansh (SRE running on a single VPS today, growing to multi-replica tomorrow) and the v0.4+ operator translating the same artifacts to Kubernetes. Companion `solution-architect`'s `auth.md` owns *what* a session is; this document owns *where the binary runs and how traffic reaches it*.

## Three deploy variants

Foundry ships one Docker image. The same image runs in all three variants below. The only thing that changes is the orchestrator wrapping it and the load balancer in front of it.

### Variant 1 — Single VPS (MVP default)

The "under-an-hour" install path (US-01). One foundry container + one postgres container, both managed by `docker compose`. No load balancer because there is only one app replica; TLS is optional and operator-supplied.

```mermaid
C4Deployment
    title Single-VPS deploy (MVP default)

    Deployment_Node(vps, "VPS / dev laptop", "Linux + Docker 24+") {
        Deployment_Node(net, "docker compose network: foundry_net", "bridge") {
            Container(app, "foundry-app", "Rust axum binary", "port 3000")
            ContainerDb(pg, "foundry-db", "Postgres 16", "port 5432, named volume pgdata")
        }
        Container_Ext(env, ".env file", "secrets injected at start")
    }

    Person(user, "User browser")
    user --> app : "HTTP :3000 (TLS optional, operator-supplied)"
    app --> pg : "TCP :5432 (single pool + 1 LISTEN conn)"
    env --> app : "env vars at container start"
```

Operationally trivial: `docker compose up -d`, two containers, one volume. This is the variant Devansh runs in his 60-minute evaluation window.

### Variant 2 — Multi-replica behind Caddy (production reference)

When the team grows past ~20 active users or when Devansh needs rolling restarts (US-02, US-04, NFR-AVAIL-01, NFR-AVAIL-02), he scales horizontally. Caddy terminates TLS and round-robins to N foundry replicas; all replicas share one Postgres.

```mermaid
C4Deployment
    title Multi-replica deploy (production reference)

    Deployment_Node(vps, "Single host (or N hosts)", "Linux + Docker") {
        Deployment_Node(net, "docker compose network: foundry_net", "bridge") {
            Container(lb, "caddy", "reverse proxy + auto-TLS", "port 80/443 external")
            Container(app1, "foundry-app#1", "Rust axum", "port 3000 internal")
            Container(app2, "foundry-app#2", "Rust axum", "port 3000 internal")
            Container(app3, "foundry-app#3", "Rust axum", "port 3000 internal")
            ContainerDb(pg, "foundry-db", "Postgres 16", "port 5432 internal")
        }
    }

    Person(user, "User browser")
    user --> lb : "HTTPS :443"
    lb --> app1 : "round-robin, no sticky"
    lb --> app2
    lb --> app3
    app1 --> pg : "pool + LISTEN conn"
    app2 --> pg
    app3 --> pg
```

Critical property: any replica can serve any request (NFR-AVAIL-01). Sessions live in Postgres (`tower-sessions-sqlx-store`, owned by solution-architect's `auth.md`); SSE realtime fans out via Postgres LISTEN/NOTIFY (see `realtime-infrastructure.md`). No sticky-session requirement is documented as a property, not just an absence — Caddy's default round-robin is correct without any session-affinity directive.

### Variant 3 — Kubernetes (future, v0.4+)

The MVP does not ship K8s manifests, but every choice above is K8s-translatable (NFR-PORT-01). When an operator translates, the mapping is mechanical:

```mermaid
C4Deployment
    title Future Kubernetes deploy (no manifests in MVP)

    Deployment_Node(cluster, "Kubernetes cluster", "v1.28+") {
        Deployment_Node(ns, "namespace: foundry", "") {
            Container(ing, "Ingress", "nginx-ingress / Traefik / cloud LB", "TLS terminated here")
            Deployment_Node(dep, "Deployment foundry-app", "replicas: 3") {
                Container(pod1, "Pod foundry-app", "Rust axum", "containerPort 3000")
            }
            Container(svc, "Service foundry-app", "ClusterIP", "port 80 -> 3000")
            ContainerDb(pgsvc, "StatefulSet foundry-db", "Postgres 16", "PVC-backed")
            Container(secret, "Secret foundry-env", "DATABASE_URL, SESSION_SECRET")
            Container(cm, "ConfigMap foundry-config", "non-secret env")
        }
    }

    Person(user, "User browser")
    user --> ing : "HTTPS :443"
    ing --> svc
    svc --> pod1 : "round-robin endpoints"
    pod1 --> pgsvc : "DNS: foundry-db.foundry.svc"
```

Translation rules that are baked into the MVP design:

| docker-compose construct | K8s equivalent | MVP rule that protects this |
|--------------------------|----------------|------------------------------|
| `service: foundry` | `Deployment` + `Service` | App must be stateless (NFR-AVAIL-01) |
| `service: postgres` | `StatefulSet` + `PVC` + headless `Service` | Postgres is the only stateful workload |
| `.env` file | `Secret` + `ConfigMap` (env-injected) | Env-only config (NFR-PORT-02), no baked-in secrets (NFR-SEC-07) |
| named volume `pgdata` | `PersistentVolumeClaim` | No host-bind volumes for app (NFR-PORT-01) |
| `caddy` container | `Ingress` resource + cert-manager | LB choice is operator-configurable; not assumed by app |
| service name DNS | cluster DNS | App resolves Postgres by hostname, not localhost (NFR-PORT-01) |
| `healthcheck:` | `livenessProbe` + `readinessProbe` | `/healthz` and `/readyz` semantics already defined (NFR-OBS-02) |

The MVP design refuses any feature that breaks this mapping. Specifically: no `host` network mode, no `extra_hosts` for service discovery, no app-side bind mounts, no `localhost` references in code or config.

## What forced these three variants (and not others)

We considered and rejected:

- **Bare-metal systemd unit (no Docker)** — would force operators to install Rust, build, manage Postgres outside Docker. Defeats the "under an hour" promise.
- **Single-binary embedded SQLite** — would block multi-replica from day one (the whole NFR-AVAIL-01 promise dies). Also incompatible with the bytea-attachment story (US-11).
- **K8s manifests in MVP** — adds a Helm/Kustomize learning curve to a 60-minute evaluation. Deferred to v0.4 explicitly.
- **Cloud-managed-only (RDS + ECS, etc.)** — violates the "self-host on your own infra" anchor of the JTBD.

## Probes — proving the substrate is honest (Principle 9)

Every deploy variant runs the same binary; the binary's startup `probe()` step verifies the substrate before serving traffic. The probes are part of the readiness gate (NFR-OBS-02) and emit structured `health.startup.refused` events when they fail. They are documented here because the *deploy variant* determines which probes are most likely to flag a problem.

| Probe | Detects | Variant where it tends to fail |
|-------|---------|--------------------------------|
| `probe.pg.fsync` — `SHOW fsync; SHOW synchronous_commit;` and verify both ON | Postgres started with `fsync=off` (data loss on power loss) | Single-VPS with operator-tuned postgres.conf |
| `probe.pg.advisory_lock` — acquire and release the migration lock ID | Postgres version too old or extension missing | All variants on upgrade |
| `probe.pg.listen_notify` — issue a self-NOTIFY on a probe channel, verify receipt within 1 s | Postgres connection pooler (PgBouncer) sitting in front, breaking LISTEN | Multi-replica / K8s with cloud-managed pgbouncer |
| `probe.fs.tmp_writable` — write+fsync+read a 4 KB file in `/tmp` | Read-only root filesystem misconfigured | K8s with `readOnlyRootFilesystem: true` |
| `probe.clock.monotonic_skew` — sample monotonic vs wall-clock drift, refuse if >2 s/min | NTP unreachable; container clock unsync'd | K8s, single-VPS without `ntpd`/`chrony` |
| `probe.env.required_set` — every required env var present and non-empty | Operator forgot `SESSION_SECRET` etc. | All variants on first run |

The probe contract lives in `solution-architect`'s code organization, but the *list of fault modes worth probing* is the infrastructure designer's responsibility and is recorded here. If a probe fails, the replica exits non-zero and the log line names the specific lie and a suggested fix (e.g., "Postgres reports `fsync=off`; data loss possible on power failure. Set `fsync=on` in postgresql.conf and restart.").

## Configuration contract (env vars consumed by every variant)

Owned and documented by `solution-architect` in `config.md`. This document only enumerates the *infra-relevant* ones to make the variant diagrams complete:

| Var | Default | Used by |
|-----|---------|---------|
| `FOUNDRY_PORT` | 3000 | App HTTP listener (bound inside container; LB maps 80/443) |
| `METRICS_PORT` | 9090 | Sidecar metrics listener (NFR-OBS-03) — never exposed via LB |
| `DATABASE_URL` | required | Postgres connection string |
| `DATABASE_MAX_CONNECTIONS` | 10 | Per-replica pool size (NFR-PERF-04) |
| `SESSION_SECRET` | required | HMAC key for cookie integrity + bootstrap tokens |
| `SHUTDOWN_GRACE_SECONDS` | 15 | Graceful shutdown window (NFR-AVAIL-02) |

## Cross-references

- Sessions and their storage implications: see this file + solution-architect's `auth.md`.
- SSE fan-out across variants: see `realtime-infrastructure.md`.
- Capacity (when to switch from Variant 1 to Variant 2): see `scaling.md`.
- LB-specific config (Caddyfile, nginx, traefik): see `loadbalancer-and-tls.md`.
- ADR-101 (Postgres-for-everything justification), ADR-102 (docker-compose as primary deploy artifact), ADR-105 (single-Postgres SPOF accepted).
