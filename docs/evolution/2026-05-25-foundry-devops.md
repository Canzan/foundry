# Evolution — foundry-devops (DEVOPS wave)

**Finalized**: 2026-05-25
**Ship commit**: [c7cb715](../../) — "DEVOPS: CI + container release + K8s manifests + observability overlay"
**Wave coverage**: DEVOPS only (single-wave platform slice; design context inherited from slice-1's `design/system/`)

## Feature summary

Platform-readiness slice between the slice-1 walking skeleton and the
slice-3 operator-grade hardening. Built end-to-end: GitHub Actions CI
(4 parallel gates), tag-driven multi-arch container release with
cosign + SBOM, plain-YAML Kubernetes manifests (9 resources, ADR-102
defers Helm to v0.4), opt-in observability overlay
(Prometheus/Loki/Promtail/Grafana with one starter dashboard),
Dependabot, `cargo xtask ci` for local replication of every remote
gate, and the `RELEASING.md` semver/tag/backout playbook.

The slice also tested slice-1's claim that every architectural choice
in `topology.md` is "K8s-translatable (NFR-PORT-01)" — and the claim
held with two documented minor notes.

## Business context

Slice 1 proved Foundry could be installed in under an hour. DEVOPS
proves it can be released, deployed to Kubernetes, and observed —
without forcing operators onto a Helm chart, a service mesh, or
managed CI tooling before v0.1.

## Key decisions

### Inherited from slice-1 design (`docs/feature/foundry-backend-mvp/design/system/adrs/`)

- **ADR-102 — docker-compose as primary deploy.** Plain-YAML K8s
  manifests are the input shape; Helm/Kustomize is a v0.4 derivation
  from these. No Helm-from-day-one premature templating.
- **ADR-104 — Minimal observability by default.** Overlay is opt-in
  (`-f docker-compose.observability.yml`); operator alerting choices
  stay with operators.
- **ADR-105 — Single-Postgres SPOF accepted for v0.1.** K8s
  manifest mirrors this exactly (single-replica `StatefulSet`); HA
  Postgres swap to Patroni / Zalando postgres-operator is a v0.4+
  story for both compose and K8s.

### From DEVOPS (`devops/plan.md`)

- **CI fans out to 4 parallel jobs:** lint+fmt, build+test against a
  postgres:16 service container, acceptance with
  `FOUNDRY_ACCEPTANCE_TAGS=all` (default + `@docker-compose`), and
  cargo-deny. Cache via `Swatinem/rust-cache` keyed on `Cargo.lock`.
  Sub-15-min typical wall-clock.
- **Forgejo CI mirror.** `.forgejo/workflows/ci.yml` is the
  GitHub-Actions-compatible equivalent; runner docs note the
  docker-backend requirement.
- **Container release on two triggers, multi-arch with cosign
  keyless + SBOM.** Push to `main` publishes `:main` + `:sha-<short>`;
  `v*.*.*` tags publish `:vX.Y.Z` + `:vX.Y` + `:latest` (with
  `:latest` gated on non-pre-release). `linux/amd64 + arm64` via
  buildx. Keyless cosign via GitHub OIDC; Syft generates the SBOM
  and cosign attests it.
- **9 K8s resources, plain YAML.** Namespace, postgres StatefulSet +
  headless Service, foundry Deployment + Service + Ingress +
  ConfigMap + Secret template + PDB. Helm/Kustomize deferred per
  ADR-102. PDB `minAvailable: 2/3`. `terminationGracePeriodSeconds: 25`
  (SSE drain + buffer per realtime-roadmap).
- **Hardened pod security context.** Non-root uid 65532, read-only
  rootfs, drops ALL caps, seccomp RuntimeDefault, emptyDir-backed
  `/tmp` (so `probe.fs.tmp_writable` passes).
- **Probes correctly split:** `/healthz` liveness, `/readyz`
  readiness. The split matters for slice-3's `db_unreachable`
  health-injection (readiness flips without poisoning liveness).
- **No HPA in this slice.** `scaling.md` identifies HA and
  rolling-deploy capability — not CPU load — as the actual reason to
  scale. An HPA on a 0.25-req/s workload would oscillate without
  benefit.
- **GitOps stays operator-side.** Operators who want ArgoCD/Flux can
  point either tool at `deploy/k8s/` today; no Foundry-side change
  needed.
- **No service mesh, no mTLS between pods, no multi-cluster.** Caddy
  ingress is the only LB; single-region per `failure-modes.md` "What
  we don't try to detect or mitigate."
- **Backup strategy is documented, not pre-bundled.** `deploy/k8s/README.md`
  enumerates Velero / pg_dump CronJob / postgres-operator options
  with trade-offs; operators pick.
- **Dependabot: daily Cargo + weekly Actions/Docker.** Minor+patch
  grouped per ecosystem, majors individual. Auto-merge stays
  operator opt-in (3 documented manual repo-settings steps); open
  question whether to ship a tiny `dependabot-auto-merge.yml`
  workflow.
- **`cargo xtask ci` mirrors every remote gate in order.** Auto-
  detects the Docker daemon (including Colima/OrbStack via
  `docker context`) and exports `DOCKER_HOST` for child processes
  so testcontainers can reach it.
- **License allow-list extended.** `deny.toml` adds 0BSD,
  BlueOak-1.0.0, CDLA-Permissive-2.0 for transitive deps. 3 dev-only
  RUSTSEC advisories ignored with written justification.

### App-side wiring (deliberate scope creep this slice)

- **`crates/foundry-app/src/metrics_server.rs` (new).** Installs the
  `metrics_exporter_prometheus` recorder and spawns a sidecar axum
  listener on `METRICS_PORT` (default 9090) serving `/metrics`. The
  workspace-level dependency was declared but no crate consumed it
  until this slice; without it the observability dashboard would
  show only empty series.
- **`crates/foundry-app/src/main.rs`.** Calls `install_recorder()`
  then `serve()` before binding the main HTTP listener. Emits the
  `foundry_app_startup_total` counter as a sanity startup signal.
  `init_tracing` switched to JSON by default
  (`RUST_LOG_FORMAT=pretty` restores human-readable for `cargo run`)
  so Grafana's Loki panel sees structured fields.
- **Instrumentation gaps preserved as visible signal.** The full set
  of metric names enumerated in `observability-infra.md`
  (`http_requests_total`, `db_connections_in_use`,
  `sse_subscribers_total`, etc.) is NOT emitted by this slice —
  handler/DB-pool instrumentation is slice 3+ work. The dashboard
  panels reference those names so the graphs light up the moment
  instrumentation lands; until then they show empty series, which
  IS the "instrument me" signal.

### K8s-translatability claim — verified

| Claim from `topology.md`                                        | Manifest realization                                                                                                            | Gap?    |
|-----------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------|---------|
| `service: foundry` → `Deployment` + `Service`                   | `foundry-deployment.yaml` + `foundry-service.yaml`; 3 replicas; rolling update; non-root + readOnlyRootFS; drop ALL caps        | none    |
| `service: postgres` → `StatefulSet` + `PVC` + headless Service  | `postgres-statefulset.yaml` + `postgres-service.yaml`; PVC via `volumeClaimTemplates`; default StorageClass                     | none    |
| `.env` → `Secret` + `ConfigMap` env-injected                    | `foundry-configmap.yaml` + `foundry-secret.example.yaml`; two-secret split so ESO can pull just one                             | none    |
| named volume `pgdata` → `PersistentVolumeClaim`                 | `volumeClaimTemplates` 50 Gi default                                                                                            | none    |
| `caddy` → `Ingress` + cert-manager                              | `foundry-ingress.yaml`; nginx-ingress SSE annotations; cert-manager.io/cluster-issuer                                          | **minor** (Note 1) |
| compose `healthcheck` → liveness/readiness probes               | Both probes on `/healthz` + `/readyz`; `terminationGracePeriodSeconds: 25` ≥ `SHUTDOWN_GRACE_SECONDS` (15) + buffer            | none    |
| App must be stateless (NFR-AVAIL-01)                            | `readOnlyRootFilesystem: true` + emptyDir-backed `/tmp`                                                                         | none    |
| No host-bind volumes (NFR-PORT-01)                              | Only PVC + emptyDir; no hostPath                                                                                                | none    |

- **Note 1 (ingress controllers).** Annotations are nginx-specific;
  operators on Caddy-ingress / Traefik / cloud-LB controllers swap
  annotations and the manifest otherwise works unchanged. v0.4
  Kustomize/Helm will templatize this.
- **Note 2 (Postgres SPOF carries over).** Same ADR-105 deferral
  applies to both compose and K8s deploys.

## Steps completed

No `deliver/execution-log.json` was emitted (DEVOPS ran via the
`/nw:devops` skill, not `/nw:deliver`). The single ship commit
`c7cb715` enumerates the delivered scope; the plan in `devops/plan.md`
is its DESIGN-equivalent SSOT for this wave.

### CI/CD

- `.github/workflows/ci.yml` (4-job parallel)
- `.github/workflows/release.yml` (multi-arch + cosign + SBOM)
- `.forgejo/workflows/ci.yml` (mirror)
- `.github/dependabot.yml`

### K8s (`deploy/k8s/`)

- 9 plain-YAML resources + `deploy/k8s/README.md`
- Prerequisites documented (ingress controller, cert-manager, StorageClass)
- Backup options enumerated (Velero / pg_dump CronJob / postgres-operator)

### Observability

- `docker-compose.observability.yml` (opt-in overlay)
- `observability/` tree (Prometheus, Loki, Promtail, Grafana with
  pre-provisioned datasources + "Foundry Overview" starter dashboard)
- macOS Colima promtail-path override documented inline

### App-side

- `crates/foundry-app/src/metrics_server.rs` (sidecar metrics listener)
- `crates/foundry-app/src/main.rs` (recorder install, JSON tracing default)

### Process docs

- `RELEASING.md` (semver, tag-driven flow, cosign verification,
  keep-a-changelog format, backout procedure)
- `CONTRIBUTING.md` additions (CI section, `cargo xtask ci` local
  replication, dependabot/auto-merge operator steps, RELEASING pointer)
- `xtask ci` subcommand with `FOUNDRY_XTASK_INCLUDE_DOCKER=1`
  opt-in for the slow `@docker-compose` group

## Verification at HEAD (`c7cb715`)

- `cargo xtask ci` → all gates green
- 55/55 acceptance scenarios (52 default-tag + 3 `@docker-compose`)
- `kubectl apply --dry-run=client` over `deploy/k8s/` passes
- Reviewer (`nw-platform-architect-reviewer`) — CONDITIONALLY_APPROVED;
  all 5 follow-up fixes applied:
  1. IMAGE_NAME org-confirmation step in RELEASING
  2. K8s backup guidance enumerated in `deploy/k8s/README.md`
  3. Forgejo runner backend hint added
  4. `xtask ci` auto-detects Docker + exports `DOCKER_HOST`
  5. Colima promtail-path override documented

## Lessons learned

1. **Test architectural claims by writing the artefact.** The
   K8s-translatability claim in `topology.md` was a slice-1
   prediction. The DEVOPS slice verified it manifest-by-manifest
   and surfaced two minor notes (ingress-controller annotations,
   inherited Postgres SPOF). A claim untested by an artefact is a
   guess.
2. **Empty dashboard panels are the right "instrument me" signal.**
   Shipping panels that reference metric names ahead of
   instrumentation creates a visible regression target. Slice 3+
   handlers landing will make graphs light up automatically —
   no separate "wire up dashboards" task needed.
3. **`cargo xtask ci` retires the "CI passes but my machine
   doesn't" class of bug.** Auto-detecting Docker via context (not
   hard-coding `unix:///var/run/docker.sock`) covers macOS Colima /
   OrbStack out of the box.
