# Changelog

All notable changes to Foundry are documented here. The format follows
[keep-a-changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org). Pre-1.0 releases permit
minor-version breaking changes, flagged with a `BREAKING` heading.

## [Unreleased]

### Added

- **Close (×) control on the issue edit dialog.** The edit dialog's header now
  carries a conventional close button (accessible name "Close", ≥24×24 px
  target, visible focus ring) so a pointer user can leave without saving —
  previously the only no-save exit was the unadvertised Esc key. One close
  mechanism, two triggers: the button is a declarative
  `data-action="close-modal"` trigger resolved by a single document-delegated
  click listener calling the existing `closeModal()`; Esc handling is
  unchanged (single-owner, BR-4). Dismissal issues no save request; typed
  edits are discarded, exactly as Esc does. Stylesheet content-hash rotated
  `7c858984` → `8ce38566`. See
  `docs/product/architecture/adr-modal-close-001-declarative-close-trigger.md`.

## [v0.3.1] - 2026-05-30

Test-suite and release-pipeline hardening. **No functional/runtime changes** —
the binary is behaviorally identical to v0.3.0; all six crates bump `0.3.0` ->
`0.3.1` for the release marker.

### Changed (internal)

- **Acceptance `@all` lane flake eliminated.** The shared Postgres testcontainer
  serves no TLS, but the connection URLs set no `sslmode`, so sqlx's default
  `prefer` SSL probe intermittently failed under the concurrent connect-storm
  (`SSLRequest: 0x00`) and starved the harness pool → `PoolTimedOut` on Background
  seed inserts. Disabled the probe (`ssl_mode=Disable`) and raised the pool
  `acquire_timeout` 5s -> 30s. Validated 5/5 green and a controlled A/B (reverted
  3/5 flaked vs fixed 0/5). Test-infrastructure only.
- **Cargo dependency-graph SBOM** (CycloneDX, 513 crates) now checked in at
  `sbom/crates.cdx.json` (deterministic via `sbom/generate.sh`) and attached to
  release images as a second cosign attestation alongside the image SBOM.

### Tests

- Added `foundry-store`'s first integration test: a cross-schema regression guard
  for `Store::probe()`'s `current_schema()` scoping (a sibling schema's columns
  must not mask a half-migrated active schema). Closes the gap that the
  string-literal fix couldn't be mutation-tested.

## [v0.3.0] - 2026-05-29

Slice 8 — the deferred observability metrics — plus a startup-probe
correctness fix and mutation-test hardening of the slice-8 coverage. All six
crates `0.2.0` -> `0.3.0`.

### Added

- **Slice 8 — deferred observability metrics.** Emits and dashboards the five
  catalog metrics that slice 6 reserved but left unproduced, so every
  "Foundry Overview" panel resolves to real data instead of "no data":
  - `outbox_pending_jobs` and `bootstrap_tokens_unclaimed` gauges, folded into
    the existing 5s pool-poll loop (no new task). (ADR-018)
  - `migration_apply_duration_seconds{migration_id}` histogram — one timing
    observation per migration that actually applies. (ADR-020)
  - `realtime_listen_disconnects_total` and `probe_failures_total{probe_name}`
    counters, incremented at their event call-sites (LISTEN reconnect; the
    `store`/`metrics` startup probes). (ADR-019)
  - Five new Grafana "Foundry Overview" panels; both labelled metrics carry
    bounded label sets.

### Fixed

- **`Store::probe()` scopes its migration-0006 column check to
  `current_schema()`.** It previously counted `comments` columns across every
  visible schema, so a half-migrated active schema could pass the startup
  probe whenever a sibling schema still carried the columns. No behaviour
  change in single-schema production deployments.

### Tests

- Feature-scoped mutation testing (cargo-mutants) of the slice-8 store/emit
  code closed three assertion gaps — the `migration_id` label *value*, and the
  `probe_failures_total` increment path — reaching a 100% kill rate on viable
  mutants. See `docs/feature/slice-8-deferred-metrics/deliver/mutation/`.

## [v0.2.0] - 2026-05-28

Initial public release of the Foundry MVP — a self-hostable, single-binary
issue tracker that `docker compose up`'s on a fresh machine. Bundles slices
1–7 plus the platform/DevOps slice and the observability hardening that
followed it.

### Added

- **Slice 1 — Backend MVP.** Operator install, admin bootstrap (signed
  token), user sign-in (argon2id, server-validated sessions, brute-force
  delay), project create, and issue file. Walking skeleton: fresh machine to
  filed issue in under an hour.
- **Slice 2 — Realtime collaboration.** US-09 realtime issue updates over
  SSE, US-10 markdown comments with sanitization, US-12 keyboard-driven
  navigation contracts.
- **DevOps / platform readiness.** GitHub Actions CI (4 parallel gates),
  tag-driven multi-arch (amd64 + arm64) container release with cosign keyless
  signing + SPDX SBOM, plain-YAML Kubernetes manifests, and an opt-in
  Prometheus/Loki/Promtail/Grafana observability overlay with a starter
  "Foundry Overview" dashboard.
- **Slice 3 — Operator-grade hardening.** US-02 multi-replica fan-out with
  shared sessions, US-03 backup/restore (`foundry doctor backup-verify`),
  US-04 rolling upgrade, US-11 attachments.
- **Slice 4 — Contributor onboarding (US-13).** README Quickstart pins a
  five-command `git clone` → green `cargo test` path with no Redis, no S3, no
  Node toolchain, and a clear too-old-Rust error.
- **Slice 5 — Comment edit/delete (US-10).** Authors edit/delete their own
  comments; workspace admins delete any; "edited" indicators and realtime
  disappearance; soft-delete tombstone preserves the moderation audit trail.
- **Slice 6 — Handler instrumentation.** A tower-middleware layer emits the
  5 metric series the Grafana dashboard references: `http_requests_total`,
  `http_request_duration_seconds`, `db_connections_in_use`, and the realtime
  subscriber gauge — register-at-0 so panels light up immediately.
- **Slice 7 — Comment tombstone GC + admin-undelete.** A daily background
  sweep hard-deletes comments tombstoned >90 days (advisory-lock-coordinated,
  per-run capped), emitting `comments_tombstones_purged_total` +
  `comments_tombstones_pending`. Operators recover in-window deletions with
  `foundry doctor restore-comment <UUID>`.

### Performance

- **argon2id password hashing runs off the async runtime.** `hash_password` /
  `verify_password` run their OWASP-grade CPU work on a blocking thread
  (`tokio::task::spawn_blocking`) so hashing never pins an async worker;
  sign-in stays responsive under concurrent load.

### Known issues / deferred

- Five metric series (`outbox_pending_jobs`, `bootstrap_tokens_unclaimed`,
  `migration_apply_duration_seconds`, `realtime_listen_disconnects_total`,
  `probe_failures_total`) are defined but not yet emitted; each needs a
  dashboard consumer first.
- Helm packaging is deferred to v0.4 (ADR-102); plain-YAML manifests ship
  today.
- A `comments_visible` SQL VIEW for defense-in-depth against missed
  soft-delete filters is deferred to v0.3 (ADR-017).

[Unreleased]: https://github.com/Canzan/foundry/compare/v0.2.0...HEAD
[v0.2.0]: https://github.com/Canzan/foundry/releases/tag/v0.2.0
