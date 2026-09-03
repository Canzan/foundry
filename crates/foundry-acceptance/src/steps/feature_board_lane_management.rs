//! board-lane-management step definitions
//! (`tests/features/board-lane-management.feature`, 24 scenarios, ALL @pending
//! — scaffolded RED per ADR-025; DELIVER un-pends one at a time).
//!
//! The production seams these steps drive are the DESIGN port signatures
//! (component-boundaries.md): `foundry_store::lanes` (`list_project_lanes`,
//! `delete_lane_with_fate`), `foundry_services::lanes` (`delete_lane_dialog`,
//! `delete_lane`, `classify_lane_delete`),
//! `foundry_services::issues::validate_project_lane`,
//! `foundry_services::board::board_view`, and the mounted `foundry-app::lanes`
//! handlers (dialog GET + confirm POST). Migration 0015 is DELIVER-owned —
//! pre-0015 the `lanes` relation does not exist, so lane-seeding Givens fail
//! with an explicit MISSING_FUNCTIONALITY(schema) panic (the honest RED).
//!
//! THE LANE-LIST ORACLE RULE (D8): every board-render assertion reads the
//! expected lane list BACK FROM THE DATABASE (`lanes` rows: slug, label,
//! position) — this module deliberately has NO static expected-column list.
//! A test-local `["Backlog", "Todo", …]` would go green over the exact
//! static-list consumers slice 01 deletes (`DEFAULT_COLUMNS`,
//! `column_label_to_state`, the hardcoded edit-dialog options).
//!
//! LAYER 3 (real adapter + real HTTP, @real-io): real Postgres via the shared
//! testcontainer + per-scenario schema; the real tower-sessions store; the
//! real double-submit CSRF middleware; the in-process axum router; REAL
//! registered EdDSA bearers for the machine-client legs. Example-based
//! (Mandates 9 + 11) — no PBT machinery at this layer. State-mutation
//! assertions follow the state-delta discipline via [`BoardUniverse`]:
//! snapshot the full declared universe before the write — lane rows
//! `(slug, label, position)`, issue rows `(key, lane, position)`,
//! change-event count, outbox count — snapshot after, and assert the declared
//! delta fail-closed (on a move fate ONLY the moved cards' (lane, position)
//! may change; anything else moving is a violation).
//!
//! The two `@needs-browser` scenarios drive a REAL headless Chrome
//! (fantoccini, `support::browser_harness`) because the HTTP lane is
//! byte-blind to the htmx dialog swap into `#modal-root`, to WHICH fate
//! button's `name=value` the browser submits (Earned Trust: htmx submitter
//! inclusion), and to the out-of-band `#board-columns` refresh.

use crate::support::browser_harness;
use crate::support::harness::{
    establish_session, fresh_schema_pool_no_migrations, post_with_cookie, signed_in_get,
    signed_in_post, InProcHarness, PostOutcome,
};
use crate::support::test_migration;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;
use std::time::Duration;

const TEST_NOW: &str = "2026-02-01T12:00:00Z";
const PRIYA_EMAIL: &str = "priya@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const MARCO_EMAIL: &str = "marco@canzan.test";
const MARCO_PASSWORD: &str = "marco-correct-horse-battery-staple";

/// DESIGN-pinned scraper markers (component-boundaries.md §4). If DELIVER
/// moves these, the dialog partial and this module move in the same change.
const MODAL_MARKER: &str = "data-modal=\"delete-lane\"";
const COUNT_ATTR: &str = "data-lane-count=\"";
const ERROR_MARKER: &str = "delete-lane-error";
const LAST_LANE_MESSAGE: &str = "A board needs at least one lane";
const OOB_BOARD_MARKER: &str = "id=\"board-columns\"";

fn lane_delete_path(team: &str, project: &str, lane: &str) -> String {
    format!("/team/{team}/project/{project}/lanes/{lane}/delete")
}

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world.harness.as_ref().expect("harness spawned by a Given")
}

fn pool(world: &FoundryWorld) -> PgPool {
    harness(world).app.state.store.pool().clone()
}

fn http(world: &FoundryWorld) -> reqwest::Client {
    world.http.as_ref().expect("http client").clone()
}

fn record_outcome(world: &mut FoundryWorld, outcome: PostOutcome) {
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

fn current_project(world: &FoundryWorld) -> String {
    world
        .blm_current_project
        .clone()
        .expect("a board Given must have named the project under test")
}

/// STORED `(team_slug, project_slug)` — read back at seed/create time, never
/// re-derived from a name (the slug-capture rule).
fn stored_slugs(world: &FoundryWorld, project_name: &str) -> (String, String) {
    world
        .blm_project_slugs
        .get(project_name)
        .unwrap_or_else(|| panic!("stored slugs for {project_name:?} must have been captured"))
        .clone()
}

fn project_id_of(world: &FoundryWorld, project_name: &str) -> uuid::Uuid {
    *world
        .blm_project_ids
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded/created by a Given"))
}

fn board_path(world: &FoundryWorld, project_name: &str) -> String {
    let (team_slug, project_slug) = stored_slugs(world, project_name);
    format!("/team/{team_slug}/project/{project_slug}")
}

// ===========================================================================
// Seeding (real Postgres via the harness pool — preconditions, never the
// behaviour under test).
// ===========================================================================

async fn seed_user(world: &FoundryWorld, email: &str, display: &str, password: &str) -> uuid::Uuid {
    let pool = pool(world);
    let email_lower = email.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&SecretString::new(password.to_string().into()))
        .await
        .expect("hash password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(&email_lower)
    .bind(email)
    .bind(display)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("insert user");
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&email_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve user id");
    id
}

#[given(regex = r"^Priya is a member of team Backend in workspace Canzan Labs$")]
async fn priya_member_of_backend(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = pool(world);
    let priya = seed_user(world, PRIYA_EMAIL, "Priya Raman", PRIYA_PASSWORD).await;
    world.blm_priya_id = Some(priya);
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(ws)
        .bind("Canzan Labs")
        .execute(&pool)
        .await
        .expect("insert workspace");
    world.blm_workspace_id = Some(ws);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(priya)
    .execute(&pool)
    .await
    .expect("insert workspace membership");
    let team = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team)
        .bind(ws)
        .bind("Backend")
        .bind("backend")
        .execute(&pool)
        .await
        .expect("insert team");
    world.blm_team_id = Some(team);
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
              VALUES ($1, $2, 'lead') ON CONFLICT DO NOTHING",
    )
    .bind(team)
    .bind(priya)
    .execute(&pool)
    .await
    .expect("insert team membership");
}

#[given(regex = r"^Marco is signed in to Canzan Labs but is not a member of team Backend$")]
async fn marco_signed_in_not_backend(world: &mut FoundryWorld) {
    let marco = seed_user(world, MARCO_EMAIL, "Marco", MARCO_PASSWORD).await;
    let ws = world.blm_workspace_id.expect("workspace seeded first");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(marco)
    .execute(&pool(world))
    .await
    .expect("insert Marco's workspace membership");
    let (is_member,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM team_memberships tm
           JOIN teams t ON t.id = tm.team_id WHERE tm.user_id = $1 AND t.slug = 'backend')",
    )
    .bind(marco)
    .fetch_one(&pool(world))
    .await
    .expect("probe team membership");
    assert!(!is_member, "Marco must NOT be a member of team Backend");
    world.blm_marco_id = Some(marco);
}

async fn seed_project(world: &mut FoundryWorld, name: &str, slug: &str, prefix: &str) {
    let pool = pool(world);
    let ws = world.blm_workspace_id.expect("workspace seeded first");
    let team = world.blm_team_id.expect("team seeded first");
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team)
    .bind(ws)
    .bind(name)
    .bind(slug)
    .bind(prefix)
    .execute(&pool)
    .await
    .expect("insert project");
    world.blm_project_ids.insert(name.to_string(), id);
    let (stored_project_slug, stored_team_slug): (String, String) = sqlx::query_as(
        "SELECT p.slug, t.slug FROM projects p JOIN teams t ON p.team_id = t.id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("read back stored slugs");
    world
        .blm_project_slugs
        .insert(name.to_string(), (stored_team_slug, stored_project_slug));
}

/// Seed one lane row — the post-0015 shape the migration/creation leaves.
/// Pre-0015 the relation does not exist; the panic message classifies the
/// failure honestly (MISSING_FUNCTIONALITY: absent production schema — the
/// migration is DELIVER-owned, ADR-025), not as a test bug.
async fn seed_lane(
    world: &FoundryWorld,
    project_id: uuid::Uuid,
    slug: &str,
    label: &str,
    position: i32,
) {
    let ws = world.blm_workspace_id.expect("workspace seeded first");
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(ws)
    .bind(slug)
    .bind(label)
    .bind(position)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| {
        panic!(
            "MISSING_FUNCTIONALITY: seeding lane ({slug:?}, {label:?}, {position}) requires the \
             `lanes` table from migration 0015, which is DELIVER-owned (ADR-025 / \
             architecture-design.md §4). This Given seeds the post-migration lane rows the \
             grandfather backfill produces. Underlying error: {err}"
        )
    });
}

const GRANDFATHER_LANES: &[(&str, &str)] = &[
    ("backlog", "Backlog"),
    ("todo", "Todo"),
    ("in_progress", "In-Progress"),
    ("done", "Done"),
];

async fn seed_grandfathered(
    world: &mut FoundryWorld,
    name: &str,
    slug: &str,
    prefix: &str,
    with_cancelled: bool,
) {
    seed_project(world, name, slug, prefix).await;
    let project_id = project_id_of(world, name);
    for (idx, (lane_slug, label)) in GRANDFATHER_LANES.iter().enumerate() {
        seed_lane(world, project_id, lane_slug, label, idx as i32).await;
    }
    if with_cancelled {
        seed_lane(world, project_id, "cancelled", "Cancelled", 4).await;
    }
    world.blm_current_project = Some(name.to_string());
}

