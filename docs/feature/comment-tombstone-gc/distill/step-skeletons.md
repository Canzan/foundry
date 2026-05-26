# Step Skeletons — Slice 7 (comment-tombstone-gc)

Cucumber-rs step signatures the DELIVER wave fills in. Live in
`crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` —
this slice-7 work is ADDITIVE; no other step file is modified.

Step-method bodies are scaffolded RED with
`panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER
finishes this")` per nw-distill § "Mandate 7" (Rust adaptation per
the polyglot matrix — `panic!` is the Rust scaffold idiom that the
cucumber-rs runner classifies as `RED (MISSING_FUNCTIONALITY)`, not
`BROKEN`).

Step-method names follow the slice-1+2+3+4+5+6 style: `fn given_*`,
`fn when_*`, `fn then_*` — see
`crates/foundry-acceptance/src/steps/us_10_comment_edit_delete.rs`
(slice 5) and
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs`
(slice 6) for tone.

## Background — inherited unchanged from slice 1 + slice 2

These phrases are defined in slice-1/2 step files; slice 7 features
call them verbatim and do not redefine them.

```rust
// us_05_bootstrap.rs (slice 1)
#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_exists_with_admin(...);

// us_07_project_create.rs (slice 1)
#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(...);
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(...);

// us_08_file_issue.rs (slice 1)
#[given(regex = r#"^the "([^"]+)" project already has issue (\w+)-(\d+)$"#)]
async fn project_has_issue(...);
```

## Inherited unchanged from slice 6

```rust
// handler_instrumentation.rs (slice 6)
#[given("the operator's foundry instance is running")]
async fn given_foundry_instance_is_running(...);

#[given(regex = r"^the operator's foundry instance has been running for at least (\d+) seconds$")]
async fn given_foundry_instance_has_been_running_for(...);

#[when("the operator scrapes the metrics endpoint")]
async fn when_operator_scrapes_metrics_endpoint(...);

#[then(regex = r#"^the scrape body contains the line "([^"]+)"$"#)]
async fn then_scrape_body_contains_line(...);

#[then(regex = r#"^the scrape body's "([^"]+)" sample has value (\d+)$"#)]
async fn then_scrape_body_sample_has_value(...);

