# Step Skeletons — Slice 3 (operator-grade)

These are signatures only; bodies arrive in DELIVER. Phrases listed
under "Background (inherited)" already exist in slice 1 / slice 2 step
files and MUST be reused unchanged — registering them twice will
collide (cucumber-rs treats step phrases as globally unique).

Layout follows the slice-1/2 organisation: one file per US-N under
`crates/foundry-acceptance/src/steps/`. Shared helpers (round-robin
proxy, multi-replica harness, pg_dump shell-out, test-migration
staging) live in `crates/foundry-acceptance/src/support/`.

## Background — inherited unchanged from slice 1 + slice 2

These are defined in slice-1 + slice-2 step files; slice 3 features
call them verbatim and do not redefine them.

```rust
// Defined in steps/us_05_bootstrap.rs (slice 1)
#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_exists_with_admin(...);

// Defined in steps/us_07_project_create.rs (slice 1)
#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(...);

// Defined in steps/us_06_signin.rs (slice 1)
#[given(regex = r"^(\w+) is signed in$")]
async fn member_is_signed_in(...);
#[given(regex = r"^(\w+) is signed out$")]
async fn member_is_signed_out(...);

// Defined in steps/us_08_file_issue.rs (slice 1)
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(...);
#[given(regex = r#"^the "([^"]+)" project already has issue (\w+)-(\d+)$"#)]
async fn project_has_issue(...);
#[when(regex = r#"^(\w+) files an issue against "([^"]+)" with title "([^"]*)"$"#)]
async fn file_issue(...);

// Defined in steps/us_09_realtime_sse.rs (slice 2)
#[given(regex = r#"^(\w+) has an open subscription to events on "([^"]+)"$"#)]
async fn member_has_open_subscription(...);
#[then(regex = r#"^within (\d+) milliseconds (\w+) observes an? "([^"]+)" event for "(\w+)-(\d+)" on "([^"]+)"$"#)]
async fn member_observes_event_within(...);
```

## US-02 — `crates/foundry-acceptance/src/steps/us_02_multi_replica.rs`

