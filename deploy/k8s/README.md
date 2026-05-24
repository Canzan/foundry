# Kubernetes manifests for Foundry

> **Scope**: plain YAML, one file per resource, **no Helm, no Kustomize**.
> The MVP ships docker-compose as the primary deploy artifact (ADR-102).
> These manifests exist so K8s-resident operators can run Foundry today
> without waiting for the v0.4 first-class chart.

## What's here

| File                              | Resource                          |
|-----------------------------------|-----------------------------------|
| `namespace.yaml`                  | Namespace `foundry`               |
| `postgres-statefulset.yaml`       | Postgres 16 StatefulSet + PVC     |
| `postgres-service.yaml`           | Headless Service for Postgres     |
| `foundry-configmap.yaml`          | Non-secret app env                |
| `foundry-secret.example.yaml`     | **Template** for app/db secrets   |
| `foundry-deployment.yaml`         | App Deployment (3 replicas)       |
| `foundry-service.yaml`            | ClusterIP Service for the app     |
| `foundry-ingress.yaml`            | Ingress with cert-manager TLS     |
| `foundry-pdb.yaml`                | PodDisruptionBudget (minAvailable 2) |

## Prerequisites the operator must supply

These are NOT bundled; install them on the cluster first.

1. **An ingress controller** — nginx-ingress, Caddy-ingress, Traefik,
   or a cloud provider's LB controller. Any class works; the Ingress
   manifest leaves `ingressClassName` unset so it inherits the cluster
   default.
2. **cert-manager** with a `ClusterIssuer` named `letsencrypt-prod`.
   The `cert-manager.io/cluster-issuer` annotation on the Ingress
   references this. Rename in `foundry-ingress.yaml` if your cluster
   uses a different issuer name.
3. **A default StorageClass**. The Postgres StatefulSet's PVC leaves
   `storageClassName` unset; this picks the default. Edit if you need
   a specific class (gp3, premium-rwo, longhorn, etc.).
4. **A registry credentials secret** if your cluster cannot pull from
   ghcr.io anonymously. Foundry's images are public, so the typical
   install does not need this.

## First-time install

```sh
# 1. Copy the secret template and fill in real values.
cp deploy/k8s/foundry-secret.example.yaml deploy/k8s/foundry-secret.yaml
$EDITOR deploy/k8s/foundry-secret.yaml
# Generate a session secret: openssl rand -base64 48
# Generate a postgres password: openssl rand -base64 24
# Make DATABASE_URL match the password you chose.

# 2. Update foundry-ingress.yaml: replace foundry.example.com with
#    the hostname your DNS points at this cluster's ingress.

# 3. Update foundry-configmap.yaml: set FOUNDRY_PUBLIC_URL to the
#    public https://hostname (must match the Ingress hostname).

# 4. Apply (order matters slightly — namespace first, then
#    secrets/configmaps, then workloads). The single `apply -f` on the
#    directory is fine; kubectl handles dependency order via retries.
kubectl apply -f deploy/k8s/namespace.yaml
kubectl apply -f deploy/k8s/foundry-secret.yaml
kubectl apply -f deploy/k8s/

# 5. Wait for postgres + the first foundry replica to be ready.
kubectl -n foundry rollout status statefulset/foundry-db
kubectl -n foundry rollout status deployment/foundry-app

# 6. Find the one-shot bootstrap claim URL (logged by the first
#    replica). The URL is one-shot and has a 30-minute TTL.
kubectl -n foundry logs deployment/foundry-app | grep '\[BOOTSTRAP\]'
```

> `foundry-secret.yaml` (no `.example.`) is gitignored. Don't commit it.

## Why one-file-per-resource and not `kubectl apply -k`

A `kustomization.yaml` would let `kubectl apply -k deploy/k8s/` work,
but it would also signal "use Kustomize" — which we are intentionally
deferring per ADR-102. The recommended invocation is
`kubectl apply -f deploy/k8s/` (recursive applies happen automatically
for files in the directory).

## Migrations

Foundry runs migrations at app startup, guarded by a Postgres
advisory lock. The first replica to start during a rollout acquires
the lock, applies pending migrations (~seconds), and releases.
Other replicas block on the lock then observe the schema is current
and skip. **No init container, no separate Job, no Helm hook.**

The full sequence and failure-mode analysis (migration fails mid-roll,
expand-contract patterns, etc.) is in
`docs/feature/foundry-backend-mvp/design/system/migrations.md`.

## Probes and graceful shutdown

- `livenessProbe -> /healthz` (returns 200 if the process can accept
  TCP connections). A failure restarts the pod.
- `readinessProbe -> /readyz` (200 only if Postgres is reachable AND
  migrations applied AND not in graceful-shutdown drain). A failure
  removes the pod from the Service endpoints, so traffic stops within
  a few seconds.
- `terminationGracePeriodSeconds: 25` gives a draining pod time to
  let in-flight HTTP requests finish and send the SSE close marker
  (realtime-roadmap.md SHUTDOWN_GRACE_SECONDS = 15 + buffer).

## Backup strategy (you MUST configure this before going live)

These manifests deploy a **single Postgres replica** with a single PVC.
ADR-105 records this as an accepted MVP trade-off — production-grade HA
Postgres (Patroni / CloudNativePG / pg_auto_failover) is a v0.4 goal,
not a v0.1 default. But "no HA" does NOT mean "no backups". A deleted
PVC, a corrupted filesystem, or a misapplied migration with no rollback
is real-data-loss territory. Pick one of these BEFORE giving users a
URL:

1. **Velero with restic / kopia** — snapshots the PVC on a schedule, can
   restore to any cluster. Lowest-effort if Velero is already on the
   cluster for other workloads.
2. **VolumeSnapshot CRDs with a CSI snapshotter** — native K8s, but the
   restore story is manual unless paired with Velero or a custom
   operator.
3. **`pg_dump` on a CronJob** — write to S3 / object storage. Simplest
   conceptually; you keep the existing `backup-restore.md` runbook from
   slice 1, and it works the same in K8s as in compose.
4. **A Postgres operator** — Zalando postgres-operator or CloudNativePG.
   This also gives you read replicas + failover (= a partial path to
   v0.4's HA goal). Largest dependency to take on, biggest payoff.

The MVP does not pick for you because the right answer depends on what
your cluster already runs. Whatever you choose, run a restore drill at
least once on a non-production cluster — an untested backup is not a
backup.

The slice-1 design doc `docs/feature/foundry-backend-mvp/design/system/backup-restore.md`
covers the `pg_dump` runbook in detail; option (3) is essentially
running that runbook from a CronJob.

## Scaling

`kubectl scale deployment/foundry-app -n foundry --replicas=N` is the
manual lever. The MVP **intentionally ships no HorizontalPodAutoscaler**
— scaling.md identifies HA and rolling-deploy capability (not CPU
load) as the actual reason to scale; an HPA on a 0.25-req/s workload
would oscillate without benefit. Operators with a real CPU-bound load
(post-MVP) can add an HPA in a separate manifest without touching
anything in this directory.

## Migration to Helm (v0.4 outlook)

ADR-102 records that a first-class Helm chart is the v0.4 target.
This directory is the input the chart will be derived from — every
parameter the chart will expose (replicas, image tag, hostname,
storage class, resource requests/limits, etc.) is already a single
field-edit in one of these files.

## Verifying without a cluster

`kubectl apply --dry-run=client -f deploy/k8s/` validates the YAML
against the OpenAPI schema bundled with your `kubectl`. CI runs this
to catch syntax / schema drift early.
