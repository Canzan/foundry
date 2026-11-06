# DEVOPS Decisions — instance-integration-setup

Read before writing:
- ✓ discuss/wave-decisions.md, outcome-kpis.md, user-stories.md "System Constraints"
- ✓ design/wave-decisions.md ("For DEVOPS"), upstream-changes.md
- ✓ ADR-IIS-001 to -005; brief.md "Instance integration setup" (For Acceptance Designer)
- ✓ docs/product/kpi-contracts.yaml; integrations-wayfinding devops/wave-decisions.md (precedent)
- ✓ AGENTS.md (Deployment independence), RELEASING.md, DEVELOPER.md "The local gate"
- ✓ Dockerfile, docker-compose.yml, deploy/k8s/ (deployment, secret example, README),
  .github/workflows/{ci,release}.yml, .forgejo/workflows/
- ✓ `run_migrator_timed` (foundry-store), `probe_secrets` (main.rs), tests/acceptance.rs lane filters,
  compose_harness.rs module docs, us-01-install.feature
- ⊘ an in-repo alert rule file: none exists; alert rules are operator configuration

None of these decisions contradicts DESIGN. Nothing was built, deployed or pushed.

Decisions fixed by the existing platform, not asked again:

| # | Decision | Value (existing) |
|---|----------|------------------|
| 1 | Deployment target | Any standalone deploy: single binary, docker compose, plain Kubernetes (AGENTS.md) |
| 2 | Orchestration | Whatever the operator runs; the repo ships the compose file and the plain k8s example |
| 3 | CI/CD | `ci.yml` and `release.yml` (GitHub) plus the Forgejo mirror build; the local gate is `cargo xtask ci` in the isolated Lima VM |
| 4 | Existing infra | Reused; this feature adds no workflow, job or service |
| 5 | Observability | Prometheus metrics on the sidecar listener; no product analytics, no log store assumed |
| 6 | Deployment strategy | Rolling, `maxUnavailable: 0`; a refused start stalls the roll with the old pods serving |
| 7 | Continuous learning | None |
| 8 | Branching | Trunk-based on `main`, no PRs; commit after `xtask smoke`, push after `xtask ci` |
| 9 | Mutation testing | Per feature (CODING_STANDARDS.md): scoped to the files the delivery modified, kill rate ≥ 80% |

## Key Decisions

- [D1] **Image.** The runtime stage is distroless (no shell, no `mkdir`). The builder stage makes
  an empty directory and the runtime stage copies it with
  `COPY --from=builder --chown=65532:65532 --chmod=0700 … /var/lib/foundry`, then
  `VOLUME /var/lib/foundry`. Mode 0700 on the directory, 0600 on the file (ADR-IIS-002).
  - A named volume is populated from the image on first mount, so it inherits the `nonroot`
    owner. A host bind mount is not: the operator chowns it to 65532 or Foundry shows I-27.
  - `docker run` without `-v` gets an anonymous volume: the key survives a container restart,
    not a container re-creation. The docs say so.
- [D2] **Compose.** `docker-compose.yml` mounts a named volume `foundry-data` at
  `/var/lib/foundry`. Scaled replicas of the `foundry` service share it, so they share one key
  file; create-if-absent makes exactly one.
  - `--scale foundry=N` (N > 1) needs `FOUNDRY_HOST_PORT=0`, because the default maps a fixed host
    port (`docker-compose.yml:61`); the docs say so.
  - `docker compose down -v` deletes `foundry-data` (the key) **and** `foundry-pg-data` (the
    database): the backup note says to back up both first.
  - A named volume keeps us-01's "zero host-bind mounts" scenario true, but breaks
    `@nfr-port-01`'s "the only persistent volume is a named volume backing postgres". That Then
    line is amended to allow exactly two named volumes, postgres data and Foundry data (DELIVER
    task 3; recorded here as an intended change to a shipped scenario).
- [D3] **Kubernetes example.** The operator instance key goes in `foundry-app-secrets` as
  `FOUNDRY_INSTANCE_KEY` (and `_PREVIOUS`, empty), replacing `FOUNDRY_SECRETS_KEY` in the
  template.
  - No volume is added at `/var/lib/foundry`. A per-pod `emptyDir` would mint a different key per
    pod (I-26 on all but one). Left on the read-only root filesystem, a pod with no operator key
    shows I-27 on every replica: uniform and explained.
  - README: generate once (`openssl rand -base64 32 | tr '+/' '-_' | tr -d '=\n'`, R-1), keep an
    offline copy apart from database backups, apply the Secret, roll. The alternative, one
    shared `ReadWriteMany` volume on every replica, is named but not shipped.
  - An existing Secret that still carries `FOUNDRY_SECRETS_KEY` keeps working. Adding
    `FOUNDRY_INSTANCE_KEY` with a different value refuses start (the roll stalls, old pods serve).