async fn seed_issue(
    world: &FoundryWorld,
    project_name: &str,
    number: i32,
    title: &str,
    state: &str,
    position: i32,
) {
    let pool = pool(world);
    let project_id = project_id_of(world, project_name);
    let ws = world.blm_workspace_id.expect("workspace seeded first");
    let author = world.blm_priya_id.expect("Priya seeded first");
    sqlx::query(
        "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id, position)
              VALUES ($1, $2, $3, $4, $5, $6, 'medium', $7, $8)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(ws)
    .bind(project_id)
    .bind(number)
    .bind(title)
    .bind(state)
    .bind(author)
    .bind(position)
    .execute(&pool)
    .await
    .unwrap_or_else(|err| panic!("seed issue #{number} in {state:?}: {err}"));
    sqlx::query(
        "UPDATE projects SET next_issue_number = GREATEST(next_issue_number, $1) WHERE id = $2",
    )
    .bind(number + 1)
    .bind(project_id)
    .execute(&pool)
    .await
    .expect("advance next_issue_number");
}

// ---- grandfathered-board Givens -------------------------------------------

#[given(regex = r#"^"([^"]+)" \(([A-Z]+)\) is a grandfathered board with its four working lanes$"#)]
async fn grandfathered_four_lanes(world: &mut FoundryWorld, name: String, prefix: String) {
    let slug = name.to_lowercase().replace(' ', "-");
    let _ = prefix;
    seed_grandfathered(world, &name, &slug, &prefix_of(&name), false).await;
}

#[given(
    regex = r#"^"([^"]+)" \(([A-Z]+)\) is a grandfathered board granted a Cancelled lane, holding ([A-Z]+)-(\d+) "([^"]+)" in Cancelled$"#
)]
async fn grandfathered_with_cancelled(
    world: &mut FoundryWorld,
    name: String,
    _prefix: String,
    _key_prefix: String,
    number: i32,
    title: String,
) {
    let slug = name.to_lowercase().replace(' ', "-");
    seed_grandfathered(world, &name, &slug, &prefix_of(&name), true).await;
    seed_issue(world, &name, number, &title, "cancelled", 0).await;
}

#[given(
    regex = r#"^"([^"]+)" \(([A-Z]+)\) is a grandfathered board whose Todo lane holds no issues$"#
)]
async fn grandfathered_empty_todo(world: &mut FoundryWorld, name: String, _prefix: String) {
    let slug = name.to_lowercase().replace(' ', "-");
    seed_grandfathered(world, &name, &slug, &prefix_of(&name), false).await;
    // One card OUTSIDE Todo so the edit-dialog / API refusal / untouched
    // assertions have a real issue to bite on (Todo itself stays empty).
    seed_issue(world, &name, 1, "Patch the NAS firmware", "backlog", 0).await;
}

/// Key prefixes are pinned by the DISCUSS domain examples; keeping the map
/// here (name → prefix) avoids parsing them out of every Given line.
fn prefix_of(project_name: &str) -> String {
    match project_name {
        "Identity Platform" => "AUTH",
        "Homelab Ops" => "OPS",
        "Reading List" => "READ",
        "Scratch" => "SCR",
        other => panic!("no key prefix pinned for project {other:?}"),
    }
    .to_string()
}

#[given(
    regex = r"^AUTH-7 sits in Backlog, AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom, AUTH-3 sits in In-Progress and AUTH-1 sits in Done$"
)]
async fn auth_full_spread(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 7, "Refresh token rotation", "backlog", 0).await;
    seed_issue(world, &p, 12, "Rotate signing keys", "todo", 0).await;
    seed_issue(world, &p, 15, "Session pinning audit", "todo", 1).await;
    seed_issue(world, &p, 18, "Passkey enrolment spike", "todo", 2).await;
    seed_issue(world, &p, 3, "OIDC discovery cache", "in_progress", 0).await;
    seed_issue(world, &p, 1, "Argon2 parameter bump", "done", 0).await;
}

#[given(regex = r"^AUTH-7 sits in Backlog$")]
async fn auth7_in_backlog(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 7, "Refresh token rotation", "backlog", 0).await;
}

#[given(regex = r"^AUTH-12 sits in Todo and AUTH-3 sits in In-Progress$")]
async fn auth12_and_auth3(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 12, "Rotate signing keys", "todo", 0).await;
    seed_issue(world, &p, 3, "OIDC discovery cache", "in_progress", 0).await;
}

#[given(
    regex = r"^AUTH-7 sits in Backlog and AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom$"
)]
async fn auth7_and_todo_three(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 7, "Refresh token rotation", "backlog", 0).await;
    seed_issue(world, &p, 12, "Rotate signing keys", "todo", 0).await;
    seed_issue(world, &p, 15, "Session pinning audit", "todo", 1).await;
    seed_issue(world, &p, 18, "Passkey enrolment spike", "todo", 2).await;
}

#[given(regex = r"^AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom$")]
async fn todo_three(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 12, "Rotate signing keys", "todo", 0).await;
    seed_issue(world, &p, 15, "Session pinning audit", "todo", 1).await;
    seed_issue(world, &p, 18, "Passkey enrolment spike", "todo", 2).await;
}

#[given(regex = r#"^project "Scratch" \(SCR\) has exactly one lane, Done$"#)]
async fn scratch_one_lane(world: &mut FoundryWorld) {
    seed_project(world, "Scratch", "scratch", "SCR").await;
    let id = project_id_of(world, "Scratch");
    seed_lane(world, id, "done", "Done", 0).await;
    world.blm_current_project = Some("Scratch".to_string());
}

#[given(
    regex = r#"^project "Scratch" \(SCR\) has lanes Backlog and Done, with SCR-2 and SCR-5 in Done$"#
)]
async fn scratch_two_lanes_with_cards(world: &mut FoundryWorld) {
    seed_project(world, "Scratch", "scratch", "SCR").await;
    let id = project_id_of(world, "Scratch");
    seed_lane(world, id, "backlog", "Backlog", 0).await;
    seed_lane(world, id, "done", "Done", 1).await;
    seed_issue(world, "Scratch", 2, "Wasm board spike", "done", 0).await;
    seed_issue(world, "Scratch", 5, "GraphQL detour spike", "done", 1).await;
    world.blm_current_project = Some("Scratch".to_string());
}

#[given(regex = r"^SCR-2 carries a comment and an attachment$")]
async fn scr2_comment_and_attachment(world: &mut FoundryWorld) {
    let pool = pool(world);
    let project_id = project_id_of(world, "Scratch");
    let ws = world.blm_workspace_id.expect("workspace seeded");
    let author = world.blm_priya_id.expect("Priya seeded");
    let (issue_id,): (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM issues WHERE project_id = $1 AND number = 2")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .expect("SCR-2 seeded");
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, 'worthless, keeping notes', '<p>worthless, keeping notes</p>')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(ws)
    .bind(issue_id)
    .bind(author)
    .execute(&pool)
    .await
    .expect("seed comment on SCR-2");
    let bytes: &[u8] = b"spike scratchpad";
    let sha = format!("{:0>64}", "ab12");
    sqlx::query(
        "INSERT INTO issue_attachments
              (id, issue_id, workspace_id, uploader_id, filename, content_type, size_bytes, sha256_hex, content)
              VALUES ($1, $2, $3, $4, 'notes.txt', 'text/plain', $5, $6, $7)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(issue_id)
    .bind(ws)
    .bind(author)
    .bind(bytes.len() as i64)
    .bind(sha)
    .bind(bytes)
    .execute(&pool)
    .await
    .expect("seed attachment on SCR-2");
}

// ---- Reading List (created through the REAL driving port) ------------------

async fn create_reading_list(world: &mut FoundryWorld) {
    if world.blm_project_ids.contains_key("Reading List") {
        world.blm_current_project = Some("Reading List".to_string());
        return;
    }
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        "/team/backend/projects",
        &[("name", "Reading List"), ("key_prefix", "READ")],
    )
    .await;
    assert!(
        outcome.status.is_redirection() || outcome.status.is_success(),
        "creating \"Reading List\" through the real create-project port must succeed; \
         got {} with body {:?}",
        outcome.status,
        outcome.body
    );
    let row: Option<(uuid::Uuid, String, String)> = sqlx::query_as(
        "SELECT p.id, p.slug, t.slug FROM projects p JOIN teams t ON p.team_id = t.id
          WHERE p.name = 'Reading List'",
    )
    .fetch_optional(&pool(world))
    .await
    .expect("query Reading List row");
    let (id, project_slug, team_slug) = row.expect("the created project must be persisted");
    world.blm_project_ids.insert("Reading List".to_string(), id);
    world
        .blm_project_slugs
        .insert("Reading List".to_string(), (team_slug, project_slug));
    world.blm_current_project = Some("Reading List".to_string());
}

#[given(regex = r#"^Priya creates project "Reading List" in team Backend$"#)]
async fn priya_creates_reading_list(world: &mut FoundryWorld) {
    create_reading_list(world).await;
}

#[given(regex = r#"^the fresh "Reading List" board$"#)]
async fn fresh_reading_list(world: &mut FoundryWorld) {
    create_reading_list(world).await;
}

// ===========================================================================
// The board-universe snapshot — the declared state-delta universe.
// Entries are port-exposed observables (stored lane rows and card placements
// as the board renders them; change-event and outbox counts as the report
// and listeners observe them) read via read-only SELECTs, the sanctioned
// db_introspect idiom — never internal struct fields.
// ===========================================================================

async fn lanes_of(pool: &PgPool, project_id: uuid::Uuid) -> Vec<(String, String, i32)> {
    sqlx::query_as(
        "SELECT slug, label, position FROM lanes WHERE project_id = $1 ORDER BY position ASC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .unwrap_or_else(|err| {
        panic!(
            "MISSING_FUNCTIONALITY: reading the project's lane rows requires migration 0015 \
             (`lanes` table), which is DELIVER-owned (ADR-025). Underlying error: {err}"
        )
    })
}

