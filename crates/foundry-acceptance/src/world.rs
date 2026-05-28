//! Cucumber-rs world struct.
//!
//! Per `distill/driver.md` §2: per-scenario state lives here.

use crate::support::compose_harness::ComposeStack;
use crate::support::harness::InProcHarness;
use crate::support::multi_replica_harness::MultiReplicaHarness;
use crate::support::pg_backup::RestoreTarget;
use crate::support::sse_client::{SseEvent, SseOpenAttempt, SseSubscription};
use crate::support::test_migration::TestMigrationsDir;
use foundry_store::MigrationReport;
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

#[derive(cucumber::World, Default, Debug)]
#[world(init = Self::default)]
pub struct FoundryWorld {
    // ---- US-01 docker-compose harness ----
    pub compose: Option<ComposeStack>,
    pub compose_bootstrap_url: Option<String>,
    pub admin_already_claimed: bool,

    // ---- US-05+ in-process harness ----
    pub harness: Option<InProcHarness>,
    pub http: Option<reqwest::Client>,

    /// Raw bootstrap-token strings indexed by the name they were minted
    /// under in the Background step (e.g. "valid-token-001").
    pub minted_tokens: HashMap<String, String>,

    /// Last response captured by a When step (consumed by Then).
    pub last_status: Option<StatusCode>,
    pub last_body: Option<String>,
    pub last_headers: Option<HeaderMap>,

    /// Identity of the latest invite generated through `/invites`, used
    /// by the "invite is recorded as valid for 7 days" assertion.
    pub last_invite_id: Option<uuid::Uuid>,

    /// Session cookie value (`foundry_session=...`) captured after a
    /// successful claim. Stored separately because `reqwest`'s cookie
    /// jar requires HTTPS for `Secure` cookies and we test over plain
    /// http://127.0.0.1.
    pub session_cookie_header: Option<String>,

    // ---- US-06 timing scratch (sign-in response latency) ----
    /// Per-sample /sign-in POST latencies for the unknown-email arm of
    /// the timing-symmetry scenario. Compared by median against the
    /// wrong-password arm to resist `spawn_blocking`-pool contention
    /// under @all (single-sample comparison was flaky).
    pub us_06_unknown_latencies_ms: Vec<u64>,
    /// Per-sample /sign-in POST latencies for the wrong-password arm,
    /// interleaved with the unknown-email arm so both see the same
    /// contention distribution.
    pub us_06_wrong_pw_latencies_ms: Vec<u64>,

    // ---- US-07 project-create scratch ----
    /// Email of the signed-in user for the current scenario (drives
    /// `signed_in_post` and the post-redirect board fetch).
    pub us_07_signed_in_email: Option<String>,
    /// Password matching `us_07_signed_in_email`. Stored in the world
    /// because the test harness re-authenticates for each follow-up
    /// HTTP request (no cookie jar — see harness::client).
    pub us_07_signed_in_password: Option<String>,
    /// Name of the last project the When step attempted to create.
    /// Used by the "no second project is created" assertion.
    pub us_07_last_attempted_name: Option<String>,
    /// Slug of the last team a When step targeted. Currently
    /// informational; reserved for diagnostics on failed assertions.
    pub us_07_last_team_slug: Option<String>,

    // ---- US-08 file-issue scratch ----
    /// Slug of the last project a US-08 When step posted to. Used by
    /// the board-fetch assertion to reconstruct the GET URL.
    pub us_08_last_project_slug: Option<String>,
    /// Team slug of the last US-08 When step's target project. Same
    /// reason as above.
    pub us_08_last_team_slug: Option<String>,
    /// Per-request latencies captured in the performance scenario.
    /// Length matches the number of POSTs the When step issued.
    pub us_08_latencies_ms: Vec<u64>,

    // ---- US-09 realtime SSE ----
    /// Active SSE subscriptions for this scenario, keyed by
    /// `(subscriber_name, project_name)` so the same scenario can hold
    /// two subscriptions (Mei + Hiroshi on the same project).
    pub us_09_subscriptions: HashMap<(String, String), SseSubscription>,
    /// Wall-clock instant the most recent When step started; the
    /// matching Then step uses it to compute per-event arrival latency
    /// (so timing is per-scenario, not per-suite).
    pub us_09_last_action_started_at: Option<Instant>,
    /// Captured open attempt for the @error 401/403 scenarios.
    pub us_09_last_open_attempt: Option<SseOpenAttempt>,
    /// Open status for Rita's authenticated-but-forbidden 403 scenario.
    pub us_09_last_open_status: Option<StatusCode>,
    /// Mei's session cookie (`foundry_session=...`) once she signs in.
    /// Cached so subsequent steps don't re-authenticate.
    pub us_09_mei_cookie: Option<String>,
    /// Rita's session cookie. Separate slot so a single scenario can
    /// hold both at once (the 403 scenario signs Rita in but never
    /// touches Mei).
    pub us_09_rita_cookie: Option<String>,
    /// Most-recent event matched by the "observes event" Then step,
    /// fed to the follow-up "the event's project key is ..." step.
    pub us_09_last_event: Option<SseEvent>,