4. **Plain YAML is the right shape for v0.1 K8s.** Helm/Kustomize
   templating would have to predict overrides we don't yet have
   data for. The 9 plain manifests are short, readable, and the
   eventual chart will be a derivation of them.
5. **Observability is opt-in for a reason.** Bundling
   Prometheus/Loki/Promtail/Grafana would have added an extra
   compose file to the contributor onboarding contract for zero
   slice-1 benefit. The overlay model preserves slice-1's
   "Postgres + Foundry, nothing else" promise.

## Issues encountered

- **DELIVER ran via `/nw:devops`, not the nWave execute
  orchestrator.** No `deliver/roadmap.json` or
  `deliver/execution-log.json` was emitted. Audit trail is the
  `c7cb715` commit body + `devops/plan.md`.
- **Open questions for next DEVOPS slice (from `plan.md` § 117–132):**
  - IMAGE_NAME org confirmation before first tag push (or override
    `IMAGE_NAME` env in `release.yml`) — RESOLVED in CONDITIONALLY_APPROVED
    follow-up.
  - Whether to ship `.github/workflows/dependabot-auto-merge.yml` or
    keep auto-merge as documented operator opt-in.
  - Whether to ship a starter `kustomization.yaml` per ingress
    controller now, or wait for v0.4 Helm/Kustomize.