async fn issues_of(world: &FoundryWorld, project_name: &str) -> Vec<(String, String, i32)> {
    let project_id = project_id_of(world, project_name);
    let prefix = prefix_of(project_name);
    let rows: Vec<(i32, String, i32)> = sqlx::query_as(
        "SELECT number, state, position FROM issues WHERE project_id = $1 ORDER BY number ASC",
    )
    .bind(project_id)
    .fetch_all(&pool(world))
    .await
    .expect("read issue rows");
    rows.into_iter()
        .map(|(number, state, position)| (format!("{prefix}-{number}"), state, position))
        .collect()
}

async fn count_of(pool: &PgPool, table: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("count {table}: {err}"));
    n
}

/// Capture the full declared universe for the current project.
async fn capture_universe(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let pool = pool(world);
    world.blm_lanes_before = Some(lanes_of(&pool, project_id).await);
    world.blm_issues_before = Some(issues_of(world, &p).await);
    world.blm_events_before = Some(count_of(&pool, "issue_change_events").await);
    world.blm_outbox_before = Some(count_of(&pool, "outbox").await);
}

/// Fail-closed all-unchanged assertion over the declared universe.
async fn assert_universe_unchanged(world: &FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let pool = pool(world);
    let lanes_after = lanes_of(&pool, project_id).await;
    assert_eq!(
        world.blm_lanes_before.as_ref().expect("lanes snapshotted"),
        &lanes_after,
        "the project's lane rows must be byte-identical (declared unchanged)"
    );
    let issues_after = issues_of(world, &p).await;
    assert_eq!(
        world
            .blm_issues_before
            .as_ref()
            .expect("issues snapshotted"),
        &issues_after,
        "every card's (lane, position) must be byte-identical (declared unchanged)"
    );
    assert_eq!(
        world.blm_events_before.expect("events snapshotted"),
        count_of(&pool, "issue_change_events").await,
        "no change event may be written (declared unchanged)"
    );
    assert_eq!(
        world.blm_outbox_before.expect("outbox snapshotted"),
        count_of(&pool, "outbox").await,
        "no outbox row may be written (declared unchanged)"
    );
}

/// The zero-laneless guard query (architecture-design.md §4 / KPI 2): no
/// issue may reference a lane its project does not have. Post-0015 the FK
/// makes a nonzero count unreachable; this asserts the observable either way.
async fn assert_zero_laneless(world: &FoundryWorld) {
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues i
          WHERE NOT EXISTS (SELECT 1 FROM lanes l
                             WHERE l.project_id = i.project_id AND l.slug = i.state)",
    )
    .fetch_one(&pool(world))
    .await
    .unwrap_or_else(|err| {
        panic!(
            "MISSING_FUNCTIONALITY: the zero-laneless guard query requires migration 0015 \
             (`lanes` table), which is DELIVER-owned (ADR-025). Underlying error: {err}"
        )
    });
    assert_eq!(
        n, 0,
        "no issue may be in a state with no lane (KPI 2 guard)"
    );
}

// ===========================================================================
// HTML parsing oracles (board columns, cards, select options).
// ===========================================================================

/// The rendered column slugs, in document order (`data-column="…"`).
fn column_order(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(pos) = rest.find("data-column=\"") {
        let after = &rest[pos + "data-column=\"".len()..];
        let end = after.find('"').expect("closing quote for data-column");
        out.push(after[..end].to_string());
        rest = &after[end..];
    }
    out
}

/// Byte range of one column's section: from its `data-column` marker to the
/// next column marker (or the end). Card order inside is document order.
fn column_slice<'a>(body: &'a str, slug: &str) -> &'a str {
    let marker = format!("data-column=\"{slug}\"");
    let start = body
        .find(&marker)
        .unwrap_or_else(|| panic!("the board must render a column [data-column={slug:?}]"));
    let after = &body[start + marker.len()..];
    let end = after.find("data-column=\"").unwrap_or(after.len());
    &after[..end]
}

fn cards_in_column(body: &str, slug: &str) -> Vec<String> {
    let section = column_slice(body, slug);
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(pos) = rest.find("data-issue-key=\"") {
        let after = &rest[pos + "data-issue-key=\"".len()..];
        let end = after.find('"').expect("closing quote for data-issue-key");
        out.push(after[..end].to_string());
        rest = &after[end..];
    }
    out
}

/// `(value, label, selected)` triples of a named `<select>`, document order.
fn select_options(body: &str, name: &str) -> Vec<(String, String, bool)> {
    let marker = format!("name=\"{name}\"");
    let start = body
        .find(&marker)
        .unwrap_or_else(|| panic!("the page must carry a <select name={name:?}>; got {body:?}"));
    let section = &body[start..];
    let end = section.find("</select>").unwrap_or(section.len());
    let section = &section[..end];
    let mut out = Vec::new();
    let mut rest = section;
    while let Some(pos) = rest.find("<option") {
        let after = &rest[pos..];
        let tag_end = after.find('>').expect("option tag close");
        let tag = &after[..tag_end];
        let value = tag
            .find("value=\"")
            .map(|v| {
                let a = &tag[v + "value=\"".len()..];
                a[..a.find('"').expect("value close")].to_string()
            })
            .unwrap_or_default();
        let selected = tag.contains("selected");
        let body_after = &after[tag_end + 1..];
        let label_end = body_after.find('<').unwrap_or(body_after.len());
        let label = body_after[..label_end].trim().to_string();
        out.push((value, label, selected));
        rest = &after[tag_end..];
    }
    out
}

// ===========================================================================
// Machine client (REAL registered EdDSA bearer — the Feature-A idiom).
// ===========================================================================

async fn machine_bearer(world: &FoundryWorld) -> String {
    let user_id = world.blm_priya_id.expect("Priya seeded");
    let workspace_id = world.blm_workspace_id.expect("workspace seeded");
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(3600);
    harness(world)
        .app
        .state
        .store
        .insert_machine_token(
            jti,
            user_id,
            workspace_id,
            None,
            exp,
            "blm automation",
            user_id,
        )
        .await
        .expect("register machine token");
    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: None,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint machine jwt")
        .expose_secret()
        .to_string()
}

async fn api_patch_state(
    world: &FoundryWorld,
    project_name: &str,
    number: i32,
    state: &str,
) -> (StatusCode, String) {
    let (team_slug, project_slug) = stored_slugs(world, project_name);
    let bearer = machine_bearer(world).await;
    let base = harness(world).base_url();
    let resp = http(world)
        .patch(format!(
            "{base}/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}"
        ))
        .bearer_auth(bearer)
        .json(&serde_json::json!({ "state": state }))
        .send()
        .await
        .expect("send machine PATCH");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

async fn api_create_issue(
    world: &FoundryWorld,
    project_name: &str,
    title: &str,
) -> (StatusCode, String) {
    let (team_slug, project_slug) = stored_slugs(world, project_name);
    let bearer = machine_bearer(world).await;
    let base = harness(world).base_url();
    let resp = http(world)
        .post(format!(
            "{base}/api/v1/teams/{team_slug}/projects/{project_slug}/issues"
        ))
        .bearer_auth(bearer)
        .json(&serde_json::json!({ "title": title }))
        .send()
        .await
        .expect("send machine create");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

fn parse_created(body: &str) -> (String, String) {
    let json: serde_json::Value = serde_json::from_str(body)
        .unwrap_or_else(|err| panic!("create reply must be JSON: {err}; body = {body:?}"));
    let key = json["key"].as_str().unwrap_or_default().to_string();
    let state = json["state"].as_str().unwrap_or_default().to_string();
    (key, state)
}

// ===========================================================================
// When — board reads
// ===========================================================================

#[when(regex = r#"^(?:Priya|she) opens the "([^"]+)" board$"#)]
async fn opens_board(world: &mut FoundryWorld, project_name: String) {
    world.blm_current_project = Some(project_name.clone());
    let path = board_path(world, &project_name);
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    record_outcome(world, outcome);
}

#[given(regex = r#"^Priya files READ-1 "Dune"$"#)]
#[when(regex = r#"^Priya files READ-1 "Dune"$"#)]
async fn priya_files_dune(world: &mut FoundryWorld) {
    let path = format!("{}/issues", board_path(world, "Reading List"));
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
        &[("title", "Dune")],
    )
    .await;
    record_outcome(world, outcome);
}

#[when(regex = r#"^(?:a machine client|her automation) files "([^"]+)" into "([^"]+)"$"#)]
async fn machine_files_issue(world: &mut FoundryWorld, title: String, project_name: String) {
    world.blm_current_project = Some(project_name.clone());
    let (status, body) = api_create_issue(world, &project_name, &title).await;
    assert!(
        status.is_success(),
        "the machine filing must succeed; got {status} with body {body:?}"
    );
    world.blm_machine_created = Some(parse_created(&body));
    world.last_status = Some(status);
    world.last_body = Some(body);
}

#[when(regex = r#"^a machine client moves ([A-Z]+)-(\d+) to "([^"]+)"$"#)]
async fn machine_moves_issue(
    world: &mut FoundryWorld,
    _prefix: String,
    number: i32,
    state: String,
) {
    capture_universe(world).await;
    let p = current_project(world);
    let (status, body) = api_patch_state(world, &p, number, &state).await;
    world.last_status = Some(status);
    world.last_body = Some(body);
}

#[when(regex = r#"^a machine client moves READ-1 to "in_progress" and then to "todo"$"#)]
async fn machine_two_moves(world: &mut FoundryWorld) {
    let first = api_patch_state(world, "Reading List", 1, "in_progress").await;
    world.blm_first_move = Some(first);
    let second = api_patch_state(world, "Reading List", 1, "todo").await;
    world.last_status = Some(second.0);
    world.last_body = Some(second.1);
}

#[when(regex = r"^Priya drags AUTH-12 to the top of In-Progress$")]
async fn drags_auth12(world: &mut FoundryWorld) {
    // The dnd drop handler POSTs the target column's slug; `after` absent ⇒
    // drop at the TOP of the column (ChangeStateForm contract).
    let p = current_project(world);
    let path = format!("{}/issues/12/state", board_path(world, &p));
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
        &[("state", "in_progress")],
    )
    .await;
    record_outcome(world, outcome);
}