    // ---- US-10 comments ----
    /// Last issue key a comment scenario targeted (e.g. "AUTH-3"). Used
    /// by the issue-page-render Then step to know which page to GET.
    pub us_10_last_issue_key: Option<String>,
    /// Body of the most recent issue-page GET captured in a US-10 Then
    /// step, so subsequent Then steps that assert different selectors
    /// on the same body don't re-fetch.
    pub us_10_last_issue_body: Option<String>,

    // ---- US-12 keyboard-nav response capture ----
    /// Body of the most-recent GET captured by a US-12 When step. The
    /// US-12 scenarios make exactly one GET and then run multiple Then
    /// assertions against the cached body.
    pub us_12_last_get_body: Option<String>,

    // ---- US-11 attachments ----
    /// Bytes the most recent upload sent, keyed by (issue_key, filename).
    /// The download assertion looks up the originally-uploaded bytes
    /// here to verify round-trip equality.
    pub us_11_uploaded_bytes: HashMap<(String, String), Vec<u8>>,
    /// SHA-256 of the most recently uploaded file, keyed by
    /// (issue_key, filename) — independent capture so a corrupted
    /// `us_11_uploaded_bytes` cannot accidentally pass the byte-
    /// identical assertion.
    pub us_11_uploaded_sha: HashMap<(String, String), String>,
    /// Status of the last upload (POST .../attachments) — captured so
    /// "the upload is accepted" / "is refused as forbidden (HTTP 403)"
    /// / "is refused with an over-limit (HTTP 413) response" Thens can
    /// share the same captured value.
    pub us_11_last_upload_status: Option<StatusCode>,
    /// Body of the last upload's response. Used by "the response body
    /// mentions the configured limit of N megabytes".
    pub us_11_last_upload_body: Option<String>,
    /// Bytes of the most recent download response.
    pub us_11_last_download_bytes: Option<Vec<u8>>,
    /// Headers of the most recent download response.
    pub us_11_last_download_headers: Option<HeaderMap>,
    /// Status of the most recent download response.
    pub us_11_last_download_status: Option<StatusCode>,

    // ---- US-03 backup-restore ----
    /// Path to the dump file produced by the most recent backup step.
    pub us_03_backup_file: Option<PathBuf>,
    /// Handle to the process-wide US-03 restore-target Postgres
    /// container. Cloning is cheap (Arc inside); the underlying
    /// container is leaked + reused across scenarios per the
    /// Mac+Colima memory-pressure mitigation in `support::pg_backup`.
    pub us_03_restore_target: Option<RestoreTarget>,
    /// Mutex guard held from the first `pg_restore` call until
    /// scenario teardown — serialises US-03 scenarios so they do not
    /// observe each other's restored state under cucumber's
    /// per-scenario concurrency.
    pub us_03_restore_guard: Option<tokio::sync::OwnedMutexGuard<()>>,
    /// Connection URL for an InProcHarness pointed at the RESTORED
    /// database. Captured after the "operator points a foundry-app
    /// replica at the restored database" step.
    pub us_03_restored_harness: Option<InProcHarness>,
    /// Captured (filename → sha256-hex) for every attachment that was
    /// uploaded BEFORE the backup. Post-restore Then steps recompute
    /// the sha256 from the restored bytes and compare.
    pub us_03_uploaded_sha: HashMap<String, String>,
    /// Captured (filename → recorded Content-Type) for every attachment
    /// uploaded before the backup. The "Content-Type preserved
    /// through the restore" assertion compares this against what the
    /// restored DB reports.
    pub us_03_uploaded_content_type: HashMap<String, String>,
    /// Captured (filename → bytes) for attachments uploaded before the
    /// backup. The byte-identical assertion compares the post-restore
    /// bytes against these.
    pub us_03_uploaded_bytes: HashMap<String, Vec<u8>>,
    /// Captured stdout from the most recent `foundry doctor
    /// backup-verify` CLI subprocess invocation.
    pub us_03_cli_stdout: Option<String>,
    /// Captured stderr from the most recent `foundry doctor
    /// backup-verify` CLI subprocess invocation.
    pub us_03_cli_stderr: Option<String>,
    /// Exit code reported by the most recent CLI subprocess.
    pub us_03_cli_exit_code: Option<i32>,
    /// Mei's session cookie captured BEFORE the backup. The
    /// "session-still-recognised" assertion presents this cookie
    /// against the restored instance.
    pub us_03_pre_backup_session_cookie: Option<String>,

