# Story: US-04 — Operator upgrades in place
# Slice: 3 (operator-grade)
# JTBD: outcome-7 (Upgrade without breakage)
#
# Driving port: foundry_store::run_migrations invoked via the production
# AppState boot path on each of two concurrently-spawned replicas. The
# advisory lock (NFR-MIG-01) is what makes the concurrent boot well-
# defined; this feature pins that contract through observable outcomes
# (one replica blocks while the other migrates; both observe the new
# schema after release).
#
# Approach: Option B (see wave-decisions.md §US-04). The test harness
# writes a test-only migration file `0099_us04_test_<scenario>.sql` into
# a per-scenario temp migrations directory; the per-scenario AppState
# points `sqlx::migrate!` at that directory. This avoids cargo-feature-
# gated binaries while exercising the real advisory-lock + sqlx-migrate
# code path. "Old replica keeps serving old SQL during migration" is the
# expand-only discipline enforced by code review and the per-migration
# header (NFR-MIG-02); we do NOT auto-test that property — see deferred.
#
# Driven adapters exercised (Strategy C — all real):
#   - real testcontainers Postgres 16
#   - real sqlx::migrate::Migrator invoked via foundry_store::run_migrations
#   - real pg_advisory_lock / pg_advisory_unlock around the migration run
#   - real `_sqlx_migrations` row inspection for idempotency assertion
#
# NFR coverage: NFR-MIG-01 (forward-only + advisory-lock + idempotent),
# NFR-MIG-02 (failed migration leaves DB unchanged).
#
# Out of scope for slice 3 (deferred):
#   - Cross-version "old replica serves old SQL while new schema is being
#     applied" — the expand-only migration rule is enforced by code review
#     + per-migration header comments, not by black-box acceptance test.
#     The pattern would require two cargo-feature-gated binaries which
#     the team rejected as test-only complexity that does not earn its keep.
#   - Non-transactional migrations (CREATE INDEX CONCURRENTLY) — covered
#     by the migrations.md runbook with a manual-recovery procedure; not
#     auto-tested.
#
# Gherkin discipline (CM-B): scenarios speak in operator-deploy terms
# ("deploys a new version", "schema update", "migration history"). The
# specific implementation (advisory-lock id, `_sqlx_migrations` table,
# pg_advisory_lock syscalls) lives in step-method bodies and the comment
# block above, not in stakeholder-readable Gherkin.

@slice3 @us-04 @rolling-upgrade
Feature: An operator deploys a new Foundry version and the schema update applies exactly once even when replicas race to start
  An operator rolls out a new Foundry version. Two replicas of the new
  version boot simultaneously against one database that needs a schema
  update. The replicas serialise on a migration lock; one wins, applies
  the schema update, commits, and releases. The other replica blocks on
  the lock, then observes the schema is already current and proceeds.
  The schema update is applied exactly once. Both replicas reach a
  healthy /readyz. A failed schema update rolls back leaving the schema
  unchanged; the replica that attempted it exits non-zero. Restarting
  an already-migrated replica is a no-op.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And the database is at schema version 0001

  @walking_skeleton @real-io @driving_adapter @nfr-mig-01
  Scenario: An operator deploys a new version and two replicas start in parallel; the schema update applies exactly once and both replicas come up healthy
    Given the new Foundry version ships a forward-compatible schema update labeled "0099" that adds a new optional field to the issues domain
    When the operator starts 2 replicas of the new version simultaneously against the same database
    Then exactly one replica reports having applied schema update "0099"
    And the other replica reports having observed schema update "0099" as already-applied
    And both replicas reach a healthy /readyz within 30 seconds
    And the new optional field is present in the issues domain on both replicas

  @real-io @nfr-mig-01
  Scenario: Restarting an already-upgraded replica applies no further schema updates
    Given a replica has already applied schema update "0099"
    When that replica is stopped and restarted against the same database
    Then the replica reports zero schema updates executed during this boot
    And the replica reaches a healthy /readyz within 30 seconds
    And the migration history records exactly one application of schema update "0099"

  @real-io @error @nfr-mig-02
  Scenario: A schema update that fails rolls back and leaves the schema unchanged; the replica exits non-zero
    Given the new Foundry version ships a broken schema update labeled "0099" that references a non-existent table
    When a replica boots and attempts to apply schema update "0099"
    Then the replica reports a schema-update error and exits with a non-zero status
    And the migration history records no application of schema update "0099"
    And every previously-applied schema update is unchanged

  @real-io @nfr-mig-01
  Scenario: A replica racing for the migration lock blocks until the holder releases, then proceeds without error
    Given the new Foundry version ships a schema update labeled "0099" that takes about 2 seconds to apply
    When the operator starts 2 replicas of the new version simultaneously and the first replica acquires the migration lock
    Then the second replica's boot is blocked for between 1500 and 3000 milliseconds
    And after the first replica releases the lock the second replica observes the schema update as already-applied
    And both replicas reach a healthy /readyz within 30 seconds