#[when(regex = r#"^Priya opens the edit dialog for ([A-Z]+)-(\d+)$"#)]
async fn opens_edit_dialog(world: &mut FoundryWorld, prefix: String, number: i32) {
    let p = current_project(world);
    assert_eq!(prefix, prefix_of(&p), "edit-dialog key prefix mismatch");
    let path = format!("{}/issues/{number}/edit", board_path(world, &p));
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    record_outcome(world, outcome);
}

// ===========================================================================
// When — the delete dialog and the two-fate confirm
// ===========================================================================

async fn fetch_delete_dialog(world: &mut FoundryWorld, project_name: &str, lane_slug: &str) {
    world.blm_current_project = Some(project_name.to_string());
    capture_universe(world).await;
    let (team_slug, project_slug) = stored_slugs(world, project_name);
    let path = lane_delete_path(&team_slug, &project_slug, lane_slug);
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    record_outcome(world, outcome);
    world.blm_dialog = Some((project_name.to_string(), lane_slug.to_string()));
}

async fn post_delete_confirm(
    world: &mut FoundryWorld,
    project_name: &str,
    lane_slug: &str,
    fields: &[(&str, &str)],
) {
    let (team_slug, project_slug) = stored_slugs(world, project_name);
    let path = lane_delete_path(&team_slug, &project_slug, lane_slug);
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
        fields,
    )
    .await;
    record_outcome(world, outcome);
}

#[when(regex = r"^Priya opens the delete dialog for the Todo lane$")]
async fn opens_delete_dialog_todo(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "todo").await;
}

#[given(regex = r"^Priya has the delete dialog for the Todo lane in front of her$")]
async fn dialog_in_front(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "todo").await;
}

#[given(
    regex = r"^Priya has the delete dialog for the Todo lane in front of her, reading 3 issues$"
)]
async fn dialog_in_front_reading_three(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "todo").await;
    let body = world.last_body.as_deref().expect("dialog captured");
    assert!(
        body.contains(&format!("{COUNT_ATTR}3\"")),
        "the dialog must state the live count of 3 issues; got {body:?}"
    );
}

#[when(regex = r"^Priya asks to delete the Todo lane and confirms in the dialog$")]
async fn delete_todo_and_confirm(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "todo").await;
    let dialog = world.last_body.as_deref().expect("dialog captured");
    assert!(
        world.last_status == Some(StatusCode::OK) && dialog.contains(MODAL_MARKER),
        "the delete trigger must serve the confirm dialog ({MODAL_MARKER}); \
         got {:?} with body {dialog:?}",
        world.last_status
    );
    post_delete_confirm(world, &p, "todo", &[("fate", "delete")]).await;
}

#[when(regex = r"^Priya asks to delete Done and confirms$")]
async fn delete_done_and_confirm(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "done").await;
    post_delete_confirm(world, &p, "done", &[("fate", "delete")]).await;
}

#[given(regex = r"^Priya deleted the empty Backlog lane, leaving In-Progress and Done$")]
async fn deleted_backlog_lane(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "backlog").await;
    post_delete_confirm(world, &p, "backlog", &[("fate", "delete")]).await;
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "deleting the empty Backlog lane must succeed (chained Given); body = {:?}",
        world.last_body
    );
}

#[when(regex = r"^Priya deletes the Todo lane choosing to move all 3 to Backlog$")]
async fn delete_todo_move_to_backlog(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "todo").await;
    // The append-at-bottom ordering oracle: capture the destination's card
    // order BEFORE the confirm.
    let board = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &board_path(world, &p),
    )
    .await;
    world.blm_dest_order_before = Some(cards_in_column(&board.body, "backlog"));
    post_delete_confirm(
        world,
        &p,
        "todo",
        &[("fate", "move"), ("destination", "backlog")],
    )
    .await;
}

#[when(
    regex = r"^Priya deletes the Done lane, reading that it holds 2 issues and cannot be undone, choosing to delete all 2 permanently$"
)]
async fn delete_done_permanently(world: &mut FoundryWorld) {
    let p = current_project(world);
    fetch_delete_dialog(world, &p, "done").await;
    let dialog = world.last_body.as_deref().expect("dialog captured");
    // The "reading" is load-bearing: the counted, permanence-stating copy must
    // be in front of her BEFORE the choice (D7).
    assert!(
        dialog.contains(&format!("{COUNT_ATTR}2\"")),
        "the dialog must state the live count of 2 issues; got {dialog:?}"
    );
    assert!(
        dialog.to_lowercase().contains("cannot be undone"),
        "the dialog must state permanence before the choice; got {dialog:?}"
    );
    post_delete_confirm(world, &p, "done", &[("fate", "delete")]).await;
}

#[when(regex = r"^she walks away without confirming$")]
async fn walks_away(world: &mut FoundryWorld) {
    // Cancel sends no request: the dialog GET already happened in the Given;
    // nothing further reaches the server.
    let _ = world;
}

#[when(
    regex = r"^her automation lands one more issue in Todo before she confirms moving all to Backlog$"
)]
async fn race_filing_then_confirm(world: &mut FoundryWorld) {
    let p = current_project(world);
    // The automation files through the real machine port: the create lands in
    // the leftmost lane, then the state move places it in the dying Todo —
    // both committed BEFORE the confirm POST (the deterministic in-lane
    // interleaving; the mid-transaction window is pinned by the FK + guard
    // query, see the feature-file header).
    let (status, body) = api_create_issue(world, &p, "Rushed automation filing").await;
    assert!(
        status.is_success(),
        "automation filing must succeed; got {status}: {body:?}"
    );
    let (key, _state) = parse_created(&body);
    let number: i32 = key
        .rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("created key {key:?} must end in a number"));
    let (status, body) = api_patch_state(world, &p, number, "todo").await;
    assert!(
        status.is_success(),
        "automation move into Todo must succeed; got {status}: {body:?}"
    );
    world.blm_machine_created = Some((key, "todo".to_string()));
    // Destination order oracle before the confirm.
    let board = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &board_path(world, &p),
    )
    .await;
    world.blm_dest_order_before = Some(cards_in_column(&board.body, "backlog"));
    post_delete_confirm(
        world,
        &p,
        "todo",
        &[("fate", "move"), ("destination", "backlog")],
    )
    .await;
}

#[when(regex = r#"^Marco sends the lane-delete confirm for Todo on "([^"]+)" directly$"#)]
async fn marco_sends_delete(world: &mut FoundryWorld, project_name: String) {
    world.blm_current_project = Some(project_name.clone());
    capture_universe(world).await;
    let (team_slug, project_slug) = stored_slugs(world, &project_name);
    let path = lane_delete_path(&team_slug, &project_slug, "todo");
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        MARCO_EMAIL,
        MARCO_PASSWORD,
        &path,
        &[("fate", "delete")],
    )
    .await;
    record_outcome(world, outcome);
}

#[when(regex = r"^a lane-delete confirm for Todo is submitted without the board's matching token$")]
async fn delete_without_csrf(world: &mut FoundryWorld) {
    let p = current_project(world);
    capture_universe(world).await;
    let (team_slug, project_slug) = stored_slugs(world, &p);
    let http = http(world);
    let session_pair = establish_session(harness(world), &http, PRIYA_EMAIL, PRIYA_PASSWORD).await;
    let outcome = post_with_cookie(
        harness(world),
        &http,
        &lane_delete_path(&team_slug, &project_slug, "todo"),
        &session_pair, // session only — deliberately NO foundry_csrf pair
        &[("fate", "delete")],
    )
    .await;
    record_outcome(world, outcome);
}

// ===========================================================================
// Then — board render oracles (lane rows from the DATABASE, never a const)
// ===========================================================================

fn rendered_board(world: &FoundryWorld) -> &str {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the board GET must render a 200 page; body = {:?}",
        world.last_body
    );
    world.last_body.as_deref().expect("board captured")
}

#[then(regex = r"^the columns are exactly the board's own lanes, in the board's own order$")]
async fn columns_match_lane_rows(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let body = rendered_board(world);
    let rendered = column_order(body);
    let expected: Vec<String> = lanes.iter().map(|(slug, _, _)| slug.clone()).collect();
    assert_eq!(
        rendered, expected,
        "the rendered columns must be exactly the project's lane rows in position order"
    );
    for (_, label, _) in &lanes {
        assert!(
            body.contains(label),
            "the board must render the lane header {label:?} from the lane row"
        );
    }
}

#[then(regex = r"^every card sits in the same column at the same position as before the upgrade$")]
async fn cards_unmoved(world: &mut FoundryWorld) {
    let p = current_project(world);
    let stored = issues_of(world, &p).await;
    let body = rendered_board(world);
    for (key, state, _) in &stored {
        let column = cards_in_column(body, state);
        assert!(
            column.contains(key),
            "{key:?} must render inside its stored lane {state:?}; column = {column:?}"
        );
    }
    // Per-column rendered order must equal the stored position order.
    let project_id = project_id_of(world, &p);
    for (slug, _, _) in lanes_of(&pool(world), project_id).await {
        let mut expected: Vec<(i32, String)> = stored
            .iter()
            .filter(|(_, state, _)| state == &slug)
            .map(|(key, _, position)| (*position, key.clone()))
            .collect();
        expected.sort();
        let expected: Vec<String> = expected.into_iter().map(|(_, k)| k).collect();
        assert_eq!(
            cards_in_column(body, &slug),
            expected,
            "the {slug:?} column must render its cards in stored position order"
        );
    }
}

#[then(regex = r"^no Cancelled column appears$")]
async fn no_cancelled_column(world: &mut FoundryWorld) {
    let body = rendered_board(world);
    assert!(
        !body.contains("data-column=\"cancelled\""),
        "a board without cancelled issues must not grow a Cancelled column (D5)"
    );
}