    // ---- US-02 multi-replica ----
    /// The N-replica harness for this scenario. None until the
    /// background step `the operator runs N foundry replicas ...` runs.
    pub us_02_multi: Option<MultiReplicaHarness>,
    /// Per-actor session cookie captured at sign-in time. Multi-replica
    /// scenarios sign Mei (and sometimes Hiroshi) in once and then
    /// re-present the cookie across every subsequent request that
    /// rotates through replicas.
    pub us_02_cookies: HashMap<String, String>,
    /// Per-replica observation counts captured by the proxy. Filled by
    /// the "Mei makes N requests" When step; read by the "every replica
    /// served at least once" Then step.
    pub us_02_replica_observations: HashMap<SocketAddr, u64>,
    /// The replica addr the most-recent SSE subscription landed on.
    /// Set by `member_subscription_landed_on_replica`; read by the
    /// "event was produced by a different replica" and "subscription
    /// landing replica is stopped" steps.
    pub us_02_sse_landing_replica: Option<SocketAddr>,
    /// The SocketAddr that served the most-recent issue-creation POST
    /// in the fan-out scenario. The Then step compares this against
    /// `us_02_sse_landing_replica` to assert cross-replica fan-out.
    pub us_02_last_writer_replica: Option<SocketAddr>,
    /// The SSE subscription open across replicas. Captured separately
    /// from `us_09_subscriptions` so the multi-replica step modules
    /// don't collide on the keying scheme.
    pub us_02_subscription: Option<SseSubscription>,
    /// The actor whose subscription is currently held in
    /// `us_02_subscription` — needed for the auto-reconnect scenarios
    /// that re-open through the proxy.
    pub us_02_subscriber: Option<String>,
    /// The project_name the active subscription points at.
    pub us_02_subscriber_project: Option<String>,
    /// Outcomes the Then step needs: did every observed request return
    /// 200? Captured during the "Mei makes 30 back-to-back" step so
    /// the assertion is per-scenario, not per-suite.
    pub us_02_all_requests_succeeded: Option<bool>,
    /// Max observed pool size across the "30 requests over 3 seconds"
    /// scenario. Sampled from each replica's `Store::pool().size()`
    /// during the When step; asserted ≤ 10 by the Then step.
    pub us_02_max_pool_size_observed: Option<u32>,
    /// The replica index that's been marked for SIGTERM/stop. Used by
    /// the "in-flight request completes successfully" assertion and
    /// the "/readyz returns 503 before completion" assertion which
    /// both need to know which replica is draining.
    pub us_02_draining_replica_idx: Option<usize>,
    /// The wall-clock at which the long-running request was issued so
    /// the "exits within 15 seconds of SIGTERM" assertion can timestamp
    /// the deadline. Stored alongside the in-flight join handle.
    #[allow(dead_code)]
    pub us_02_in_flight_started_at: Option<Instant>,
    /// JoinHandle for the in-flight long-running request. The "Mei's
    /// in-flight request completes successfully" assertion `.await`s
    /// this handle and asserts the response status is 2xx.
    pub us_02_in_flight_handle:
        Option<tokio::task::JoinHandle<Result<(StatusCode, SocketAddr), String>>>,

    // ---- US-04 rolling-upgrade ----
    /// The staged per-scenario migrations dir (production base copy +
    /// 0099_*.sql). Kept alive on the world so the temp dir doesn't
    /// drop mid-scenario. Set by the "ships a schema update labeled
    /// '0099' ..." Given steps.
    pub us_04_migrations_dir: Option<TestMigrationsDir>,
    /// Per-replica migration reports captured at boot by the
    /// concurrent harness. Indexed by replica slot.
    pub us_04_migration_reports: Vec<MigrationReport>,
    /// Per-replica boot durations captured at boot by the concurrent
    /// harness. Indexed by replica slot. Used by the slow-lock-race
    /// "second replica blocked between N and M ms" assertion.
    pub us_04_boot_durations: Vec<Duration>,
    /// The concurrent multi-replica harness instance for this scenario
    /// (US-04 spawn_concurrent path). Held separately from
    /// `us_02_multi` so US-04 scenarios that also touch /readyz can
    /// drive the proxy without colliding on world keys.
    pub us_04_concurrent: Option<MultiReplicaHarness>,
    /// The SpawnConcurrentError raised by a failed boot, if any. The
    /// broken-migration scenario asserts this is Some(MigrationFailed).
    pub us_04_spawn_error: Option<String>,
    /// Slow-migration delay (ms) recorded by the slow-update Given so
    /// the When step can pass it into
    /// `spawn_concurrent_sharing_schema_with_delay`. Per-AppState
    /// rather than process-global keeps parallel scenarios isolated.
    pub us_04_slow_migration_delay_ms: Option<u64>,

