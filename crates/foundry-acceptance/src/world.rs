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

    /// Captured bootstrap-claim refusals (status + full body) for the
    /// enumeration-oracle regression scenario, one entry per arm
    /// (already-used / expired / unknown). Asserted byte-identical.
    pub bootstrap_refusals: Vec<(StatusCode, String)>,

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
    /// Connection URL for an InProcHarness pointed at the RESTORED
    /// database. Captured after the "operator points a foundry-app
    /// replica at the restored database" step.
    ///
    /// DROP-ORDER MATTERS (see `us_03_restore_guard` below): this
    /// harness owns a sqlx pool with `min_connections(1)` open against
    /// the SHARED restore target. It MUST drop (tearing down those
    /// connections) BEFORE `us_03_restore_guard` releases — otherwise a
    /// waiting sibling scenario acquires the guard and runs
    /// `pg_restore --clean` (DROP TABLE), which blocks forever on the
    /// relation lock held by this still-open connection. Struct fields
    /// drop in declaration order, so this field is declared BEFORE the
    /// guard. The `After` hook (`close_us03_restored_pool`) also closes
    /// the pool explicitly while the guard is still held, because
    /// `PgPool::Drop` is non-blocking and cannot await connection
    /// teardown on its own.
    pub us_03_restored_harness: Option<InProcHarness>,
    /// Mutex guard held from the first `pg_restore` call until
    /// scenario teardown — serialises US-03 scenarios so they do not
    /// observe each other's restored state under cucumber's
    /// per-scenario concurrency.
    ///
    /// Declared AFTER `us_03_restored_harness` so it drops (releasing
    /// the serialisation lock) only after the restored harness pool has
    /// torn down its connections to the shared restore target. See the
    /// drop-order note on `us_03_restored_harness`.
    pub us_03_restore_guard: Option<tokio::sync::OwnedMutexGuard<()>>,
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

    // ---- Slice 8: deferred-metrics ----
    /// Dedicated, restartable Postgres container the listen-disconnect
    /// scenario (#7b) owns. Restarting it forces a REAL LISTEN drop on
    /// the subprocess's `run_pg_listener` task (no production seam,
    /// DD-5). Kept on the world so its Drop (container removal) fires at
    /// scenario teardown.
    pub slice8_dedicated_db: Option<crate::steps::slice_8_deferred_metrics::DedicatedDb>,
    /// Staged per-scenario migrations dir for the migration-timing
    /// scenarios (#5). Reuses the slice-4 `support::test_migration`
    /// staging seam (production base copy + one extra). Held so the
    /// tempdir lives for the scenario.
    pub slice8_staged_migrations: Option<crate::support::test_migration::TestMigrationsDir>,
    /// The PG schema (already migrated) that the migration-no-op
    /// scenario (#6) and the cardinality scenario (#11) point a fresh
    /// subprocess at, so the boot applies ZERO new migrations.
    pub slice8_migrated_schema: Option<String>,
    /// The migration-timing observation count captured by the
    /// "the migration-timing observation count has been recorded" Given
    /// (#6 baseline). The follow-up "has not grown" Then re-scrapes and
    /// asserts the count did not increase.
    pub slice8_recorded_observation_count: Option<u64>,
    /// Pre-bound TCP listener squatting on the metrics port for the
    /// probe-failure scenario (#8). Holds the port so the subprocess's
    /// `metrics_server::serve` bind fails (slice-6 ADR-014 precedent).
    /// Dropped at scenario teardown.
    pub slice8_prebound_metrics_listener: Option<std::net::TcpListener>,
    /// The metrics port the prebound listener (#8) is squatting on, so
    /// the spawn step can hand `METRICS_PORT=<this>` to the subprocess.
    pub slice8_prebound_metrics_port: Option<u16>,
    /// Captured (exit_code, stdout, stderr) of the probe-failure
    /// subprocess (#8) that refused to start. The Then steps assert the
    /// non-zero exit + the `health.startup.refused` log line + the
    /// probe-name in the captured stdout/stderr.
    pub slice8_refused_start_outcome: Option<(Option<i32>, String, String)>,
    /// The (database_url, schema) of a schema migrated EXCEPT for the
    /// migration-0006 comments columns (dropped after migrating), for the
    /// store-probe-failure scenario. The schema is migrated enough that the
    /// pre-probe boot steps (e.g. the `workspaces` bootstrap check) succeed,
    /// so the `store` startup probe's 0006-column check is the sole
    /// refuse-to-start cause — exercising `record_probe_result`. Held
    /// between the Given (provision) and the When (spawn, migrations skipped).
    pub slice8_store_probe_db: Option<(String, String)>,

    // ---- token-mutations-metric-export ----
    /// A real EdDSA management bearer (JWT) + the (team_slug, project_slug,
    /// jti) of a registered, revocable machine token, seeded by the
    /// tick-on-mutation Given so the When can drive a real `DELETE
    /// .../tokens/{jti}` against the subprocess. The revoke decision flows
    /// through `RateLimiter::check`, ticking `foundry_token_mutations_total`.
    pub tmm_revoke_target: Option<(String, String, String, uuid::Uuid)>,

    // ---- Feature A "Programmatic Foundry" (web-tier-extraction) ----
    /// Name of the project the most recent Feature-A When step targeted,
    /// used to reconstruct the `/api/v1/...` URL and resolve the team slug.
    pub fa_last_project_name: Option<String>,
    /// Title of the last issue a Feature-A write step attempted to file.
    pub fa_last_title: Option<String>,
    /// The bearer credential string the client presents on the next API
    /// request. `None` = no credential (the missing-credential 401 path).
    /// DISTILL stores a placeholder; DELIVER mints a real JWT.
    pub fa_credential: Option<String>,
    /// Whether the admin has revoked the active credential (the revoked-token
    /// refusal scenario). The client still presents it; the server denies.
    pub fa_credential_revoked: bool,
    /// The `jti` of the most-recently minted machine credential, so the
    /// revoke Given can stamp `revoked_at` on that exact registry row.
    pub fa_credential_jti: Option<uuid::Uuid>,
    /// Whether Mei is watching the board in real time (the API-write-visible-
    /// to-UI scenario).
    pub fa_watching: bool,
    /// Email of the browser member for the "browser path unchanged" regression
    /// scenario (NFR-WEB-API-SEC-01).
    pub fa_browser_email: Option<String>,
    /// Password the browser-path regression scenario signs in with (reset onto
    /// the seeded member by the `a member account ... with password` Given).
    pub fa_browser_password: Option<String>,
    /// Whether the browser sign-in completed in the unchanged-path scenario.
    pub fa_browser_signed_in: bool,
    /// Body of the most recent HTML board GET, for the JSON-vs-HTML parity
    /// assertion (US-W05a "same set of issues").
    pub fa_last_html_body: Option<String>,
    /// The planted boundary violation kind for a US-W06 copy-of-tree scenario
    /// (`None` = the clean tree).
    pub fa_guard_violation: Option<String>,
    /// Exit code captured from the `cargo xtask check-arch` subprocess.
    pub fa_guard_exit_code: Option<i32>,
    /// Combined stdout+stderr from the boundary-check subprocess.
    pub fa_guard_stderr: Option<String>,

    // ---- Feature B "htmx Web Tier" (htmx-web-tier) ----
    /// Email of the persona signed in for the current Feature-B scenario,
    /// set by the `<persona> is signed in as a Backend member` /
    /// `... as the workspace admin` Givens. Drives the authenticated GETs.
    pub b_signed_in_email: Option<String>,
    /// Password matching `b_signed_in_email` (the harness re-authenticates
    /// per request — no cookie jar).
    pub b_signed_in_password: Option<String>,
    /// Body of the most recent board / issue / sign-in / forgot GET captured
    /// by a Feature-B When step, reused by the Then assertions on the same
    /// surface so they don't re-fetch.
    pub b_last_body: Option<String>,
    /// Status of the most recent Feature-B GET/POST.
    pub b_last_status: Option<StatusCode>,
    /// Headers of the most recent Feature-B GET/POST (for the session-cookie /
    /// anti-forgery-cookie assertions on US-B04).
    pub b_last_headers: Option<HeaderMap>,
    /// Body of the most recent htmx OOB fragment returned by a live comment
    /// post / issue file (the live card, for the US-B03 live-vs-reloaded
    /// parity assertion).
    pub b_live_fragment: Option<String>,
    /// Body of the reloaded issue page captured AFTER a live post (the
    /// reloaded card, compared against `b_live_fragment`).
    pub b_reloaded_page: Option<String>,
    /// Whether the current US-B01 scenario asked the board template to fail
    /// rendering (the clean-500 error path).
    pub b_force_template_failure: bool,
    /// Raw bytes / text of the most recent `/static/...` asset GET.
    pub b_asset_body: Option<String>,
    /// Content-Type header of the most recent `/static/...` asset GET.
    pub b_asset_content_type: Option<String>,
    /// Cache-Control header of the most recent `/static/...` asset GET.
    pub b_asset_cache_control: Option<String>,
    /// Status of the most recent `/static/...` asset GET.
    pub b_asset_status: Option<StatusCode>,

    // ---- Feature "machine-token-admin-ux" (admin machine-token surface) ----
    /// Email of the persona signed in to the token surface for the current
    /// scenario (the admin "devansh@acme.com" or the non-admin member
    /// "mei@acme.com"). Drives the authenticated GET/POST /admin/tokens.
    pub mt_actor_email: Option<String>,
    /// Whether the harness for this scenario is issuer-configured (signer
    /// present). `false` ⇒ verifier-only (US-MT00 scenario 2 / US-MT01 sc 3).
    pub mt_issuer: bool,
    /// Status of the most recent /admin/tokens GET/POST.
    pub mt_last_status: Option<StatusCode>,
    /// Body of the most recent /admin/tokens GET/POST (the rendered page or
    /// fragment), reused across the Then assertions on the same surface.
    pub mt_last_body: Option<String>,
    /// Headers of the most recent /admin/tokens response.
    pub mt_last_headers: Option<HeaderMap>,
    /// Body of the one-time mint response (the only surface that ever carries a
    /// token value), captured separately so the "shown once" assertion can
    /// compare it against the later list body.
    pub mt_mint_response_body: Option<String>,
    /// The token VALUE the most recent mint exposed once (parsed out of the
    /// one-time display), so later scenarios can assert it never reappears and
    /// can present it to the API to prove it authenticates.
    pub mt_minted_value: Option<String>,
    /// The `jti` of the most-recently minted/seeded token, so a revoke step can
    /// target that exact row and a denylist cross-check can look it up.
    pub mt_last_jti: Option<uuid::Uuid>,
    /// Label → jti for tokens seeded/minted this scenario (list + revoke steps
    /// address a token by its human label).
    pub mt_jti_by_label: HashMap<String, uuid::Uuid>,
    /// A jti that belongs to ANOTHER workspace (the cross-workspace evil-user
    /// path), so the revoke step can attempt a foreign target.
    pub mt_foreign_jti: Option<uuid::Uuid>,

    // ---- Feature "token-management-api" (JSON /api/v1/.../tokens adapter) ----
    /// Body of the FIRST refusal in the cross-workspace-vs-unknown-id
    /// non-enumerability comparison (US-TMA05), captured so the second
    /// unknown-id revoke can be asserted byte-identical.
    pub tma_first_refusal: Option<String>,
    /// Status of that first refusal (must be the identical 404).
    pub tma_first_refusal_status: Option<StatusCode>,
    /// Status of the revoke that precedes a read-after-write re-list
    /// (US-TMA04), so the re-list Then can confirm the revoke returned 204.
    pub tma_revoke_status: Option<StatusCode>,
    /// The token's list entry captured from the read IMMEDIATELY BEFORE the
    /// revoke in the read-after-write scenario (US-TMA04), so the post-revoke
    /// re-list Then can assert every field EXCEPT `revoked`/`last_used_at` is
    /// byte-identical — making "every other field unchanged" a real comparison.
    pub tma_pre_revoke_entry: Option<serde_json::Value>,
    /// Per-request HTTP statuses of the rate-guardrail burst (US-TMA05), so the
    /// within-/beyond-guardrail Thens can classify them.
    pub tma_burst_statuses: Vec<u16>,
    /// Per-request HTTP statuses of the sub-burst fired AFTER the mock clock was
    /// advanced (US-TMA05), proving the bucket refilled deterministically via the
    /// SHIPPED clock seam (NO wall-clock sleep).
    pub tma_burst_after_refill: Vec<u16>,
    /// Response body of the FIRST throttled (429) revoke in the burst (US-TMA05),
    /// so the Then can assert the stable `rate_limited` ErrorBody envelope.
    pub tma_throttle_body: Option<String>,
    /// jti of the caller's OWN authenticating bearer (the provisioning
    /// credential). A management bearer is itself a `machine_tokens` row, so it
    /// necessarily appears in its OWN token list — `list_tokens` lists every
    /// workspace token, including the caller's. The "empty registry" scenario
    /// asserts no token OTHER than this bootstrap credential is listed.
    pub tma_self_bearer_jti: Option<uuid::Uuid>,

    // ---- Feature "Remaining-Surfaces Templating" (remaining-surfaces-templating) ----
    /// Email of the persona signed in for the current remaining-surfaces
    /// scenario, set by the reused `<persona> is signed in as a Backend member`
    /// / `... has no current browser session` Feature-B Givens. Drives the
    /// authenticated GETs. (Distinct slot from `b_signed_in_email` so the two
    /// feature step modules cannot collide within a scenario.)
    pub r_signed_in_email: Option<String>,
    /// Body of the most recent remaining-surfaces GET/POST captured by a When
    /// step, reused by the Then assertions on the same surface.
    pub r_last_body: Option<String>,
    /// Status of the most recent remaining-surfaces GET/POST.
    pub r_last_status: Option<StatusCode>,
    /// Headers of the most recent remaining-surfaces GET/POST (for the
    /// signed-out 303 Location assertion on US-R04).
    pub r_last_headers: Option<HeaderMap>,

    // ---- Feature "multi-workspace-tenancy" — Slice 1 (coexistence + resolution) ----
    /// workspace name -> workspace_id for every workspace seeded this scenario.
    /// Slice 1 is the FIRST fixture that holds more than one entry; the second
    /// `INSERT INTO workspaces` fails RED until the `0002` migration drops
    /// `uniq_one_workspace` (and the bootstrap.rs:289 409 guard is gone).
    pub mwt_workspace_ids: HashMap<String, uuid::Uuid>,
    /// (workspace name, project name) -> (team_slug, project_slug) so a When
    /// step can reconstruct the `/api/v1/teams/{team}/projects/{project}/issues`
    /// URL for a workspace-scoped project.
    pub mwt_project_route: HashMap<(String, String), (String, String)>,
    /// member email -> the bearer JWT minted bound to that member's workspace
    /// (the `token.workspace_id` resolution seam, ADR-001).
    pub mwt_bearer_by_email: HashMap<String, String>,
    /// A credential whose holder belongs to NO workspace (the fail-closed
    /// resolution scenario). Presented like any bearer; resolution must refuse.
    pub mwt_no_workspace_bearer: Option<String>,
    /// Issue keys recorded for a workspace BEFORE the guard is dropped, so the
    /// no-rewrite scenario can assert before/after equality.
    pub mwt_issues_before_by_workspace: HashMap<String, Vec<String>>,
    /// The workspace_id captured before the guard-drop, to assert identity is
    /// unchanged afterward.
    pub mwt_workspace_id_before: Option<uuid::Uuid>,
    /// Status of the most recent `/api/v1/.../issues` GET captured by a When
    /// step, reused by the Then assertions.
    pub mwt_last_status: Option<StatusCode>,
    /// Body of the most recent `/api/v1/.../issues` GET, parsed by the Then
    /// assertions for the listed issue keys.
    pub mwt_last_body: Option<String>,
    /// The Acme list answer captured in the disjoint-set scenario (so a second
    /// When can capture the Globex answer and the Then compares both).
    pub mwt_acme_answer: Option<String>,
    /// The Globex list answer captured in the disjoint-set scenario.
    pub mwt_globex_answer: Option<String>,

    // ---- Feature "multi-workspace-tenancy" — Slice 2 (web-tier boundary) ----
    /// Email of the member signed in on the WEB for the current slice-2 scenario,
    /// set by `"<email>" is signed in on the web acting on workspace "<ws>"`.
    /// Drives the authenticated web GET/POST (the harness re-authenticates per
    /// request — no cookie jar — so the password is recorded too).
    pub mwt2_web_email: Option<String>,
    /// Password matching `mwt2_web_email`. Seeds use a per-role constant so the
    /// web sign-in can authenticate the member/admin/contractor.
    pub mwt2_web_password: Option<String>,
    /// The workspace name the signed-in web member is ACTING on (the session's
    /// active workspace, ADR-005). Recorded so a When step can resolve the
    /// acting-workspace route + the switch step can re-stamp it.
    pub mwt2_acting_ws: Option<String>,
    /// Body of the most recent web GET/POST captured by a slice-2 When step,
    /// reused by the Then assertions on the same surface so they don't re-fetch.
    pub mwt2_last_body: Option<String>,
    /// Status of the most recent slice-2 web GET/POST.
    pub mwt2_last_status: Option<StatusCode>,
    /// Body of the FIRST refusal in a foreign-id-vs-never-existed-id comparison
    /// (the non-enumerability core), captured so the second (never-existed)
    /// request can be asserted observationally identical.
    pub mwt2_first_refusal_body: Option<String>,
    /// Status of that first (foreign-id) refusal — must equal the second.
    pub mwt2_first_refusal_status: Option<StatusCode>,
    /// label -> jti for a credential seeded into a NAMED workspace (the admin-
    /// cannot-cross scenario seeds a Globex credential and addresses it by label).
    pub mwt2_credential_jti_by_label: HashMap<String, uuid::Uuid>,
    /// (workspace name, jti) snapshot of a foreign credential's `revoked_at` BEFORE
    /// the cross-tenant revoke attempt, so the Then can assert it is unchanged.
    pub mwt2_credential_revoked_before: Option<bool>,

    // ---- Feature "multi-workspace-tenancy" — Slice 3 (API + machine-token + session) ----
    /// label -> jti for a `machine_tokens` row seeded into a NAMED real workspace
    /// (the residual-closure fixtures: an Acme token + a REAL Globex token, so the
    /// cross-tenant list/revoke proof uses real fixtures, not a synthetic uuid).
    pub mwt3_token_jti_by_label: HashMap<String, uuid::Uuid>,
    /// A fresh bearer minted + then REVOKED (its jti on the per-request denylist),
    /// for the verify-path-unchanged regression — its next call must be 401.
    pub mwt3_revoked_bearer: Option<String>,
    /// Body of the FIRST refusal in a foreign-resource-vs-never-existed comparison
    /// (the API non-enumerability core), captured so the second (never-existed)
    /// request can be asserted byte-identical.
    pub mwt3_first_refusal_body: Option<String>,
    /// Status of that first (foreign) refusal — must equal the second.
    pub mwt3_first_refusal_status: Option<StatusCode>,
    /// The member whose session-resolution contract is under test (US-MWT04).
    pub mwt3_resolution_user: Option<String>,
    /// The workspace the resolution is EXPECTED to yield (`None` for the
    /// fail-closed no-membership case).
    pub mwt3_expected_workspace: Option<String>,
    /// The workspace `resolve_active_workspace` actually returned (`None` =
    /// fail-closed, no workspace resolved).
    pub mwt3_resolved_workspace: Option<uuid::Uuid>,
    /// Whether the resolution When step ran (guards against a Then false-pass on
    /// an un-run resolution).
    pub mwt3_resolution_ran: bool,

    // ---- Feature "multi-workspace-tenancy" — Slice 4 (non-enumerability matrix) ----
    /// Body of the FIRST refusal (the FOREIGN-resource reach) in a slice-4 matrix
    /// cell, captured so the second (never-existed) reach can be asserted
    /// byte-identical. Distinct slot from the slice-2/3 ones so a unified-matrix
    /// scenario that touches BOTH web and API surfaces does not collide.
    pub mwt4_first_refusal_body: Option<String>,
    /// Status of that first (foreign) refusal — must equal the second.
    pub mwt4_first_refusal_status: Option<StatusCode>,
    /// The foreign id/slug strings that MUST NOT appear in any refusal body (the
    /// oracle-hunt no-echo assertion), accumulated per scenario.
    pub mwt4_foreign_identifiers: Vec<String>,
    /// Every cross-tenant refusal status observed in an oracle-hunt scenario, so
    /// the Then can assert NONE is a 403 and ALL are 404.
    pub mwt4_refusal_statuses: Vec<StatusCode>,
    /// label -> attachment_id seeded into a NAMED real workspace's issue (the
    /// foreign attachment-download target — the `find_attachment_for_requester`
    /// idiom).
    pub mwt4_attachment_id_by_label: HashMap<String, uuid::Uuid>,
    /// Snapshot count of comments in a workspace BEFORE a cross-tenant comment
    /// write, so the Then can assert the foreign workspace gained none.
    pub mwt4_foreign_comment_count_before: Option<i64>,
    /// Snapshot count of attachments in a workspace BEFORE a cross-tenant upload,
    /// so the Then can assert the foreign workspace gained none.
    pub mwt4_foreign_attachment_count_before: Option<i64>,
    /// Snapshot of a Globex issue's state BEFORE a cross-tenant state-change, so
    /// the Then can assert it is unchanged.
    pub mwt4_foreign_issue_state_before: Option<String>,

    // ---- Feature "multi-workspace-provisioning" — Slice 5 (migration guarantee) ----
    /// Per-scenario schema name for the pre-feature single-workspace install, so
    /// the After hook (or scenario end) can drop it.
    pub mwt5_schema: Option<String>,
    /// Raw per-scenario pool pinned to `mwt5_schema`. Migrations are applied to it
    /// via the real `run_migrations_from_dir` runner (NOT the embedded set), so the
    /// pre-feature history then forward-only upgrade can be staged on disk.
    pub mwt5_pool: Option<sqlx::PgPool>,
    /// Handle to the staged on-disk migrations dir (pre-feature subset, then the
    /// canonical forward-only set). Held so the temp dir lives for the scenario.
    pub mwt5_staged: Option<TestMigrationsDir>,
    /// The existing workspace's id, captured at seed time — must be unchanged and
    /// not duplicated by the upgrade (and re-upgrade).
    pub mwt5_workspace_id: Option<uuid::Uuid>,
    /// Row-level snapshot of every tenant table AFTER the first upgrade, keyed by
    /// table name → ordered list of row-JSON strings. Compared for equality after a
    /// second upgrade to prove idempotence (no row rewritten or duplicated).
    pub mwt5_snapshot_after_first: HashMap<String, Vec<String>>,
    /// Row-level snapshot of every tenant table taken BEFORE the upgrade is applied,
    /// keyed by table name → ordered list of row-JSON strings. Compared for equality
    /// AFTER the upgrade to prove the forward-only migrations touched no tenant row
    /// (the walking-skeleton data-safety proof).
    pub mwt5_snapshot_before_upgrade: HashMap<String, Vec<String>>,
    /// The admin email seeded for the pre-feature install, so the sign-in seam can
    /// resolve the carried-over user to workspace 1 after the upgrade.
    pub mwt5_admin_email: Option<String>,
    /// The `jti` of the machine token seeded BEFORE the upgrade (bound to the admin
    /// plus workspace 1), captured so the carried-credential resolution proof (sc 3)
    /// can look the SAME token up after the upgrade and assert it still acts on
    /// workspace 1 with no re-issue or re-binding.
    pub mwt5_machine_token_jti: Option<uuid::Uuid>,
    /// The member's board view (visible issue titles + project names) captured
    /// through the SHIPPED resolution + scoped-read seam BEFORE the upgrade, so
    /// step 04-05's regression proof can assert the post-upgrade view is
    /// byte-identical — nothing added, removed, or reordered (NFR-MWT-REL-02).
    /// `None` until the upgrade `When` step records it at its start.
    pub mwt5_pre_upgrade_board: Option<(Vec<String>, Vec<String>)>,

    // ---- Feature "multi-workspace-provisioning" — Slice 6 (provisioning) ----
    /// In-process harness whose migrated schema the provisioning CLI subprocess
    /// also targets (via DATABASE_URL pinned to the same search_path). Reused to
    /// drive the SHIPPED sign-in + resolution seam for the "first admin acts on
    /// the new workspace" leg.
    pub mwt6_harness: Option<InProcHarness>,
    /// Email of the bootstrap-claiming super-admin seeded in the Background.
    pub mwt6_superadmin_email: Option<String>,
    /// Captured exit code of the last `provision-workspace` CLI subprocess.
    pub mwt6_cli_exit: Option<i32>,
    /// Captured stdout of the last `provision-workspace` CLI subprocess (carries
    /// the new workspace id + first-admin invite link).
    pub mwt6_cli_stdout: Option<String>,
    /// The provisioned workspace's id, parsed from the CLI stdout.
    pub mwt6_provisioned_workspace_id: Option<uuid::Uuid>,
    /// Name → id of every workspace seeded/provisioned in the scenario.
    pub mwt6_workspace_ids: HashMap<String, uuid::Uuid>,
    /// Workspace count snapshot before an unauthorized provisioning attempt, so
    /// the refusal can prove no new workspace was created (fail-closed gate).
    pub mwt6_workspaces_before_attempt: Option<i64>,
    /// Row-level before-snapshot of the EXISTING workspace's tenant data, keyed
    /// by `(table, workspace_id)`. Recorded before provisioning a new workspace
    /// so the after-snapshot can prove every pre-existing tenant's rows are
    /// unchanged row-for-row (NFR-MWT-REL-01). See step 03-02.
    pub mwt6_existing_snapshot: HashMap<String, Vec<String>>,
    /// Email of the provisioned workspace's first admin (step 03-03 isolation
    /// leg) — used to seed her team membership and resolve her acting workspace.
    pub mwt6_first_admin_email: Option<String>,
    /// Provisioned-tenant issue titles by workspace name (step 03-03), so the
    /// isolation read can assert the admin sees EXACTLY her own workspace's data.
    pub mwt6_provisioned_issue_titles: HashMap<String, Vec<String>>,
    /// Issue titles returned by the last scoped board read (step 03-03), driven
    /// through the SHIPPED resolution + scoped-read seam as the provisioned admin.
    pub mwt6_listed_issue_titles: Vec<String>,
    /// The real address (team slug, project slug, issue number) of a
    /// provisioned-tenant issue (step 03-04 non-enumerability leg). An
    /// existing-workspace member reaches THIS foreign address and a
    /// never-existed one, and the two refusals must be byte-identical.
    pub mwt6_foreign_issue_address: Option<(String, String, i32)>,
    /// (status, body) of the FIRST request in the cross-tenant non-enumerability
    /// comparison — the existing member's reach for a REAL provisioned-tenant
    /// issue by its real address (step 03-04). Captured so the second
    /// never-existed reach can be asserted byte-identical.
    pub mwt6_first_refusal: Option<(StatusCode, String)>,
    /// (status, body) of the SECOND request — the existing member's reach for an
    /// issue that never existed (step 03-04). Must equal `mwt6_first_refusal`.
    pub mwt6_second_refusal: Option<(StatusCode, String)>,
    /// (exit code, stdout, stderr) of the FIRST unauthorized provisioning attempt
    /// against a name matching an EXISTING workspace (step 03-05 non-enumerable
    /// authz leg). The authz gate denies BEFORE any workspace lookup, so this must
    /// be byte-identical to the never-existed attempt below — the refusal carries
    /// no oracle for whether the target already exists.
    pub mwt6_authz_refusal_existing: Option<(i32, String, String)>,
    /// (exit code, stdout, stderr) of the SECOND unauthorized provisioning attempt
    /// against a name that NEVER existed (step 03-05). Must equal
    /// `mwt6_authz_refusal_existing` exactly (same exit code + same output).
    pub mwt6_authz_refusal_never_existed: Option<(i32, String, String)>,
    /// A REAL Acme-bound EdDSA machine bearer (step 03-06): the most-privileged
    /// bearer credential a caller could hold (workspace-1-bound, registered so the
    /// jti denylist admits it) — used to prove provisioning is unreachable on the
    /// bearer surface EVEN for a fully-valid token.
    pub mwt6_bearer: Option<String>,
    /// The workspace count recorded BEFORE any bearer-surface provisioning probe
    /// (step 03-06). The "no new workspace created" assertion compares the count
    /// after the probes against this baseline.
    pub mwt6_bearer_probe_ws_before: Option<i64>,
    /// The `(status, body)` of EVERY plausible provisioning address probed over
    /// `/api/v1` with the Acme-bound bearer (step 03-06). Each must be a
    /// non-enumerable 404 — NOT a provisioning success (2xx) — proving no
    /// provisioning path is reachable on the bearer surface.
    pub mwt6_bearer_probe_responses: Vec<(StatusCode, String)>,
    /// (status, body) of EACH web grant POST submitted for the SAME operator in the
    /// idempotent-grant scenario (web-provisioning-flow 01-04). Two grants for one
    /// existing member must BOTH confirm (200 + confirmation marker), and after both
    /// the operator must be recorded a super-admin exactly once (no duplicate
    /// `instance_admins` row) — proving the SHIPPED `INSERT … ON CONFLICT DO NOTHING`
    /// idempotence behind the new web grant adapter.
    pub mwt6_grant_responses: Vec<(StatusCode, String)>,

    /// The email address SUBMITTED on each web grant POST, parallel to
    /// `mwt6_grant_responses`. Used by the non-enumerability scenario
    /// (web-provisioning-flow 02-01) to normalise the caller-supplied email out of
    /// each confirmation body before comparing: the response echoes the submitted
    /// address (which the caller already knows — not an oracle), so byte-identity
    /// is asserted on the email-NORMALISED bodies. What MUST be identical is the
    /// confirmation TEMPLATE; what may differ is only the echoed input.
    pub mwt6_grant_submitted_emails: Vec<String>,

    /// (route, status, body) of EACH `/admin/instance/…` route an unauthorised
    /// caller probed in the non-enumerability scenarios (web-provisioning-flow
    /// 02-02 signed-out / 02-03 non-super-admin). Every entry must be
    /// BYTE-IDENTICAL (status + full body) to `mwt6_admin_never_existed` — no 403,
    /// 401, or login redirect distinguishes the admin surface from a never-existed
    /// path (ADR-002 response-mapping contract).
    pub mwt6_admin_surface_refusals: Vec<(String, StatusCode, String)>,
    /// METHOD → (status, body) of an unauthorised caller's request to a path that
    /// never existed, captured PER HTTP METHOD — the control each admin-surface
    /// refusal is compared against. Per-method because a non-safe method (POST)
    /// is screened by the double-submit CSRF layer BEFORE routing, so a
    /// never-existed POST and a never-existed GET refuse through different layers;
    /// the non-enumerability property is that an `/admin/instance/…` route refuses
    /// IDENTICALLY to a never-existed path requested with the SAME method.
    pub mwt6_admin_never_existed: HashMap<String, (StatusCode, String)>,
    /// (route, status, body) of EACH `/admin/instance/…` route a SIGNED-OUT caller
    /// probed, captured as the cross-cause baseline by the non-super-admin
    /// non-enumerability scenario (web-provisioning-flow 02-03). The 02-03 scenario
    /// drives BOTH a signed-in ordinary member AND a signed-out caller against every
    /// route so it can assert the non-super-admin refusal is BYTE-IDENTICAL to the
    /// signed-out refusal for the SAME route — proving the CAUSE of refusal
    /// (not-signed-in vs not-authorized) is indistinguishable (ADR-002 response-mapping
    /// rows 1 and 2 collapse to the same uniform 404).
    pub mwt6_signed_out_refusals: Vec<(String, StatusCode, String)>,
    /// The email of the ordinary signed-in member driving the non-super-admin
    /// non-enumerability scenario (web-provisioning-flow 02-03). Set by the
    /// `"<email>" is signed in on the web and is not a super-admin` Given and read
    /// by the When that drives each /admin/instance route as that member.
    pub mwt6_acting_member_email: Option<String>,

    // ---- invite-accept-flow (us-invite-accept) ----
    /// The in-process harness for the invite-accept scenarios. Its session_secret
    /// signs the InviteToken the Background mints, so the GET/POST handlers verify
    /// the SAME secret. Separate from `mwt6_harness` (a different feature's seed).
    pub ia_harness: Option<InProcHarness>,
    /// The provisioned workspace name → its id (the landing tenant). Seeded by the
    /// Background via the SHIPPED `Store::provision_workspace`.
    pub ia_workspace_ids: HashMap<String, uuid::Uuid>,
    /// The live invite id minted in the Background (the `invites` row PK).
    pub ia_invite_id: Option<uuid::Uuid>,
    /// The HMAC signature over `invite_id|expires_at` (the `sig` URL param).
    pub ia_invite_sig: Option<String>,
    /// The first-admin's user id (the invite's `created_by` — the row the consume
    /// TX writes the chosen password onto).
    pub ia_admin_user_id: Option<uuid::Uuid>,
    /// The first-admin's `password_hash` snapshotted at Background seed time (the
    /// throwaway initial credential, before any accept). The "no password has yet
    /// been set" assertion compares the post-GET hash against this baseline to prove
    /// the non-committal GET wrote no password — and the falsifiability litmus
    /// (a GET that writes the chosen password) reds it.
    pub ia_seeded_password_hash: Option<String>,
    /// The session cookie (`foundry_session=...`) captured from the POST 303 — the
    /// auto-sign-in credential proving "no separate login step".
    pub ia_session_cookie: Option<String>,
    /// The POST accept response (status, body) — the 303 redirect on success.
    pub ia_post_status: Option<StatusCode>,
    pub ia_post_location: Option<String>,
    /// The signed-in landing page body fetched with the session cookie.
    pub ia_landing_body: Option<String>,
    /// The CANONICAL refusal arm (status + full body) captured by the expired
    /// scenario (02-01) — scenarios 6/7/8 assert byte-identity AGAINST this.
    pub ia_refusal_status: Option<StatusCode>,
    pub ia_refusal_body: Option<String>,
    /// The just-past-expiry refusal arm (status + full body) captured by the
    /// 02-02 scenario, held across the in-scenario recompute of the canonical
    /// expired-one-day arm so the two can be asserted byte-identical.
    pub ia_just_past_refusal_status: Option<StatusCode>,
    pub ia_just_past_refusal_body: Option<String>,
    /// The four invalid-link refusal arms (expired, already-used,
    /// tampered-signature, unknown-id), each captured as (status, full body) by
    /// the 02-05 consolidated non-enumerability scenario. The byte-identity Then
    /// asserts all four are MUTUALLY identical (status + full body), proving an
    /// attacker cannot distinguish WHY a link is invalid.
    pub ia_four_refusals: Vec<(StatusCode, String)>,
    /// The first-admin's observable state snapshotted AFTER a successful accept
    /// (02-06 single-use): the real argon2id `password_hash` written by the
    /// consume TX, plus the `used_at`/`used_by` set on the consumed invite row.
    /// The "no new password is set and no session is created" assertion compares
    /// the post-SECOND-attempt state against this snapshot — the falsifiability
    /// litmus (dropping the guard's `used_at IS NULL` clause, so the second
    /// accept re-consumes + re-writes a NEW password) reds it.
    pub ia_consumed_password_hash: Option<String>,
    pub ia_consumed_used_at: Option<time::OffsetDateTime>,
    pub ia_consumed_used_by: Option<uuid::Uuid>,
    /// The outcomes of N CONCURRENT accept POSTs of ONE live invite (02-07
    /// single-use under concurrency), each captured as (status, session_cookie,
    /// password_sent). Exactly one must be a 303 SEE_OTHER carrying a session
    /// cookie (the winner); the rest must be the uniform 200 refusal with no
    /// session. The atomic guarded UPDATE (`... WHERE used_at IS NULL ...
    /// RETURNING`) serializes the race: the DB admits exactly one winner.
    pub ia_concurrent_outcomes: Vec<(StatusCode, Option<String>, String)>,
    /// Every response body observed across a FULL accept cycle (02-09 no-secret-
    /// leakage): the GET set-password form, the success POST 303, the signed-in
    /// landing page, and a hostile prober's uniform refusal. The no-leak Then
    /// scans this collected surface for the invite `sig` and the submitted
    /// password. This is the strongest available log observable: NO in-process
    /// tracing-capture seam is wired into the harness (tracing is global-only,
    /// initialised in main.rs, not the test harness), so per the step's guidance
    /// the response-body surface stands in for "the logs", backed by the
    /// tracing-keyed-on-invite_id design citation (invites_accept.rs:83/120/166/
    /// 178/202 — every tracing line carries ONLY %invite_id + %err, never the sig
    /// or password). The falsifiability litmus injects a `tracing`-shaped leak
    /// (rendering the sig into a refusal/landing body) and proves the scan reds.
    pub ia_cycle_bodies: Vec<String>,
    /// The hostile prober's supplied (tampered) `sig` from the 02-09 no-leak
    /// cycle. The no-signature-in-logs Then asserts neither this nor the genuine
    /// holder's sig appears in any collected log-surface body (a refusal must be
    /// non-committal on the queried sig — NFR-3/NFR-5).
    pub ia_prober_sig: Option<String>,
    /// The holder's OWN GET set-password form body from the 02-09 no-leak cycle.
    /// DELIBERATELY kept OUT of `ia_cycle_bodies` (the no-SIG scan set): the form
    /// legitimately carries the sig in its hidden field — it is the holder's own
    /// valid link round-tripped to her, NOT a log surface. It is captured here so
    /// the no-PASSWORD Then can still scan it (the form must never echo the
    /// cleartext password). See `no_signature_in_logs` for the assertion-site guard
    /// that pins this exclusion as deliberate-by-design.
    pub ia_get_form_body: Option<String>,

    // ---- workspace-member-invites (us-member-invites) ----
    /// The in-process harness for the member-invites scenarios. Its session_secret
    /// signs the InviteToken the REAL issuance handler mints, so the accept GET/POST
    /// verify the SAME secret. Separate from `ia_harness`/`mwt6_harness` (each a
    /// different feature's per-scenario seed).
    pub mi_harness: Option<InProcHarness>,
    /// The provisioned workspace name → its id (the inviting/landing tenant). Seeded
    /// by the Background via the SHIPPED `Store::provision_workspace`.
    pub mi_workspace_ids: HashMap<String, uuid::Uuid>,
    /// The inviting admin's email (Dana) — used to authenticate her web issuance POST.
    pub mi_admin_email: Option<String>,
    /// The inviting admin's user id (the invite's `created_by`).
    pub mi_admin_user_id: Option<uuid::Uuid>,
    /// The member invite id minted by the REAL issuance handler (parsed from the
    /// emitted accept link in the rendered "invite sent" fragment).
    pub mi_invite_id: Option<uuid::Uuid>,
    /// The HMAC signature parsed from the emitted accept link (the `sig` URL param).
    pub mi_invite_sig: Option<String>,
    /// The accept POST response (status, location) — the 303 redirect on success.
    pub mi_post_status: Option<StatusCode>,
    pub mi_post_location: Option<String>,
    /// The auto-sign-in session cookie captured from the accept POST 303 (proving
    /// "no separate login step").
    pub mi_session_cookie: Option<String>,
    /// The CANONICAL member-invite refusal arm (status + full body) captured by
    /// the expired-one-day scenario (02-01) — scenarios 13/15/16/17 assert
    /// byte-identity AGAINST this. The accept route is shared, so this is the
    /// SHIPPED `invite_refusal_page()` (200 OK, non-leaking, byte-identical).
    pub mi_refusal_status: Option<StatusCode>,
    pub mi_refusal_body: Option<String>,
    /// The email-collision invite-under-test id (02-03, scenario 17), stashed before
    /// the shared byte-identity Then recomputes the canonical expired arm on a fresh
    /// control invite (which overwrites `mi_invite_id`). Lets the "no second account
    /// / invite not consumed" Then assert the collision invite stayed unconsumed.
    pub mi_collision_invite_id: Option<uuid::Uuid>,
    /// The FIVE invalid-accept arm drive recipes (id, sig, optional password) for
    /// the byte-identity property (02-02, scenario 18): expired, already-used,
    /// tampered-signature, unknown-id, and email-already-a-user. The When replays
    /// each; the Then asserts the captured responses are mutually byte-identical.
    pub mi_five_arms: Vec<crate::steps::feature_member_invites::FiveArm>,
    /// The five invalid-accept arms' captured user-visible responses (status + full
    /// body), in arm order — asserted MUTUALLY byte-identical by scenario 18.
    pub mi_five_responses: Vec<(StatusCode, String)>,
    /// The `used_at` snapshot taken right after the TOCTOU out-of-band consume
    /// (scenario 21, 02-04) — the stale-POST Then asserts it is UNCHANGED, proving
    /// the refused stale POST did not re-stamp the consumed invite.
    pub mi_consumed_used_at: Option<time::OffsetDateTime>,
    /// The `foundry_csrf=...` cookie the LIVE member-accept GET minted (scenario 21,
    /// 02-04) — captured at arrival so the now-stale POST reuses the GET-time
    /// double-submit token, making its refusal fire on the TX guard, not CSRF.
    pub mi_get_csrf_cookie: Option<String>,
    /// Issuance-surface refusals (scenarios 22/23, step 02-05): each `(method url,
    /// status, body)` probe of `/workspace/invites` by a non-admin or signed-out
    /// caller. Asserted BYTE-IDENTICAL (status + full body) to the same-method
    /// never-existed control. Per-method because a CSRF-screened POST and a
    /// gate-refused GET must be compared like-for-like.
    pub mi_issuance_refusals: Vec<(String, StatusCode, String)>,
    /// The same-method never-existed-path controls (GET + POST) for the issuance
    /// non-enumerability comparison (scenarios 22/23). Keyed by HTTP method.
    pub mi_issuance_never_existed: std::collections::HashMap<String, (StatusCode, String)>,
    /// The SIGNED-OUT issuance refusals captured alongside the non-admin probes
    /// (scenario 23) so the cross-cause byte-identity (signed-out == non-admin) is
    /// asserted route-for-route. Each `(method url, status, body)`.
    pub mi_issuance_signed_out_refusals: Vec<(String, StatusCode, String)>,
    /// The SIGNED-IN NON-ADMIN issuance refusals (Marco) captured in scenario 23 as
    /// the cross-cause baseline the signed-out refusal is asserted byte-identical to,
    /// route-for-route. Each `(method url, status, body)`.
    pub mi_issuance_non_admin_refusals: Vec<(String, StatusCode, String)>,

    // ---- per-workspace-backup (US-PWB-01/02/03) ----
    /// The in-process harness for the per-workspace-backup scenarios. Its migrated
    /// schema is the one the `export-workspace` CLI subprocess targets via
    /// DATABASE_URL, so the export reads against the very rows the Background seeded.
    pub pwb_harness: Option<InProcHarness>,
    /// Seeded workspace name → its id (e.g. "Globex LLC" → uuid). Resolves the
    /// selector token ("globex"/"acme") in the When step to the real workspace id.
    pub pwb_workspace_ids: HashMap<String, uuid::Uuid>,
    /// The output path the export CLI was told to write the archive to (a path
    /// inside a per-scenario `TempDir`, held open by `pwb_tempdir`).
    pub pwb_out_path: Option<PathBuf>,
    /// The per-scenario TempDir backing `pwb_out_path` — held so the archive file
    /// survives until the Then steps inspect it.
    pub pwb_tempdir: Option<tempfile::TempDir>,
    /// The export CLI's exit code (port-exposed observable).
    pub pwb_cli_exit: Option<i32>,
    /// The export CLI's captured stdout (the per-table report + `status:` line).
    pub pwb_cli_stdout: Option<String>,
    /// The verify-export CLI's captured stderr — failure diagnostics (exit 4
    /// completeness / exit 6 isolation) go to stderr, so the Then steps surface it
    /// when a confirmation line is missing.
    pub pwb_cli_stderr: Option<String>,
    /// The workspace name the id-selector export (step 01-02) is expected to
    /// resolve to — set when exporting by id so the Then step can assert the
    /// resolver picked the right tenant.
    pub pwb_expected_name: Option<String>,
    /// Whole-instance tenant-table snapshot captured BEFORE the export runs
    /// (step 01-03 read-only proof): table name → ordered list of whole-row JSON
    /// strings. The export must leave every tenant row byte-for-byte unchanged, so
    /// the Then steps re-snapshot and assert equality against this baseline.
    pub pwb_snapshot_before_export: std::collections::HashMap<String, Vec<String>>,
    /// The user id made a member of BOTH named workspaces (step 02-02, scenario 8 /
    /// OD-PWB-1 dual-membership fixture). The Then steps assert this shared user is
    /// included in the target archive as a legitimate member and is NOT flagged by
    /// verification as a sibling-workspace leak.
    pub pwb_shared_user_id: Option<uuid::Uuid>,
}

impl FoundryWorld {
    /// Close the US-03 restored harness's sqlx pool, awaiting connection
    /// teardown, while the `us_03_restore_guard` is still held.
    ///
    /// Wired as a cucumber `After` hook (see `tests/acceptance.rs`) so it
    /// runs BEFORE the World drops. `PgPool::Drop` is non-blocking and
    /// cannot await, so relying on field-drop order alone could let a
    /// sibling scenario acquire the guard and run `pg_restore --clean`
    /// (DROP TABLE) before this pool's `min_connections(1)` connection is
    /// actually closed — blocking the DROP forever on the relation lock.
    /// Closing here, while the guard is held, guarantees the connections
    /// are gone before the lock is released.
    pub async fn close_us03_restored_pool(&mut self) {
        if let Some(harness) = self.us_03_restored_harness.as_ref() {
            harness.app.state.store.pool().close().await;
        }
    }
}