#[then(regex = r"^a Cancelled column renders after Done, holding OPS-9$")]
async fn cancelled_after_done(world: &mut FoundryWorld) {
    let body = rendered_board(world);
    let order = column_order(body);
    let done = order
        .iter()
        .position(|s| s == "done")
        .expect("Done column rendered");
    let cancelled = order
        .iter()
        .position(|s| s == "cancelled")
        .unwrap_or_else(|| panic!("a Cancelled column must render; columns = {order:?}"));
    assert!(
        cancelled > done,
        "Cancelled must render after Done; columns = {order:?}"
    );
    let cards = cards_in_column(body, "cancelled");
    assert!(
        cards.contains(&"OPS-9".to_string()),
        "the previously-invisible OPS-9 must hold a card in Cancelled; cards = {cards:?}"
    );
}

#[then(regex = r"^the columns are exactly Backlog, In-Progress and Done, in that order$")]
async fn columns_are_three_defaults(world: &mut FoundryWorld) {
    // Oracle discipline: assert BOTH that the render matches the stored lane
    // rows AND that those rows are the three defaults D4 pins.
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let seeded: Vec<(String, String)> = lanes
        .iter()
        .map(|(slug, label, _)| (slug.clone(), label.clone()))
        .collect();
    assert_eq!(
        seeded,
        vec![
            ("backlog".to_string(), "Backlog".to_string()),
            ("in_progress".to_string(), "In-Progress".to_string()),
            ("done".to_string(), "Done".to_string()),
        ],
        "project creation must seed exactly Backlog, In-Progress, Done in that order (D4)"
    );
    let body = rendered_board(world);
    assert_eq!(
        column_order(body),
        vec!["backlog", "in_progress", "done"],
        "the fresh board must render exactly the three seeded lanes"
    );
}

/// The project's LEFTMOST lane, read from the lane ROWS (`position ASC LIMIT
/// 1`) — the D6 landing rule's own oracle. Asserting the literal "backlog"
/// alone would go green TODAY over the legacy hardcoded `DEFAULT 'backlog'`
/// (the RED-classification run caught exactly that false GREEN): the
/// behaviour under test is "leftmost LANE ROW", so the expectation must
/// derive from lane data. Pre-0015 this reds structurally.
async fn leftmost_lane_slug(world: &FoundryWorld, project_name: &str) -> String {
    let project_id = project_id_of(world, project_name);
    let lanes = lanes_of(&pool(world), project_id).await;
    lanes
        .first()
        .map(|(slug, _, _)| slug.clone())
        .unwrap_or_else(|| panic!("{project_name:?} must have at least one lane row (D6)"))
}

#[then(regex = r"^READ-1 appears as a card in Backlog$")]
async fn read1_in_backlog(world: &mut FoundryWorld) {
    let leftmost = leftmost_lane_slug(world, "Reading List").await;
    assert_eq!(
        leftmost, "backlog",
        "the fresh project's leftmost LANE ROW must be Backlog (D4 seeding)"
    );
    let path = board_path(world, "Reading List");
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    assert_eq!(outcome.status, StatusCode::OK, "board must render");
    let cards = cards_in_column(&outcome.body, &leftmost);
    assert!(
        cards.contains(&"READ-1".to_string()),
        "READ-1 must land in the leftmost lane (D6, derived from lane data); {leftmost} = {cards:?}"
    );
}

#[then(regex = r"^the reply says the new issue landed in Backlog$")]
async fn reply_names_backlog(world: &mut FoundryWorld) {
    let (key, state) = world
        .blm_machine_created
        .clone()
        .expect("machine filing captured");
    // The oracle is the LEFTMOST LANE ROW, not the literal "backlog": the
    // hardcoded `CreatedIssue.state = "backlog"` echo (the seventh
    // static-list consumer, ripple surface 6) would satisfy the literal today
    // — a false GREEN the RED-classification run caught.
    let leftmost = leftmost_lane_slug(world, "Reading List").await;
    assert_eq!(
        leftmost, "backlog",
        "the fresh project's leftmost lane row is Backlog"
    );
    assert_eq!(
        state, leftmost,
        "the filing reply must echo the ACTUAL landing lane (the leftmost lane ROW); \
         {key} landed in {state:?}"
    );
}

#[then(regex = r"^the board shows it there$")]
async fn board_shows_created(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (key, state) = world
        .blm_machine_created
        .clone()
        .expect("machine filing captured");
    let path = board_path(world, &p);
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    assert_eq!(outcome.status, StatusCode::OK, "board must render");
    let cards = cards_in_column(&outcome.body, &state);
    assert!(
        cards.contains(&key),
        "{key:?} must render in the {state:?} column the reply named; cards = {cards:?}"
    );
}

#[then(
    regex = r"^the reply says the new issue landed in In-Progress and the board shows it there$"
)]
async fn reply_and_board_in_progress(world: &mut FoundryWorld) {
    let (key, state) = world
        .blm_machine_created
        .clone()
        .expect("machine filing captured");
    assert_eq!(
        state, "in_progress",
        "after Backlog's deletion the leftmost lane is In-Progress (D6): the reply must \
         echo the ACTUAL landing lane; {key} landed in {state:?}"
    );
    board_shows_created(world).await;
}

// ===========================================================================
// Then — edit-dialog options (from lane rows, never a hardcoded list)
// ===========================================================================

#[then(regex = r"^the Status options are exactly the board's five lanes, in board order$")]
async fn options_are_five_lanes(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    assert_eq!(
        lanes.len(),
        5,
        "the grandfathered+cancelled board has five lanes"
    );
    assert_options_match(world, &lanes);
}

#[then(regex = r"^the Status options are exactly Backlog, In-Progress and Done$")]
async fn options_are_three_lanes(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    assert_eq!(lanes.len(), 3, "the fresh board has exactly three lanes");
    assert_options_match(world, &lanes);
}

fn assert_options_match(world: &FoundryWorld, lanes: &[(String, String, i32)]) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the edit dialog must render; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("dialog captured");
    let options = select_options(body, "state");
    let got: Vec<(String, String)> = options
        .into_iter()
        .map(|(value, label, _)| (value, label))
        .collect();
    let expected: Vec<(String, String)> = lanes
        .iter()
        .map(|(slug, label, _)| (slug.clone(), label.clone()))
        .collect();
    assert_eq!(
        got, expected,
        "the Status options must be exactly the project's lane rows, board order (D8)"
    );
}

// ===========================================================================
// Then — machine refusals + landing invariants
// ===========================================================================

#[then(regex = r"^the move is refused as invalid and ([A-Z]+)-(\d+) has not moved$")]
async fn move_refused_unmoved(world: &mut FoundryWorld, _prefix: String, _number: i32) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "a write to a lane the board does not have must be refused as a validation error \
         (D8); body = {:?}",
        world.last_body
    );
    assert_universe_unchanged(world).await;
}

#[then(regex = r"^no issue on any board is without a lane$")]
async fn no_laneless_issue(world: &mut FoundryWorld) {
    assert_zero_laneless(world).await;
}

#[then(
    regex = r"^the first move succeeds and the second is refused as invalid, with READ-1 still In-Progress$"
)]
async fn first_ok_second_refused(world: &mut FoundryWorld) {
    let (first_status, first_body) = world.blm_first_move.clone().expect("first move captured");
    assert!(
        first_status.is_success(),
        "the move to a lane the board HAS must succeed; got {first_status} with {first_body:?}"
    );
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the move to \"todo\" must be refused — \"Reading List\" has no such lane; body = {:?}",
        world.last_body
    );
    let project_id = project_id_of(world, "Reading List");
    let (state,): (String,) =
        sqlx::query_as("SELECT state FROM issues WHERE project_id = $1 AND number = 1")
            .bind(project_id)
            .fetch_one(&pool(world))
            .await
            .expect("READ-1 row");
    assert_eq!(state, "in_progress", "READ-1 must still be In-Progress");
}

// ===========================================================================
// Then — dnd regression
// ===========================================================================

#[then(regex = r"^AUTH-12 renders at the top of In-Progress and stays there on reload$")]
async fn auth12_top_of_in_progress(world: &mut FoundryWorld) {
    assert!(
        world
            .last_status
            .map(|s| s.is_success() || s.is_redirection())
            .unwrap_or(false),
        "the drop must be accepted; got {:?} with body {:?}",
        world.last_status,
        world.last_body
    );
    let p = current_project(world);
    let path = board_path(world, &p);
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "board must render on reload"
    );
    let cards = cards_in_column(&outcome.body, "in_progress");
    assert_eq!(
        cards.first().map(String::as_str),
        Some("AUTH-12"),
        "AUTH-12 must render at the TOP of In-Progress on reload; column = {cards:?}"
    );
    let project_id = project_id_of(world, &p);
    let (position,): (i32,) =
        sqlx::query_as("SELECT position FROM issues WHERE project_id = $1 AND number = 12")
            .bind(project_id)
            .fetch_one(&pool(world))
            .await
            .expect("AUTH-12 row");
    assert_eq!(
        position, 0,
        "the stored position must persist the top drop (0012)"
    );
}

#[then(regex = r"^the change report records AUTH-12's move$")]
async fn report_records_auth12(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issue_change_events e JOIN issues i ON i.id = e.issue_id
          WHERE i.project_id = $1 AND i.number = 12 AND e.field = 'status'
            AND e.old_value = 'todo' AND e.new_value = 'in_progress'",
    )
    .bind(project_id)
    .fetch_one(&pool(world))
    .await
    .expect("count status events");
    assert_eq!(
        n, 1,
        "exactly one status change event must record the move (0013)"
    );
    let path = format!("{}/report", board_path(world, &p));
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the change report must render"
    );
    assert!(
        outcome.body.contains("AUTH-12"),
        "the change report must show AUTH-12's move"
    );
}

// ===========================================================================
// Then — delete-dialog markup oracles
// ===========================================================================

