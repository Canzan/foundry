# Feature: comment-tombstone-gc
# Slice: 7 (closes ADR-007 v0.2 GC commitment + slice-5 D5 deferred
#          admin-undelete operator runbook)
#
# JTBD: outcome-2 (operators stand up Foundry and trust it to honour
#       its retention promise) + outcome-4 (in-Foundry discussion is
#       reversible within the audit window).
#
# Inheritance (slice 5 — comment-edit-delete):
#   - Soft-delete schema: `comments.deleted_at` + `deleted_by`
#     (migration 0006). ADR-007 § Decision committed the v0.2 GC at
#     the 90-day threshold using exactly this schema. NO new migration
#     this slice (confirmed via grep on migrations/).
#   - The `@soft-delete-invariant` scenario (slice-5 #9) continues to
#     hold; this slice ADDS the hard-delete sweep that runs 90 days
#     later + the operator-undelete affordance.
#
# Inheritance (slice 3 — foundry-operator-grade):
#   - The `foundry doctor backup-verify` CLI subcommand established
#     the `assert_cmd::Command::cargo_bin("foundry")` subprocess
#     pattern + the `foundry doctor <subcommand>` shape. Slice 7 adds
#     a sibling action `foundry doctor restore-comment <uuid>`.
#
# Inheritance (slice 6 — handler-instrumentation):
#   - Background `tokio::spawn` + `tokio::time::interval` task pattern
#     (the slice-6 pool-poll task in main.rs lines 160-183). Slice 7
#     spawns the GC task next to it. Per ADR-015 § hosting decision.
#   - The `metrics_exporter_prometheus` recorder + `/metrics` sidecar
#     are already wired; slice 7 ADDS 2 new bounded-cardinality metric
#     emissions (no new metric families beyond what slice-6 D0 deferred
#     list already named in spirit).
#
# Slice-7 driving adapters (per architecture.md):
#   - Background tombstone GC tokio task in main.rs — INTERNAL driver
#     (not externally invocable; observed via the GC's effect on the
#     `comments` table + emitted metrics + `/metrics` scrape).
#   - `foundry doctor restore-comment <uuid>` CLI subcommand — NEW
#     driving adapter exercised via subprocess.
#
# Driven adapters exercised (ALL reused; ZERO new infrastructure per
# DESIGN § Reuse Analysis):
#   - Postgres `comments` table write path (DELETE for GC; UPDATE for
#     undelete). Inherited slice-1 PgPool.
#   - Postgres advisory lock (`pg_try_advisory_lock(TOMBSTONE_GC_LOCK_ID)`).
#     Same shape as slice-1 `MIGRATION_LOCK_ID`; distinct literal so
#     `pg_locks` distinguishes them during operational triage.
#   - `metrics_exporter_prometheus` recorder + `/metrics` sidecar
#     (slice-6 DEVOPS wiring). Slice 7 adds 2 emissions; no parser
#     change required in `support/metrics_scrape.rs` (it already
#     handles arbitrary counter + gauge families).
#   - `assert_cmd::Command::cargo_bin("foundry")` subprocess driver
#     (slice-3 inherited; same mechanism as the existing infra-policy
#     row for `foundry doctor backup-verify`).
#   - Postgres per-scenario schema (slice-1 inherited).
#   - `reqwest::Client` against the foundry subprocess (slice-1
#     inherited via slice-6 subprocess pattern).
#
# Layer / PBT mode declaration (per nw-test-design-mandates Mandate 9):
#   - Layer 3+ (real subprocess + real Postgres + real testcontainers).
#   - Example-only. No proptest. Sad paths enumerated explicitly per
#     Mandate 11. PBT belongs at layers 1-2 (unit), which is DELIVER's
#     responsibility (the date-arithmetic SELECT predicate, the batch
#     loop termination, the advisory-lock try-acquire semantics, the
#     undelete UPDATE return-value mapping).
#
# Test invocation pattern (mirrors slice-6 subprocess pattern, NOT the
# slice-2/5 in-process pattern):
#   - The GC task runs as a real background task inside the foundry
#     binary; the only way to observe its effects honestly is to spawn
#     the foundry subprocess and let the task run. The acceptance
#     suite drives the cadence via FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1
#     (per DISTILL Q1 = A — env-var override; FakeClock extension
#     deferred until a second test needs it).
#   - The CLI subprocess scenarios use the slice-3 backup-verify shape
#     verbatim: `assert_cmd::Command::cargo_bin("foundry").args(["doctor",
#     "restore-comment", "<uuid>"])`.
#   - Per-scenario PG schema (slice-1 pattern); ephemeral METRICS_PORT
#     + FOUNDRY_PORT (slice-6 pattern).
#
# Two `@walking_skeleton` scenarios (mirrors slice-6 precedent — flagged
# as decision-driven invented detail #1):
#   - #1 covers the full GC tick path (spawn → tick → DELETE → counter
#     increment → gauge update — operator-visible "GC ran and removed
#     ancient tombstones").
#   - #7 covers the admin-undelete CLI path (spawn → CLI subprocess →
#     UPDATE → exit 0 — operator-visible "I restored an
#     accidentally-deleted comment").
#   Each is a structurally different end-to-end loop with distinct
#   operator-facing value. Slice-6 DD-11 set the precedent for two WS
#   in one feature file when the loops are independent.
#
# Time-warp fixture (per DISTILL Q4 = direct SQL):
#   - The slice-5 soft-delete handler always sets `deleted_at = now()`,
#     unhelpful for date-threshold testing. The slice-7 GC scenarios
#     insert tombstoned rows DIRECTLY via SQL with
#     `deleted_at = now() - interval '<N> days'` to span the 90-day
#     boundary. This is the test-only fixture; production code is
#     untouched.
#
# Failure-injection fixture (per DISTILL Q5 = extend mark_db_unreachable):
#   - The slice-3 `AppState::mark_db_unreachable` health-injection flag
#     (cfg-gated test hook) is extended in DELIVER to also cause
#     `Store::gc_tombstoned_comments` to return a synthetic
#     `StoreError::Sqlx(...)`. The "failure survives the task" scenario
#     uses this seam to verify the D7 = A "log + continue" precedent.
#
# CLI exit code contract (per DISTILL Q6 = consolidate to 4):
#   0 = restored        — UPDATE matched 1 row; `deleted_at` now NULL.
#   2 = invalid UUID    — argument did not parse as a UUID.
#   3 = DB connect fail — DATABASE_URL unreachable or auth failure.
#   4 = not restorable  — UPDATE matched 0 rows (comment not found OR
#                          comment exists but `deleted_at IS NULL`).
#   Codes are per-subcommand contracts (per architecture.md Constraint
#   9), not promised stable across other `foundry doctor` subcommands.

