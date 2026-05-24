# Step Definition Skeletons — Slice 1

Proposed Rust step-definition signatures. No implementations — DELIVER writes the bodies. The cucumber attribute + signature for each step phrase is the contract between DISTILL and DELIVER.

cucumber-rs 0.21.x conventions used: `#[given/when/then(regex = "...")]` with `&mut FoundryWorld` first param; async step bodies via `#[tokio::main]` test runner; capture groups passed in argument order after the world.

## Shared support (cross-feature, `crates/foundry-acceptance/src/support/`)

```rust
// support/spawn_app.rs
pub struct TestApp {
    pub addr: std::net::SocketAddr,
    pub pool: sqlx::PgPool,
    pub fake_smtp: std::sync::Arc<FakeEmailSender>,
    pub fake_clock: std::sync::Arc<MockClock>,
    pub _shutdown: tokio::sync::oneshot::Sender<()>,
}
pub async fn spawn_app() -> TestApp;          // bootstraps fresh schema + binds 127.0.0.1:0

// support/db.rs
pub async fn fresh_schema() -> (String, sqlx::PgPool);
pub async fn drop_schema(schema: &str, pool: &sqlx::PgPool);

// support/fake_smtp.rs
pub struct SentEmail { pub to: String, pub subject: String, pub body: String }
pub struct FakeEmailSender { /* Mutex<Vec<SentEmail>> */ }
impl FakeEmailSender {
    pub fn sent(&self) -> Vec<SentEmail>;
    pub fn count_to(&self, addr: &str) -> usize;
    pub fn last_to(&self, addr: &str) -> Option<SentEmail>;
}

// support/fake_clock.rs
pub struct MockClock { /* AtomicI64 epoch_ms + Mutex<Vec<RecordedSleep>> */ }
pub struct RecordedSleep { pub at_ms: i64, pub duration_ms: u64 }
impl MockClock {
    pub fn advance(&self, by: std::time::Duration);
    pub fn now(&self) -> chrono::DateTime<chrono::Utc>;
    pub fn recorded_sleeps(&self) -> Vec<RecordedSleep>;
}

// support/cookie_assertions.rs
pub struct ParsedSetCookie {
    pub name: String, pub value: String,
    pub http_only: bool, pub same_site: Option<String>, pub secure: bool,
    pub max_age_seconds: Option<i64>,
}
pub fn parse_set_cookie(header_value: &str) -> ParsedSetCookie;

// support/html_assertions.rs
pub fn body_contains_text(body: &str, needle: &str) -> bool;
pub fn body_contains_column(body: &str, column_name: &str) -> bool;
pub fn body_is_htmx_fragment(body: &str, headers: &reqwest::header::HeaderMap) -> bool;
//   ^ checks NOT-doctype + presence of `HX-Trigger` or fragment-only structure

// support/db_introspect.rs (read-only assertion helpers, NOT for state setup)
pub async fn count_workspaces(pool: &sqlx::PgPool) -> i64;
pub async fn count_users_by_email(pool: &sqlx::PgPool, email: &str) -> i64;
pub async fn session_row_exists(pool: &sqlx::PgPool, session_id: &str) -> bool;
pub async fn issue_key_by_id(pool: &sqlx::PgPool, issue_id: uuid::Uuid) -> Option<String>;
pub async fn project_exists(pool: &sqlx::PgPool, team_slug: &str, project_slug: &str) -> bool;
pub async fn count_outbox_events(pool: &sqlx::PgPool, event_type: &str) -> i64;
```

## US-01 — `steps/us_01_install.rs`

