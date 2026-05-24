# ADR-102: docker-compose as primary deploy artifact (vs. helm chart from day one)

## Status

Accepted (2026-05-23).

## Context

Foundry must be deployable by the operator persona (Devansh: SRE at a 12-person SaaS startup, comfortable with Docker, suspicious of long YAML). The JTBD framing is "under an hour from `docker compose up` to first issue filed" (US-01). The question: ship docker-compose, ship a Helm chart, or ship both?

Some open-source projects ship Helm-first (e.g., GitLab, Sentry). Others ship docker-compose first with Helm following (e.g., Plausible, Forgejo, Outline). The MVP target segment — 2-20-person teams self-hosting on their own infra — sits well to the "compose-first" side of this distribution.

## Decision

The MVP ships **docker-compose as the primary, documented, supported deploy artifact**. Kubernetes manifests are deferred to a future wave (v0.4 target). Until then:

- The repo includes `docker-compose.yml` (Variant 1: single-replica) and `docker-compose.scaled.yml` (Variant 2: multi-replica with Caddy).
- An optional `docker-compose.observability.yml` overlay ships Prometheus + Loki + Grafana for operators who want it.
- Every architectural choice is K8s-translatable (NFR-PORT-01) so the v0.4 manifests are a translation exercise, not a re-architecture.
- The README's quickstart says `docker compose up -d`; nothing else.

## Alternatives considered

### A — Helm chart from day one

- **Pros**: covers the operator segment that's already on K8s; cloud-native posture from launch.
- **Cons** (decisive):
  - 60-minute install promise dies: even a "trivial" Helm install requires the operator to have a K8s cluster, kubectl, helm, ingress controller, and cert-manager. Realistic time-to-running for a fresh operator is 2-4 hours.
  - Helm is overkill for the 20-person target: most are on a single VPS, possibly without ever having touched K8s.
  - Maintaining a high-quality Helm chart is a meaningful ongoing cost (test against multiple K8s versions, multiple ingress controllers, multiple Postgres operators); the 2-developer team would be paying it from week 1.

### B — Both from day one

- **Pros**: covers both audiences.
- **Cons**: doubles the testing matrix; the MVP team can't credibly support both; one will inevitably lag behind in features, leading to operator confusion ("which one is canonical?").

### C — Helm-only, abandoning the compose audience

- Rejected immediately: violates the JTBD's named segment (5-50-person teams, indie-first funnel).

## Consequences

### Positive

- US-01 hour-to-running is achievable.
- 2-container default compose has minimum cognitive load for a new operator.
- The compose file is also the dev environment: contributors (US-13) use the same artifact in development as in production.
- The K8s-translatability rules (NFR-PORT-01) are explicit and testable — no surprises when the v0.4 manifests land.

### Negative (explicit trade-offs)

- Operators already on K8s have an awkward translation step. They can hand-write manifests from the compose file (mechanical) or wait for v0.4. We accept this for the MVP because that audience is the smaller of the two segments at launch.
- Docker Swarm and Podman compatibility are not first-class. They likely work (Foundry's compose file uses no Swarm-specific features), but we don't test them. Documented as "should work; report issues."
- No service mesh, no observability sidecars, no advanced K8s-native features (HPA, PodDisruptionBudgets, etc.). These are post-v0.4 concerns.

### Migration to K8s (v0.4 outlook)

The v0.4 manifest set will be a mechanical translation per the table in `topology.md`:

| compose | K8s |
|---------|-----|
| `foundry-app` service with `deploy.replicas: 3` | `Deployment` with `spec.replicas: 3` |
| `foundry-db` service with named volume | `StatefulSet` with `volumeClaimTemplates` |
| `caddy` service | `Ingress` + cert-manager Issuer |
| `.env` injection | `Secret` + `ConfigMap` |
| compose `healthcheck` | `livenessProbe` + `readinessProbe` |

Helm vs Kustomize for v0.4 is an open question; this ADR doesn't decide it. The MVP-time commitment is only "compose is primary and complete."

## Review trigger

Revisit when any of:

1. >30% of operator-survey respondents are on K8s and asking for first-class manifests.
2. A major MVP feature (e.g., HA Postgres in v0.4) requires K8s-native operators (Patroni-on-K8s, Zalando's postgres-operator) such that the compose path becomes second-class.
3. The "compose-first" promise becomes a liability for enterprise adoption — i.e., procurement teams demand a chart.