@slice7 @comment-tombstone-gc
Feature: Storage stays bounded and operators can recover an accidentally-deleted comment — the 90-day tombstone sweep removes old comment tombstones, advisory-locked single-replica execution prevents double-delete, observability tells operators the sweep is working, and the doctor CLI subcommand restores a comment whose deletion the operator regrets
  Foundry's privacy posture promises hard-deletion 90 days after a
  comment is soft-deleted. A background daily cleanup task sweeps
  tombstones older than 90 days, capped per run for safety against
  misconfigured timestamps. Concurrent replicas cooperate via a
  Postgres advisory lock — only one replica deletes per tick. Two
  bounded-cardinality metrics let operators alert on a stalled GC.
  Within the 90-day window, a workspace admin runs
  `foundry doctor restore-comment <uuid>` to undo a deletion;
  outside the window the row is gone and only backup-restore can
  recover it.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-3

  @walking_skeleton @real-io @gc-tick @nfr-obs-03
  Scenario: A daily tombstone sweep removes comment tombstones older than 90 days and increments the purged-total counter
    Given the operator's foundry instance is running with the tombstone sweep cadence set to 2 second
    And 3 ancient tombstoned comments exist on "AUTH-3" with deletion age 91 days
    When the operator's foundry instance has been running for at least 2 seconds
    And the operator scrapes the metrics endpoint
    Then the scrape body's "comments_tombstones_purged_total" sample has value 3
    And the issue page for "AUTH-3" shows 0 tombstoned comments older than 90 days

  @real-io @gc-threshold
  Scenario: The sweep keeps tombstones still inside the 90-day audit window
    Given the operator's foundry instance is running with the tombstone sweep cadence set to 2 second
    And 3 ancient tombstoned comments exist on "AUTH-3" with deletion age 91 days
    And 3 recent tombstoned comments exist on "AUTH-3" with deletion age 89 days
    When the operator's foundry instance has been running for at least 2 seconds
    Then the database holds 3 tombstoned comments on "AUTH-3"
    And the database holds 0 tombstoned comments older than 90 days on "AUTH-3"

  @real-io @gc-cap @slow
  Scenario: A single sweep tick deletes at most the per-run cap of tombstones; the remainder drain on the next tick
    Given the operator's foundry instance is running with the tombstone sweep cadence set to 6 second and per-run cap set to 10000
    And 11000 ancient tombstoned comments exist on "AUTH-3" with deletion age 91 days
    When the operator's foundry instance has been running for at least 7 seconds
    And the operator scrapes the metrics endpoint
    Then the database holds 1000 tombstoned comments older than 90 days on "AUTH-3"
    And the scrape body's "comments_tombstones_purged_total" sample has value 10000
    When the operator's foundry instance has been running for at least 6 seconds
    And the operator scrapes the metrics endpoint
    Then the database holds 0 tombstoned comments older than 90 days on "AUTH-3"
    And the scrape body's "comments_tombstones_purged_total" sample has value 11000

  @real-io @gc-lock
  Scenario: When two replicas attempt the sweep concurrently exactly one performs the work
    Given the operator's foundry instance is running with the tombstone sweep cadence set to 2 second
    And another replica is holding the tombstone-sweep advisory lock
    And 3 ancient tombstoned comments exist on "AUTH-3" with deletion age 91 days
    When the operator's foundry instance has been running for at least 2 seconds
    And the operator scrapes the metrics endpoint
    Then the database holds 3 tombstoned comments older than 90 days on "AUTH-3"
    And the scrape body's "comments_tombstones_purged_total" sample has value 0
    When the other replica releases the tombstone-sweep advisory lock
    And the operator's foundry instance has been running for at least 2 seconds
    And the operator scrapes the metrics endpoint
    Then the database holds 0 tombstoned comments older than 90 days on "AUTH-3"
    And the scrape body's "comments_tombstones_purged_total" sample has value 3

  @real-io @gc-failure
  Scenario: A transient sweep failure does not kill the background task and the next tick succeeds
    Given the operator's foundry instance is running with the tombstone sweep cadence set to 2 second
    And the next tombstone sweep tick will fail with a synthetic database error
    And 3 ancient tombstoned comments exist on "AUTH-3" with deletion age 91 days
    When the operator's foundry instance has been running for at least 2 seconds
    Then the database holds 3 tombstoned comments older than 90 days on "AUTH-3"
    And the foundry subprocess is alive
    When the synthetic database error is cleared
    And the operator's foundry instance has been running for at least 2 seconds
    And the operator scrapes the metrics endpoint
    Then the database holds 0 tombstoned comments older than 90 days on "AUTH-3"
    And the scrape body's "comments_tombstones_purged_total" sample has value 3

  @real-io @gc-metrics @nfr-obs-03
  Scenario: The pending-tombstones gauge reflects the count of comments awaiting deletion at each tick
    Given the operator's foundry instance is running with the tombstone sweep cadence set to 4 second and per-run cap set to 2
    And 5 ancient tombstoned comments exist on "AUTH-3" with deletion age 91 days
    When the operator's foundry instance has been running for at least 5 seconds
    And the operator scrapes the metrics endpoint
    Then the scrape body contains the line "comments_tombstones_pending"
    And the scrape body's "comments_tombstones_pending" sample has value 3
    When the operator's foundry instance has been running for at least 4 seconds
    And the operator scrapes the metrics endpoint
    Then the scrape body's "comments_tombstones_pending" sample has value 1
    When the operator's foundry instance has been running for at least 4 seconds
    And the operator scrapes the metrics endpoint
    Then the scrape body's "comments_tombstones_pending" sample has value 0

  @walking_skeleton @real-io @driving_adapter @admin-cli
  Scenario: An operator restores an accidentally-deleted comment by running the doctor subcommand
    Given the operator's foundry instance is running
    And a tombstoned comment "abandoned-thought" exists on "AUTH-3" with deletion age 5 days authored by Mei
    When the operator runs `foundry doctor restore-comment <comment-id>` as a subprocess against the live database
    Then the doctor subprocess exits with code 0
    And the doctor subprocess stdout contains "status: restored"
    And the database holds 0 tombstoned comments on "AUTH-3"
    And the issue page for "AUTH-3" shows a comment by Mei containing the text "abandoned-thought"

  @real-io @error @admin-cli
  Scenario: An operator who passes a UUID that does not match any tombstoned comment gets a non-zero exit
    Given the operator's foundry instance is running
    When the operator runs `foundry doctor restore-comment <missing-uuid>` as a subprocess against the live database
    Then the doctor subprocess exits with code 4
    And the doctor subprocess stderr mentions "not restorable"

  @real-io @error @admin-cli
  Scenario: An operator who passes a malformed UUID gets the invalid-argument exit code
    Given the operator's foundry instance is running
    When the operator runs `foundry doctor restore-comment not-a-uuid` as a subprocess against the live database
    Then the doctor subprocess exits with code 2
    And the doctor subprocess stderr mentions "invalid UUID"