- [D4] **Configuration docs** (RELEASING.md "Integrations" table and R-1, R-2, R-5; k8s README;
  secret example):
  - `FOUNDRY_INSTANCE_KEY`: 32 random bytes, base64url, padded or not; generated once.
  - `FOUNDRY_INSTANCE_KEY_PREVIOUS`: empty except while the instance key changes.
  - `FOUNDRY_SECRETS_KEY` / `_PREVIOUS`: legacy aliases, still honoured; the same value under
    both names counts as the legacy name (direct mode).
  - `FOUNDRY_DATA_DIR`: default `/var/lib/foundry`; holds `instance.key` only when no operator
    key is set.
  - `FOUNDRY_INTEGRATIONS_RELOAD_MS`: leave unset (default 5000); a test knob, like
    `FOUNDRY_INTEGRATIONS_POLL_MS`.
  - Start is refused (`health.startup.refused`, names only) for a malformed operator key or the
    two names set to different values. An empty value is unset, under either name, as today.
  - Secrets key creation and the Slack app move to the UI (`/admin/instance/integrations`); the
    env `SLACK_*` trio still wins when all three are set.
  - **Backup.** Back up the instance key (the Secret value, or `instance.key` via
    `docker compose cp foundry:/var/lib/foundry/instance.key .`) apart from the database dump. A
    dump alone opens nothing. Keep the key of every period whose dumps you keep: a dump restored
    under a later instance key shows I-26 until that key is set as `_PREVIOUS`.
- [D5] **Observability.** No new metric or log event (DESIGN: none required). Today both alert
  inputs are set only at boot (`main.rs:504-507`). DELIVER makes the reload path (ADR-IIS-004 r4)
  update them too: `foundry_integration_secrets_degraded` follows each applied reload's outcome,
  and a reload that reaches I-26 increments `probe_failures_total{probe_name="secrets"}` and
  logs the degraded line (task 8). Recommended alert rules are documented in RELEASING R-8 as
  operator configuration:
  - `max(foundry_integration_secrets_degraded) == 1 for 15m` (warning). The 15 m tolerates the
    Generate/Move roll window, where a previous-release replica cannot open the secrets key.
    Persisting past the roll means a replica holds a different instance key (I-26), or an
    earlier-release replica is still running.
  - `increase(probe_failures_total{probe_name="secrets"}[15m]) > 0` (info): a boot or reload
    probe failed. After Generate/Move it is expected only from previous-release replicas that
    restart before the roll replaces them; one that keeps running shows nothing until its next
    boot (its deliveries still wait as `Transient(secret_unavailable)`).
  - Reload lag and replica heartbeat have no metric. The runbook uses a read-only store query on
    `integration_replica_states`: a replica whose `applied_version` is below
    `integration_config_version` for more than 60 s, or whose `beat_at` is older than 30 s, is
    stale. A replica of an earlier release writes no row. Rows older than 1 h are pruned.
  - Runbook lines for I-27 (directory not writable: check the volume and its owner), I-31 (restore
    `instance.key` or set `FOUNDRY_INSTANCE_KEY`, restart) and I-30 (operator key took over the
    file: nothing to do; the file may be removed after a backup of it).
- [D6] **Rollout.** Rolling, through the existing path. Migration 0033 adds tables only and is
  forward-only (no down migration, like every earlier one).
  - Previous-release binaries boot after it: `run_migrator_timed` iterates the binary's own
    migrations and never rejects an applied version it does not know.
  - **Upgrade with `FOUNDRY_SECRETS_KEY`:** zero-touch. The instance stays in direct mode (I-2e),
    deliveries continue. Keep `FOUNDRY_SECRETS_KEY` as it is: the previous release reads only that
    name, so renaming early removes the rollback target's key. Press Move only after every
    replica runs the new release (the orchestrator's rollout status; old replicas are invisible to
    `integration_replica_states`). Rename to `FOUNDRY_INSTANCE_KEY` (same value) only after Move
    has soaked and the rollback window has closed.
  - **Fresh or file mode:** Generate after the roll completes; same advice.
  - Release notes carry `### Migration notes`: 0033 new tables, seconds, safe for rolling deploys.