```rust
use cucumber::{given, when, then};
use crate::world::FoundryWorld;

// Background ----------------------------------------------------------------

#[given(regex = r"^an empty working directory with a Foundry checkout and a default `\.env`$")]
async fn empty_working_dir(world: &mut FoundryWorld);

#[given(regex = r"^no Foundry containers or volumes exist on this machine$")]
async fn no_containers(world: &mut FoundryWorld);

// Scenario 1 — fresh install ------------------------------------------------

#[when(regex = r"^the operator starts the Foundry stack with `docker compose up -d`$")]
async fn compose_up(world: &mut FoundryWorld);

#[then(regex = r"^within (\d+) seconds the foundry container reports healthy on `/healthz`$")]
async fn foundry_healthy(world: &mut FoundryWorld, timeout_seconds: u64);

#[then(regex = r"^the postgres container reports healthy$")]
async fn postgres_healthy(world: &mut FoundryWorld);

#[then(regex = r"^the foundry container logs contain exactly one line beginning with `\[BOOTSTRAP\]`$")]
async fn one_bootstrap_log_line(world: &mut FoundryWorld);

#[then(regex = r"^that line contains a URL with a token query parameter$")]
async fn bootstrap_url_has_token(world: &mut FoundryWorld);

// Scenario 2 — re-run idempotent --------------------------------------------

#[given(regex = r"^the operator has already claimed admin from a prior install$")]
async fn admin_already_claimed_prior_install(world: &mut FoundryWorld);

#[when(regex = r"^the operator runs `docker compose up -d` a second time$")]
async fn compose_up_again(world: &mut FoundryWorld);

#[then(regex = r"^the foundry container logs contain zero new lines beginning with `\[BOOTSTRAP\]`$")]
async fn zero_new_bootstrap_lines(world: &mut FoundryWorld);

// Scenario 3 — no host-bind volumes -----------------------------------------

#[when(regex = r"^the operator inspects the foundry service definition in `docker-compose\.yml`$")]
async fn inspect_compose_yml(world: &mut FoundryWorld);

#[then(regex = r"^the foundry service declares zero host-bind mounts under `volumes`$")]
async fn no_host_binds(world: &mut FoundryWorld);

#[then(regex = r"^the only persistent volume is a named volume backing postgres$")]
async fn only_named_volume(world: &mut FoundryWorld);

// Scenario 4 — manual ------ no automated steps. The scenario is skipped by
// the test runner via tag filter `not @manual`; a separate process renders
// it into the manual UAT checklist artifact.
```

## US-05 — `steps/us_05_bootstrap.rs`

```rust
#[given(regex = r"^a fresh Foundry instance with no workspace and no users$")]
async fn fresh_instance(world: &mut FoundryWorld);

#[given(regex = r#"^the bootstrap token "([^"]+)" was minted (\d+) minutes? ago with a (\d+)-minute TTL$"#)]
async fn bootstrap_token_minted(world: &mut FoundryWorld, token: String, minted_ago_min: i64, ttl_min: i64);

#[when(regex = r#"^the admin submits the bootstrap claim form via "([^"]+)" with$"#)]
async fn submit_bootstrap_form(world: &mut FoundryWorld, url: String, form: cucumber::gherkin::Step);
// reads form rows from the data-table

#[then(regex = r"^the response redirects the admin to the workspace dashboard$")]
async fn redirected_to_dashboard(world: &mut FoundryWorld);

#[then(regex = r#"^the response sets a session cookie named "([^"]+)"$"#)]
async fn session_cookie_set(world: &mut FoundryWorld, cookie_name: String);

#[then(regex = r"^that cookie is HttpOnly and SameSite=Lax and Secure$")]
async fn cookie_security_flags(world: &mut FoundryWorld);

#[then(regex = r#"^the workspace "([^"]+)" exists with (\w+) as its only admin$"#)]
async fn workspace_exists_with_admin(world: &mut FoundryWorld, ws_name: String, admin_display: String);

#[then(regex = r#"^a default team named "([^"]+)" exists in that workspace$"#)]
async fn default_team_exists(world: &mut FoundryWorld, team_name: String);

#[then(regex = r#"^a default project named "([^"]+)" exists in the (\w+) team$"#)]
async fn default_project_exists(world: &mut FoundryWorld, project_name: String, team_name: String);

// Replay / expired ---------------------------------------------------------

#[given(regex = r#"^the admin has already claimed the workspace using "([^"]+)"$"#)]
async fn admin_already_claimed(world: &mut FoundryWorld, token: String);

#[when(regex = r#"^a second visitor opens the bootstrap URL "([^"]+)"$"#)]
async fn second_visit_bootstrap(world: &mut FoundryWorld, url: String);

#[when(regex = r#"^a visitor opens the bootstrap URL "([^"]+)"$"#)]
async fn visit_bootstrap(world: &mut FoundryWorld, url: String);

#[then(regex = r"^the response status is (\d+) Gone$")]
async fn response_status_gone(world: &mut FoundryWorld, code: u16);

#[then(regex = r"^the page body explains the link has already been used$")]
async fn body_explains_used(world: &mut FoundryWorld);

#[then(regex = r"^the page body explains the link has expired$")]
async fn body_explains_expired(world: &mut FoundryWorld);

#[then(regex = r"^no second workspace is created$")]
async fn no_second_workspace(world: &mut FoundryWorld);

#[then(regex = r"^no workspace, user, or session is created$")]
async fn no_state_created(world: &mut FoundryWorld);

// Invite link --------------------------------------------------------------

#[given(regex = r#"^the admin has claimed "([^"]+)" and is signed in$"#)]
async fn admin_claimed_and_signed_in(world: &mut FoundryWorld, ws_name: String);

#[when(regex = r"^the admin opens the invite-teammates panel and requests a shareable link$")]
async fn request_invite_link(world: &mut FoundryWorld);

#[then(regex = r"^the response contains an invite URL$")]
async fn response_contains_invite_url(world: &mut FoundryWorld);

#[then(regex = r"^the invite URL carries a signed token parameter$")]
async fn invite_url_signed_token(world: &mut FoundryWorld);

#[then(regex = r"^the invite is recorded as valid for 7 days$")]
async fn invite_valid_7d(world: &mut FoundryWorld);

// Single-workspace constraint ----------------------------------------------

#[when(regex = r#"^the admin submits a workspace-create form with name "([^"]+)"$"#)]
async fn submit_workspace_create(world: &mut FoundryWorld, name: String);

#[then(regex = r"^the response status is 409 Conflict$")]
async fn status_409(world: &mut FoundryWorld);

#[then(regex = r"^the page body explains that only one workspace per instance is supported$")]
async fn body_explains_single_workspace(world: &mut FoundryWorld);
```