```rust
use crate::support::harness::InProcHarness;
use crate::support::multi_replica_harness::MultiReplicaHarness;
use crate::support::round_robin_proxy::ProxyHandle;
use crate::support::sse_client::{open_sse_subscription, SseSubscription};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::StatusCode;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

// --- Given: spawn the cluster ---------------------------------------

#[given(regex = r"^the operator runs (\d+) foundry replicas behind a round-robin load balancer$")]
async fn operator_runs_n_replicas(world: &mut FoundryWorld, n: usize);
// Spawns MultiReplicaHarness, stores in world.us_02_multi.

#[given(regex = r"^all (\d+) replicas report ready through their /readyz endpoint$")]
async fn all_replicas_ready(world: &mut FoundryWorld, expected_n: usize);

// --- Given: subscription landing on a specific replica --------------

#[given(regex = r#"^(\w+) has an open subscription to events on "([^"]+)" that landed on a specific replica$"#)]
async fn member_subscription_landed_on_replica(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
);
// Opens the SSE subscription through the proxy; reads the X-Foundry-Replica
// header to record which replica it landed on; stores both subscription
// and landing-replica in world.

#[given(regex = r#"^(\w+) has just submitted a long-running request that is being served by a specific replica$"#)]
async fn member_long_running_request_in_flight(world: &mut FoundryWorld, who: String);
// Posts to a slice-3 test-only endpoint /__test/slow that holds for
// ~3 seconds; captures the serving-replica from X-Foundry-Replica.

// --- When: distributed requests + replica failures ------------------

#[when(regex = r"^(\w+) makes (\d+) requests through the load balancer that visit each of the (\d+) replicas at least once$")]
async fn member_makes_n_distributed_requests(
    world: &mut FoundryWorld,
    who: String,
    request_count: u32,
    expected_replicas: u32,
);

#[when(regex = r#"^(\w+) files an issue against "([^"]+)" with title "([^"]*)" via a different replica than (\w+)'s subscription$"#)]
async fn member_files_issue_via_different_replica(
    world: &mut FoundryWorld,
    actor: String,
    project_name: String,
    title: String,
    subscriber: String,
);

#[when(regex = r"^the replica serving (\w+)'s subscription is stopped$")]
async fn stop_replica_serving_subscription(world: &mut FoundryWorld, who: String);

#[when(regex = r"^the replica serving (\w+)'s request receives SIGTERM$")]
async fn sigterm_replica_serving_request(world: &mut FoundryWorld, who: String);

#[when(regex = r"^Postgres becomes unreachable from every replica$")]
async fn postgres_unreachable_from_every_replica(world: &mut FoundryWorld);
// Drops the per-scenario PgPool from every replica's AppState OR
// flips an injected health-check switch; DELIVER picks the cleaner path.

#[when(regex = r"^(\w+) issues (\d+) requests through the load balancer back-to-back over (\d+) seconds$")]
async fn member_issues_n_requests_over_n_seconds(
    world: &mut FoundryWorld,
    who: String,
    n: u32,
    seconds: u32,
);

// --- Then: distribution + reconnection + readyz + pool ceiling ------

#[then(regex = r"^every request observes (\w+) as signed in$")]
async fn every_request_observed_signed_in(world: &mut FoundryWorld, who: String);

#[then(regex = r"^no request prompts (\w+) to re-authenticate$")]
async fn no_request_prompts_reauth(world: &mut FoundryWorld, who: String);

#[then(regex = r"^the workspace dashboard renders (\w+)'s display name on every response$")]
async fn dashboard_renders_display_name_every_response(world: &mut FoundryWorld, who: String);

#[then(regex = r"^the event was produced by a different replica than the one serving (\w+)'s subscription$")]
async fn event_produced_by_different_replica(world: &mut FoundryWorld, subscriber: String);

#[then(regex = r"^within (\d+) milliseconds (\w+)'s SSE client has reconnected to a different healthy replica$")]
async fn sse_reconnected_within(
    world: &mut FoundryWorld,
    timeout_ms: u64,
    who: String,
);

#[then(regex = r"^subsequent issue events on \"([^\"]+)\" are delivered to (\w+) within (\d+) milliseconds of being produced$")]
async fn subsequent_events_delivered_within(
    world: &mut FoundryWorld,
    project_name: String,
    who: String,
    ms: u64,
);

#[then(regex = r"^within (\d+) milliseconds every replica's /readyz endpoint returns 503$")]
async fn every_readyz_returns_503_within(world: &mut FoundryWorld, ms: u64);

#[then(regex = r"^the load balancer removes every replica from rotation$")]
async fn lb_removes_every_replica(world: &mut FoundryWorld);

#[then(regex = r"^a subsequent request through the load balancer receives an upstream-unavailable response$")]
async fn lb_returns_upstream_unavailable(world: &mut FoundryWorld);

#[then(regex = r"^(\w+)'s in-flight request completes successfully$")]
async fn in_flight_request_completes(world: &mut FoundryWorld, who: String);

#[then(regex = r"^the replica's /readyz endpoint returns 503 before its in-flight request completes$")]
async fn replica_readyz_503_before_completion(world: &mut FoundryWorld);

#[then(regex = r"^the replica exits within (\d+) seconds of receiving SIGTERM$")]
async fn replica_exits_within_seconds(world: &mut FoundryWorld, secs: u64);

#[then(regex = r"^no replica's Postgres pool ever exceeds (\d+) active connections$")]
async fn no_pool_exceeds(world: &mut FoundryWorld, ceiling: u32);
// Reads pg_stat_activity from each replica's pool over the duration
// of the When step; assertion is on the max sampled value.

#[then(regex = r"^every request returns a successful response$")]
async fn every_request_successful(world: &mut FoundryWorld);

// --- @docker-compose scenario ---------------------------------------

#[given(regex = r"^the docker-compose multi-replica stack is up with Caddy in front of (\d+) foundry-app replicas$")]
async fn docker_compose_stack_up(world: &mut FoundryWorld, n: u32);

#[given(regex = r#"^an admin has bootstrapped a workspace "([^"]+)" with member "([^"]+)"$"#)]
async fn admin_bootstrapped_workspace_with_member(
    world: &mut FoundryWorld,
    workspace_name: String,
    member_email: String,
);

#[given(regex = r"^(\w+) is signed in through the Caddy load balancer$")]
async fn member_signed_in_through_caddy(world: &mut FoundryWorld, who: String);

#[when(regex = r"^(\w+) makes (\d+) requests through Caddy that visit each replica at least once$")]
async fn member_makes_n_requests_through_caddy(
    world: &mut FoundryWorld,
    who: String,
    n: u32,
);

#[then(regex = r"^the Caddy access log shows requests distributed across all (\d+) replica upstreams$")]
async fn caddy_log_shows_distribution(world: &mut FoundryWorld, n: u32);
```