#[then("the foundry subprocess is alive")]
async fn then_foundry_subprocess_is_alive(...);
```

**Important constraint on slice-7 Background**: same as slice 6, the
slice-1..5 Background steps populate the per-scenario PG schema via
DIRECT SQL (through the in-process slice-1 harness). The slice-7
scenarios then spawn a foundry SUBPROCESS that connects to the SAME
schema. The slice-7 Givens for tombstone seeding (via
`tombstone_factory`) write through the SAME in-process pool — the
subprocess sees the tombstoned rows because both connections point
at the same schema.

**Special slice-7 wrinkle**: the GC scenarios need the subprocess to
spawn with `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1` env override.
The slice-6 `FoundrySubprocess::spawn` accepts only
`(database_url_with_schema, db_schema)` — DELIVER either extends the
signature (adds an `env_overrides: HashMap<String, String>` arg) OR
the slice-7 step file provides a wrapper `spawn_with_gc_cadence`
that sets the env vars before calling the existing `spawn` (the
existing function uses `assert_cmd::Command` internally and supports
`.env(...)` chaining at construction). DELIVER picks the exact
mechanism; DISTILL flags this as the only DELIVER-time wiring
adjustment needed.

## World additions

`crates/foundry-acceptance/src/world.rs` — append AFTER the slice-6
handler-instrumentation block.

```rust
// ---- US-10 tombstone GC (slice 7) ----
/// UUIDs of tombstoned comments inserted via tombstone_factory for
/// the current scenario. Indexed by (issue_key_prefix, issue_number)
/// so scenarios that seed multiple ages can address each cohort.
pub slice7_tombstones_by_issue: HashMap<(String, i32), Vec<uuid::Uuid>>,
/// The single tombstoned UUID created by the admin-undelete WS
/// scenario #7. Captured separately so the When step ("the operator
/// runs `foundry doctor restore-comment <comment-id>` ...") can
/// substitute it into the argv argument without a HashMap lookup
/// dance.
pub slice7_admin_undelete_target: Option<uuid::Uuid>,
/// Captured stdout from the most recent `foundry doctor
/// restore-comment` subprocess invocation (mirrors slice-3
/// `us_03_cli_stdout`).
pub slice7_cli_stdout: Option<String>,
/// Captured stderr from the most recent `foundry doctor
/// restore-comment` subprocess invocation (mirrors slice-3
/// `us_03_cli_stderr`).
pub slice7_cli_stderr: Option<String>,
/// Exit code reported by the most recent `restore-comment`
/// subprocess (mirrors slice-3 `us_03_cli_exit_code`).
pub slice7_cli_exit_code: Option<i32>,
/// Holder PgPool acquired by the "another replica is holding the
/// tombstone-sweep advisory lock" Given step for scenario #4. The
/// holder calls `pg_advisory_lock` so the foundry subprocess's GC
/// tick sees a contended lock and returns Ok(0). Dropped when the
/// "the other replica releases ..." When step fires (or at scenario
/// teardown).
pub slice7_lock_holder_pool: Option<sqlx::PgPool>,
```

The slice-6 fields (`slice6_foundry`, `slice6_last_scrape`,
`slice6_last_scrape_status`, `slice6_schema`) are REUSED by the
slice-7 step bodies. The slice-2 `us_10_last_issue_body` is reused as
the issue-page-fetch invalidation slot for the WS #1 + WS #7
issue-page assertions.

## Step force-link

`crates/foundry-acceptance/tests/acceptance.rs` — append next to the
existing `_us_10_edit` import:

```rust
#[allow(unused_imports)]
use foundry_acceptance::steps::us_10_tombstone_gc as _us_10_gc;
```

`crates/foundry-acceptance/src/lib.rs` — append next to
`pub mod us_10_comment_edit_delete;` inside the `pub mod steps`
block:

```rust
pub mod us_10_tombstone_gc;
```

`crates/foundry-acceptance/src/support/mod.rs` — append next to
existing module declarations:

```rust
pub mod tombstone_factory;
```

## Step signatures (the slice-7 contract DELIVER fills in)

Full Rust source with attribute macros + DELIVER implementation
outlines is the SSOT file
`crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs`.
The signatures below mirror that file for review convenience.

### Givens (4 new)

```rust
#[given(regex = r"^the operator's foundry instance is running with the tombstone sweep cadence set to (\d+) second$")]
async fn given_foundry_running_with_gc_cadence(
    world: &mut FoundryWorld,
    cadence_seconds: u64,
);

#[given(regex = r"^the operator's foundry instance is running with the tombstone sweep cadence set to (\d+) second and per-run cap set to (\d+)$")]
async fn given_foundry_running_with_gc_cadence_and_cap(
    world: &mut FoundryWorld,
    cadence_seconds: u64,
    cap: u64,
);

#[given(regex = r#"^(\d+) ancient tombstoned comments exist on "(\w+)-(\d+)" with deletion age (\d+) days$"#)]
async fn given_ancient_tombstones_exist(
    world: &mut FoundryWorld,
    count: u64,
    prefix: String,
    n: i32,
    age_days: i64,
);

#[given(regex = r#"^(\d+) recent tombstoned comments exist on "(\w+)-(\d+)" with deletion age (\d+) days$"#)]
async fn given_recent_tombstones_exist(
    world: &mut FoundryWorld,
    count: u64,
    prefix: String,
    n: i32,
    age_days: i64,
);

#[given("another replica is holding the tombstone-sweep advisory lock")]
async fn given_another_replica_holds_lock(world: &mut FoundryWorld);

#[given("the next tombstone sweep tick will fail with a synthetic database error")]
async fn given_next_tick_will_fail(world: &mut FoundryWorld);

#[given(regex = r#"^a tombstoned comment "([\s\S]+)" exists on "(\w+)-(\d+)" with deletion age (\d+) days authored by (\w+)$"#)]
async fn given_single_tombstoned_comment_for_undelete(
    world: &mut FoundryWorld,
    body: String,
    prefix: String,
    n: i32,
    age_days: i64,
    who: String,
);
```

(Counts as 4 NEW Givens in the slice-7 inventory because the two
`given_foundry_running_with_*` phrases share the cadence-set
mechanism and the two `given_*_tombstones_exist` phrases share the
seeding mechanism — but each regex is distinct and registers
separately with cucumber-rs.)

Inventory note: the slice-7 file ALSO defines the standalone
"another replica is holding ...", "the next tombstone sweep tick
will fail ...", and "a tombstoned comment ... authored by ..."
phrases — 7 distinct Given phrases registered in `us_10_tombstone_gc.rs`.
Listing them as "4 Givens" above counts the conceptual categories
(cadence-running × 2; tombstone-seed × 2 + 1-for-CLI; lock-holder;
failure-injection). Final compile count: **7 Givens** registered
under 7 distinct attribute-macro decorations.

### Whens (5 new)

```rust
#[when("the synthetic database error is cleared")]
async fn when_synthetic_error_is_cleared(world: &mut FoundryWorld);