## US-06 — `steps/us_06_signin.rs`

```rust
#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_with_admin(world: &mut FoundryWorld, ws: String, admin_email: String);

#[given(regex = r#"^a member "([^"]+)" is registered with password "([^"]+)"$"#)]
async fn member_registered(world: &mut FoundryWorld, email: String, password: String);

// Happy path ---------------------------------------------------------------

#[given(regex = r"^(\w+) has no current session$")]
async fn user_no_session(world: &mut FoundryWorld, who: String);

#[when(regex = r#"^(\w+) submits the sign-in form via "([^"]+)" with email "([^"]+)" and password "([^"]+)"$"#)]
async fn submit_signin(world: &mut FoundryWorld, who: String, url: String, email: String, password: String);

#[then(regex = r#"^the response redirects (\w+) to "([^"]+)"$"#)]
async fn response_redirects(world: &mut FoundryWorld, who: String, to: String);

#[then(regex = r"^the session is recorded as valid for 30 days$")]
async fn session_30d(world: &mut FoundryWorld);

#[then(regex = r"^requesting a protected page with that cookie returns a successful response$")]
async fn protected_page_with_cookie(world: &mut FoundryWorld);

// Wrong creds --------------------------------------------------------------

#[when(regex = r#"^(\w+) submits the sign-in form with email "([^"]+)" and password "([^"]+)"$"#)]
async fn submit_signin_no_url(world: &mut FoundryWorld, who: String, email: String, password: String);

#[when(regex = r#"^a visitor submits the sign-in form with email "([^"]+)" and password "([^"]+)"$"#)]
async fn visitor_submit_signin(world: &mut FoundryWorld, email: String, password: String);

#[then(regex = r"^the response status is 401 or shows an inline error$")]
async fn status_401_or_inline(world: &mut FoundryWorld);

#[then(regex = r#"^the response body contains "([^"]+)"$"#)]
async fn body_contains(world: &mut FoundryWorld, needle: String);

#[then(regex = r"^no session cookie is set$")]
async fn no_session_cookie(world: &mut FoundryWorld);

#[then(regex = r"^the response time is within 50ms of the wrong-password response time$")]
async fn response_time_within_50ms(world: &mut FoundryWorld);

// Brute force --------------------------------------------------------------

#[given(regex = r"^(\w+) has failed sign-in (\d+) times in the last 15 minutes$")]
async fn failed_attempts(world: &mut FoundryWorld, who: String, n: u32);

#[when(regex = r"^(\w+) submits a sixth failed sign-in attempt$")]
async fn submit_sixth(world: &mut FoundryWorld, who: String);

#[then(regex = r"^the handler records a scheduled delay of at least (\d+) milliseconds before responding$")]
async fn delay_recorded(world: &mut FoundryWorld, ms: u64);

// Sign-out -----------------------------------------------------------------

#[given(regex = r"^(\w+) is signed in with an active session$")]
async fn user_signed_in_session(world: &mut FoundryWorld, who: String);

#[when(regex = r#"^(\w+) posts to "([^"]+)"$"#)]
async fn post_to(world: &mut FoundryWorld, who: String, url: String);

#[then(regex = r"^the server-side session row for (\w+)'s session id no longer exists$")]
async fn session_row_gone(world: &mut FoundryWorld, who: String);

#[then(regex = r"^presenting (\w+)'s prior cookie to a protected page returns an anonymous-redirect response$")]
async fn anonymous_redirect_old_cookie(world: &mut FoundryWorld, who: String);

// Password reset -----------------------------------------------------------

#[given(regex = r"^the SMTP transport is configured$")]
async fn smtp_configured(world: &mut FoundryWorld);

#[when(regex = r#"^a visitor submits the forgot-password form with email "([^"]+)"$"#)]
async fn submit_forgot_password(world: &mut FoundryWorld, email: String);

#[then(regex = r#"^exactly one email is recorded as sent to "([^"]+)"$"#)]
async fn one_email_sent(world: &mut FoundryWorld, email: String);

#[then(regex = r"^the recorded email body contains a reset link valid for 1 hour$")]
async fn email_body_reset_link_1h(world: &mut FoundryWorld);
```