- [D7] **Rollback.**
  - Before any Generate, Move or Slack-app save: redeploy the previous release. Lossless; the new
    tables are ignored.
  - After Generate or Move: the previous release knows only the environment key. Credentials
    sealed under the secrets key do not open: `foundry_integration_secrets_degraded` reads 1 and
    deliveries wait as `Transient(secret_unavailable)` (not marked lost, no email). In file mode
    the previous release has no key at all. A saved Slack app is ignored (W-2) unless the env
    trio is set; inbound Slack events during the rollback answer 401.
  - Recovery is roll forward. Credentials, connections and issues are intact and every sealed row
    opens again. Deliveries still pending resume; any that reached the 24 h give-up meanwhile are
    already `failed` / `gave_up_24h` (the expiry sweep) and are **not** resumed. R-6 skips stale
    events instead. Do not run the previous release's `doctor rotate-secrets` (it exits 4 and
    changes nothing).
  - Hence Move and Generate are the point after which rollback costs integration deliveries, not
    board data. The operator presses them after the release has soaked. The RELEASING rollback
    note (task 5) says the same.
- [D8] **Test lanes.**
  - All scenarios live in `@instance-integration-setup` features and run in the default lane and
    in `all`. `cargo xtask ci` runs acceptance only with `FOUNDRY_XTASK_INCLUDE_DOCKER=1`
    (`xtask/src/main.rs:225-236`); without it the gate is green with zero acceptance runs. Every
    gate run that counts as a KPI measurement sets it. None is `@docker-compose`; Postgres comes
    from testcontainers, as for every acceptance run.
  - Single-replica (`InProcHarness`): US-IIS-01, 03, 04, 06, most of 08; KPI-IIS-2, 3, 4, 5, G1, G2.
  - Two replicas (`MultiReplicaHarness`, shared `MockClock`, short reload): tag `@iis-replica`;
    KPI-IIS-1 and the walking skeleton (US-IIS-02 scenario 1).
  - Subprocess with a temp `FOUNDRY_DATA_DIR`: tag `@iis-subprocess`; KPI-IIS-6 and the
    shared-volume race. It spawns the binary from xtask ci's `cargo build --all --release`
    (`xtask/src/main.rs:181`). **Chosen:** that step sets
    `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false` itself (task 9), as `scripts/eip-lanes.sh:20`
    does, so the gate is self-sufficient on every host. It affects only build scripts and proc
    macros, not the shipped binary. Targeted single-tag runs keep using `eip_warm`.
  - Per-KPI tags `@kpi-iis-<n>`; select with `FOUNDRY_ACCEPTANCE_TAGS=kpi-iis-1` (single-tag
    mode). No new runner exclusion.
  - KPI-IIS-G3: `@external-integration-plugins` and `@integrations-wayfinding`, unchanged.
  - Image and volume shape: a `@docker-compose` scenario on the shared acceptance image runs
    `docker create`, then `docker cp -a` of `/var/lib/foundry` and `docker image inspect`, and
    asserts owner `65532:65532`, mode `0700` and `/var/lib/foundry` under `Config.Volumes`. It
    tests the built image, not the Dockerfile's text.
  - Reload alerting (D5): an `@iis-replica` scenario reaches I-26 on one replica through reload
    and asserts that replica's `/metrics` shows `foundry_integration_secrets_degraded 1` and a
    raised `probe_failures_total{probe_name="secrets"}`.
  - Gates: `xtask smoke` before commit; `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` in the
    Lima VM before push, bounded with `timeout`.
- [D9] **Mutation testing** (per feature, ≥ 80%): `key_file.rs`, the key mode and keyring
  decisions, `reload.rs`, the offer rules in `handlers/instance.rs`,
  `foundry-store/src/integrations/instance.rs` and the name and alias rules in `secret_box.rs`.
  The pure decisions (key mode, offers, conflicting names) matter most.
- [D10] **Release.** Deliverable A ships as **v0.19.0** (confirmed by the user 2026-11-06; minor: new capability, new settings,
  migration 0033) through the existing path (CHANGELOG, crate bump, `openapi.json` version, tag,
  `release.yml`, mirror build). Deliverable B ships later as its own minor (v0.20.0). Tag and push
  wait for the user's explicit go.