#[when("the other replica releases the tombstone-sweep advisory lock")]
async fn when_other_replica_releases_lock(world: &mut FoundryWorld);

#[when("the operator runs `foundry doctor restore-comment <comment-id>` as a subprocess against the live database")]
async fn when_operator_runs_restore_comment_with_captured_uuid(
    world: &mut FoundryWorld,
);

#[when("the operator runs `foundry doctor restore-comment <missing-uuid>` as a subprocess against the live database")]
async fn when_operator_runs_restore_comment_with_missing_uuid(
    world: &mut FoundryWorld,
);

#[when(regex = r#"^the operator runs `foundry doctor restore-comment ([\S]+)` as a subprocess against the live database$"#)]
async fn when_operator_runs_restore_comment_with_literal_arg(
    world: &mut FoundryWorld,
    literal_arg: String,
);
```

The `<comment-id>` and `<missing-uuid>` literal-text Whens are
DISTINCT phrases (not regex placeholders) — the step body
substitutes from `world.slice7_admin_undelete_target` (for `<comment-id>`)
or generates a random unused UUID (for `<missing-uuid>`). The
literal-arg variant covers the malformed-UUID case (the .feature
passes `not-a-uuid` verbatim). cucumber-rs phrase ordering matches
exact strings first; the regex fallback applies only to non-matching
inputs.

### Thens (8 new)

```rust
#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows (\d+) tombstoned comments older than (\d+) days$"#)]
async fn then_issue_page_shows_n_tombstones_older_than(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    expected_count: u64,
    age_days: i64,
);

#[then(regex = r#"^the database holds (\d+) tombstoned comments on "(\w+)-(\d+)"$"#)]
async fn then_database_holds_n_tombstones_on_issue(
    world: &mut FoundryWorld,
    expected_count: u64,
    prefix: String,
    n: i32,
);

#[then(regex = r#"^the database holds (\d+) tombstoned comments older than (\d+) days on "(\w+)-(\d+)"$"#)]
async fn then_database_holds_n_tombstones_older_than_on_issue(
    world: &mut FoundryWorld,
    expected_count: u64,
    age_days: i64,
    prefix: String,
    n: i32,
);

#[then(regex = r#"^the doctor subprocess exits with code (\d+)$"#)]
async fn then_doctor_subprocess_exits_with_code(
    world: &mut FoundryWorld,
    expected_code: i32,
);

#[then(regex = r#"^the doctor subprocess stdout contains "([\s\S]+)"$"#)]
async fn then_doctor_subprocess_stdout_contains(
    world: &mut FoundryWorld,
    substring: String,
);

