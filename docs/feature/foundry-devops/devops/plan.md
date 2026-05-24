# Foundry DEVOPS — slice plan

## Scope of this turn

Built end-to-end:

- **CI** — `.github/workflows/ci.yml` (lint/format, build/test against Postgres
  service container, full acceptance suite incl. `@docker-compose`, cargo-deny).
  Mirror at `.forgejo/workflows/ci.yml`.
- **Container release** — `.github/workflows/release.yml`. Triggers on `push:
  main` (publishes `:main` + `:sha-<short>`) and on `tags: ['v*.*.*']`
  (publishes `:vX.Y.Z`, `:vX.Y`, `:latest`). Multi-arch (amd64 + arm64),
  cosign keyless signing via OIDC, CycloneDX/SPDX SBOM via Syft, gha cache.
- **Kubernetes manifests** — plain YAML in `deploy/k8s/`. 9 resources covering
  namespace, postgres StatefulSet + headless Service, foundry Deployment +
  Service + Ingress (cert-manager) + ConfigMap + Secret template + PDB. No
  Helm, no Kustomize (ADR-102 defers to v0.4). README documents prerequisites
  (ingress controller + cert-manager + StorageClass) and first-time install.
- **Observability overlay** — `docker-compose.observability.yml` + the
  `observability/` config tree (Prometheus scraping `foundry:9090`, Loki
  storing 30 d, Promtail tailing docker container logs, Grafana with
  provisioned datasources + the "Foundry Overview" starter dashboard).
- **Dependabot** — `.github/dependabot.yml`. Daily cargo, weekly
  github-actions + docker. Minor/patch grouped per ecosystem; majors
  individual.
- **Release docs** — `RELEASING.md` (semver, tag-driven release flow, cosign
  verification recipe, keep-a-changelog format, backout procedure).
- **CONTRIBUTING.md additions** — CI section, `cargo xtask ci` local
  replication, dependabot/auto-merge operator steps, RELEASING pointer.
- **`cargo xtask ci`** — new subcommand that runs every gate CI runs, in
  order, exiting non-zero on first failure. `FOUNDRY_XTASK_INCLUDE_DOCKER=1`
  opt-in for the slow @docker-compose acceptance group.

## Deferred (explicit)

The following are deliberately NOT in this slice:

- **Helm chart / Kustomize overlays** — v0.4 per ADR-102. The plain-YAML
  manifests are the input that future chart will be derived from.
- **HorizontalPodAutoscaler** — scaling.md identifies HA and rolling-deploy
  capability (not CPU load) as the actual reason to scale. An HPA on a
  0.25-req/sec workload would oscillate without benefit.
- **GitOps (ArgoCD / Flux)** — operators who want it can point either tool
  at `deploy/k8s/` today; no Foundry-side change needed.
- **Service mesh / mTLS between pods** — Caddy ingress is the only LB.
- **Multi-cluster / multi-region deploys** — single-region in MVP per
  failure-modes.md "What we don't try to detect or mitigate."
- **External Secrets Operator / sealed-secrets bundle** — documented as
  the production path in `deploy/k8s/README.md` and in the secret template
  file, but not bundled. Both are <1-day adds when an operator asks.
- **Pre-commit hooks** — `cargo xtask ci` covers the local-gate story for
  now. A `lefthook` / `pre-commit` config that mirrors the remote commit
  stage is a small follow-up.
- **CHANGELOG.md** — RELEASING.md mandates the keep-a-changelog format,
  but the file itself is not pre-populated. First v0.2.0 tag is when it
  gets created.
- **Alerting rules shipped with the observability overlay** — failure-modes.md
  enumerates the starter set; we expose the metrics but leave operator
  alerting choices to operators (ADR-104 minimal-by-default posture).

## K8s-translatability claim — true now?

`topology.md` claims every MVP architectural choice is "K8s-translatable
(NFR-PORT-01)." This slice tests the claim by writing the manifests.
Verdict: **true, with two notes**.