## US-03 — `crates/foundry-acceptance/src/steps/us_03_backup_restore.rs`

```rust
use crate::support::harness::InProcHarness;
use crate::support::pg_backup::{
    dump_to_file, restore_from_file, spawn_restore_target, truncate_dump, RestoreTarget,
};
use crate::world::FoundryWorld;
use assert_cmd::Command;
use cucumber::{given, then, when};
use std::path::PathBuf;

// --- Given: seed workspace state ------------------------------------

#[given(regex = r#"^the workspace contains (\d+) issues with titles "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#)]
async fn workspace_contains_issues_range(
    world: &mut FoundryWorld,
    count: u32,
    prefix_lo: String,
    lo: i32,
    prefix_hi: String,
    hi: i32,
);

#[given(regex = r#"^issue "(\w+)-(\d+)" has an attachment "([^"]+)" of (\d+) kilobytes$"#)]
async fn issue_has_attachment_of_size(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    filename: String,
    kb: u32,
);

#[given(regex = r#"^issue "(\w+)-(\d+)" has (\d+) attachments of (\d+), (\d+), and (\d+) kilobytes respectively$"#)]
async fn issue_has_n_attachments_of_sizes(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    count: u32,
    kb1: u32,
    kb2: u32,
    kb3: u32,
);

#[given(regex = r#"^the workspace contains issues "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#)]
async fn workspace_contains_issue_range(
    world: &mut FoundryWorld,
    prefix_lo: String,
    lo: i32,
    prefix_hi: String,
    hi: i32,
);

#[given(regex = r"^the workspace contains (\d+) issues, (\d+) comments, (\d+) attachment, and (\d+) active session for (\w+)$")]
async fn workspace_contains_mixed_state(
    world: &mut FoundryWorld,
    issues: u32,
    comments: u32,
    attachments: u32,
    sessions: u32,
    who: String,
);

#[given(regex = r"^the workspace contains (\d+) issues and (\d+) attachments$")]
async fn workspace_contains_issues_and_attachments(
    world: &mut FoundryWorld,
    issues: u32,
    attachments: u32,
);

#[given(regex = r"^the operator has dumped the database to a backup file$")]
async fn operator_has_dumped_db(world: &mut FoundryWorld);

#[given(regex = r"^the backup file has been truncated to its first (\d+) bytes$")]
async fn backup_file_truncated(world: &mut FoundryWorld, keep_bytes: u32);

// --- When: dump + restore + foundry doctor --------------------------

#[when(regex = r"^the operator runs `pg_dump -Fc -d foundry` against the running Postgres and saves the output to a backup file$")]
async fn operator_runs_pg_dump(world: &mut FoundryWorld);

#[when(regex = r"^the operator boots a fresh Postgres and runs `pg_restore -d foundry` against it using that backup file$")]
async fn operator_runs_pg_restore_on_fresh(world: &mut FoundryWorld);

#[when(regex = r"^the operator points a foundry-app replica at the restored Postgres$")]
async fn operator_points_replica_at_restored(world: &mut FoundryWorld);

#[when(regex = r"^the operator dumps and restores the database$")]
async fn operator_dumps_and_restores(world: &mut FoundryWorld);
// Composite of the three steps above; convenience for sibling scenarios.

#[when(regex = r"^the operator dumps the database and then drops every foundry-related table from the source Postgres$")]
async fn operator_dumps_then_drops_source_tables(world: &mut FoundryWorld);

#[when(regex = r"^the operator restores the dump into a clean Postgres$")]
async fn operator_restores_into_clean(world: &mut FoundryWorld);

#[when(regex = r#"^(\w+) files a new issue against "([^"]+)" with title "([^"]*)" on the restored instance$"#)]
async fn member_files_issue_on_restored(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
    title: String,
);

#[when(regex = r"^the operator runs `foundry doctor backup-verify <backup-file>` as a subprocess$")]
async fn operator_runs_doctor_backup_verify(world: &mut FoundryWorld);

// --- Then: assertions over restored state ---------------------------

#[then(regex = r#"^signing in as "([^"]+)" with the same password succeeds against the restored instance$"#)]
async fn signin_succeeds_on_restored(world: &mut FoundryWorld, email: String);

#[then(regex = r#"^the workspace "([^"]+)" contains the same (\d+) issues "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#)]
async fn restored_workspace_contains_issues(
    world: &mut FoundryWorld,
    workspace: String,
    count: u32,
    prefix_lo: String,
    lo: i32,
    prefix_hi: String,
    hi: i32,
);

#[then(regex = r#"^the attachment "([^"]+)" on "(\w+)-(\d+)" downloads with a sha256 matching the original$"#)]
async fn attachment_downloads_with_matching_sha(
    world: &mut FoundryWorld,
    filename: String,
    prefix: String,
    number: i32,
);

#[then(regex = r#"^each of the (\d+) attachments on "(\w+)-(\d+)" downloads from the restored instance with the same sha256 as the original$"#)]
async fn each_attachment_round_trips(
    world: &mut FoundryWorld,
    count: u32,
    prefix: String,
    number: i32,
);

#[then(regex = r"^the Content-Type recorded for each attachment is preserved through the restore$")]
async fn content_type_preserved_through_restore(world: &mut FoundryWorld);

#[then(regex = r#"^the new issue's key is "(\w+)-(\d+)"$"#)]
async fn new_issue_key_is(world: &mut FoundryWorld, prefix: String, number: i32);

#[then(regex = r"^the restored instance contains all (\d+) issues, all (\d+) comments, and the (\d+) attachment with matching sha256$")]
async fn restored_contains_all_state(
    world: &mut FoundryWorld,
    issues: u32,
    comments: u32,
    attachments: u32,
);

#[then(regex = r"^(\w+)'s session cookie from before the dump is still recognised by the restored instance$")]
async fn session_cookie_recognised_post_restore(world: &mut FoundryWorld, who: String);

// --- Then: CLI subcommand assertions --------------------------------

#[then(regex = r"^the exit code is (\d+)$")]
async fn exit_code_is(world: &mut FoundryWorld, expected: i32);

#[then(regex = r"^the exit code is non-zero$")]
async fn exit_code_nonzero(world: &mut FoundryWorld);

#[then(regex = r#"^the stdout contains a row-count entry for the "([^"]+)" table with the value (\d+)$"#)]
async fn stdout_row_count_for(world: &mut FoundryWorld, table: String, count: u32);

#[then(regex = r#"^the stdout contains a "([^"]+)" line$"#)]
async fn stdout_contains_line(world: &mut FoundryWorld, line: String);

#[then(regex = r"^the stdout or stderr identifies the dump as unreadable or truncated$")]
async fn stdout_or_stderr_identifies_corrupt(world: &mut FoundryWorld);
```