- [D11] **KPIs** use existing instrument kinds: `acceptance` for KPI-IIS-1 to 6, G1, G2 and G3,
  `store-query` as context for KPI-IIS-1 (replica states), `dogfood` for KPI-IIS-M. No runtime
  instrument. Recorded in kpi-contracts.yaml (status: designed).

## DELIVER tasks (not made here)

1. Dockerfile: data directory and `VOLUME` (D1). The new generated-key reader under
   `integrations-test-seam` **must** carry a marker string, and the Dockerfile gets a
   `! grep -q` line for it beside the two existing seam checks.
2. The `@docker-compose` image-shape scenario (D8).
3. docker-compose.yml: `foundry-data` named volume (D2), and amend us-01-install.feature
   `@nfr-port-01` "the only persistent volume is a named volume backing postgres" to the two
   named volumes.
4. deploy/k8s: secret example (D3), README "Integrations" section (D3, D4).
5. RELEASING.md: settings table, R-1, R-2, R-5 reworded, R-8 alerts and runbook (D4, D5), a
   rollback note after Move/Generate with D7's wording (24 h give-up not resumed, events 401),
   the keep-the-legacy-name advice (D6), and the compose scale and `down -v` notes (D2).
6. `.env.example`: commented `FOUNDRY_INSTANCE_KEY` and `FOUNDRY_DATA_DIR`.
7. CHANGELOG `### Migration notes` for 0033 at release (D6).
8. Reload path updates `foundry_integration_secrets_degraded` and
   `probe_failures_total{probe_name="secrets"}` (D5), with the `@iis-replica` scenario (D8).
9. xtask ci's release build step sets `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false` (D8).

## Infrastructure Summary

- Deployment: standalone (binary, compose, plain k8s); rolling; no new service.
- Storage: one small volume per deploy at `FOUNDRY_DATA_DIR`, only for the zero-config key file.
- CI/CD: unchanged workflows; local gates `xtask smoke` and `xtask ci`.
- Observability: existing `foundry_integration_secrets_degraded` and
  `probe_failures_total{probe_name="secrets"}`; replica state by store query.
- Mutation testing: per feature, ≥ 80% on the modified files.

## Constraints Established

- Every IIS scenario except the image-shape one runs without a compose image; a KPI measurement
  is a `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` run.
- KPI-IIS-6's "zero key settings" holds for compose and the single binary. On the plain k8s
  example a pod with no operator key shows I-27 until `FOUNDRY_INSTANCE_KEY` is set (D3).
- An empty `FOUNDRY_INSTANCE_KEY` or `FOUNDRY_SECRETS_KEY` is unset; only two non-empty,
  different values refuse start (the shipped k8s template depends on it).
- The k8s example never mounts a per-pod volume at `FOUNDRY_DATA_DIR`.
- Docs and examples name no hosting environment, host, cluster or repo.
- External steps are operator configuration only: put a generated `FOUNDRY_INSTANCE_KEY` in the
  deployment's secret, back it up, and add the D5 alert rules to the operator's Prometheus.

## Upstream Changes

- None to DISCUSS or DESIGN. The empty-means-unset rule (Constraints) makes explicit what the
  shipped secrets-key rule already does; DISTILL adds a scenario for it.
- Shipped scenario changed: us-01-install.feature `@nfr-port-01` (D2, task 3).

## Peer review (2026-11-06, iteration 1: needs_revision, then fixed)

| # | Finding | Resolution |
|---|---|---|
| 1 | Named volume breaks `@nfr-port-01` | D2, task 3 |
| 2a | Rollback overstated the 24 h give-up | D7, task 5 |
| 2b | Early rename removes the previous release's key | D6 |
| 3 | Alert inputs are boot-only | D5, D8, task 8 |
| 4 | Acceptance needs `FOUNDRY_XTASK_INCLUDE_DOCKER=1`; strip env for subprocess lane | D8, task 9 |
| 5 | Dockerfile-text test; optional seam marker | D8 image scenario; task 1 mandatory |
| 6 | Compose scale port, `down -v` deletes postgres | D2, task 5 |
| 7 | KPI-IIS-6 scope; check-arch rules not yet built | Constraints; kpi-contracts |
| 8 | "once per old replica" | D5 |