| Claim from topology.md                                  | Manifest realization                                                                                  | Gap? |
|---------------------------------------------------------|-------------------------------------------------------------------------------------------------------|------|
| `service: foundry` -> `Deployment` + `Service`          | `foundry-deployment.yaml` + `foundry-service.yaml`. 3 replicas; rolling update; non-root + readOnlyRootFS; drop ALL caps. | none |
| `service: postgres` -> `StatefulSet` + `PVC` + headless Service | `postgres-statefulset.yaml` + `postgres-service.yaml`. PVC via `volumeClaimTemplates`; default StorageClass. | none |
| `.env` -> `Secret` + `ConfigMap` env-injected           | `foundry-configmap.yaml` + `foundry-secret.example.yaml`. Two-secret split (db credentials + app secrets) so ESO can pull just one. | none |
| named volume `pgdata` -> `PersistentVolumeClaim`        | `volumeClaimTemplates` 50Gi default                                                                   | none |
| `caddy` -> `Ingress` + cert-manager                     | `foundry-ingress.yaml`. Annotations for nginx-ingress SSE; cert-manager.io/cluster-issuer. **Note 1**: SSE annotations are nginx-specific; Caddy-ingress reads its own set. Documented inline. | minor |
| compose `healthcheck` -> liveness/readiness probes      | Both probes on `/healthz` + `/readyz`. `terminationGracePeriodSeconds: 25` >= `SHUTDOWN_GRACE_SECONDS` (15) + buffer per realtime-roadmap.md. | none |
| App must be stateless (NFR-AVAIL-01)                    | `readOnlyRootFilesystem: true` + emptyDir-backed `/tmp` (so `probe.fs.tmp_writable` still passes). | none |
| No host-bind volumes (NFR-PORT-01)                      | Only PVC + emptyDir; no hostPath.                                                                     | none |

**Note 1 (Ingress controllers)**: We annotated for nginx-ingress because
that's the most common K8s default. Operators on Caddy-ingress, Traefik, or
a cloud LB controller swap the annotations (or just ignore them — the
SSE-disable-buffering knobs are controller-specific) and the manifest
otherwise works unchanged. A future slice could templatize this via
Kustomize overlays once we move to Kustomize/Helm in v0.4.

**Note 2 (Postgres SPOF carries over)**: The K8s `StatefulSet` runs a
single Postgres replica, exactly mirroring the compose deploy. The K8s
form does NOT introduce HA Postgres — that's the ADR-105 deferral, and
the same v0.4+ swap (Patroni or Zalando's postgres-operator) applies to
both the compose and K8s deploys. No infra gap; just an inherited risk.

## Inline app changes made this slice

To make the metrics overlay functional, I wired the metrics endpoint
inline (it was declared in `Cargo.toml [workspace.dependencies]` but no
crate consumed it). Two changes, both in `foundry-app`:

1. **`crates/foundry-app/src/metrics_server.rs`** (new). Installs the
   `metrics_exporter_prometheus` recorder and spawns a second axum
   listener on `METRICS_PORT` (default 9090) serving `/metrics`.
2. **`crates/foundry-app/src/main.rs`**. Calls `install_recorder()`
   then `serve()` before binding the main HTTP listener. Emits a
   one-counter sanity startup signal (`foundry_app_startup_total`).
   Also switched `init_tracing` to JSON by default (`RUST_LOG_FORMAT=pretty`
   restores the human-readable form for `cargo run`) so the
   `foundry-overview` dashboard's Loki panel sees structured fields.

The Cargo.toml change is a one-line addition of two already-declared
workspace dependencies. Build + tests still pass.

The full set of metric names enumerated in `observability-infra.md`
(http_requests_total, db_connections_in_use, sse_subscribers_total, etc.)
is NOT yet emitted — instrumenting handlers and the DB pool is slice 3+
work. The dashboard panels reference those names so the graphs light up
the moment instrumentation lands; until then they show empty series,
which is the "instrument me" signal.

## Open questions for the reviewer / next devops slice

1. The release workflow uses `ghcr.io/${{ github.repository_owner }}/foundry`,
   which resolves to `ghcr.io/foundry-project/foundry` if the repo lives at
   `foundry-project/foundry`. Confirm the org name before the first tag push,
   or override `IMAGE_NAME` env at the top of the workflow.
2. Auto-merge for dependabot is documented as an operator opt-in (three
   manual repo-settings steps). Should we ship a tiny `.github/workflows/
   dependabot-auto-merge.yml` that does the `gh pr merge --auto --squash`
   call when CI is green and the PR is labeled `patch`? Trade-off: yet
   another workflow file vs. one-command auto-merge.
3. The K8s ingress annotations are nginx-specific. When v0.4 ships Helm,
   we'll templatize the ingress-controller choice. For now, the manifest
   silently no-ops on Caddy-ingress / Traefik. Is "documented inline, no
   warning" acceptable, or do we want a starter `kustomization.yaml`
   per controller now?