## US-04 — `crates/foundry-acceptance/src/steps/us_04_rolling_upgrade.rs`

```rust
use crate::support::harness::InProcHarness;
use crate::support::multi_replica_harness::MultiReplicaHarness;
use crate::support::test_migration::{stage_test_migration, MigrationsDir};
use crate::world::{FoundryWorld, MigrationOutcome};
use cucumber::{given, then, when};
use std::time::{Duration, Instant};

// --- Given: stage a test migration + initial state ------------------

#[given(regex = r"^the database is at migration version (\d+)$")]
async fn db_at_migration_version(world: &mut FoundryWorld, version: u32);

#[given(regex = r#"^the operator has staged a test-only migration "0099_add_dummy_column\.sql" that adds a nullable column "([^"]+) ([^"]+)" to the "([^"]+)" table$"#)]
async fn staged_add_nullable_column(
    world: &mut FoundryWorld,
    column_name: String,
    column_type: String,
    table: String,
);

#[given(regex = r#"^the operator has staged a broken migration "0099_broken\.sql" that references a non-existent table$"#)]
async fn staged_broken_migration(world: &mut FoundryWorld);

#[given(regex = r#"^the operator has staged a test-only migration "0099_slow\.sql" that artificially takes (\d+) seconds to apply$"#)]
async fn staged_slow_migration(world: &mut FoundryWorld, secs: u32);

#[given(regex = r#"^a replica has already applied migration "0099_add_dummy_column\.sql"$"#)]
async fn replica_already_applied_99(world: &mut FoundryWorld);

// --- When: spawn replicas concurrently / restart --------------------

#[when(regex = r"^(\d+) replicas boot simultaneously pointing at the same Postgres$")]
async fn n_replicas_boot_simultaneously(world: &mut FoundryWorld, n: usize);

#[when(regex = r"^that replica is stopped and restarted against the same Postgres$")]
async fn replica_stopped_and_restarted(world: &mut FoundryWorld);

#[when(regex = r#"^a replica boots and attempts to apply migration "0099_broken"$"#)]
async fn replica_boots_attempts_broken(world: &mut FoundryWorld);

#[when(regex = r"^(\d+) replicas boot simultaneously and the first replica acquires the migration lock$")]
async fn n_replicas_boot_first_acquires_lock(world: &mut FoundryWorld, n: usize);

// --- Then: per-replica migration outcomes ---------------------------

#[then(regex = r#"^exactly one replica reports having executed migration "(\d+)"$"#)]
async fn exactly_one_executed_migration(world: &mut FoundryWorld, version: u32);

#[then(regex = r#"^the other replica reports having observed migration "(\d+)" as already-applied$"#)]
async fn other_replica_observed_already_applied(world: &mut FoundryWorld, version: u32);

#[then(regex = r"^both replicas reach a healthy /readyz within (\d+) seconds$")]
async fn both_replicas_healthy_within(world: &mut FoundryWorld, secs: u64);

#[then(regex = r#"^the "([^"]+)" table contains the new "([^"]+)" column$"#)]
async fn table_contains_new_column(world: &mut FoundryWorld, table: String, column: String);

#[then(regex = r"^the replica reports zero migrations executed during this boot$")]
async fn replica_zero_migrations(world: &mut FoundryWorld);

#[then(regex = r"^the replica reaches a healthy /readyz within (\d+) seconds$")]
async fn replica_healthy_within(world: &mut FoundryWorld, secs: u64);

#[then(regex = r#"^the "_sqlx_migrations" table contains exactly one row for version "(\d+)"$"#)]
async fn sqlx_migrations_one_row_for(world: &mut FoundryWorld, version: u32);

#[then(regex = r"^the replica reports a migration error and exits with a non-zero status$")]
async fn replica_migration_error_nonzero(world: &mut FoundryWorld);

#[then(regex = r#"^the "_sqlx_migrations" table contains no row for version "(\d+)"$"#)]
async fn sqlx_migrations_no_row_for(world: &mut FoundryWorld, version: u32);

#[then(regex = r"^every pre-existing migration row is unchanged$")]
async fn preexisting_migration_rows_unchanged(world: &mut FoundryWorld);

#[then(regex = r"^the second replica's boot is blocked for between (\d+) and (\d+) milliseconds$")]
async fn second_replica_blocked_between(world: &mut FoundryWorld, lo: u64, hi: u64);

#[then(regex = r"^after the first replica releases the lock the second replica observes the migration as already-applied$")]
async fn second_replica_observes_already_applied_post_release(world: &mut FoundryWorld);
```