## US-07 — `steps/us_07_project_create.rs`

```rust
#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(world: &mut FoundryWorld, email: String, team: String);

#[given(regex = r"^(\w+) is signed in$")]
async fn user_signed_in(world: &mut FoundryWorld, who: String);

#[when(regex = r#"^(\w+) creates a project under "([^"]+)" with name "([^"]+)" and key prefix "([^"]*)"$"#)]
async fn create_project(world: &mut FoundryWorld, who: String, team: String, name: String, key: String);

#[then(regex = r#"^the response redirects to "([^"]+)"$"#)]
async fn response_redirects_url(world: &mut FoundryWorld, url: String);

#[then(regex = r#"^the response body lists the columns "([^"]+)", "([^"]+)", "([^"]+)", "([^"]+)"$"#)]
async fn body_lists_columns(world: &mut FoundryWorld, c1: String, c2: String, c3: String, c4: String);

#[then(regex = r#"^the response body contains "([^"]+)"$"#)]
async fn body_contains_text_step(world: &mut FoundryWorld, needle: String);

#[then(regex = r#"^the project "([^"]+)" is recorded in the "([^"]+)" team with key prefix "([^"]+)"$"#)]
async fn project_recorded(world: &mut FoundryWorld, name: String, team: String, key: String);

// Duplicate-key / duplicate-name ------------------------------------------

#[given(regex = r#"^a project named "([^"]+)" with key prefix "([^"]+)" already exists in "([^"]+)"$"#)]
async fn project_already_exists(world: &mut FoundryWorld, name: String, key: String, team: String);

#[when(regex = r#"^(\w+) attempts to create a project under "([^"]+)" with name "([^"]+)" and key prefix "([^"]+)"$"#)]
async fn attempt_create_project(world: &mut FoundryWorld, who: String, team: String, name: String, key: String);

#[then(regex = r"^the response body explains the project key is already in use$")]
async fn body_explains_dup_key(world: &mut FoundryWorld);

#[then(regex = r"^the response shows an inline error explaining the name must be unique within the team$")]
async fn body_explains_dup_name(world: &mut FoundryWorld);

#[then(regex = r"^no second project is created$")]
async fn no_second_project(world: &mut FoundryWorld);

// Non-team-member ---------------------------------------------------------

#[given(regex = r#"^(\w+) is a workspace member but not a member of the "([^"]+)" team$"#)]
async fn non_team_member(world: &mut FoundryWorld, who: String, team: String);

#[then(regex = r"^the response status is 403 Forbidden$")]
async fn status_403(world: &mut FoundryWorld);

#[then(regex = r#"^no project named "([^"]+)" exists in any team$"#)]
async fn no_project_named_anywhere(world: &mut FoundryWorld, name: String);

// Property outline --------------------------------------------------------

#[when(regex = r#"^Mei attempts to create a project under "Backend" with name "Probe" and key prefix "([^"]*)"$"#)]
async fn attempt_create_with_key(world: &mut FoundryWorld, key: String);

#[then(regex = r#"^the project-create outcome is "(accepted|rejected)"$"#)]
async fn project_create_outcome(world: &mut FoundryWorld, outcome: String);
```

## US-08 — `steps/us_08_file_issue.rs`

