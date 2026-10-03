# Changelog

All notable changes to Foundry are documented here. The format follows
[keep-a-changelog](https://keepachangelog.com/en/1.1.0/) and the project
follows [Semantic Versioning](https://semver.org). Pre-1.0 releases permit
minor-version breaking changes, flagged with a `BREAKING` heading.

## [Unreleased]

## [v0.6.0] - 2026-10-03

Card drag on phones and tablets (card-pointer-drag slices 02 and 03, which
completes the feature), opt-in Keycloak role provisioning, a per-workspace
`list-users`, and a fail-closed `backup-verify`. `git log v0.5.0..v0.6.0` is
the full list.

### Added

- **Cards drag on touch and pen.** Every card carries a grip. A touch or pen
  press on the grip lifts the card once it has travelled 3 px, with no wait and
  no page scroll; a tap on the grip opens nothing. A press on the card's text
  is a hold: the card visibly arms (it eases to 96% scale and 70% opacity over
  the 500 ms hold, dim-only under `prefers-reduced-motion`) and then lifts. A
  swipe that starts on the text still scrolls the page or the board, and a tap
  still opens the card. Once lifted, touch carries the card exactly as the
  mouse does: the lane lights, the marker shows the landing slot, and the drop
  sends the same move request. Cards gain a 48 px minimum height so the grip is
  easy to hit on one-line cards. Stylesheet content-hash rotated `f9143163` →
  `7fa13f60` (the grip) → `6b3e4436` (the arming cue).
- **Carried cards scroll the board and the page at the edges.** Holding a
  carried card within 48 px of the board's side edge scrolls the board towards
  the off-screen lanes, 14 px per frame, stopping at the board's end; holding
  it near the top or bottom of the window scrolls the page, so a long lane's
  end is reachable on a phone. The marker and the landing slot are recomputed
  after every scroll step. This works for mouse, touch and pen.
- **Opt-in provisioning of Keycloak realm-role holders.** Set
  `FOUNDRY_OIDC_PROVISION_ROLE` to a realm role name. A verified identity with
  no Foundry account whose ID token carries that role (exact, case-sensitive,
  realm roles only) gets a password-less `member` account in the instance's
  original workspace on first sign-in. It is never made an admin. Unset or
  blank keeps the previous link-only behaviour. A missing role, an unconfirmed
  address or provisioning switched off all return the same generic refusal as
  a wrong password. Password-less members can set a password through
  forgot-password.
- **`foundry doctor list-users --workspace <id|name>`** lists one workspace's
  members, each with a `workspace-role: admin|member` line. The selector is
  the same as `export-workspace`'s: an id, or an exact case-insensitive name.
  It exits 2 when no workspace matches and 3 when the database is unreachable.

### Changed

- **Interruptions leave nothing behind on touch.** When the system takes the
  pointer from a carried card (a notification shade, a second gesture), the
  card goes back to its exact slot and nothing is sent. A hold the system
  interrupts before the lift leaves no arming cue, and a release off every lane
  changes nothing.

### Migration notes

- **`0016_nullable_password_hash`**: `ALTER TABLE users ALTER COLUMN
  password_hash DROP NOT NULL`. This is a catalog-only change with no backfill,
  so it runs instantly at any size. It is safe for rolling deploys: v0.5.0
  never writes a NULL hash. It becomes **one-way** once role provisioning
  creates a password-less account, because v0.5.0 cannot read a NULL hash and
  `NOT NULL` cannot be restored while one exists. Rolling back after that is a
  forward migration: delete those accounts or give them passwords first. If
  `FOUNDRY_OIDC_PROVISION_ROLE` stays unset, no NULL row is ever written and a
  rollback to v0.5.0 stays possible.

### Fixed

- **`foundry doctor backup-verify` fails closed.** It used to print
  `status: OK` and exit 0 when it could not count a single row: its row
  counts shelled out to a bare `psql`, and every failure (no `psql` on PATH,
  an unreachable probe, a dump of the wrong database) was swallowed as "table
  not present". Row counts and the probe-schema `DROP` now run in-binary over
  sqlx with quoted identifiers, so `pg_restore` is the only external tool.
  Only a genuinely absent table is skipped. **Runs that used to report a false
  OK now exit non-zero:** 8 when the probe database is unreachable for row
  counts, 9 when counting a table that exists fails, 10 when the restored
  schema holds none of the known Foundry tables ("not a Foundry backup?").
  CI no longer installs `postgresql-client-16`.

## [v0.5.0] - 2026-09-26

The release footer and the first slice of Pointer Events card drag. The
board-lane-reorder finalize and the card-pointer-drag DISCUSS/DESIGN/DISTILL
commits in this range are docs and pending tests only; `git log
v0.4.0..v0.5.0` is the full list.

### Added

- **Release version in the page footer.** Every full page, signed in or out,
  now ends with a small muted `Foundry vX.Y.Z` footer, so an operator can see
  which release is serving without shell access. The version is
  `foundry-app`'s `CARGO_PKG_VERSION`, exposed as `views::RELEASE_VERSION` and
  read from `base.html`; htmx fragments never extend the base template and
  carry no footer. The footer sits in the app shell's bottom padding, so no
  authed page (the board included) gains a scrollbar. Stylesheet content-hash
  rotated `438142d2` → `f9143163`.

### Changed

- **Card drag runs on Pointer Events for the mouse.** `board-dnd.js` moves its
  event layer from HTML5 drag-and-drop to Pointer Events
  (ADR-BOARD-CARD-004): a primary-button press on a card lifts past 6 px, the
  card stays in its slot while a fixed clone ghost follows the pointer, and
  release over a lane lands at the live marker with the same POST as before.
  Release off every lane, Escape (through `keyboard.js::closeTopLayer()`), or
  `pointercancel` after the lift ends the drag cancelled. One gesture means one
  thing: a lifted release never also opens the card dialog, only the primary
  button starts a drag, and a card drag never moves a lane (nor a header drag a
  card). Desktop files dragged onto the board are still swallowed. Touch and
  pen come in a later release (card-pointer-drag slices 02–03). Stylesheet
  content-hash rotated `f7c36a08` → `438142d2`.

### Changed (internal)

- **`check-arch` forbids keydown listeners in `board-*.js`.** Escape has one
  owner, `keyboard.js::closeTopLayer()` (BR-4); the new rule flags any
  `addEventListener("keydown"`, `.onkeydown =` or `on("keydown"` registration
  in a board script (comments stripped, a missing js directory fails closed).

## [v0.4.0] - 2026-09-19

First tagged release since v0.3.1: everything on `main` since then, including
the card drag-and-drop landing indicator, the issue-card delete fixes, native
OIDC sign-in, and the rustls bump for RUSTSEC-2026-0285. The entries below
are the ones recorded at the time; `git log v0.3.1..v0.4.0` is the full list.

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

[Unreleased]: https://github.com/Canzan/foundry/compare/v0.5.0...HEAD
[v0.5.0]: https://github.com/Canzan/foundry/compare/v0.4.0...v0.5.0
[v0.4.0]: https://github.com/Canzan/foundry/compare/v0.3.1...v0.4.0
[v0.2.0]: https://github.com/Canzan/foundry/releases/tag/v0.2.0