#[then(regex = r"^the dialog states the lane holds no issues and that this cannot be undone$")]
async fn dialog_confirm_only(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the safe dialog GET must render; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("dialog captured");
    assert!(
        body.contains(MODAL_MARKER),
        "the dialog must carry [{MODAL_MARKER}]; got {body:?}"
    );
    assert!(
        body.contains(&format!("{COUNT_ATTR}0\"")),
        "the empty-lane dialog must state a count of 0; got {body:?}"
    );
    assert!(
        body.to_lowercase().contains("holds no issues"),
        "the confirm-only copy must state the lane holds no issues; got {body:?}"
    );
    assert!(
        body.to_lowercase().contains("cannot be undone"),
        "removing a lane is destructive configuration — the copy must say so; got {body:?}"
    );
    assert!(
        !body.contains("name=\"destination\""),
        "an empty lane has no fate to choose — no destination picker (D7); got {body:?}"
    );
    assert!(
        body.contains("data-action=\"close-modal\""),
        "close is the declarative data-action trigger ONLY (BR-4); got {body:?}"
    );
}

#[then(regex = r"^the board, its lanes and every card are untouched$")]
async fn board_untouched(world: &mut FoundryWorld) {
    assert_universe_unchanged(world).await;
}

#[then(regex = r"^the dialog states the lane holds 3 issues$")]
async fn dialog_states_three(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the dialog GET must render; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("dialog captured");
    assert!(
        body.contains(MODAL_MARKER),
        "the dialog must carry [{MODAL_MARKER}]"
    );
    assert!(
        body.contains(&format!("{COUNT_ATTR}3\"")),
        "the dialog must state the LIVE count of 3; got {body:?}"
    );
    assert!(
        body.contains("3 issues"),
        "the copy must state the count in words; got {body:?}"
    );
}

#[then(
    regex = r"^the destination picker lists exactly Backlog, In-Progress and Done with Backlog preselected$"
)]
async fn picker_lists_survivors(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().expect("dialog captured");
    let options = select_options(body, "destination");
    let values: Vec<&str> = options.iter().map(|(v, _, _)| v.as_str()).collect();
    assert_eq!(
        values,
        vec!["backlog", "in_progress", "done"],
        "the picker must list exactly the SURVIVING lanes in board order (dying lane excluded)"
    );
    assert!(
        options.first().map(|(_, _, sel)| *sel).unwrap_or(false),
        "the leftmost survivor must be preselected (D7); options = {options:?}"
    );
}

// ===========================================================================
// Then — delete outcomes
// ===========================================================================

async fn lane_exists(world: &FoundryWorld, project_name: &str, slug: &str) -> bool {
    let project_id = project_id_of(world, project_name);
    let (exists,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM lanes WHERE project_id = $1 AND slug = $2)")
            .bind(project_id)
            .bind(slug)
            .fetch_one(&pool(world))
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "MISSING_FUNCTIONALITY: probing lane rows requires migration 0015 (`lanes` \
             table), DELIVER-owned (ADR-025). Underlying error: {err}"
                )
            });
    exists
}

#[then(regex = r"^the Todo column is gone without a full page reload and remains gone on reload$")]
async fn todo_gone_and_stays_gone(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the confirm must answer the swap fragment; body = {:?}",
        world.last_body
    );
    let fragment = world
        .last_body
        .as_deref()
        .expect("confirm response captured");
    // The no-reload proof in the HTTP lane: the response IS the out-of-band
    // board-columns fragment (house OOB idiom), already free of Todo.
    assert!(
        fragment.contains(OOB_BOARD_MARKER) && fragment.contains("hx-swap-oob"),
        "the confirm response must carry the out-of-band #board-columns refresh; got {fragment:?}"
    );
    assert!(
        !fragment.contains("data-column=\"todo\""),
        "the refreshed columns must no longer include Todo; got {fragment:?}"
    );
    let p = current_project(world);
    assert!(
        !lane_exists(world, &p, "todo").await,
        "the todo lane row must be removed (persisted, not cosmetic)"
    );
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &board_path(world, &p),
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "board must render on reload"
    );
    assert!(
        !outcome.body.contains("data-column=\"todo\""),
        "the Todo column must remain gone on reload"
    );
}

#[then(
    regex = r"^the edit dialog no longer offers Todo and a client can no longer move a card there$"
)]
async fn todo_gone_from_dialog_and_api(world: &mut FoundryWorld) {
    let p = current_project(world);
    // The edit dialog for the surviving card (OPS-1) must list only survivors.
    let path = format!("{}/issues/1/edit", board_path(world, &p));
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the edit dialog must render"
    );
    let options = select_options(&outcome.body, "state");
    assert!(
        !options.iter().any(|(value, _, _)| value == "todo"),
        "the edit dialog must no longer offer Todo; options = {options:?}"
    );
    // And the machine port must refuse it identically (DD10 single seam).
    let (status, body) = api_patch_state(world, &p, 1, "todo").await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a client move into the deleted lane must be refused; body = {body:?}"
    );
    assert_zero_laneless(world).await;
}

#[then(
    regex = r#"^she is refused with the reason "A board needs at least one lane" and the lane remains$"#
)]
async fn last_lane_refused(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the last-lane refusal is a validation refusal (D6); body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("fragment captured");
    assert!(
        body.contains(LAST_LANE_MESSAGE),
        "the refusal must state {LAST_LANE_MESSAGE:?}; got {body:?}"
    );
    assert!(
        body.contains(ERROR_MARKER),
        "the refusal must be the bare error fragment ([{ERROR_MARKER}]) form-errors.js \
         routes into [data-error-slot]; got {body:?}"
    );
    assert!(
        !body.contains("<html"),
        "the error fragment must be BARE (no base.html double-wrap); got {body:?}"
    );
    let p = current_project(world);
    assert!(
        lane_exists(world, &p, "done").await,
        "the sole lane must survive the refused delete"
    );
}

#[then(regex = r"^Marco asking for the delete dialog is answered identically$")]
async fn marco_get_answered_identically(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (team_slug, project_slug) = stored_slugs(world, &p);
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        MARCO_EMAIL,
        MARCO_PASSWORD,
        &lane_delete_path(&team_slug, &project_slug, "todo"),
    )
    .await;
    assert_eq!(
        Some(outcome.status),
        world.last_status,
        "the dialog GET and the confirm POST must share one refusal status for a non-member \
         (uniform 404 on BOTH verbs — DESIGN refinement 3)"
    );
    assert_eq!(
        Some(outcome.body.as_str()),
        world.last_body.as_deref(),
        "the GET refusal must be byte-identical to the POST refusal (no enumeration oracle)"
    );
}

#[then(regex = r"^the Todo lane is still on the board$")]
async fn todo_still_on_board(world: &mut FoundryWorld) {
    let p = current_project(world);
    assert!(
        lane_exists(world, &p, "todo").await,
        "the refused delete must leave the todo lane row in place"
    );
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &board_path(world, &p),
    )
    .await;
    assert!(
        outcome.body.contains("data-column=\"todo\""),
        "the board must still render the Todo column"
    );
}

#[then(regex = r"^the delete is refused before any change is made$")]
async fn refused_before_handler(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::FORBIDDEN),
        "a POST without its double-submit pair must be refused by the CSRF middleware \
         before the handler runs (D10); body = {:?}",
        world.last_body
    );
    assert_universe_unchanged(world).await;
}

// ===========================================================================
// Then — the two fates
// ===========================================================================

const MOVED_KEYS: &[&str] = &["AUTH-12", "AUTH-15", "AUTH-18"];

#[then(
    regex = r"^the Todo column is gone and AUTH-12, AUTH-15 and AUTH-18 sit at the bottom of Backlog in that order$"
)]
async fn moved_to_backlog_bottom(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the move-fate confirm must succeed; body = {:?}",
        world.last_body
    );
    let p = current_project(world);
    assert!(
        !lane_exists(world, &p, "todo").await,
        "the todo lane row must be gone"
    );

    // State-delta over the declared universe, fail-closed: ONLY the moved
    // cards' (lane, position) may differ from the pre-confirm snapshot.
    let before = world
        .blm_issues_before
        .clone()
        .expect("universe snapshotted");
    let after = issues_of(world, &p).await;
    let dest_before = world
        .blm_dest_order_before
        .clone()
        .expect("destination order captured BEFORE the confirm");
    let dest_count = dest_before.len() as i32;
    for (key, state, position) in &after {
        if let Some(idx) = MOVED_KEYS.iter().position(|k| k == key) {
            assert_eq!(
                state, "backlog",
                "{key} must have moved to the chosen destination"
            );
            assert_eq!(
                *position,
                dest_count + idx as i32,
                "{key} must append at the destination's BOTTOM preserving relative order (0012)"
            );
        } else {
            let was = before
                .iter()
                .find(|(k, _, _)| k == key)
                .unwrap_or_else(|| panic!("{key} appeared out of nowhere"));
            assert_eq!(
                (state, position),
                (&was.1, &was.2),
                "{key} was NOT part of the fate and must be byte-identical (fail-closed universe)"
            );
        }
    }
    assert_eq!(
        before.len(),
        after.len(),
        "no card may appear or vanish on a move fate"
    );

    // The rendered column agrees with the stored rows.
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &board_path(world, &p),
    )
    .await;
    let rendered = cards_in_column(&outcome.body, "backlog");
    let mut expected = dest_before;
    expected.extend(MOVED_KEYS.iter().map(|k| k.to_string()));
    assert_eq!(
        rendered, expected,
        "Backlog must render its prior cards followed by the moved three, in order"
    );
    assert!(
        !outcome.body.contains("data-column=\"todo\""),
        "the Todo column must be gone from the rendered board"
    );
    assert_zero_laneless(world).await;
}

