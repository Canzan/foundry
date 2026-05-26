# DISTILL Driver Design — Slice 7 Acceptance Harness (comment-tombstone-gc)

Owner: acceptance-designer (DISTILL). Companion: `step-skeletons.md`,
`coverage-matrix.md`, `wave-decisions.md`. This document is an
**additive delta** to:

- `docs/feature/foundry-backend-mvp/distill/driver.md` (slice 1)
- `docs/feature/foundry-realtime-collab/distill/driver.md` (slice 2 — SSE consumer + HTML assertions)
- `docs/feature/foundry-operator-grade/distill/driver.md` (slice 3 — multi-replica + backup-restore + attachments + `assert_cmd` subprocess pattern)
- `docs/feature/foundry-contributor-onboarding/distill/driver.md` (slice 4 — subprocess walking skeleton)
- `docs/feature/comment-edit-delete/distill/driver.md` (slice 5 — zero new infra; additive step file only)
- `docs/feature/handler-instrumentation/distill/driver.md` (slice 6 — subprocess test harness + `support/metrics_scrape.rs` Prometheus parser)

Everything not mentioned here is inherited unchanged.

## 1. What slice 7 reuses (overwhelming majority)

| Adapter / helper | Reused from | Slice-7 use |
|---|---|---|
| `support::harness::ensure_postgres()` + `fresh_schema_pool_with_url()` | slice 1 `harness.rs` | All 9 slice-7 scenarios use a per-scenario PG schema. The schema URL is passed to the foundry subprocess via `DATABASE_URL` env (slice-6 pattern). |
| Testcontainers Postgres-16 container | slice 1 | Same shared container; no new resource pressure beyond slice-6's per-scenario subprocess. |
| `assert_cmd::Command::cargo_bin("foundry")` | slice 3 (`us_03_backup_restore.rs` line 456) | Slice-7 admin-undelete CLI scenarios (#7, #8, #9) reuse this verbatim against the new `restore-comment` subcommand. NO new dep; NO new policy row. |
| Slice-6 subprocess pattern (`FoundrySubprocess::spawn`) | slice 6 (`steps/handler_instrumentation.rs`) | All 9 slice-7 scenarios spawn a foundry subprocess. The 6 GC scenarios add the `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1` override + (for #3, #6) the per-run cap override; the slice-6 helper already accepts arbitrary env vars per its `spawn(database_url_with_schema, db_schema)` signature — DELIVER adds an env-overrides arg or analogous mechanism. |
| `support/metrics_scrape.rs` | slice 6 | Slice-7 GC scenarios (#1, #3, #6) scrape `/metrics` for the 2 new metric families (`comments_tombstones_purged_total` counter + `comments_tombstones_pending` gauge). The parser is generic over metric NAME; no parser change required. |
| HTML scraping helpers (`support/html_assertions.rs`) | slice 2 | Slice-7 WS #1 issue-page assertion ("shows 0 tombstoned comments older than 90 days") + WS #7 issue-page assertion ("shows a comment by Mei containing the text 'abandoned-thought'") reuse `parse`, `select_all`, `assert_comment_has_element_with_text`, etc. |
| `reqwest::Client` | slice 1+ | Used inside `metrics_scrape` + for issue-page GETs against the subprocess. |
| `signed_in_post` / sign-in helper (slice-2 + slice-5 `us_10_comments::sign_in_and_capture_cookie`) | slice 2 + slice 5 | Slice-7 WS #7 needs to GET the issue page authenticated as Mei to assert the restored comment is rendered. The helper pattern is inherited; DELIVER may extract a shared `support/auth_helper.rs` per the slice-5 "open decisions" suggestion (optional, not required). |

## 2. What slice 7 adds to the harness

**ONE new support module + ONE new step file + minimal world/lib/test edits.** Production code is untouched per task brief.

### 2a. NEW: `support/tombstone_factory.rs` — direct-SQL tombstone insertion

Per DD-7 + D4 = A. ~50 LOC helper that inserts tombstoned comments
directly into the per-scenario PG schema with controllable
`deleted_at` ages. Bypasses the production soft-delete handler (which
always sets `deleted_at = now()` — useless for testing the 90-day
threshold).

Mirrors slice-3 `pg_backup.rs` shape (small focused module with a
narrow direct-SQL surface) and slice-6 `metrics_scrape.rs` shape (a
narrow support helper next to the step files that consume it).

Public surface (DISTILL ships SIGNATURES + RED-scaffold bodies;
DELIVER fills in):

```rust
/// Insert a single tombstoned comment whose `deleted_at` is set to
/// `now() - interval '<deletion_age_days> days'`. Used by scenarios
/// that need a small, addressable set of tombstones.
///
/// Returns the inserted comment's UUID so the scenario can reference
/// it (the admin-undelete CLI scenario #7 needs the UUID to pass to
/// the subprocess).
pub async fn insert_tombstoned_comment(
    pool: &sqlx::PgPool,
    issue_id: uuid::Uuid,
    author_id: uuid::Uuid,
    body: &str,
    deletion_age_days: i64,
) -> uuid::Uuid;

/// Bulk-insert N tombstoned comments at the same `deleted_at` age.
/// Used by the cap scenario (#3, 11k rows). Implementation strategy
/// (batched multi-row INSERT vs COPY) is DELIVER's call; 11k rows is
/// small enough that batched INSERT (~10ms) is fine.
///
/// Returns the inserted UUIDs in insertion order (length = count).
pub async fn bulk_insert_tombstoned_comments(
    pool: &sqlx::PgPool,
    issue_id: uuid::Uuid,
    author_id: uuid::Uuid,
    count: u64,
    deletion_age_days: i64,
) -> Vec<uuid::Uuid>;

/// Count tombstoned comments on a given issue, optionally filtered to
/// those older than a threshold. Used by Then-step assertions that
/// avoid a HTTP round-trip (the issue-page GET filters tombstones
/// from the rendered list per the slice-5 @soft-delete-invariant
/// contract).
pub async fn count_tombstoned_comments_on_issue(
    pool: &sqlx::PgPool,
    issue_id: uuid::Uuid,
    older_than_days: Option<i64>,
) -> u64;
```

The helper writes raw SQL (no `Store::*` method exists for these
test-only patterns) via `sqlx::query_as!` / `sqlx::query!`. Because
slice-5 ADR-007's migration `0006_comments_edit_delete.sql` already
shipped `deleted_at` + `deleted_by`, the helper does NOT touch the
schema — it just writes rows with the columns set to test-controlled
values.

Author + issue UUIDs are passed in (resolved by the calling step
body from the slice-1 Background-seeded state). The helper does NOT
create users or issues; that is the caller's responsibility (matches
the slice-2 SSE helper convention).

### 2b. Subprocess helper — INHERITED from slice 6

Slice 7 does NOT introduce a new subprocess helper. It reuses
`steps::handler_instrumentation::FoundrySubprocess` (or, equivalently,
the step file extracts the helper into `support/foundry_subprocess.rs`
during DELIVER as a refactor — optional, not required for slice-7
GREEN). The slice-7 step file imports the slice-6 `FoundrySubprocess`
directly:

```rust
use crate::steps::handler_instrumentation::FoundrySubprocess;
```

If DELIVER decides to extract the subprocess helper out of slice-6's
step module into a shared `support/` location, slice-7 step file
follows. This is a DELIVER-time refactoring decision, NOT a DISTILL
contract.

### 2c. NEW step file: `steps/us_10_tombstone_gc.rs`

Per DD-3, the slice-7 step file continues the US-10 lineage:

| Step file | Slice | US-10 surface |
|---|---|---|
| `us_10_comments.rs` | slice 2 | POST + GET + sanitize |
| `us_10_comment_edit_delete.rs` | slice 5 | PATCH + DELETE + admin-delete + 410-Gone |
| `us_10_tombstone_gc.rs` | slice 7 | GC tick + admin-undelete CLI |

The slice-7 work lands in exactly ONE new step file + 5 small
test-side edits:

1. NEW: `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` (the step body file — scaffolded RED in DISTILL, filled in by DELIVER).
2. NEW: `crates/foundry-acceptance/src/support/tombstone_factory.rs` (the direct-SQL helper — minimal RED scaffolds in DISTILL so the step bodies type-check; DELIVER fills in bodies).
3. EDIT: `crates/foundry-acceptance/src/lib.rs` — append one line in the `pub mod steps { ... }` block to register `us_10_tombstone_gc`.
4. EDIT: `crates/foundry-acceptance/src/support/mod.rs` — append one line to register `tombstone_factory`.
5. EDIT: `crates/foundry-acceptance/tests/acceptance.rs` — append one force-link `use foundry_acceptance::steps::us_10_tombstone_gc as _us_10_gc;`.
6. EDIT: `crates/foundry-acceptance/src/world.rs` — append six `Option`/`HashMap`-typed fields under a new `// ---- US-10 tombstone GC (slice 7) ----` block at the bottom of the `FoundryWorld` struct (matching slice-5/slice-6 conventions).

All six edits are test-infrastructure changes; production code is untouched per the task brief.

## 3. World struct additions (`FoundryWorld`)

Slice 7 adds six fields. All default to empty / `None` / `0`;
existing slice-1-through-slice-6 scenarios are unaffected.

```rust
// ---- US-10 tombstone GC (slice 7) ----
/// UUIDs of tombstoned comments inserted via tombstone_factory for
/// the current scenario. Indexed by (issue_key_prefix, issue_number)
/// so scenarios that seed multiple ages can address each cohort. The
/// admin-undelete CLI scenario #7 also stores the SINGLE tombstoned
/// UUID it inserts so the CLI subprocess invocation can pass it as
/// the argument.
pub slice7_tombstones_by_issue: HashMap<(String, i32), Vec<uuid::Uuid>>,
/// The single tombstoned UUID created by the admin-undelete WS
/// scenario #7. Captured separately from `slice7_tombstones_by_issue`
/// so the When step ("the operator runs `foundry doctor
/// restore-comment <comment-id>` ...") can substitute it into the
/// argv argument without a HashMap lookup dance.
pub slice7_admin_undelete_target: Option<uuid::Uuid>,
/// Captured stdout from the most recent `foundry doctor
/// restore-comment` subprocess invocation (mirrors slice-3
/// `us_03_cli_stdout`).
pub slice7_cli_stdout: Option<String>,
/// Captured stderr from the most recent `foundry doctor
/// restore-comment` subprocess invocation (mirrors slice-3
/// `us_03_cli_stderr`).
pub slice7_cli_stderr: Option<String>,
/// Exit code reported by the most recent `restore-comment` subprocess
/// (mirrors slice-3 `us_03_cli_exit_code`).
pub slice7_cli_exit_code: Option<i32>,
/// Holder PgPool acquired by the "another replica is holding the
/// tombstone-sweep advisory lock" Given step for scenario #4. The
/// holder calls `pg_advisory_lock` (NOT `pg_try_advisory_lock`) so the
/// foundry subprocess's GC tick sees a contended lock and returns
/// Ok(0). Dropped when the "the other replica releases ..." When step
/// fires (or at scenario teardown as a safety net).
pub slice7_lock_holder_pool: Option<sqlx::PgPool>,
```

The slice-6 `slice6_foundry`, `slice6_last_scrape`,
`slice6_last_scrape_status`, and `slice6_schema` fields are REUSED
by the slice-7 step bodies — the subprocess and scrape state shape is
identical between slice 6 and slice 7. The slice-2
`us_10_last_issue_body` is reused as the issue-page-fetch invalidation
slot for the WS #1 + WS #7 issue-page assertions.

## 4. Step phrase contracts (slice-7 inventory)

Per `step-skeletons.md`. Slice 7 registers NEW phrases only — no
existing slice-1..6 phrase is touched. The new phrases (counts at the
top):

- **Givens (4 new)**
- **Whens (5 new)**
- **Thens (8 new)**

Phrases that LOOK similar to slice-6 phrases but are NEW (different
regex):

- `the operator's foundry instance is running with the tombstone sweep cadence set to (\d+) second` — extends slice-6's `the operator's foundry instance is running` to inject the cadence env override. DISTINCT phrase, NO collision with the slice-6 phrase (cucumber-rs matches the most specific regex first; an exact-text match wins over a partial regex anyway).
- Slice-6's `the operator's foundry instance has been running for at least (\d+) seconds$` is REUSED VERBATIM (DD-6 — vocabulary continuity across slice-6 and slice-7 scenarios that both use cadence-wait gates).
- Slice-6's `the operator scrapes the metrics endpoint$` is REUSED VERBATIM.

Inventory of NEW phrases lives in `step-skeletons.md` § "Step signatures".

cucumber-rs treats step phrases as globally unique; the new phrases
were verified non-colliding by compile-and-run (`cargo check -p
foundry-acceptance --tests` + `cargo test -p foundry-acceptance
--test acceptance -- -t "@slice7"` ; both gates pass).

## 5. Per-scenario isolation — subprocess pattern (slice-6 inheritance)

Slice-1 invariant holds: per-scenario PG schema, shared container.
Slice-6 invariant holds: per-scenario foundry subprocess. Slice 7
adds nothing new at the isolation layer.

Step-by-step per-scenario lifecycle for a GC scenario (e.g. #2 gc-threshold):

1. Cucumber-rs calls World::default() — fresh empty world.
2. Background steps (slice-1) seed the per-scenario PG schema with workspace + admin + member + project + issue via direct SQL through the slice-1 in-process `PgPool` for the schema.
3. The slice-7 Given "the operator's foundry instance is running with the tombstone sweep cadence set to 1 second" spawns the subprocess with `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1` + `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS=90` (default) + the per-scenario `DATABASE_URL` (slice-6 pattern).
4. The slice-7 Givens "N ancient tombstoned comments exist ..." + "N recent tombstoned comments exist ..." call `tombstone_factory::insert_tombstoned_comment` (or `bulk_*` for #3) against the per-scenario in-process pool. The subprocess sees these rows because both the in-process pool and the subprocess connect to the same schema.
5. The slice-7 When "the operator's foundry instance has been running for at least 2 seconds" (slice-6 phrase) sleeps for 2s wall-clock — by the end, the first GC tick has fired (cadence=1s + first-tick-soon).
6. Then steps assert via `tombstone_factory::count_tombstoned_comments_on_issue` (cheaper than a HTTP round-trip) + (for #1 + #6) the `metrics_scrape` helper.
7. Teardown: subprocess dropped (slice-6 `FoundrySubprocess::Drop` kills + reaps); schema dropped (slice-1); lock-holder pool (for #4) dropped.

For the admin-undelete CLI scenarios (#7, #8, #9), step 3 spawns the
subprocess WITHOUT the GC env overrides (default 24h cadence is fine
— the GC won't tick during the scenario's wall-clock budget). The
When step then invokes `assert_cmd::Command::cargo_bin("foundry")`
subprocess against the SAME `DATABASE_URL` the foundry subprocess
uses — they cooperate via the shared PG schema.

Concurrency: slice-3's `--max-concurrent-scenarios 6` cap holds. The
9 slice-7 scenarios fit comfortably within the slice-6 RAM budget
(~50-80MB per subprocess × 6 concurrent = ~300-500MB peak).

## 6. Real-I/O budget — slice 7 adds ~34.5s default (~44.5-49.5s with `@slow`)

Per `wave-decisions.md` § "Suite-time budget":

| Scenario | Cost estimate | Notes |
|---|---|---|
| 1 walking-skeleton GC tick (3 ancient rows, 1 tick) | ~4.0 s | subprocess + 3-row insert + 2s wait + scrape + database state check |
| 2 gc-threshold (6 rows straddling 90-day boundary) | ~4.5 s | subprocess + 6-row insert + 2s wait + 2 database state checks |
| 3 gc-cap (`@slow` — 11k rows, 2 ticks) | ~10-15 s | subprocess + 11k-row bulk insert (~3-5s; depends on COPY vs batched) + 2 ticks (~2s each) + 2 scrapes + assertions |
| 4 gc-lock (two-replica race) | ~5.0 s | subprocess + lock acquisition by holder + 3-row insert + 2s + state check + lock release + 2s + state check |
| 5 gc-failure (synthetic error mid-tick) | ~5.0 s | subprocess + 3-row insert + flag-set + 2s + alive check + flag-clear + 2s + state check |
| 6 gc-metrics (pending gauge across 3 ticks) | ~6.0 s | subprocess (cap=2) + 5-row insert + 3 × (2s + scrape) |
| 7 walking-skeleton admin-undelete CLI (happy path) | ~4.0 s | subprocess + 1-row insert + CLI subprocess (~0.5s) + database + issue-page state checks |
| 8 admin-cli not-restorable | ~3.0 s | subprocess + CLI subprocess + exit-code + stderr-substring check |
| 9 admin-cli invalid-UUID | ~3.0 s | subprocess + CLI subprocess + exit-code + stderr-substring check |
| **Default subtotal (excludes `@slow` #3)** | **~34.5 s** | within the 40s slice-7 budget |
| **Full subtotal (includes `@slow` #3)** | **~44.5-49.5 s** | with `FOUNDRY_ACCEPTANCE_TAGS=all` |

After slice 7, total suite wall-clock projects to ~105s default
fast-loop (~70s slice-1..6 baseline per slice-6 wave-decisions.md +
~34.5s slice 7). Per slice-7 wave-decisions.md § "Fast-loop budget
drift", recommendation is accept-and-re-baseline for v0.2 RC; CI
sharding (slice-6 DEVOPS plan option a) remains available.

For slice-7-only iteration:
`FOUNDRY_ACCEPTANCE_TAGS="@slice7 and not @slow" cargo test ...`
runs in ~35s; the `@slow` cap scenario is opt-in.

## 7. Tag conventions (additions only)

Inherited (unchanged): see `wave-decisions.md` § "Tag conventions added".

Added in slice 7:
- `@slice7`, `@comment-tombstone-gc`, `@gc-tick`, `@gc-threshold`, `@gc-cap`, `@gc-lock`, `@gc-failure`, `@gc-metrics`, `@admin-cli`, `@slow` (NEW project-wide tag).

`@nfr-obs-03`, `@real-io`, `@driving_adapter`, `@error`,
`@walking_skeleton` are reused unchanged.

## 8. CI invocation (delta only)

The slice-1/2/3/4/5/6 invocations stay as-is. The slice-7 scenarios
pick up automatically because they live under the same feature-files
root. The `--max-concurrent-scenarios 6` cap holds.

Local fast loop for slice-7-only iteration:

```bash
cargo test -p foundry-acceptance --test acceptance -- -t "@slice7 and not @slow"
```

Local full slice-7 (includes the `@slow` cap scenario):

```bash
cargo test -p foundry-acceptance --test acceptance -- -t "@slice7"
```

CI full-fat (recommended for the v0.2-RC nightly build):

```bash
FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance --test acceptance
```

## 9. Standing rules carried into DELIVER (additions)

- The `tombstone_factory` helper inserts rows DIRECTLY via SQL with `deleted_at = now() - interval 'N days'`. DELIVER MUST honor the slice-7 D4 = A contract: NO production debug-only handler `POST /debug/age-tombstone` (rejected as production-pollution per slice-3 precedent).
- Every scenario MUST tear down its subprocess + lock-holder pool via the World's Drop chain. The slice-6 `FoundrySubprocess::Drop` impl kills + reaps; the slice-7 `slice7_lock_holder_pool: Option<PgPool>` field drops at scenario teardown.
- The CLI subprocess pattern follows slice-3 verbatim: `tokio::task::spawn_blocking(move || AssertCommand::cargo_bin("foundry").env("DATABASE_URL", ...).args(["doctor", "restore-comment", &uuid]).output())`. NO new mechanism.
- The cadence-override env var name is `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` (per ACCEPTED invented-detail #2). DELIVER MUST NOT rename.
- The per-run cap env var name is `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN` (per ACCEPTED invented-detail #2). DELIVER MUST NOT rename.
- The failure-injection mechanism per D5 = A is the env-var flag `FOUNDRY_TEST_HOOK_GC_FAIL_NEXT=1` (DISTILL clarification of DESIGN's in-process recommendation; DELIVER may converge to a different mechanism so long as the step phrase "the next tombstone sweep tick will fail with a synthetic database error" continues to wire through honestly).
- The `@slow` exclusion in `tests/acceptance.rs` default-filter closure is a ONE-LINE EDIT alongside the existing `@manual` / `@manual-trigger` / `@docker-compose` exclusions. DELIVER adds it.
- Substring matches on CLI stderr (`stderr mentions "not restorable"`, `stderr mentions "invalid UUID"`) keep the exact literal strings DELIVER-pickable — a future copy polish does not red the suite.