## US-11 — `crates/foundry-acceptance/src/steps/us_11_attachments.rs`

```rust
use crate::support::harness::{signed_in_post, InProcHarness};
use crate::support::html_assertions::{assert_element_with_text, collect_attributes};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::multipart::{Form, Part};
use reqwest::StatusCode;

// --- Given: env + pre-existing attachments --------------------------

#[given(regex = r"^the FILE_UPLOAD_MAX_MB env var is set to (\d+) for this scenario$")]
async fn file_upload_max_mb_set_to(world: &mut FoundryWorld, mb: u32);
// Plumbs into AppState before spawn_app — see driver.md §2b
// (multi_replica_harness shares this env mechanism, but US-11 uses
// the single-replica InProcHarness with an overridable env).

#[given(regex = r#"^(\w+) has attached a (\d+)-kilobyte PNG named "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn member_has_attached_png_of_size(
    world: &mut FoundryWorld,
    who: String,
    kb: u32,
    filename: String,
    prefix: String,
    number: i32,
);

#[given(regex = r#"^(\w+) has attached a (\d+)-kilobyte text file named "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn member_has_attached_text_of_size(
    world: &mut FoundryWorld,
    who: String,
    kb: u32,
    filename: String,
    prefix: String,
    number: i32,
);

// --- When: upload + download ----------------------------------------

#[when(regex = r#"^(\w+) attaches a (\d+)-kilobyte PNG named "([^"]+)" with content-type "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn member_attaches_png_with_content_type(
    world: &mut FoundryWorld,
    who: String,
    kb: u32,
    filename: String,
    content_type: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^(\w+) attaches a (\d+)-megabyte PDF named "([^"]+)" with content-type "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn member_attaches_pdf_of_mb(
    world: &mut FoundryWorld,
    who: String,
    mb: u32,
    filename: String,
    content_type: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^(\w+) attempts to attach a (\d+)-megabyte file named "([^"]+)" with content-type "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn member_attempts_attach_megabyte_file(
    world: &mut FoundryWorld,
    who: String,
    mb: u32,
    filename: String,
    content_type: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^(\w+) attempts to attach a (\d+)-kilobyte file named "([^"]+)" with content-type "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn member_attempts_attach_kb_file(
    world: &mut FoundryWorld,
    who: String,
    kb: u32,
    filename: String,
    content_type: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^an anonymous request attempts to attach a (\d+)-kilobyte file named "([^"]+)" with content-type "([^"]+)" to "(\w+)-(\d+)"$"#)]
async fn anonymous_attempts_attach(
    world: &mut FoundryWorld,
    kb: u32,
    filename: String,
    content_type: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^(\w+) downloads the attachment "([^"]+)" from "(\w+)-(\d+)"$"#)]
async fn member_downloads_attachment(
    world: &mut FoundryWorld,
    who: String,
    filename: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^(\w+) attempts to download the attachment "([^"]+)" from "(\w+)-(\d+)"$"#)]
async fn member_attempts_download_attachment(
    world: &mut FoundryWorld,
    who: String,
    filename: String,
    prefix: String,
    number: i32,
);

#[when(regex = r#"^the operator deletes the issue "(\w+)-(\d+)"$"#)]
async fn operator_deletes_issue(world: &mut FoundryWorld, prefix: String, number: i32);

// --- Then: response status, headers, sha256, table state ------------

#[then(regex = r"^the upload response is a success status$")]
async fn upload_success_status(world: &mut FoundryWorld);

#[then(regex = r"^the response status from the upload is a success status$")]
async fn upload_response_success(world: &mut FoundryWorld);

#[then(regex = r"^the upload response status is (\d+)$")]
async fn upload_response_status(world: &mut FoundryWorld, status: u16);

#[then(regex = r"^the download response status is (\d+)$")]
async fn download_response_status(world: &mut FoundryWorld, status: u16);

#[then(regex = r#"^the attachment is listed on the (\w+)-(\d+) issue page with filename "([^"]+)"$"#)]
async fn issue_page_lists_attachment(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    filename: String,
);

#[then(regex = r#"^the attachment is listed on the (\w+)-(\d+) issue page with filename "([^"]+)" and size "([^"]+)"$"#)]
async fn issue_page_lists_attachment_with_size(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    filename: String,
    size_label: String,
);

#[then(regex = r"^the downloaded bytes have a sha256 matching the uploaded bytes$")]
async fn download_sha256_matches_upload(world: &mut FoundryWorld);

#[then(regex = r#"^the Content-Disposition response header names the file as "([^"]+)"$"#)]
async fn content_disposition_names(world: &mut FoundryWorld, filename: String);

#[then(regex = r#"^the Content-Type response header is "([^"]+)"$"#)]
async fn content_type_header_is(world: &mut FoundryWorld, expected: String);

#[then(regex = r"^the response body mentions the configured limit of (\d+) megabytes$")]
async fn body_mentions_limit_mb(world: &mut FoundryWorld, mb: u32);

#[then(regex = r#"^the "([^"]+)" table contains zero rows for "(\w+)-(\d+)"$"#)]
async fn table_contains_zero_rows_for(
    world: &mut FoundryWorld,
    table: String,
    prefix: String,
    number: i32,
);
```