## Permanent artefact locations

`docs/feature/foundry-devops/devops/plan.md` is preserved in place —
zero external references, but the DEVOPS-wave plan is the SSOT for
this slice's design rationale (filling the role
`design/architecture.md` plays for slice 1). The shipped artefacts
themselves live at their intended permanent locations
(`.github/workflows/`, `.forgejo/workflows/`, `deploy/k8s/`,
`observability/`, `RELEASING.md`, `crates/foundry-app/src/metrics_server.rs`)
— DEVOPS is the wave where "permanent location" and "delivery
location" naturally coincide.

## Open items for v0.1 RC

1. **`dependabot-auto-merge.yml`** — decide ship vs. keep documented
   opt-in before v0.1.
2. **Per-controller ingress kustomization** — defer to v0.4
   Helm/Kustomize per ADR-102, but ensure operator-facing docs
   surface the swap-annotation pattern at first-deploy time.
3. **Slice-3+ handler instrumentation** — the dashboard panels are
   waiting; instrument `http_requests_total`,
   `db_connections_in_use`, `sse_subscribers_total` so the empty
   series resolve.
4. **`CHANGELOG.md`** — RELEASING.md mandates keep-a-changelog
   format; the file itself is created on first v0.2.0 tag.
5. **Pre-commit hooks** — small follow-up; `cargo xtask ci` covers
   the local-gate story for now.