#[then(
    regex = r"^the change report shows a move from Todo to Backlog for each of the three, attributed to Priya$"
)]
async fn report_shows_three_moves(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let priya = world.blm_priya_id.expect("Priya seeded");
    for number in [12, 15, 18] {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT count(*) FROM issue_change_events e JOIN issues i ON i.id = e.issue_id
              WHERE i.project_id = $1 AND i.number = $2 AND e.field = 'status'
                AND e.old_value = 'todo' AND e.new_value = 'backlog' AND e.actor_id = $3",
        )
        .bind(project_id)
        .bind(number)
        .bind(priya)
        .fetch_one(&pool(world))
        .await
        .expect("count status events");
        assert_eq!(
            n, 1,
            "exactly one Todo→Backlog status event must exist for AUTH-{number}, \
             attributed to Priya, written in the SAME transaction (0013)"
        );
    }
    // Outbox parity: one IssueUpdated row per moved card (same tx).
    let outbox_after = count_of(&pool(world), "outbox").await;
    let outbox_before = world.blm_outbox_before.expect("outbox snapshotted");
    assert_eq!(
        outbox_after - outbox_before,
        3,
        "the move fate must write exactly one outbox row per moved card"
    );
    // And the human-facing report renders the moves with lane LABELS.
    let path = format!("{}/report", board_path(world, &p));
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &path,
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the change report must render"
    );
    for key in MOVED_KEYS {
        assert!(
            outcome.body.contains(key),
            "the change report must show the move of {key}"
        );
    }
    assert!(
        outcome.body.contains("Todo") && outcome.body.contains("Backlog"),
        "the report must label the move with the lane labels (live label or the \
         historical fallback for the deleted Todo — DESIGN refinement 2)"
    );
}

#[then(
    regex = r"^the lane and both cards are gone from the board and neither issue is findable in search$"
)]
async fn lane_and_cards_gone(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the delete-fate confirm must succeed; body = {:?}",
        world.last_body
    );
    let p = current_project(world);
    assert!(
        !lane_exists(world, &p, "done").await,
        "the Done lane row must be gone"
    );
    let project_id = project_id_of(world, &p);
    let (remaining,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM issues WHERE project_id = $1 AND number IN (2, 5)")
            .bind(project_id)
            .fetch_one(&pool(world))
            .await
            .expect("count deleted issues");
    assert_eq!(remaining, 0, "SCR-2 and SCR-5 must be permanently removed");
    let board = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &board_path(world, &p),
    )
    .await;
    assert!(
        !board.body.contains("SCR-2") && !board.body.contains("SCR-5"),
        "neither card may render anywhere on the board"
    );
    let search = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &format!("{}/search?q=spike", board_path(world, &p)),
    )
    .await;
    assert!(
        !search.body.contains("SCR-2") && !search.body.contains("SCR-5"),
        "neither issue may be findable in search after permanent deletion"
    );
    assert_zero_laneless(world).await;
}

#[then(regex = r"^nothing of SCR-2 remains, neither its comment nor its attachment$")]
async fn scr2_cascade_gone(world: &mut FoundryWorld) {
    // Per-scenario schema isolation: the seeded comment + attachment were the
    // only rows in this schema, so zero totals prove the hard cascade
    // (delete_issue_cascade shape, D7 — no tombstone).
    let pool = pool(world);
    assert_eq!(
        count_of(&pool, "comments").await,
        0,
        "the comment must cascade away"
    );
    assert_eq!(
        count_of(&pool, "issue_attachments").await,
        0,
        "the attachment must cascade away"
    );
}

#[then(regex = r"^the Todo lane, all three cards and the change history are untouched$")]
async fn cancel_leaves_untouched(world: &mut FoundryWorld) {
    let p = current_project(world);
    assert!(
        lane_exists(world, &p, "todo").await,
        "the Todo lane must survive a cancel"
    );
    assert_universe_unchanged(world).await;
}

#[then(regex = r"^all four cards that were in Todo at confirm time sit in Backlog$")]
async fn all_four_in_backlog(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the move-fate confirm must succeed; body = {:?}",
        world.last_body
    );
    let p = current_project(world);
    assert!(
        !lane_exists(world, &p, "todo").await,
        "the todo lane row must be gone"
    );
    let (late_key, _) = world
        .blm_machine_created
        .clone()
        .expect("the automation's filing captured");
    let after = issues_of(world, &p).await;
    for key in MOVED_KEYS
        .iter()
        .map(|k| k.to_string())
        .chain([late_key.clone()])
    {
        let (_, state, _) = after
            .iter()
            .find(|(k, _, _)| k == &key)
            .unwrap_or_else(|| panic!("{key} must still exist"));
        assert_eq!(
            state, "backlog",
            "{key} was in Todo at confirm time and must be in Backlog — the fate binds to \
             confirm-time membership, the dialog's count was advisory (D7)"
        );
    }
}

// ===========================================================================
// Migration oracle (staged 0001..0014 → seed pre-0015 data → canonical twice)
// ===========================================================================

fn canonical_migrations_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../foundry-store/migrations")
}

#[given(
    regex = r#"^a database from before the upgrade holds "Identity Platform" with issues in its four working states and "Homelab Ops" with one cancelled issue$"#
)]
async fn pre_upgrade_database(world: &mut FoundryWorld) {
    let (schema, pool, _url) = fresh_schema_pool_no_migrations().await;
    let staged = test_migration::stage_subset(14).expect("stage pre-feature migrations 0001..0014");
    foundry_store::run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("apply pre-feature migrations");
    // Seed the pre-0015 shape directly (the CHECK still admits `cancelled`).
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
        .bind(ws)
        .execute(&pool)
        .await
        .expect("seed workspace");
    let user = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'priya@canzan.test', 'priya@canzan.test', 'Priya', 'x')",
    )
    .bind(user)
    .execute(&pool)
    .await
    .expect("seed user");
    let team = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
    )
    .bind(team)
    .bind(ws)
    .execute(&pool)
    .await
    .expect("seed team");
    let mut seed_project = |name: &str, slug: &str, prefix: &str| {
        let id = uuid::Uuid::now_v7();
        world.blm_mig_project_ids.insert(name.to_string(), id);
        (id, name.to_string(), slug.to_string(), prefix.to_string())
    };
    let projects = [
        seed_project("Identity Platform", "identity-platform", "AUTH"),
        seed_project("Homelab Ops", "homelab-ops", "OPS"),
    ];
    for (id, name, slug, prefix) in &projects {
        sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(team)
        .bind(ws)
        .bind(name)
        .bind(slug)
        .bind(prefix)
        .execute(&pool)
        .await
        .expect("seed project");
    }
    let ip = world.blm_mig_project_ids["Identity Platform"];
    let ho = world.blm_mig_project_ids["Homelab Ops"];
    let issues: &[(uuid::Uuid, i32, &str, &str)] = &[
        (ip, 7, "Refresh token rotation", "backlog"),
        (ip, 12, "Rotate signing keys", "todo"),
        (ip, 3, "OIDC discovery cache", "in_progress"),
        (ip, 1, "Argon2 parameter bump", "done"),
        (ho, 9, "Replace UPS battery", "cancelled"),
    ];
    for (project_id, number, title, state) in issues {
        sqlx::query(
            "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id, position)
                  VALUES ($1, $2, $3, $4, $5, $6, 'medium', $7, 0)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(ws)
        .bind(project_id)
        .bind(number)
        .bind(title)
        .bind(state)
        .bind(user)
        .execute(&pool)
        .await
        .expect("seed pre-upgrade issue");
    }
    // Zero-shuffle oracle anchor: the full issue-row surface before 0015.
    let rows: Vec<(i32, String, i32)> =
        sqlx::query_as("SELECT number, state, position FROM issues ORDER BY number ASC")
            .fetch_all(&pool)
            .await
            .expect("snapshot issues");
    world.blm_mig_issues_before = Some(
        rows.into_iter()
            .map(|(n, s, p)| (format!("#{n}"), s, p))
            .collect(),
    );
    world.blm_mig_schema = Some(schema);
    world.blm_mig_pool = Some(pool);
    world.blm_mig_staged = Some(staged);
}

fn mig_pool(world: &FoundryWorld) -> PgPool {
    world.blm_mig_pool.clone().expect("migration-oracle pool")
}

#[when(regex = r"^the upgrade migrations run, and then run again$")]
async fn upgrade_runs_twice(world: &mut FoundryWorld) {
    let pool = mig_pool(world);
    let dir = canonical_migrations_dir();
    foundry_store::run_migrations_from_dir(&pool, &dir)
        .await
        .expect("first canonical migration run");
    // Idempotency: a second run applies nothing and errors nothing (the seed
    // is ON CONFLICT DO NOTHING; applied migrations are skipped).
    foundry_store::run_migrations_from_dir(&pool, &dir)
        .await
        .expect("second canonical migration run (idempotency)");
}

async fn mig_lanes(world: &FoundryWorld, project_name: &str) -> Vec<(String, String, i32)> {
    let project_id = *world
        .blm_mig_project_ids
        .get(project_name)
        .expect("pre-upgrade project seeded");
    sqlx::query_as(
        "SELECT slug, label, position FROM lanes WHERE project_id = $1 ORDER BY position ASC",
    )
    .bind(project_id)
    .fetch_all(&mig_pool(world))
    .await
    .unwrap_or_else(|err| {
        panic!(
            "MISSING_FUNCTIONALITY: the canonical migration set does not yet create the \
             `lanes` table — migration 0015 is DELIVER-owned (ADR-025). Underlying error: {err}"
        )
    })
}

#[then(
    regex = r#"^"Identity Platform" has exactly the lanes Backlog, Todo, In-Progress and Done, in that order$"#
)]
async fn mig_ip_lanes(world: &mut FoundryWorld) {
    let lanes = mig_lanes(world, "Identity Platform").await;
    let got: Vec<(String, String)> = lanes
        .iter()
        .map(|(s, l, _)| (s.clone(), l.clone()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("backlog".to_string(), "Backlog".to_string()),
            ("todo".to_string(), "Todo".to_string()),
            ("in_progress".to_string(), "In-Progress".to_string()),
            ("done".to_string(), "Done".to_string()),
        ],
        "the grandfather seed must give every existing project its four rendered lanes, \
         labels byte-equal to today's headers (D5) — and NOT a Cancelled lane without \
         cancelled issues"
    );
}