#[then(regex = r#"^the doctor subprocess stderr mentions "([\s\S]+)"$"#)]
async fn then_doctor_subprocess_stderr_mentions(
    world: &mut FoundryWorld,
    substring: String,
);

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing the text "([\s\S]+)"$"#)]
async fn then_issue_page_shows_comment_with_text(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    text: String,
);
```

Note: the last Then (`shows a comment by X containing the text Y`)
LOOKS identical to a slice-5 phrase
(`us_10_comment_edit_delete.rs` line registering an identically-
shaped Then). Slice 7's WS #7 needs this assertion for the restored
comment. **PHRASE COLLISION CHECK**: if slice-5 already registers
this exact regex, slice 7 REUSES it (no second registration). The
slice-5 .feature uses identical wording at line 122. ACTION FOR
DELIVER: verify the slice-5 phrase via `grep` BEFORE adding a
duplicate registration in `us_10_tombstone_gc.rs`. If duplicate,
remove the slice-7 registration and rely on the slice-5 phrase.
DISTILL flags this for the DELIVER pre-flight checklist.

Inventory note: final compile count: **6-7 Thens** registered (the
last Then may be a slice-5 reuse rather than a slice-7 new
registration).

### Subprocess wrapper (lives in the step file)

```rust
/// Spawn the foundry subprocess with the slice-7 GC-cadence env
/// override applied. Delegates to the slice-6
/// `handler_instrumentation::FoundrySubprocess::spawn` for the
/// actual spawn mechanics; this wrapper just sets the env vars
/// BEFORE calling the inner spawn.
///
/// DELIVER decides the exact wiring: either extend
/// `FoundrySubprocess::spawn` to accept an env-overrides arg, OR
/// have the subprocess helper expose a builder pattern, OR
/// duplicate the spawn body here with the env overrides applied
/// inline (least invasive; slice-7 ships this third option as the
/// default).
async fn spawn_foundry_with_gc_cadence(
    cadence_seconds: u64,
    cap: Option<u64>,
    threshold_days: Option<i64>,
    pool: &sqlx::PgPool,
    db_schema: &str,
) -> FoundrySubprocess;
```

## DELIVER Pre-flight Checklist (slice-7 sub-deliverables)

DELIVER must satisfy these before merging. Categorized per the
sub-deliverables enumerated in `wave-decisions.md` § "DELIVER
Pre-flight Checklist":

### Sub-deliverable A — Tombstone GC background task in `crates/foundry-app/src/main.rs`

- [ ] `tokio::spawn` block added next to the slice-6 pool-poll task
- [ ] `tokio::time::interval(Duration::from_secs(gc_interval_seconds))` with `MissedTickBehavior::Skip`
- [ ] Env-var parsing for `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` + `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS` + `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN`
- [ ] Per-tick body: call `store.gc_tombstoned_comments(...)`, log result, emit metrics; on error log + continue
- [ ] Test-hook env-var `FOUNDRY_TEST_HOOK_GC_FAIL_NEXT` honoured (cfg-gated `feature = "test-hooks"`; once-and-clear semantics)
- [ ] Acceptance scenarios 1, 2, 5 GREEN

### Sub-deliverable B — Store methods in `crates/foundry-store/src/lib.rs`

- [ ] `pub const TOMBSTONE_GC_LOCK_ID: i64 = 0x_60_C0_DE_60_C0_DE_60_u64 as i64;`
- [ ] `Store::gc_tombstoned_comments(older_than: Duration, batch: usize, cap: usize) -> Result<u64, StoreError>`
- [ ] `Store::count_pending_tombstones(older_than: Duration) -> Result<u64, StoreError>`
- [ ] `Store::undelete_comment(comment_id: Uuid) -> Result<u64, StoreError>` — single UPDATE returning rows affected
- [ ] Slice-1 `sqlx prepare` cache updated (3 new queries)
- [ ] Acceptance scenarios 1, 2, 3, 4 GREEN (for B alone — F and the test infra also required)

### Sub-deliverable C — Metric emission in main.rs

- [ ] `metrics::counter!("comments_tombstones_purged_total").increment(deleted_count)` after each successful tick
- [ ] `metrics::gauge!("comments_tombstones_pending").set(pending_count as f64)` after each tick
- [ ] Both metrics registered at value 0 at startup (slice-6 D4 precedent)
- [ ] Both metrics UNLABELLED (slice-6 D2 cardinality invariant covers them)
- [ ] `docs/feature/foundry-backend-mvp/design/system/observability-infra.md` metric-naming table updated (2 new rows)
- [ ] Acceptance scenarios 1, 6 GREEN (+ counter assertion in #3)

### Sub-deliverable D — `restore-comment` CLI subcommand

- [ ] `crates/foundry-app/src/admin_cli.rs::run_restore_comment(comment_id: &str) -> i32`
- [ ] Dispatch arm in `crates/foundry-app/src/main.rs::dispatch_subcommand` next to `"backup-verify"` arm
- [ ] CLI reads `DATABASE_URL` (NOT `FOUNDRY_DOCTOR_PROBE_URL`)
- [ ] Exit codes per D6 = A: {0, 2, 3, 4}
- [ ] DELIVER PBT unit test for exit-code 3 (DB connect failure)
- [ ] Acceptance scenarios 7, 8, 9 GREEN

### Sub-deliverable E — RELEASING.md operator runbook addition

- [ ] New subsection "Recovering an accidentally-deleted comment" after `foundry doctor backup-verify` section
- [ ] Path 1 (CLI) + Path 2 (psql) per architecture.md
- [ ] One-paragraph quarterly drill recommendation per D2 = A
- [ ] No automated test; reviewer reads runbook in the PR

### Sub-deliverable F — Acceptance harness wiring

- [ ] `crates/foundry-acceptance/src/support/tombstone_factory.rs` filled in per DD-7 signatures
- [ ] `crates/foundry-acceptance/src/support/mod.rs` registers the new module
- [ ] `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` step bodies filled in (replacing RED scaffolds)
- [ ] PHRASE COLLISION CHECK on the `shows a comment by X containing the text Y` Then — remove the slice-7 registration if slice-5 already provides it
- [ ] `crates/foundry-acceptance/src/world.rs` slice-7 fields used + reset between scenarios (cucumber-rs `Default::default()` per scenario handles reset automatically)
- [ ] `tests/acceptance.rs` default-filter closure excludes `@slow` (one-line edit) per D3 = A
- [ ] All 9 slice-7 scenarios GREEN end-to-end (default 8 + `@slow` 1)

### Cross-cutting regression

- [ ] No regression in the existing ~65 scenarios across slice 1+2+3+4+5+6
- [ ] `cargo check -p foundry-acceptance --tests` passes
- [ ] `cargo deny check` passes (zero new deps)
- [ ] `cargo xtask check-arch` passes
- [ ] Step-phrase contract: the new phrases MUST NOT be renamed in GREEN

## Production-side scaffolds (Mandate 7) — NOT done by slice-7 DISTILL

Per the task brief:
> DO NOT touch production code outside `crates/foundry-acceptance/`.

This is a project-specific deviation from the nw-distill § "Mandate 7:
RED-Ready Scaffolding" default. The slice-7 task explicitly defers
production-side scaffolding to DELIVER's RED phase (per ADR-025 D2:
DELIVER unskips, writes PBT, then implements). The RED classification
in slice 7 is achieved entirely by step-body panics in
`crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` + the
`support/tombstone_factory.rs` helper scaffolds — no production-side
`panic!`-shaped scaffolds.

DELIVER picks up production-side scaffolds (or full implementations)
from a clean slate. The acceptance step bodies are the RED contract.

## DELIVER read-back instructions

When DELIVER picks up slice 7:

1. The 9 slice-7 scenarios are all live (no `@skip` / `@ignore` tag). Each panics on its first slice-7-specific Given — typically the cadence-setting Given for GC scenarios, or the tombstone-seeding Given for the admin-undelete scenario. The 3 admin-cli scenarios (#7, #8, #9) panic at their first slice-7 Given which is either the slice-6 "operator's foundry instance is running" (REUSED — GREEN) followed by the slice-7 tombstone-seeding (RED for #7) or the slice-7 CLI invocation When (RED for #8 + #9).
2. Cucumber-rs treats `panic!` from a step body as a step failure with the panic message as the captured output. DELIVER does NOT need to change the step bodies' panic-to-implementation pattern — replace the body verbatim with the real implementation.
3. The step phrases (regex strings) registered in `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` ARE the contract between DISTILL and DELIVER. They MUST NOT change during GREEN. Awkward phrasings → DELIVER → DISTILL retro item, not unilateral rename.
4. `Store::gc_tombstoned_comments` is the SINGLE most-leveraged implementation: 4 scenarios depend on it directly (#1, #2, #3, #4), and #5 + #6 depend on it indirectly (the tick body that calls it). DELIVER's first move is most efficiently to land `Store::gc_tombstoned_comments` + `Store::count_pending_tombstones` (sub-deliverable B) + the tick body (sub-deliverable A); that unblocks scenarios #1, #2, #6 in one shot.
5. The 6 sub-deliverables (A through F) are roughly INDEPENDENT but have a natural ordering:
   - F.tombstone_factory (test infra) — unblocks RED reclassification
   - B.Store methods — unblocks #1, #2
   - A.main.rs tick spawn + C.metric emission — unblocks #1 (the metric assertion), #6
   - F.cadence-override wiring — unblocks the cadence-aware Givens
   - F.lock-holder Given + A.advisory-lock logic — unblocks #4
   - A.test-hook env var + F.failure-injection Given — unblocks #5
   - D.restore-comment CLI + main.rs dispatch arm — unblocks #7, #8, #9
   - E.RELEASING.md — runbook, no test impact