```rust
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(world: &mut FoundryWorld, name: String, key: String, team: String);

#[when(regex = r#"^(\w+) files an issue against "([^"]+)" with title "([^"]*)"$"#)]
async fn file_issue(world: &mut FoundryWorld, who: String, project: String, title: String);

#[then(regex = r#"^the new issue is assigned the key "([^"]+)"$"#)]
async fn issue_assigned_key(world: &mut FoundryWorld, key: String);

#[then(regex = r#"^the issue's state is "([^"]+)"$"#)]
async fn issue_state(world: &mut FoundryWorld, state: String);

#[then(regex = r#"^the issue's priority is "([^"]+)"$"#)]
async fn issue_priority(world: &mut FoundryWorld, priority: String);

#[then(regex = r"^the issue's author is (\w+)$")]
async fn issue_author(world: &mut FoundryWorld, who: String);

#[then(regex = r"^the response contains a fragment showing (\w+-\d+) in the Backlog column$")]
async fn response_fragment_shows_issue(world: &mut FoundryWorld, key: String);

#[then(regex = r#"^opening "([^"]+)" lists (\w+-\d+) in the Backlog column$"#)]
async fn opening_page_lists(world: &mut FoundryWorld, url: String, key: String);

// Sequential keys --------------------------------------------------------

#[given(regex = r#"^the "([^"]+)" project already has issues (\w+-\d+) through (\w+-\d+)$"#)]
async fn project_has_issues_range(world: &mut FoundryWorld, project: String, first: String, last: String);

#[given(regex = r#"^the "([^"]+)" project already has issue (\w+-\d+)$"#)]
async fn project_has_issue(world: &mut FoundryWorld, project: String, key: String);

// Empty title ------------------------------------------------------------

#[then(regex = r"^the response status is 400 or 422$")]
async fn status_400_or_422(world: &mut FoundryWorld);

#[then(regex = r#"^the response is an htmx fragment containing "([^"]+)"$"#)]
async fn response_htmx_fragment_containing(world: &mut FoundryWorld, needle: String);

#[then(regex = r"^the response is not a full HTML page$")]
async fn response_not_full_page(world: &mut FoundryWorld);

#[then(regex = r#"^no issue is created in "([^"]+)"$"#)]
async fn no_issue_created_in(world: &mut FoundryWorld, project: String);

// Forbidden cross-team ---------------------------------------------------

// (uses non_team_member, status_403 and no_issue_created_in already declared)

// Performance ------------------------------------------------------------

#[when(regex = r#"^(\w+) files (\d+) issues against "([^"]+)" sequentially, each with a unique title$"#)]
async fn file_n_issues(world: &mut FoundryWorld, who: String, n: u32, project: String);

#[then(regex = r"^all (\d+) issues are persisted with sequential keys (\w+-\d+) through (\w+-\d+)$")]
async fn n_issues_sequential(world: &mut FoundryWorld, n: u32, first: String, last: String);

#[then(regex = r"^the P95 server-side response time across those (\d+) requests is at most (\d+) milliseconds$")]
async fn p95_at_most(world: &mut FoundryWorld, n: u32, ms: u64);

// Title length boundary --------------------------------------------------

#[when(regex = r#"^Mei files an issue against "Auth v2" with title of length (\d+)$"#)]
async fn file_issue_title_length(world: &mut FoundryWorld, length: u32);

#[then(regex = r#"^the file-issue outcome is "(accepted|rejected)"$"#)]
async fn file_issue_outcome(world: &mut FoundryWorld, outcome: String);
```

## Step-vocabulary smell checks (Pillar 1 / 2)

- Every step phrase uses the project glossary (workspace, team, project, issue, member, admin, sign-in, sign-out, bootstrap, invite, file an issue). No `POST`, `endpoint`, `cookie attribute`, `HMAC`, `bytea`, `sqlx`, `axum` appears in phrase text — those terms appear only in step bodies and in `support/` helpers.
- HTTP status codes (200/303/401/403/409/410/413/422/500) appear in `.feature` files ONLY where the status code IS the user-observable contract (the page-not-found / forbidden / gone pages users see). For internal status codes (200 OK from a happy POST followed by an htmx swap), the step says "the response redirects" or "the response contains a fragment" — Pillar 1 preserved.
- The `Given` of US-08 happy-path reuses `Given a member belongs to the team Backend` (US-07 step) plus `Given a project exists in the team` (US-07 step) — chained narrative, Pillar 2. The shared `member_belongs_to_team` lives in `us_07_project_create.rs` because that's where it's introduced; US-08 imports it through the cucumber-rs auto-discovery.