    // ---- US-13 contributor onboarding ----
    /// Lazy-loaded README contents for the current scenario. Loaded by
    /// the "contributor is reading the project README" step; reused by
    /// all downstream Then assertions in the same scenario.
    pub us_13_readme_text: Option<String>,
    /// Lazy-loaded `rust-toolchain.toml` contents for the current
    /// scenario. Read on demand by the MSRV-pin assertion.
    pub us_13_rust_toolchain_text: Option<String>,
    /// Outcome of the walking-skeleton subprocess invocation:
    /// (exit_status, captured_stdout, captured_stderr).
    pub us_13_self_test_outcome: Option<(std::process::ExitStatus, String, String)>,

    // ---- US-10 edit/delete (slice 5) ----
    /// Map (issue_key_prefix, issue_number, author_email) -> comment_id
    /// captured by the "previously posted a comment" Given. The
    /// matching When step looks up the id to address PATCH/DELETE.
    /// Keyed by author so a single scenario can hold both Mei's and
    /// Hiroshi's comments at once (non-author-403 scenario).
    pub us_10_5_last_comment_id_by_author: HashMap<(String, i32, String), uuid::Uuid>,
    /// Map (issue_key_prefix, issue_number, body_substring) -> comment_id
    /// captured by the "previously posted a comment" Given. Lets the
    /// soft-delete-invariant scenario address a specific one of Mei's
    /// two comments by body fragment (since both are by the same
    /// author, the author-keyed map collapses them).
    pub us_10_5_last_comment_id_by_body: HashMap<(String, i32, String), uuid::Uuid>,
    /// Body of the most recent GET /comments/{id}/edit response. Cached
    /// by the When step that requests it so multiple Then assertions
    /// on the form fragment can share the same response body.
    pub us_10_5_last_edit_form_body: Option<String>,
    /// Raw markdown source of the most recently posted comment per
    /// (issue_key_prefix, issue_number, author_email). Used by the
    /// "textarea value is the raw markdown source" assertion in the
    /// PATCH walking-skeleton scenario.
    pub us_10_5_last_posted_body: HashMap<(String, i32, String), String>,

    // ---- Slice 6: handler-instrumentation ----
    /// The foundry subprocess for the current scenario. Spawned by
    /// the "the operator's foundry instance is running" Given.
    /// Dropped at scenario teardown (its Drop impl kills + reaps
    /// the child process).
    pub slice6_foundry: Option<crate::steps::handler_instrumentation::FoundrySubprocess>,
    /// Most-recent `ScrapeSnapshot` captured by a When step. Then
    /// steps read it for assertions (label-key set, sample sum, line
    /// presence).
    pub slice6_last_scrape: Option<crate::support::metrics_scrape::ScrapeSnapshot>,
    /// Status code returned by the most-recent raw scrape (used by
    /// the startup-probe success scenario #9 which asserts 200
    /// explicitly).
    pub slice6_last_scrape_status: Option<StatusCode>,
    /// Count of HTTP requests the When step has issued against the
    /// subprocess's main listener. Used by scenario #4 (counter sum
    /// == N).
    pub slice6_request_count: u64,
    /// Map (route_template, method) -> count of requests issued.
    /// Used by scenario #2 (per-route + per-method breakdown).
    pub slice6_request_count_by_route: HashMap<(String, String), u64>,
    /// The SSE subscription opened in scenarios #7 + #8. Distinct
    /// from `us_09_subscriptions` because this one rides through a
    /// foundry SUBPROCESS, not the in-process harness. Drop =
    /// client-side abrupt close (used by scenario #8 to trigger
    /// SubscriberGauge::Drop on the server side).
    pub slice6_sse_subscription: Option<reqwest::Response>,
    /// The connection acquired-and-held in scenario #5 (forces
    /// `db_connections_in_use` to be > 0 for at least one poll
    /// tick). Held as a long-lived sqlx connection from the
    /// per-scenario schema pool. Dropped to release.
    pub slice6_held_connection: Option<sqlx::pool::PoolConnection<sqlx::Postgres>>,
    /// Per-scenario PG schema name (slice-1 pattern). Captured so
    /// teardown can drop it. The subprocess connected via
    /// DATABASE_URL with this schema pinned via search_path.
    pub slice6_schema: Option<String>,

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
}