#[then(regex = r#"^"Homelab Ops" additionally has a Cancelled lane after Done$"#)]
async fn mig_ho_lanes(world: &mut FoundryWorld) {
    let lanes = mig_lanes(world, "Homelab Ops").await;
    let got: Vec<&str> = lanes.iter().map(|(s, _, _)| s.as_str()).collect();
    assert_eq!(
        got,
        vec!["backlog", "todo", "in_progress", "done", "cancelled"],
        "a project holding a cancelled issue must be granted a Cancelled lane, last (D5)"
    );
}

#[then(regex = r"^no issue row was rewritten by the upgrade$")]
async fn mig_zero_shuffle(world: &mut FoundryWorld) {
    let rows: Vec<(i32, String, i32)> =
        sqlx::query_as("SELECT number, state, position FROM issues ORDER BY number ASC")
            .fetch_all(&mig_pool(world))
            .await
            .expect("snapshot issues after");
    let after: Vec<(String, String, i32)> = rows
        .into_iter()
        .map(|(n, s, p)| (format!("#{n}"), s, p))
        .collect();
    assert_eq!(
        world
            .blm_mig_issues_before
            .as_ref()
            .expect("pre-upgrade snapshot"),
        &after,
        "0015 must be zero-shuffle: every (number, state, position) byte-identical (D5, \
         the 0012 discipline)"
    );
}

#[then(regex = r"^the store structurally refuses an issue without a lane$")]
async fn mig_fk_refuses(world: &mut FoundryWorld) {
    let pool = mig_pool(world);
    // The composite FK must exist…
    let (fk_exists,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_issues_lane')",
    )
    .fetch_one(&pool)
    .await
    .expect("probe pg_constraint");
    assert!(
        fk_exists,
        "MISSING_FUNCTIONALITY: the composite FK fk_issues_lane (project_id, state) → \
         lanes (project_id, slug) must exist after 0015 (ADR-BOARD-LANE-001)"
    );
    // …and actually bite: an INSERT naming a lane the project does not have
    // must be refused by the schema, not by convention.
    let ip = world.blm_mig_project_ids["Identity Platform"];
    let (ws, author): (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT workspace_id, (SELECT id FROM users LIMIT 1) FROM projects WHERE id = $1",
    )
    .bind(ip)
    .fetch_one(&pool)
    .await
    .expect("resolve seed ids");
    let refused = sqlx::query(
        "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id, position)
              VALUES ($1, $2, $3, 999, 'strand attempt', 'no_such_lane', 'medium', $4, 0)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(ws)
    .bind(ip)
    .bind(author)
    .execute(&pool)
    .await;
    assert!(
        refused.is_err(),
        "an issue naming a lane its project does not have must be structurally refused \
         (the no-stranded-card invariant is a schema fact, not a test assertion)"
    );
}

// ===========================================================================
// @needs-browser — the DOM oracle
// ===========================================================================

const PAGE_MARKER_SET: &str = "window.__blm_page_marker = 'alive';";
const PAGE_MARKER_GET: &str = "return window.__blm_page_marker || null;";

#[given(regex = r#"^Priya has the "([^"]+)" board open in her browser$"#)]
async fn board_open_in_browser(world: &mut FoundryWorld, project_name: String) {
    ensure_harness(world).await;
    world.blm_current_project = Some(project_name.clone());
    let path = board_path(world, &project_name);
    let browser = browser_harness::new_session().await;
    {
        let harness = world.harness.as_ref().expect("harness");
        browser_harness::sign_in_through_browser(&browser, harness, PRIYA_EMAIL, PRIYA_PASSWORD)
            .await;
        browser
            .goto(&format!("{}{path}", harness.base_url()))
            .await
            .expect("open the board in the browser");
    }
    browser
        .execute(PAGE_MARKER_SET, vec![])
        .await
        .expect("plant the page-lifetime marker");
    world.browser = Some(browser);
}

/// RE-PREMISED by board-lane-overflow-menu (D3/D13, step 01-03).
///
/// The armed `×` this used to click is GONE: the destructive action now lives
/// behind the per-column `⋯` overflow menu, so reaching the delete dialog costs
/// one more interaction. The SCENARIOS are unchanged and still assert exactly
/// what they asserted before — that a delete dialog opens for this lane and
/// behaves as it did. Only the route to the control moved, which is the whole
/// point of the successor feature.
///
/// This premise break was pre-registered in board-lane-overflow-menu's DISCUSS
/// wave (D13) and is deliberate, not a regression.
async fn click_lane_delete(world: &mut FoundryWorld, lane_slug: &str) {
    let browser = world.browser.as_ref().expect("browser session");
    let trigger_selector =
        format!("button[data-action=\"toggle-lane-menu\"][data-lane=\"{lane_slug}\"]");
    let trigger = browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css(&trigger_selector))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "every rendered lane header must carry its ⋯ menu trigger \
                 ([data-action=toggle-lane-menu][data-lane={lane_slug:?}], \
                 board-lane-overflow-menu component-boundaries.md §1.1): {err}"
            )
        });
    trigger.click().await.expect("open the lane ⋯ menu");
    // Scoped to the OPEN menu: every column renders a "Delete list" item, and
    // an unscoped match returns the leftmost column's hidden one.
    let item = browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::XPath(
            "//*[@data-lane-menu and not(@hidden)]//*[normalize-space(text())='Delete list']",
        ))
        .await
        .unwrap_or_else(|err| {
            panic!("the open {lane_slug:?} menu must offer a Delete list item: {err}")
        });
    item.click().await.expect("choose Delete list");
}

#[when(regex = r"^she clicks the delete control on the Todo column$")]
async fn clicks_delete_control(world: &mut FoundryWorld) {
    click_lane_delete(world, "todo").await;
}

async fn wait_for_dialog(world: &FoundryWorld) -> fantoccini::elements::Element {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root [data-modal=\"delete-lane\"]"))
        .await
        .expect("the delete dialog must htmx-swap into #modal-root")
}

#[then(regex = r"^a dialog appears stating the lane holds 3 issues$")]
async fn browser_dialog_states_three(world: &mut FoundryWorld) {
    let dialog = wait_for_dialog(world).await;
    let count = dialog
        .attr("data-lane-count")
        .await
        .expect("read data-lane-count")
        .unwrap_or_default();
    assert_eq!(count, "3", "the dialog must show the LIVE card count");
}

#[then(regex = r"^the delete dialog appears$")]
async fn browser_dialog_appears(world: &mut FoundryWorld) {
    let _ = wait_for_dialog(world).await;
}

#[when(regex = r"^she confirms moving all 3 to Backlog$")]
async fn browser_confirms_move(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    // Backlog is the leftmost survivor and PRESELECTED — she keeps it and
    // clicks the move fate; htmx must include the clicked submitter's
    // name=value (the Earned Trust probe).
    browser
        .find(Locator::Css(
            "[data-modal=\"delete-lane\"] button[name=\"fate\"][value=\"move\"]",
        ))
        .await
        .expect("the fate dialog must carry the move submit")
        .click()
        .await
        .expect("click the move fate");
}

async fn wait_gone(browser: &fantoccini::Client, css: &str, what: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if browser.find(Locator::Css(css)).await.is_err() {
            return;
        }
        if std::time::Instant::now() > deadline {
            panic!("{what} must be gone, but {css:?} is still present");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn assert_marker_alive(world: &FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let marker = browser
        .execute(PAGE_MARKER_GET, vec![])
        .await
        .expect("read the page-lifetime marker");
    assert_eq!(
        marker.as_str(),
        Some("alive"),
        "the page-lifetime marker must survive — this must NOT be a full reload"
    );
}

#[then(regex = r"^the Todo column disappears without the page reloading$")]
async fn browser_todo_disappears(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session").clone();
    wait_gone(&browser, "[data-column=\"todo\"]", "the Todo column").await;
    assert_marker_alive(world).await;
}

#[then(regex = r"^the three cards appear at the bottom of the Backlog column$")]
async fn browser_cards_at_bottom(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let keys = browser
        .execute(
            "return Array.prototype.map.call(
               document.querySelectorAll('[data-column=\"backlog\"] [data-issue-key]'),
               function (el) { return el.getAttribute('data-issue-key'); });",
            vec![],
        )
        .await
        .expect("read the Backlog column's card keys");
    let keys: Vec<String> = keys
        .as_array()
        .expect("array of keys")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert_eq!(
        keys,
        vec!["AUTH-7", "AUTH-12", "AUTH-15", "AUTH-18"],
        "Backlog must render its prior card followed by the moved three, in order"
    );
}

#[when(regex = r"^she dismisses it with the close control$")]
async fn browser_dismisses_with_close(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css(
            "[data-modal=\"delete-lane\"] [data-action=\"close-modal\"]",
        ))
        .await
        .expect("the dialog must carry the declarative close trigger (BR-4)")
        .click()
        .await
        .expect("click the close control");
}

#[then(
    regex = r"^the dialog is gone, the Todo column is still on the board, and the page did not reload$"
)]
async fn browser_dialog_gone_board_intact(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session").clone();
    wait_gone(
        &browser,
        "[data-modal=\"delete-lane\"]",
        "the delete dialog",
    )
    .await;
    browser
        .find(Locator::Css("[data-column=\"todo\"]"))
        .await
        .expect("the Todo column must still be on the board");
    assert_marker_alive(world).await;
}

#[when(regex = r"^she reopens the dialog and presses Esc$")]
async fn browser_reopen_and_esc(world: &mut FoundryWorld) {
    click_lane_delete(world, "todo").await;
    let _ = wait_for_dialog(world).await;
    let browser = world.browser.as_ref().expect("browser session").clone();
    browser_harness::press_key(&browser, "Escape").await;
}

#[then(regex = r"^the dialog is gone again with the board untouched$")]
async fn browser_dialog_gone_again(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session").clone();
    wait_gone(
        &browser,
        "[data-modal=\"delete-lane\"]",
        "the delete dialog",
    )
    .await;
    browser
        .find(Locator::Css("[data-column=\"todo\"]"))
        .await
        .expect("the Todo column must still be on the board");
    let p = current_project(world);
    assert!(
        lane_exists(world, &p, "todo").await,
        "Esc must cancel: the lane row survives, nothing was written"
    );
}