## Production-side RED scaffolds (Mandate 7)

The scaffolds DISTILL produces are Rust source stubs that compile but
panic when invoked. They live in the production crates so step-
definition imports succeed and the failures are classified RED (not
BROKEN). The DELIVER wave replaces them with real implementations.

Files to add (each carries `// SCAFFOLD: true` per the Rust scaffold
convention):

```
crates/foundry-app/src/admin_cli.rs           # `foundry doctor backup-verify` subcommand entry
crates/foundry-app/src/routes/attachments.rs  # POST/GET /issues/:id/attachments
crates/foundry-app/src/test_hooks.rs          # /__test/slow long-running endpoint (compiled only under cfg(feature = "test-hooks"))
crates/foundry-app/src/replica_header.rs      # injects X-Foundry-Replica only when AppState::test_replica_addr is set
crates/foundry-store/src/attachments.rs       # IssueAttachmentRow + bytea I/O
crates/foundry-store/src/migrator_runtime.rs  # run_migrations_from_dir(pool, path) — runtime sibling of compile-time run_migrations
crates/foundry-store/migrations/0004_issue_attachments.sql  # bytea table per data-access.md
```

Each function body is `panic!("Not yet implemented -- RED scaffold");`
so the Red Gate Snapshot classifies the test as RED. The
`/__test/slow` route is opt-in via a cargo feature so it never ships
to production.

## Step inventory summary

| File | Given | When | Then | Total |
|---|---:|---:|---:|---:|
| `us_02_multi_replica.rs` | 5 | 8 | 16 | 29 |
| `us_03_backup_restore.rs` | 8 | 9 | 13 | 30 |
| `us_04_rolling_upgrade.rs` | 5 | 4 | 9 | 18 |
| `us_11_attachments.rs` | 3 | 9 | 11 | 23 |
| **Subtotal** | **21** | **30** | **49** | **100** |

Reuses from slice 1+2 keep this manageable: ~15 of the 21 Givens are
inherited verbatim and not re-declared.
