//! board-lane-overflow-menu step definitions
//! (`tests/features/board-lane-overflow-menu.feature`, 23 scenarios, ALL
//! `@pending` — scaffolded RED per ADR-025; DELIVER un-pends one at a time and
//! never re-authors).
//!
//! The production seams these steps drive are the DESIGN port signatures
//! (`design/component-boundaries.md`): `foundry_core::lane_slug`,
//! `foundry_store::lanes` (`rename_lane`, `insert_lane_at`),
//! `foundry_services::lanes` (`edit_lane_dialog`, `rename_lane`,
//! `insert_lane_dialog`, `insert_lane`), and the mounted `foundry-app::lanes`
//! handlers (dialog GETs + confirm POSTs, all answering a clean 501 until
//! DELIVER). Unlike the predecessor wave, the `lanes` TABLE already exists —
//! migration 0015 shipped — so lane-seeding Givens succeed. The RED here comes
//! from two honest places: the menu markup does not exist yet
//! (MISSING_FUNCTIONALITY(markup)), and every write port is a panicking
//! scaffold reached through a 501 handler (MISSING_FUNCTIONALITY(port)).
//!
//! THE LANE-LIST ORACLE RULE: every lane expectation reads lane rows BACK FROM
//! THE DATABASE (`lanes`: slug, label, position). This module deliberately has
//! NO static expected-lane list — one would go green over exactly the
//! static-list consumers the `check-arch` rule exists to forbid.
//!
//! THE CONTIGUITY RULE: Postgres enforces position UNIQUENESS, never
//! contiguity. A gap after an insert would be invisible to the schema and
//! merely cosmetic to `ORDER BY position`. If [`assert_contiguous`] does not
//! assert it, nothing in the system does.
//!
//! THE IDENTITY RULE: the rename oracles compare `slug`, `position` and every
//! `issues.state` from the STORE before and after. A DOM-only assertion would
//! pass over a rename that also rewrote issue states — which is the whole risk
//! `brief.md` §lanes ("slugs are identity, labels are display") exists to name.
//!
//! LAYER 3 (real adapter + real HTTP, `@real-io`): real Postgres via the shared
//! testcontainer + per-scenario schema; the real tower-sessions store; the real
//! double-submit CSRF middleware; the in-process axum router; REAL registered
//! EdDSA bearers for machine-client legs. Example-based (Mandates 9 + 11).
//! State-mutation assertions follow the state-delta discipline: snapshot the
//! declared universe before the write, snapshot after, assert the delta
//! fail-closed. A rename and an insert must each move ZERO issue rows and write
//! ZERO change events.
//!
//! The five `@needs-browser` scenarios drive a REAL headless Chrome
//! (fantoccini, `support::browser_harness`) because the HTTP lane is byte-blind
//! to menu open/close, to focus return, to `Escape` reaching
//! `keyboard.js::closeTopLayer()`, and to a menu surviving (or not surviving)
//! the out-of-band `#board-columns` swap.

use crate::support::browser_harness;
use crate::support::harness::{
    establish_session, post_with_cookie, signed_in_get, signed_in_post, InProcHarness, PostOutcome,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::StatusCode;
use secrecy::{ExposeSecret, SecretString};
use sqlx::PgPool;

const TEST_NOW: &str = "2026-03-01T12:00:00Z";
const PRIYA_EMAIL: &str = "priya.blo@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const MARCO_EMAIL: &str = "marco.blo@canzan.test";
const MARCO_PASSWORD: &str = "marco-correct-horse-battery-staple";

// --- DESIGN-pinned scraper markers (component-boundaries.md §1.1). If DELIVER
// --- moves these, the template and this module move in the SAME change.
const MENU_TRIGGER: &str = "data-action=\"toggle-lane-menu\"";
const MENU_CONTAINER: &str = "data-lane-menu=\"";
/// The affordance this feature REMOVES (D3). Its continued presence anywhere in
/// the rendered board is a failure, not a leftover.
const OLD_DELETE_MARKER: &str = "data-lane-delete=";
const MENU_ITEMS: &[&str] = &[
    "Edit list",
    "Insert list before",
    "Insert list after",
    "Delete list",
];
const DELETE_MODAL_MARKER: &str = "data-modal=\"delete-lane\"";
const CLOSE_TRIGGER: &str = "data-action=\"close-modal\"";
const ERROR_SLOT: &str = "data-error-slot";
const OOB_BOARD_MARKER: &str = "id=\"board-columns\"";

// ------------------------------------------------------------------ plumbing

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse TEST_NOW")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build http client")
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

fn current_project(world: &FoundryWorld) -> String {
    world
        .blo_current_project
        .clone()
        .expect("a board Given must have named the project under test")
}

/// STORED slugs, read back at seed time — never re-derived from a name
/// (ADR-PROJECT-RENAME-001; `fn slugify(` is banned under `foundry-app/src`).
fn stored_slugs(world: &FoundryWorld, project_name: &str) -> (String, String) {
    world
        .blo_project_slugs
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded by a Given"))
        .clone()
}

fn project_id_of(world: &FoundryWorld, project_name: &str) -> uuid::Uuid {
    *world
        .blo_project_ids
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded by a Given"))
}

fn board_path(world: &FoundryWorld, project_name: &str) -> String {
    let (team, project) = stored_slugs(world, project_name);
    format!("/team/{team}/project/{project}")
}

fn lane_edit_path(team: &str, project: &str, lane: &str) -> String {
    format!("/team/{team}/project/{project}/lanes/{lane}/edit")
}

fn lane_insert_path(team: &str, project: &str, lane: &str, side: &str) -> String {
    format!("/team/{team}/project/{project}/lanes/{lane}/insert/{side}")
}

fn lane_delete_path(team: &str, project: &str, lane: &str) -> String {
    format!("/team/{team}/project/{project}/lanes/{lane}/delete")
}

// ------------------------------------------------------------------- seeding

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

async fn seed_project(world: &mut FoundryWorld, name: &str, slug: &str, prefix: &str) {
    let pool = pool(world);
    let ws = world.blo_workspace_id.expect("workspace seeded first");
    let team = world.blo_team_id.expect("team seeded first");
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
    world.blo_project_ids.insert(name.to_string(), id);
    // Read the slugs BACK — the slug-capture rule.
    let (team_slug, project_slug): (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug FROM projects p JOIN teams t ON t.id = p.team_id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("read back stored slugs");
    world
        .blo_project_slugs
        .insert(name.to_string(), (team_slug, project_slug));
}

async fn seed_lane(
    world: &FoundryWorld,
    project_id: uuid::Uuid,
    slug: &str,
    label: &str,
    position: i32,
) {
    let ws = world.blo_workspace_id.expect("workspace seeded first");
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
    .unwrap_or_else(|err| panic!("seed lane ({slug:?}, {label:?}, {position}): {err}"));
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
    let ws = world.blo_workspace_id.expect("workspace seeded first");
    let author = world.blo_priya_id.expect("Priya seeded");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, state, position, author_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(ws)
    .bind(number)
    .bind(title)
    .bind(state)
    .bind(position)
    .bind(author)
    .execute(&pool)
    .await
    .unwrap_or_else(|err| panic!("seed issue {number} into {state:?}: {err}"));
}

// -------------------------------------------------------------- store oracles

/// Lane rows straight from the DB — the ONLY source of lane expectations.
async fn lanes_of(pool: &PgPool, project_id: uuid::Uuid) -> Vec<(String, String, i32)> {
    sqlx::query_as(
        "SELECT slug, label, position FROM lanes WHERE project_id = $1 ORDER BY position",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
    .expect("read lane rows")
}

async fn issues_of(world: &FoundryWorld, project_name: &str) -> Vec<(String, String, i32)> {
    let project_id = project_id_of(world, project_name);
    let prefix = prefix_of(project_name);
    let rows: Vec<(i32, String, i32)> = sqlx::query_as(
        "SELECT number, state, position FROM issues WHERE project_id = $1 ORDER BY number",
    )
    .bind(project_id)
    .fetch_all(&pool(world))
    .await
    .expect("read issue rows");
    rows.into_iter()
        .map(|(n, state, pos)| (format!("{prefix}-{n}"), state, pos))
        .collect()
}

async fn count_of(pool: &PgPool, table: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("count {table}: {err}"));
    n
}

fn prefix_of(project_name: &str) -> String {
    match project_name {
        "Homelab Ops" => "OPS".to_string(),
        "Identity Platform" => "AUTH".to_string(),
        other => other.chars().take(3).collect::<String>().to_uppercase(),
    }
}

/// Snapshot the declared universe before a write (state-delta discipline).
async fn capture_universe(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let pool = pool(world);
    world.blo_lanes_before = Some(lanes_of(&pool, project_id).await);
    world.blo_issues_before = Some(issues_of(world, &p).await);
    world.blo_events_before = Some(count_of(&pool, "issue_change_events").await);
    world.blo_outbox_before = Some(count_of(&pool, "outbox").await);
}

/// Positions must be contiguous from zero AND unique. Postgres enforces only
/// the uniqueness half; this is where contiguity is actually checked.
fn assert_contiguous(lanes: &[(String, String, i32)]) {
    let positions: Vec<i32> = lanes.iter().map(|(_, _, p)| *p).collect();
    let expected: Vec<i32> = (0..lanes.len() as i32).collect();
    assert_eq!(
        positions, expected,
        "lane positions must be contiguous from zero and unique; got {positions:?} for lanes {:?}. \
         Postgres enforces uniqueness only — a gap is invisible to the schema, so this assertion \
         is the system's ONLY contiguity guard.",
        lanes.iter().map(|(s, _, _)| s).collect::<Vec<_>>()
    );
}

async fn assert_zero_laneless(world: &FoundryWorld) {
    let (orphans,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues i
          WHERE NOT EXISTS (SELECT 1 FROM lanes l
                             WHERE l.project_id = i.project_id AND l.slug = i.state)",
    )
    .fetch_one(&pool(world))
    .await
    .expect("zero-laneless guard query");
    assert_eq!(
        orphans, 0,
        "no issue may reference a lane its project does not have (the composite-FK invariant, \
         ADR-BOARD-LANE-001); found {orphans}"
    );
}

/// Assert a rename/insert wrote ZERO issue rows and ZERO change events.
async fn assert_no_issue_writes(world: &FoundryWorld) {
    let p = current_project(world);
    let pool = pool(world);
    let before = world
        .blo_issues_before
        .as_ref()
        .expect("universe captured before the write");
    let after = issues_of(world, &p).await;
    assert_eq!(
        *before, after,
        "a lane rename or insert must move ZERO issue rows (AC-2.2 / AC-3.3); issue rows drifted"
    );
    let events_before = world.blo_events_before.expect("event count captured");
    let events_after = count_of(&pool, "issue_change_events").await;
    assert_eq!(
        events_before, events_after,
        "a lane rename or insert writes NO 0013 change event; count moved \
         {events_before} -> {events_after}"
    );
    let outbox_before = world.blo_outbox_before.expect("outbox count captured");
    let outbox_after = count_of(&pool, "outbox").await;
    assert_eq!(
        outbox_before, outbox_after,
        "a lane rename or insert writes NO outbox row; count moved \
         {outbox_before} -> {outbox_after}"
    );
}

// ------------------------------------------------------------- HTML scraping

fn column_order(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = "data-column=\"";
    let mut rest = body;
    while let Some(idx) = rest.find(needle) {
        rest = &rest[idx + needle.len()..];
        if let Some(end) = rest.find('"') {
            out.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    out
}

fn column_slice<'a>(body: &'a str, slug: &str) -> &'a str {
    let marker = format!("data-column=\"{slug}\"");
    let start = body
        .find(&marker)
        .unwrap_or_else(|| panic!("column {slug:?} is not rendered on this board"));
    let rest = &body[start..];
    match rest[1..].find("<section class=\"column\"") {
        Some(end) => &rest[..end + 1],
        None => rest,
    }
}

// ---------------------------------------------------------------- HTTP verbs

async fn priya_get(world: &mut FoundryWorld, path: &str) -> PostOutcome {
    ensure_harness(world).await;
    signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        path,
    )
    .await
}

async fn priya_post(world: &mut FoundryWorld, path: &str, form: &[(&str, &str)]) -> PostOutcome {
    ensure_harness(world).await;
    signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        path,
        form,
    )
    .await
}

async fn marco_get(world: &mut FoundryWorld, path: &str) -> PostOutcome {
    ensure_harness(world).await;
    signed_in_get(
        harness(world),
        &http(world),
        MARCO_EMAIL,
        MARCO_PASSWORD,
        path,
    )
    .await
}

async fn marco_post(world: &mut FoundryWorld, path: &str, form: &[(&str, &str)]) -> PostOutcome {
    ensure_harness(world).await;
    signed_in_post(
        harness(world),
        &http(world),
        MARCO_EMAIL,
        MARCO_PASSWORD,
        path,
        form,
    )
    .await
}

/// The never-existed path every refusal is compared against, byte for byte.
async fn never_existed(world: &mut FoundryWorld, as_marco: bool) -> PostOutcome {
    let (team, project) = stored_slugs(world, &current_project(world));
    let path = lane_edit_path(&team, &project, "no_such_lane_at_all");
    if as_marco {
        marco_get(world, &path).await
    } else {
        priya_get(world, &path).await
    }
}

async fn machine_bearer(world: &FoundryWorld) -> String {
    let user_id = world.blo_priya_id.expect("Priya seeded");
    let workspace_id = world.blo_workspace_id.expect("workspace seeded");
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
            "blo automation",
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

// ------------------------------------------------------------------ browser

async fn ensure_browser(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    if world.browser.is_none() {
        let client = browser_harness::new_session().await;
        browser_harness::sign_in_through_browser(
            &client,
            harness(world),
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
        )
        .await;
        world.browser = Some(client);
    }
}

fn browser(world: &FoundryWorld) -> &fantoccini::Client {
    world.browser.as_ref().expect("browser session opened")
}

async fn open_board_in_browser(world: &mut FoundryWorld) {
    ensure_browser(world).await;
    let p = current_project(world);
    let path = board_path(world, &p);
    let base = harness(world).base_url();
    browser(world)
        .goto(&format!("{base}{path}"))
        .await
        .expect("navigate to board");
    browser_harness::wait_for_board_ready(browser(world)).await;
}

/// The menu trigger for one lane. Absent until DELIVER builds the markup —
/// which is this suite's honest MISSING_FUNCTIONALITY(markup) RED.
async fn menu_trigger(world: &FoundryWorld, lane: &str) -> fantoccini::elements::Element {
    let selector = format!("button[{}][data-lane=\"{lane}\"]", MENU_TRIGGER);
    browser(world)
        .find(Locator::Css(&selector))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "MISSING_FUNCTIONALITY(markup): the {lane:?} column has no ⋯ menu trigger \
                 ({selector}, component-boundaries.md §1.1). DELIVER slice 01 renders it. \
                 Underlying error: {err}"
            )
        })
}

/// Is THIS lane's menu open? Answered by a script that returns a definite
/// boolean, and a probe that cannot run is a test FAILURE rather than a
/// `false`.
///
/// Visibility is judged by the RECT, never by `offsetParent`. `offsetParent` is
/// `null` for a `position: fixed` element by spec, so the obvious-looking
/// `offsetParent !== null` check reported every menu as closed the moment
/// fix-lane-menu-clipped-mobile made the menu fixed — six scenarios failed at
/// once, none of them because the menu was actually broken.
///
/// The direction matters: this predicate is used in NEGATIVE assertions
/// ("Escape closed the menu"), where swallowing a probe error as `false` would
/// pass the very assertion it failed to evaluate. Fail-closed by construction.
async fn menu_is_open(world: &FoundryWorld, lane: &str) -> bool {
    let script = format!(
        "var m = document.querySelector('[data-lane-menu=\"{lane}\"]'); \
         if (!m) {{ return false; }} \
         var r = m.getBoundingClientRect(); \
         return !m.hidden && r.width > 0 && r.height > 0;"
    );
    browser(world)
        .execute(&script, vec![])
        .await
        .unwrap_or_else(|err| panic!("could not probe the {lane:?} lane menu's open state: {err}"))
        .as_bool()
        .unwrap_or_else(|| {
            panic!("the {lane:?} lane-menu open-state probe did not return a boolean")
        })
}

// =========================================================================
// Given
// =========================================================================

#[given(regex = r"^Priya is a Backend team member shaping her own boards$")]
async fn priya_backend_member(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = pool(world);
    let priya = seed_user(world, PRIYA_EMAIL, "Priya Raman", PRIYA_PASSWORD).await;
    world.blo_priya_id = Some(priya);
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(ws)
        .bind("Canzan Labs")
        .execute(&pool)
        .await
        .expect("insert workspace");
    world.blo_workspace_id = Some(ws);
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
    world.blo_team_id = Some(team);
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

#[given(regex = r"^Marco is signed in and is not a member of team Backend$")]
async fn marco_not_a_member(world: &mut FoundryWorld) {
    let marco = seed_user(world, MARCO_EMAIL, "Marco", MARCO_PASSWORD).await;
    let ws = world.blo_workspace_id.expect("workspace seeded first");
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
    world.blo_marco_id = Some(marco);
}

#[given(regex = r#"^"([^"]+)" \(([A-Z]+)\) is a board with lanes Backlog, In-Progress and Done$"#)]
async fn board_with_three_lanes(world: &mut FoundryWorld, name: String, prefix: String) {
    let slug = name.to_lowercase().replace(' ', "-");
    seed_project(world, &name, &slug, &prefix).await;
    let project_id = project_id_of(world, &name);
    for (idx, (lane_slug, label)) in [
        ("backlog", "Backlog"),
        ("in_progress", "In-Progress"),
        ("done", "Done"),
    ]
    .iter()
    .enumerate()
    {
        seed_lane(world, project_id, lane_slug, label, idx as i32).await;
    }
    world.blo_current_project = Some(name);
}

#[given(regex = r"^OPS-3 and OPS-7 sit in In-Progress$")]
async fn ops3_and_ops7(world: &mut FoundryWorld) {
    seed_issue(world, "Homelab Ops", 3, "Rotate certs", "in_progress", 0).await;
    seed_issue(world, "Homelab Ops", 7, "Patch the NAS", "in_progress", 1).await;
}

#[given(regex = r"^OPS-3 and OPS-7 sit in In-Progress top to bottom$")]
async fn ops3_and_ops7_ordered(world: &mut FoundryWorld) {
    ops3_and_ops7(world).await;
}

#[given(regex = r"^OPS-3 sits in In-Progress$")]
async fn ops3_only(world: &mut FoundryWorld) {
    seed_issue(world, "Homelab Ops", 3, "Rotate certs", "in_progress", 0).await;
    seed_issue(world, "Homelab Ops", 7, "Patch the NAS", "backlog", 0).await;
}

#[given(regex = r"^OPS-3 sits in In-Progress and OPS-9 sits in Done$")]
async fn ops3_and_ops9(world: &mut FoundryWorld) {
    seed_issue(world, "Homelab Ops", 3, "Rotate certs", "in_progress", 0).await;
    seed_issue(world, "Homelab Ops", 9, "Replace UPS battery", "done", 0).await;
}

#[given(regex = r#"^the In-Progress lane has been renamed to "([^"]+)"$"#)]
async fn lane_already_renamed(world: &mut FoundryWorld, new_label: String) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_edit_path(&team, &project, "in_progress");
    let outcome = priya_post(world, &path, &[("label", &new_label)]).await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the rename Given must succeed through the real write port; body = {}",
        outcome.body
    );
}

#[given(regex = r#"^Priya has inserted a lane named "([^"]+)" before In-Progress$"#)]
async fn lane_already_inserted(world: &mut FoundryWorld, label: String) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "in_progress", "before");
    let outcome = priya_post(world, &path, &[("label", &label)]).await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the insert Given must succeed through the real write port; body = {}",
        outcome.body
    );
}

#[given(regex = r"^Priya has opened the In-Progress column's menu$")]
async fn menu_already_open(world: &mut FoundryWorld) {
    open_board_in_browser(world).await;
    let before = browser(world).source().await.expect("board source");
    world.blo_board_before = Some(before);
    let trigger = menu_trigger(world, "in_progress").await;
    trigger.click().await.expect("click the ⋯ trigger");
    assert!(
        menu_is_open(world, "in_progress").await,
        "MISSING_FUNCTIONALITY(markup): activating the ⋯ trigger did not open the In-Progress menu"
    );
}

#[given(regex = r"^Priya has opened the keyboard help overlay over an open lane menu$")]
async fn help_over_menu(world: &mut FoundryWorld) {
    menu_already_open(world).await;
    browser_harness::press_key(browser(world), "?").await;
    // `wait_for_kb_ready` only asserts keyboard.js INITIALISED — it is true
    // before `?` is ever pressed. `openHelp()` loads the overlay with an async
    // `fetch`, so the layered assertion must wait for the overlay's CONTENT to
    // land or it races an empty host and reads "help never opened".
    browser_harness::wait_for_page(
        browser(world),
        "the keyboard help overlay above the lane menu",
        Locator::Css("#kb-overlay-root > *"),
    )
    .await;
    assert!(
        menu_is_open(world, "in_progress").await,
        "precondition: opening the help overlay must not have closed the lane menu"
    );
}

// =========================================================================
// When
// =========================================================================

#[when(regex = r#"^Priya views the "([^"]+)" board$"#)]
async fn priya_views_board(world: &mut FoundryWorld, name: String) {
    world.blo_current_project = Some(name.clone());
    let path = board_path(world, &name);
    let outcome = priya_get(world, &path).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(regex = r"^Priya opens the In-Progress menu and chooses Delete list$")]
async fn menu_choose_delete(world: &mut FoundryWorld) {
    open_board_in_browser(world).await;
    let trigger = menu_trigger(world, "in_progress").await;
    trigger.click().await.expect("click the ⋯ trigger");
    // Scoped to the OPEN menu. An unscoped `//*[@data-lane-menu]//…` matches the
    // same item in EVERY column and returns Backlog's hidden one first —
    // ElementNotInteractable, and a test failure that says nothing about the
    // feature. Caught on the first green run of step 01-02.
    let item = browser(world)
        .find(Locator::XPath(
            "//*[@data-lane-menu and not(@hidden)]//*[normalize-space(text())='Delete list']",
        ))
        .await
        .unwrap_or_else(|err| {
            panic!("MISSING_FUNCTIONALITY(markup): no 'Delete list' item in the menu: {err}")
        });
    item.click().await.expect("choose Delete list");
    // The item's hx-get is ASYNC. Asserting on the page source immediately
    // after the click races the swap — the first run of this step failed for
    // exactly that reason, which would have read as "the menu does not reach
    // the dialog" when the menu was fine.
    browser_harness::wait_for_page(
        browser(world),
        "the delete-lane dialog opened from the ⋯ menu",
        Locator::Css("[data-modal='delete-lane']"),
    )
    .await;
}

#[when(regex = r"^Priya presses Escape$")]
async fn priya_presses_escape(world: &mut FoundryWorld) {
    browser_harness::press_key(browser(world), "Escape").await;
}

#[when(regex = r"^Priya presses Escape once$")]
async fn priya_presses_escape_once(world: &mut FoundryWorld) {
    browser_harness::press_key(browser(world), "Escape").await;
}

#[when(regex = r"^Priya reaches the In-Progress menu trigger by keyboard and activates it$")]
async fn reach_trigger_by_keyboard(world: &mut FoundryWorld) {
    open_board_in_browser(world).await;
    // Assert the trigger exists first, so an absent one fails as
    // MISSING_FUNCTIONALITY(markup) rather than as a silent focus no-op.
    let _ = menu_trigger(world, "in_progress").await;
    // Focus WITHOUT a pointer, then activate from the keyboard. The pointer is
    // deliberately never used here, so a mouse-only menu cannot pass this leg.
    let focus_script = format!(
        "var t = document.querySelector('button[{MENU_TRIGGER}][data-lane=\"in_progress\"]'); \
         if (t) {{ t.focus(); }} return !!t;"
    );
    let found: bool = browser(world)
        .execute(&focus_script, vec![])
        .await
        .expect("focus the trigger")
        .as_bool()
        .unwrap_or(false);
    assert!(
        found,
        "the In-Progress ⋯ trigger must be focusable without a pointer"
    );
    browser_harness::press_key(browser(world), "Enter").await;
}

#[when(regex = r#"^Marco requests every lane route for In-Progress on "([^"]+)" directly$"#)]
async fn marco_requests_lane_routes(world: &mut FoundryWorld, name: String) {
    world.blo_current_project = Some(name.clone());
    // The lane-set-unchanged Then compares against this snapshot.
    capture_universe(world).await;
    let (team, project) = stored_slugs(world, &name);
    let mut seen = Vec::new();
    // Each route gets a WELL-FORMED body for its own contract. A malformed one
    // would be refused at form-parse — before the authz gate runs — and the
    // scenario would then be asserting parse order rather than the D11 promise
    // that a non-member cannot tell a real lane from an absent one. (The
    // shipped delete route answers a fate-less POST with a 422 for ANY
    // signed-in caller, member or not, so it leaks nothing; it simply is not
    // what this scenario is about.)
    for (path, form) in [
        (
            lane_edit_path(&team, &project, "in_progress"),
            vec![("label", "Sneaky")],
        ),
        (
            lane_insert_path(&team, &project, "in_progress", "before"),
            vec![("label", "Sneaky")],
        ),
        (
            lane_delete_path(&team, &project, "in_progress"),
            vec![("fate", "delete")],
        ),
    ] {
        let g = marco_get(world, &path).await;
        seen.push((g.status, g.body));
        let p = marco_post(world, &path, &form).await;
        seen.push((p.status, p.body));
    }
    world.blo_refusals = seen;
}

#[when(regex = r#"^Priya renames the In-Progress lane to "([^"]+)"$"#)]
async fn priya_renames(world: &mut FoundryWorld, new_label: String) {
    capture_universe(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_edit_path(&team, &project, "in_progress");
    let outcome = priya_post(world, &path, &[("label", &new_label)]).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(
    regex = r"^Priya drags OPS-3 within the board and a machine client moves OPS-7 to the renamed lane$"
)]
async fn drag_and_machine_move(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    // The dnd port: the state POST names the lane SLUG, which a rename never
    // touches. If a rename had rewritten identity, this is where it shows.
    let path = format!("/team/{team}/project/{project}/issues/3/state");
    let drag = priya_post(world, &path, &[("state", "in_progress")]).await;
    world.blo_refusals.push((drag.status, drag.body));
    let (status, body) = api_patch_state(world, &p, 7, "in_progress").await;
    world.blo_refusals.push((status, body));
}

#[when(regex = r"^Priya opens the edit dialog for that lane$")]
async fn open_edit_dialog(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_edit_path(&team, &project, "in_progress");
    let outcome = priya_get(world, &path).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(
    regex = r"^Priya submits a rename of In-Progress to each of an empty name, a blank name and a 65-character name$"
)]
async fn rename_bad_names(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_edit_path(&team, &project, "in_progress");
    let long = "x".repeat(65);
    let mut seen = Vec::new();
    for label in ["", "   ", long.as_str()] {
        let outcome = priya_post(world, &path, &[("label", label)]).await;
        seen.push((outcome.status, outcome.body));
    }
    world.blo_refusals = seen;
}

#[when(
    regex = r"^a rename of In-Progress is submitted without the board's matching token, and then with it$"
)]
async fn rename_without_then_with_token(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_edit_path(&team, &project, "in_progress");
    let session =
        establish_session(harness(world), &http(world), PRIYA_EMAIL, PRIYA_PASSWORD).await;
    // Leg 1: a session cookie but NO `_csrf` body field — the middleware must
    // refuse before the handler runs.
    let tokenless = post_with_cookie(
        harness(world),
        &http(world),
        &path,
        &session,
        &[("label", "Sneaky")],
    )
    .await;
    // Leg 2: the SAME request WITH a matching token. This leg is what makes the
    // scenario about the TOKEN rather than about everything being refused.
    let tokened = priya_post(world, &path, &[("label", "Doing")]).await;
    world.blo_refusals = vec![
        (tokenless.status, tokenless.body),
        (tokened.status, tokened.body),
    ];
}

#[when(regex = r#"^Priya renames In-Progress to "Doing" and then renames Backlog to "Doing"$"#)]
async fn rename_both_to_doing(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let mut seen = Vec::new();
    for lane in ["in_progress", "backlog"] {
        let path = lane_edit_path(&team, &project, lane);
        let outcome = priya_post(world, &path, &[("label", "Doing")]).await;
        seen.push((outcome.status, outcome.body));
    }
    world.blo_refusals = seen;
}

#[when(regex = r#"^Priya inserts a lane named "([^"]+)" before In-Progress$"#)]
async fn insert_before_in_progress(world: &mut FoundryWorld, label: String) {
    capture_universe(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "in_progress", "before");
    let outcome = priya_post(world, &path, &[("label", &label)]).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(regex = r#"^Priya inserts a lane named "([^"]+)" after Done$"#)]
async fn insert_after_done(world: &mut FoundryWorld, label: String) {
    capture_universe(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "done", "after");
    let outcome = priya_post(world, &path, &[("label", &label)]).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(regex = r"^Priya moves OPS-3 into Staging and opens its edit dialog$")]
async fn move_into_staging(world: &mut FoundryWorld) {
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let staging = lane_slug_named(world, "Staging").await;
    let path = format!("/team/{team}/project/{project}/issues/3/state");
    let moved = priya_post(world, &path, &[("state", &staging)]).await;
    world.blo_refusals.push((moved.status, moved.body));
    let dialog = priya_get(
        world,
        &format!("/team/{team}/project/{project}/issues/3/edit"),
    )
    .await;
    world.blo_dialog = Some((dialog.status, dialog.body));
}

#[when(regex = r#"^Priya tries to insert a lane named "([^"]+)" before In-Progress$"#)]
async fn insert_colliding(world: &mut FoundryWorld, label: String) {
    capture_universe(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "in_progress", "before");
    let outcome = priya_post(world, &path, &[("label", &label)]).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(regex = r#"^Priya tries to insert a lane named each of "\.\.\.", "!!!" and a blank name$"#)]
async fn insert_unusable_names(world: &mut FoundryWorld) {
    capture_universe(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "in_progress", "before");
    let mut seen = Vec::new();
    for label in ["...", "!!!", "   "] {
        let outcome = priya_post(world, &path, &[("label", label)]).await;
        seen.push((outcome.status, outcome.body));
    }
    world.blo_refusals = seen;
}

#[when(regex = r"^Priya requests an insert dialog with a side that is neither before nor after$")]
async fn insert_bad_side(world: &mut FoundryWorld) {
    capture_universe(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "in_progress", "sideways");
    let outcome = priya_get(world, &path).await;
    world.blo_dialog = Some((outcome.status, outcome.body));
}

#[when(regex = r#"^Marco sends the insert confirm for "([^"]+)" directly$"#)]
async fn marco_inserts(world: &mut FoundryWorld, name: String) {
    world.blo_current_project = Some(name.clone());
    capture_universe(world).await;
    let (team, project) = stored_slugs(world, &name);
    let path = lane_insert_path(&team, &project, "in_progress", "before");
    let g = marco_get(world, &path).await;
    let p = marco_post(world, &path, &[("label", "Sneaky")]).await;
    world.blo_refusals = vec![(g.status, g.body), (p.status, p.body)];
}

#[when(regex = r"^two operators each insert a lane before Done at the same moment$")]
async fn concurrent_inserts(world: &mut FoundryWorld) {
    capture_universe(world).await;
    ensure_harness(world).await;
    let p = current_project(world);
    let (team, project) = stored_slugs(world, &p);
    let path = lane_insert_path(&team, &project, "done", "before");
    let url = path.clone();
    // Two REAL concurrent confirms through the real adapter. Without the
    // FOR UPDATE lock the loser aborts with a raw duplicate-key error — the
    // failure mode measured during the DESIGN spike (adr-board-lane-003).
    let h = harness(world);
    let client = http(world);
    let (a, b) = tokio::join!(
        signed_in_post(
            h,
            &client,
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
            &url,
            &[("label", "Review")],
        ),
        signed_in_post(
            h,
            &client,
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
            &url,
            &[("label", "Staging")],
        )
    );
    world.blo_concurrent = vec![(a.status, a.body), (b.status, b.body)];
}

#[when(regex = r"^the board's columns are refreshed out of band beneath the open menu$")]
async fn oob_refresh_beneath_menu(world: &mut FoundryWorld) {
    // Drive the REAL out-of-band swap the shipped delete confirm produces, by
    // replacing #board-columns' contents the way htmx does. The point is that
    // the menu's DOM node is destroyed underneath the open-state tracker.
    let script = "var host = document.getElementById('board-columns');\
                  if (host) { host.innerHTML = host.innerHTML; }";
    browser(world)
        .execute(script, vec![])
        .await
        .expect("simulate the out-of-band #board-columns refresh");
}

// =========================================================================
// Then
// =========================================================================

fn rendered(world: &FoundryWorld) -> &str {
    let (status, body) = world
        .blo_dialog
        .as_ref()
        .expect("a When must have captured a response");
    assert_eq!(
        *status,
        StatusCode::OK,
        "expected a 200 page; body = {body}"
    );
    body
}

#[then(regex = r"^every column header carries one lane menu trigger$")]
async fn every_header_has_a_trigger(world: &mut FoundryWorld) {
    let body = rendered(world).to_string();
    let columns = column_order(&body);
    assert!(!columns.is_empty(), "the board rendered no columns at all");
    for slug in &columns {
        let slice = column_slice(&body, slug);
        assert!(
            slice.contains(MENU_TRIGGER),
            "MISSING_FUNCTIONALITY(markup): column {slug:?} carries no ⋯ menu trigger \
             ({MENU_TRIGGER}). DELIVER slice 01 renders it."
        );
        assert_eq!(
            slice.matches(MENU_TRIGGER).count(),
            1,
            "column {slug:?} must carry exactly ONE menu trigger"
        );
    }
}

#[then(regex = r"^no column header carries a lane delete control$")]
async fn no_delete_control(world: &mut FoundryWorld) {
    let body = rendered(world);
    assert!(
        !body.contains(OLD_DELETE_MARKER),
        "the armed × delete control ({OLD_DELETE_MARKER}) must be GONE from the board (D3); \
         it is still rendered"
    );
}

#[then(
    regex = r"^the In-Progress column's menu offers exactly Edit list, Insert list before, Insert list after and Delete list$"
)]
async fn menu_offers_four(world: &mut FoundryWorld) {
    let body = rendered(world).to_string();
    let slice = column_slice(&body, "in_progress");
    assert!(
        slice.contains(MENU_CONTAINER),
        "MISSING_FUNCTIONALITY(markup): the In-Progress column has no menu container \
         ({MENU_CONTAINER}…)"
    );
    for item in MENU_ITEMS {
        assert!(
            slice.contains(item),
            "the In-Progress menu is missing the {item:?} item"
        );
    }
    // Order is part of the contract, not an accident.
    let mut cursor = 0usize;
    for item in MENU_ITEMS {
        let at = slice[cursor..]
            .find(item)
            .unwrap_or_else(|| panic!("menu items are out of order at {item:?}"));
        cursor += at + item.len();
    }
}

#[then(regex = r"^the delete-lane dialog opens naming In-Progress and its live count of (\d+)$")]
async fn delete_dialog_opens(world: &mut FoundryWorld, count: i64) {
    let source = browser(world).source().await.expect("page source");
    assert!(
        source.contains(DELETE_MODAL_MARKER),
        "the shipped delete-lane dialog did not open from the menu ({DELETE_MODAL_MARKER})"
    );
    assert!(
        source.contains(&format!("data-lane-count=\"{count}\"")),
        "the dialog must carry the LIVE count of {count}"
    );
    assert!(
        source.contains("In-Progress"),
        "the dialog must name the lane it is about to delete"
    );
}

#[then(regex = r"^the dialog offers both the move fate and the permanent delete fate$")]
async fn dialog_offers_both_fates(world: &mut FoundryWorld) {
    let source = browser(world).source().await.expect("page source");
    assert!(
        source.contains("name=\"fate\" value=\"move\"")
            || source.contains("value=\"move\" name=\"fate\""),
        "the move fate must still be offered — this feature changes how the dialog is REACHED, \
         never what it offers"
    );
    assert!(
        source.contains("name=\"fate\" value=\"delete\"")
            || source.contains("value=\"delete\" name=\"fate\""),
        "the permanent delete fate must still be offered"
    );
}

#[then(regex = r"^the menu is closed and focus has returned to the In-Progress menu trigger$")]
async fn menu_closed_focus_returned(world: &mut FoundryWorld) {
    assert!(
        !menu_is_open(world, "in_progress").await,
        "Escape must close the open lane menu"
    );
    let focused: String = browser(world)
        .execute(
            "return document.activeElement ? \
             (document.activeElement.getAttribute('data-lane') || '') : ''",
            vec![],
        )
        .await
        .expect("read document.activeElement")
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        focused, "in_progress",
        "closing the menu must return focus to the ⋯ trigger it was opened from \
         (ADR-BOARD-LANE-005 rule 4); focus is on {focused:?}"
    );
}

#[then(regex = r"^the board renders exactly as it did before the menu was opened$")]
async fn board_unchanged_by_menu(world: &mut FoundryWorld) {
    let before = world
        .blo_board_before
        .clone()
        .expect("board captured before the menu opened");
    let after = browser(world).source().await.expect("page source");

    // WHAT THIS ORACLE MEANS: opening and closing the menu leaves the board as
    // it was — no menu left on screen, no column or card disturbed.
    //
    // It compares the markup with ONE normalisation: the inline `style` on a
    // `[data-lane-menu]` element is ignored. That attribute is where
    // fix-lane-menu-clipped-mobile writes the fixed-position coordinates, and
    // `closeLaneMenu()` removes it on the way out — verified directly in a
    // browser: after Escape the element has no `style` attribute at all
    // (`hasAttribute("style") === false`).
    //
    // In the headless lane, however, an EMPTY `style=""` survives on the hidden
    // menu. `hasAttribute("style")` reads true immediately after
    // `removeAttribute("style")` returns, which should not be possible; it did
    // not reproduce in a headful Chrome loading the same stylesheet, the same
    // scripts (htmx included) and the same markup. It is recorded as an
    // unexplained headless difference rather than papered over.
    //
    // Normalising it is sound because an EMPTY style attribute on a hidden
    // element cannot change what is rendered — and the assertions below check
    // the things that would actually matter if the residue were ever more than
    // empty: no open menu, and the same columns and cards.
    let normalise = |src: &str| strip_lane_menu_style(src).replace(char::is_whitespace, "");
    assert_eq!(
        normalise(&before),
        normalise(&after),
        "opening and closing the menu must leave the board as it was"
    );

    // The residue must be EMPTY, never a live coordinate: a menu left carrying
    // `left`/`top` would be a real leak, and normalising above would hide it.
    let leaked: serde_json::Value = browser(world)
        .execute(
            "var out = []; \
             var ms = document.querySelectorAll('[data-lane-menu]'); \
             for (var i = 0; i < ms.length; i++) { \
               var v = ms[i].getAttribute('style'); \
               if (v) { out.push({lane: ms[i].getAttribute('data-lane-menu'), style: v}); } \
             } \
             return { leaked: out, \
                      openMenus: document.querySelectorAll('[data-lane-menu]:not([hidden])').length };",
            vec![],
        )
        .await
        .expect("probe for leaked menu positioning");
    assert_eq!(
        leaked["openMenus"], 0,
        "no lane menu may be left open after Escape"
    );
    let leaked_styles = leaked["leaked"].as_array().expect("array");
    assert!(
        leaked_styles.is_empty(),
        "a closed menu must not keep its positioning: {leaked_styles:?}"
    );
}
#[then(
    regex = r"^the menu is open and each of its four items can be reached by keyboard in listed order$"
)]
async fn menu_keyboard_operable(world: &mut FoundryWorld) {
    assert!(
        menu_is_open(world, "in_progress").await,
        "activating the trigger from the keyboard must open the menu (D10)"
    );
    for item in MENU_ITEMS {
        let xpath =
            format!("//*[@data-lane-menu='in_progress']//*[normalize-space(text())='{item}']");
        browser(world)
            .find(Locator::XPath(&xpath))
            .await
            .unwrap_or_else(|err| panic!("menu item {item:?} not present: {err}"));
        // Reachability is asked of the DOM by TEXT, so this probe does not
        // depend on a class or id DELIVER has not chosen yet.
        let probe = format!(
            "var items = document.querySelectorAll(\
               '[data-lane-menu=\"in_progress\"] a, [data-lane-menu=\"in_progress\"] button'); \
             for (var i = 0; i < items.length; i++) {{ \
               if (items[i].textContent.trim() === {item:?}) {{ \
                 var e = items[i]; \
                 var r = e.getBoundingClientRect(); \
                 return !e.disabled && e.tabIndex > -1 && r.width > 0 && r.height > 0; \
               }} }} \
             return false;"
        );
        let reachable: bool = browser(world)
            .execute(&probe, vec![])
            .await
            .expect("probe keyboard reachability")
            .as_bool()
            .unwrap_or(false);
        assert!(
            reachable,
            "menu item {item:?} is not keyboard-reachable (D10 — full menu semantics)"
        );
    }
}

#[then(regex = r"^each answer is byte-identical to a never-existed path, on both verbs$")]
async fn refusals_byte_identical_both_verbs(world: &mut FoundryWorld) {
    let baseline = never_existed(world, true).await;
    let seen = world.blo_refusals.clone();
    assert!(!seen.is_empty(), "the When recorded no responses");
    for (status, body) in seen {
        assert_eq!(
            status, baseline.status,
            "a refusal's STATUS must match a never-existed path exactly"
        );
        assert_eq!(
            body, baseline.body,
            "a refusal's BODY must be byte-identical to a never-existed path — otherwise the \
             pair is an enumeration oracle for which lanes exist"
        );
    }
}

#[then(regex = r"^the answer is byte-identical to a never-existed path, on both verbs$")]
async fn answer_byte_identical_both_verbs(world: &mut FoundryWorld) {
    refusals_byte_identical_both_verbs(world).await;
}

#[then(regex = r"^the answer is byte-identical to a never-existed path$")]
async fn answer_byte_identical(world: &mut FoundryWorld) {
    let (status, body) = world
        .blo_dialog
        .clone()
        .expect("a When captured a response");
    let baseline = never_existed(world, false).await;
    assert_eq!(
        status, baseline.status,
        "an unrecognised insert side must answer exactly as an unknown lane does — never a 400 \
         (DD6), or the pair enumerates which lanes a project has"
    );
    assert_eq!(
        body, baseline.body,
        "the refusal body must match byte for byte"
    );
}

#[then(regex = r#"^the "([^"]+)" lane set is unchanged$"#)]
async fn lane_set_unchanged(world: &mut FoundryWorld, name: String) {
    let before = world
        .blo_lanes_before
        .clone()
        .unwrap_or_else(|| panic!("the universe must be captured before the write"));
    let project_id = project_id_of(world, &name);
    let after = lanes_of(&pool(world), project_id).await;
    assert_eq!(
        before, after,
        "a refused operation must leave the lane set byte-identical"
    );
}

#[then(regex = r"^the column header reads Doing$")]
async fn header_reads_doing(world: &mut FoundryWorld) {
    let p = current_project(world);
    let path = board_path(world, &p);
    let outcome = priya_get(world, &path).await;
    let slice = column_slice(&outcome.body, "in_progress");
    assert!(
        slice.contains("Doing"),
        "the renamed lane's header must read Doing; column slice = {slice}"
    );
}

#[then(regex = r#"^the column header reads "([^"]+)"$"#)]
async fn header_reads_quoted(world: &mut FoundryWorld, label: String) {
    let p = current_project(world);
    let path = board_path(world, &p);
    let outcome = priya_get(world, &path).await;
    assert!(
        outcome.body.contains(&label),
        "the board must render the lane label {label:?} verbatim"
    );
}

#[then(regex = r"^OPS-3 and OPS-7 sit in that same column at the same positions$")]
async fn cards_unmoved_by_rename(world: &mut FoundryWorld) {
    let p = current_project(world);
    let before = world.blo_issues_before.clone().expect("universe captured");
    let after = issues_of(world, &p).await;
    assert_eq!(
        before, after,
        "a rename must leave every card in the same lane at the same position"
    );
}

#[then(regex = r"^no issue row and no change event was written$")]
async fn no_issue_writes(world: &mut FoundryWorld) {
    assert_no_issue_writes(world).await;
}

#[then(regex = r"^both succeed against the lane slug in_progress$")]
async fn both_succeed_against_slug(world: &mut FoundryWorld) {
    let seen = world.blo_refusals.clone();
    assert_eq!(seen.len(), 2, "expected the drag and the machine PATCH");
    for (status, body) in seen {
        assert!(
            status.is_success(),
            "a rename must not disturb the lane SLUG that dnd and the API both name; got \
             {status} / {body}"
        );
    }
}

#[then(
    regex = r"^every lane slug, every lane position and every issue key is unchanged by the rename$"
)]
async fn identity_untouched(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let slugs: Vec<&String> = lanes.iter().map(|(s, _, _)| s).collect();
    assert!(
        slugs.contains(&&"in_progress".to_string()),
        "the renamed lane must KEEP its slug in_progress (brief.md §lanes: slugs are identity, \
         labels are display); slugs are {slugs:?}"
    );
    assert_contiguous(&lanes);
    assert_zero_laneless(world).await;
}

#[then(regex = r"^the dialog's name field contains Doing$")]
async fn dialog_prefilled(world: &mut FoundryWorld) {
    let body = rendered(world);
    assert!(
        body.contains("value=\"Doing\""),
        "the edit dialog must pre-fill the lane's CURRENT label; body = {body}"
    );
}

#[then(regex = r"^the dialog carries the board's matching token and a declarative close trigger$")]
async fn dialog_has_csrf_and_close(world: &mut FoundryWorld) {
    let body = rendered(world);
    assert!(
        body.contains("name=\"_csrf\""),
        "the dialog's confirm form must carry the double-submit `_csrf` field"
    );
    assert!(
        body.contains(CLOSE_TRIGGER),
        "close is the declarative {CLOSE_TRIGGER} trigger ONLY — never a new listener (BR-4)"
    );
}

#[then(regex = r"^each is refused with a reason rendered into the dialog's error slot$")]
async fn each_refused_into_slot(world: &mut FoundryWorld) {
    let seen = world.blo_refusals.clone();
    assert!(!seen.is_empty(), "the When recorded no submissions");
    for (status, body) in seen {
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "a validation refusal is a 422 (the form-errors.js contract); body = {body}"
        );
        assert!(
            body.contains(ERROR_SLOT) || body.contains("data-hx-fragment"),
            "the refusal must be the bare fragment form-errors.js routes into \
             [{ERROR_SLOT}]; body = {body}"
        );
    }
}

#[then(regex = r"^each is refused with a reason asking for letters or numbers$")]
async fn refused_asking_for_letters(world: &mut FoundryWorld) {
    let seen = world.blo_refusals.clone();
    assert_eq!(seen.len(), 3, "expected three unusable names");
    for (status, body) in seen {
        assert_eq!(
            status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "a name with no usable characters is a 422 refusal (D7), never a 500 and never a \
             silently-invented slug; body = {body}"
        );
        let lowered = body.to_lowercase();
        assert!(
            lowered.contains("letter") || lowered.contains("number"),
            "the refusal must ask for letters or numbers; body = {body}"
        );
    }
}

#[then(regex = r"^the In-Progress lane still carries its original label$")]
async fn label_unchanged(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let (_, label, _) = lanes
        .iter()
        .find(|(s, _, _)| s == "in_progress")
        .expect("the In-Progress lane must still exist");
    assert_eq!(
        label, "In-Progress",
        "a refused rename must leave the label untouched"
    );
}

#[then(regex = r"^the tokenless rename was refused before the handler ran$")]
async fn tokenless_refused_pre_handler(world: &mut FoundryWorld) {
    let (status, body) = world
        .blo_refusals
        .first()
        .cloned()
        .expect("the tokenless leg");
    assert!(
        status == StatusCode::FORBIDDEN || status == StatusCode::BAD_REQUEST,
        "a tokenless mutating POST is refused by the CSRF middleware BEFORE the handler — which \
         is only true if the route is mounted UNDER csrf_middleware; got {status} / {body}"
    );
    assert_ne!(
        status,
        StatusCode::NOT_IMPLEMENTED,
        "reaching the 501 scaffold means the CSRF middleware did NOT refuse first"
    );
}

#[then(regex = r"^the same rename carrying the token is accepted and takes effect$")]
async fn tokened_rename_accepted(world: &mut FoundryWorld) {
    let (status, body) = world.blo_refusals.get(1).cloned().expect("the tokened leg");
    assert!(
        status.is_success(),
        "the SAME rename carrying a matching token must be accepted — otherwise the refusal \
         above proves nothing about the token. This leg exists because the DISTILL \
         classification run caught the one-legged version passing over a feature that did not \
         exist: the shipped CSRF middleware refuses a tokenless POST to ANY mounted route. \
         Got {status} / {body}"
    );
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let (_, label, _) = lanes
        .iter()
        .find(|(s, _, _)| s == "in_progress")
        .expect("the In-Progress lane must still exist");
    assert_eq!(
        label, "Doing",
        "the tokened rename must actually take effect"
    );
}

#[then(regex = r"^both renames succeed and two columns read Doing$")]
async fn both_renames_succeed(world: &mut FoundryWorld) {
    for (status, body) in world.blo_refusals.clone() {
        assert!(
            status.is_success(),
            "duplicate LABELS are permitted — only slugs are unique (AC-2.6); got {status} / {body}"
        );
    }
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let doing = lanes.iter().filter(|(_, l, _)| l == "Doing").count();
    assert_eq!(doing, 2, "two lanes must be able to carry the same label");
}

#[then(regex = r"^the two lanes still carry their distinct slugs$")]
async fn distinct_slugs(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let slugs: Vec<&String> = lanes.iter().map(|(s, _, _)| s).collect();
    let mut deduped = slugs.clone();
    deduped.sort();
    deduped.dedup();
    assert_eq!(
        slugs.len(),
        deduped.len(),
        "labels may collide; slugs may NOT — they are identity"
    );
}

async fn assert_lane_labels_in_order(world: &mut FoundryWorld, expected: &[&str]) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let labels: Vec<String> = lanes.iter().map(|(_, l, _)| l.clone()).collect();
    assert_eq!(
        labels,
        expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "the board's lanes must read in exactly this order"
    );
    // And the RENDERED board must agree with the store — otherwise a static
    // list somewhere is still driving the columns.
    let path = board_path(world, &p);
    let outcome = priya_get(world, &path).await;
    let rendered_slugs = column_order(&outcome.body);
    let stored_slugs_in_order: Vec<String> = lanes.iter().map(|(s, _, _)| s.clone()).collect();
    assert_eq!(
        rendered_slugs, stored_slugs_in_order,
        "the rendered columns must be exactly the stored lane rows, in position order"
    );
}

#[then(regex = r"^the board's lanes read Backlog, Staging, In-Progress, Done in that order$")]
async fn lanes_read_with_staging(world: &mut FoundryWorld) {
    assert_lane_labels_in_order(world, &["Backlog", "Staging", "In-Progress", "Done"]).await;
}

#[then(regex = r"^the board's lanes read Backlog, In-Progress, Done, Archive Box in that order$")]
async fn lanes_read_with_archive(world: &mut FoundryWorld) {
    assert_lane_labels_in_order(world, &["Backlog", "In-Progress", "Done", "Archive Box"]).await;
}

#[then(regex = r"^the Staging column is empty and no existing card has moved$")]
async fn staging_empty_nothing_moved(world: &mut FoundryWorld) {
    let p = current_project(world);
    let staging = lane_slug_named(world, "Staging").await;
    let after = issues_of(world, &p).await;
    assert!(
        after.iter().all(|(_, state, _)| *state != staging),
        "a freshly inserted lane must land EMPTY"
    );
    let before = world.blo_issues_before.clone().expect("universe captured");
    assert_eq!(
        before, after,
        "no existing card may move when a lane is inserted"
    );
}

#[then(regex = r"^the lane positions are contiguous from zero and unique$")]
async fn positions_contiguous(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    assert_contiguous(&lanes);
}

#[then(regex = r"^a newly filed issue still lands in the leftmost lane$")]
async fn new_issue_lands_leftmost(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    // Leftmost is DERIVED from the rows (position ASC), never assumed to be
    // "backlog" — the false-GREEN the predecessor wave caught and fixed.
    let (leftmost, _, _) = lanes.first().expect("a board keeps at least one lane");
    let (team, project) = stored_slugs(world, &p);
    let bearer = machine_bearer(world).await;
    let base = harness(world).base_url();
    let resp = http(world)
        .post(format!(
            "{base}/api/v1/teams/{team}/projects/{project}/issues"
        ))
        .bearer_auth(bearer)
        .json(&serde_json::json!({ "title": "Landing probe" }))
        .send()
        .await
        .expect("file an issue");
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains(&format!("\"state\":\"{leftmost}\"")),
        "a newly filed issue must land in the leftmost lane ({leftmost:?}); reply = {body}"
    );
}

#[then(regex = r"^the move succeeds and Staging appears among the dialog's Status options$")]
async fn move_and_options(world: &mut FoundryWorld) {
    let (status, body) = world
        .blo_refusals
        .last()
        .cloned()
        .expect("the move was recorded");
    assert!(
        status.is_success(),
        "moving a card into a freshly inserted lane must succeed; got {status} / {body}"
    );
    let dialog = rendered(world);
    assert!(
        dialog.contains("Staging"),
        "an inserted lane must appear among the edit dialog's Status options immediately"
    );
}

#[then(regex = r"^a machine client may move an issue to the Staging lane's slug$")]
async fn machine_move_to_staging(world: &mut FoundryWorld) {
    let p = current_project(world);
    let staging = lane_slug_named(world, "Staging").await;
    let (status, body) = api_patch_state(world, &p, 3, &staging).await;
    assert!(
        status.is_success(),
        "the /api/v1 validation must accept a freshly inserted lane's slug; got {status} / {body}"
    );
}

#[then(regex = r"^a machine client may move an issue to that lane's slug$")]
async fn machine_move_to_new_lane(world: &mut FoundryWorld) {
    let p = current_project(world);
    let slug = lane_slug_named(world, "2024 Review").await;
    let (status, body) = api_patch_state(world, &p, 3, &slug).await;
    assert!(
        status.is_success(),
        "the /api/v1 validation must accept the inserted lane's slug {slug:?}; got {status} / {body}"
    );
}

#[then(regex = r"^every issue still has a lane its board renders$")]
async fn every_issue_has_a_lane(world: &mut FoundryWorld) {
    assert_zero_laneless(world).await;
}

#[then(regex = r"^the refusal names the conflict and renders into the dialog's error slot$")]
async fn conflict_named(world: &mut FoundryWorld) {
    let (status, body) = world
        .blo_dialog
        .clone()
        .expect("a When captured a response");
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "a slug collision is a 422 refusal, never a raw duplicate-key 500 (D7 / DD8); \
         body = {body}"
    );
    assert!(
        !body.to_lowercase().contains("duplicate key"),
        "the raw Postgres duplicate-key error must NEVER reach the operator — it is pre-checked \
         inside the insert's lock (DD8); body = {body}"
    );
    assert!(
        body.contains("Done"),
        "the refusal must name the conflicting lane; body = {body}"
    );
}

#[then(regex = r"^the lane's slug satisfies the lane slug rule$")]
async fn slug_satisfies_rule(world: &mut FoundryWorld) {
    let slug = lane_slug_named(world, "2024 Review").await;
    let bytes = slug.as_bytes();
    assert!(
        !slug.is_empty() && bytes[0].is_ascii_lowercase(),
        "a lane slug must be letter-anchored (`^[a-z]`, the lanes_slug_check CHECK); got {slug:?}"
    );
    assert!(
        slug.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
        "a lane slug may hold only [a-z0-9_] — note UNDERSCORES, not the hyphens \
         foundry_core::slugify emits (adr-board-lane-004); got {slug:?}"
    );
}

#[then(regex = r"^both lanes exist, neither operator saw a database error$")]
async fn both_concurrent_inserts_landed(world: &mut FoundryWorld) {
    let seen = world.blo_concurrent.clone();
    assert_eq!(seen.len(), 2, "two concurrent confirms were driven");
    for (status, body) in &seen {
        assert!(
            status.is_success(),
            "BOTH concurrent inserts must commit. Without the FOR UPDATE lock the loser aborts \
             with a raw duplicate-key error — the failure measured during the DESIGN spike \
             (adr-board-lane-003). Got {status} / {body}"
        );
        assert!(
            !body.to_lowercase().contains("duplicate key"),
            "no operator may ever see a raw Postgres constraint error; body = {body}"
        );
    }
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    let labels: Vec<&String> = lanes.iter().map(|(_, l, _)| l).collect();
    assert!(
        labels.contains(&&"Review".to_string()) && labels.contains(&&"Staging".to_string()),
        "both inserted lanes must exist; labels are {labels:?}"
    );
}

#[then(regex = r"^Escape is not a silent no-op and no menu is left on screen$")]
async fn escape_not_silent_noop(world: &mut FoundryWorld) {
    assert!(
        !menu_is_open(world, "in_progress").await,
        "ADR-BOARD-LANE-005 rule 2: menu-open state must be DOM-DERIVED. A stored handle is left \
         DETACHED by the out-of-band #board-columns swap, and Escape then no-ops while a menu is \
         still on screen. A menu is on screen."
    );
    let stray: bool = browser(world)
        .execute(
            "return !!document.querySelector('[data-lane-menu]:not([hidden])');",
            vec![],
        )
        .await
        .expect("probe for a stray open menu")
        .as_bool()
        .unwrap_or(true);
    assert!(!stray, "no lane menu may be left visible after Escape");
}

#[then(regex = r"^only the help overlay has closed and the lane menu is still open$")]
async fn only_help_closed(world: &mut FoundryWorld) {
    let help_open: bool = browser(world)
        .execute(
            "var h = document.getElementById('kb-overlay-root'); \
             return !!h && h.childElementCount > 0;",
            vec![],
        )
        .await
        .expect("probe the help overlay")
        .as_bool()
        .unwrap_or(true);
    assert!(
        !help_open,
        "one Escape must close the TOP layer — the help overlay"
    );
    assert!(
        menu_is_open(world, "in_progress").await,
        "BR-4: exactly ONE layer peels per press. The lane menu must still be open after the \
         first Escape; if both closed, a second Escape listener is racing closeTopLayer()."
    );
}

#[then(regex = r"^a second Escape closes the lane menu and leaves the board alone$")]
async fn second_escape_closes_menu(world: &mut FoundryWorld) {
    browser_harness::press_key(browser(world), "Escape").await;
    assert!(
        !menu_is_open(world, "in_progress").await,
        "the second Escape must peel the lane menu"
    );
    let columns_intact: bool = browser(world)
        .execute("return !!document.getElementById('board-columns');", vec![])
        .await
        .expect("probe the board")
        .as_bool()
        .unwrap_or(false);
    assert!(
        columns_intact,
        "Escape must never navigate away or tear down the board (ADR-003 §2: an empty stack is a \
         no-op)"
    );
    let _ = OOB_BOARD_MARKER;
}

/// Resolve a lane's slug BY ITS LABEL, from the store — never guessed. The
/// slug an insert mints is DELIVER's to choose within the rule; this suite
/// asserts the rule, not a particular spelling.
async fn lane_slug_named(world: &FoundryWorld, label: &str) -> String {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let lanes = lanes_of(&pool(world), project_id).await;
    lanes
        .iter()
        .find(|(_, l, _)| l == label)
        .map(|(s, _, _)| s.clone())
        .unwrap_or_else(|| {
            panic!(
                "no lane labelled {label:?} exists on {p:?}; lanes are {:?}",
                lanes
            )
        })
}

// =========================================================================
// REGRESSION — fix-lane-menu-clipped-mobile
//
// Why these steps use HIT TESTING rather than `is_displayed()`: a menu clipped
// by an ancestor's `overflow` still reports as displayed and still has a
// non-zero bounding rect. The only thing it loses is a point the operator can
// touch. `document.elementFromPoint` at each item's centre is therefore the
// only oracle that distinguishes "rendered" from "reachable" — the exact
// distinction this defect turned on.
// =========================================================================

/// The phone. Uses the SHIPPED `open_mobile_session()` (pwa-mobile-rendering,
/// ADR-003): real chromedriver mobileEmulation, not a narrowed desktop window —
/// a narrow window would not reproduce the defect's own preconditions.
#[given(regex = r"^Priya is holding a phone$")]
async fn priya_on_a_phone(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let client = browser_harness::open_mobile_session().await;
    browser_harness::sign_in_through_browser(&client, harness(world), PRIYA_EMAIL, PRIYA_PASSWORD)
        .await;
    world.browser = Some(client);
}

#[when(regex = r"^Priya opens the In-Progress column's menu on the phone$")]
async fn open_menu_on_phone(world: &mut FoundryWorld) {
    let p = current_project(world);
    let path = board_path(world, &p);
    let base = harness(world).base_url();
    browser(world)
        .goto(&format!("{base}{path}"))
        .await
        .expect("navigate to board on the phone");
    browser_harness::wait_for_board_ready(browser(world)).await;
    let trigger = menu_trigger(world, "in_progress").await;
    trigger.click().await.expect("open the ⋯ menu on the phone");
    assert!(
        menu_is_open(world, "in_progress").await,
        "the menu must open on a phone before its reachability can be judged"
    );
}

#[when(regex = r"^Priya views the board on the phone without touching anything$")]
async fn view_board_on_phone(world: &mut FoundryWorld) {
    let p = current_project(world);
    let path = board_path(world, &p);
    let base = harness(world).base_url();
    browser(world)
        .goto(&format!("{base}{path}"))
        .await
        .expect("navigate to board on the phone");
    browser_harness::wait_for_board_ready(browser(world)).await;
}

#[then(regex = r"^every one of the six items can be touched, including the last$")]
async fn every_item_touchable(world: &mut FoundryWorld) {
    let report: serde_json::Value = browser(world)
        .execute(
            "var menu = document.querySelector('[data-lane-menu=\"in_progress\"]'); \
             if (!menu) { return {error: 'no menu'}; } \
             var items = menu.querySelectorAll('[role=\"menuitem\"]'); \
             var out = []; \
             for (var i = 0; i < items.length; i++) { \
               var r = items[i].getBoundingClientRect(); \
               var hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); \
               out.push({ label: items[i].textContent.trim(), \
                          reachable: !!(hit && menu.contains(hit)), \
                          hit: hit ? (hit.textContent || hit.tagName).trim().slice(0, 30) : 'NOTHING' }); \
             } \
             var b = document.getElementById('board-columns'); \
             return { items: out, boardOverflowX: getComputedStyle(b).overflowX, \
                      viewport: window.innerWidth }; ",
            vec![],
        )
        .await
        .expect("probe menu-item reachability");
    let items = report["items"]
        .as_array()
        .unwrap_or_else(|| panic!("probe returned no items: {report}"));
    // SIX since board-lane-reorder D5 added Move list left / Move list right
    // between the Insert pair and Delete list. This scenario is a HIT TEST, so
    // the count matters: it is the guard that no item was silently dropped
    // before the reachability check ran. Re-premised deliberately, the way
    // board-lane-overflow-menu D13 re-premised the two lane-delete scenarios it
    // displaced — the menu's contract changed, the scenario did not weaken.
    assert_eq!(
        items.len(),
        6,
        "the menu must offer all six operations; probe = {report}"
    );
    let unreachable: Vec<String> = items
        .iter()
        .filter(|i| i["reachable"] != serde_json::Value::Bool(true))
        .map(|i| format!("{} (point hits {})", i["label"], i["hit"]))
        .collect();
    assert!(
        unreachable.is_empty(),
        "every menu item must be TOUCHABLE on a phone, not merely rendered. Unreachable: {:?}. \
         `.board` overflow-x is {:?} at viewport {}px — an ancestor with non-visible overflow \
         clips absolutely-positioned descendants, which is what put these items out of reach.",
        unreachable,
        report["boardOverflowX"],
        report["viewport"]
    );
}

#[then(regex = r"^each menu trigger carries a visible edge before it is hovered or focused$")]
async fn trigger_visible_at_rest(world: &mut FoundryWorld) {
    let report: serde_json::Value = browser(world)
        .execute(
            "var out = []; \
             var ts = document.querySelectorAll('[data-action=\"toggle-lane-menu\"]'); \
             for (var i = 0; i < ts.length; i++) { \
               var cs = getComputedStyle(ts[i]); \
               out.push({ lane: ts[i].getAttribute('data-lane'), \
                          borderColor: cs.borderTopColor, borderWidth: cs.borderTopWidth, \
                          background: cs.backgroundColor }); \
             } return out;",
            vec![],
        )
        .await
        .expect("probe trigger resting style");
    let triggers = report.as_array().expect("probe returned an array");
    assert!(!triggers.is_empty(), "the board rendered no menu triggers");
    let invisible: Vec<String> = triggers
        .iter()
        .filter(|t| {
            let border = t["borderColor"].as_str().unwrap_or("");
            let bg = t["background"].as_str().unwrap_or("");
            let transparent = |c: &str| c.contains("rgba(0, 0, 0, 0)") || c == "transparent";
            transparent(border) && transparent(bg)
        })
        .map(|t| format!("{}", t["lane"]))
        .collect();
    assert!(
        invisible.is_empty(),
        "a menu trigger must read as pressable AT REST. A phone has no hover, so an affordance \
         that only appears on :hover is invisible to the operator who needs it. Triggers with \
         both a transparent border and a transparent background: {invisible:?}"
    );
}

#[then(regex = r"^each menu trigger is at least 44 by 44$")]
async fn trigger_meets_touch_target(world: &mut FoundryWorld) {
    let report: serde_json::Value = browser(world)
        .execute(
            "var out = []; \
             var ts = document.querySelectorAll('[data-action=\"toggle-lane-menu\"]'); \
             for (var i = 0; i < ts.length; i++) { \
               var r = ts[i].getBoundingClientRect(); \
               out.push({ lane: ts[i].getAttribute('data-lane'), \
                          w: Math.round(r.width), h: Math.round(r.height) }); \
             } return out;",
            vec![],
        )
        .await
        .expect("probe trigger size");
    let small: Vec<String> = report
        .as_array()
        .expect("array")
        .iter()
        .filter(|t| t["w"].as_i64().unwrap_or(0) < 44 || t["h"].as_i64().unwrap_or(0) < 44)
        .map(|t| format!("{}: {}x{}", t["lane"], t["w"], t["h"]))
        .collect();
    assert!(
        small.is_empty(),
        "the project's own mobile rule gives every other control a 44px thumb target; the lane \
         menu trigger must meet the same floor. Undersized: {small:?}"
    );
}

#[then(regex = r"^no menu trigger overlaps the first card in its column$")]
async fn trigger_does_not_overlap_card(world: &mut FoundryWorld) {
    let report: serde_json::Value = browser(world)
        .execute(
            "var out = []; \
             var cols = document.querySelectorAll('[data-column]'); \
             for (var i = 0; i < cols.length; i++) { \
               var t = cols[i].querySelector('[data-action=\"toggle-lane-menu\"]'); \
               var card = cols[i].querySelector('.issue-card'); \
               if (!t || !card) { continue; } \
               var a = t.getBoundingClientRect(), b = card.getBoundingClientRect(); \
               var y = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top); \
               var x = Math.min(a.right, b.right) - Math.max(a.left, b.left); \
               if (y > 0 && x > 0) { \
                 out.push({ lane: cols[i].getAttribute('data-column'), \
                            overlapY: Math.round(y), overlapX: Math.round(x) }); \
               } \
             } return out;",
            vec![],
        )
        .await
        .expect("probe trigger/card overlap");
    let overlaps = report.as_array().expect("array");
    assert!(
        overlaps.is_empty(),
        "the trigger must sit in its own header band, never on top of a card — an operator \
         reaching for the first issue must not hit the lane menu instead. Overlaps: {overlaps:?}"
    );
}

/// Remove the inline `style` attribute from every `[data-lane-menu]` tag, and
/// nothing else. Written by hand rather than with a regex because
/// `foundry-acceptance` does not depend on `regex`, and one attribute on one
/// element does not justify a new dependency in the test crate.
fn strip_lane_menu_style(src: &str) -> String {
    const TAG: &str = "<div class=\"lane-menu\"";
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(at) = rest.find(TAG) {
        // Everything up to and including the tag name.
        out.push_str(&rest[..at + TAG.len()]);
        rest = &rest[at + TAG.len()..];
        // The remainder of THIS tag, up to its closing `>`.
        let close = match rest.find('>') {
            Some(i) => i,
            None => break, // malformed; leave the tail untouched
        };
        let (tag_rest, after) = rest.split_at(close);
        match tag_rest.find(" style=\"") {
            Some(sat) => {
                out.push_str(&tag_rest[..sat]);
                // Skip to the closing quote of the style value.
                let value_start = sat + " style=\"".len();
                match tag_rest[value_start..].find('"') {
                    Some(qend) => out.push_str(&tag_rest[value_start + qend + 1..]),
                    None => out.push_str(&tag_rest[sat..]),
                }
            }
            None => out.push_str(tag_rest),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod strip_lane_menu_style_tests {
    use super::strip_lane_menu_style;

    #[test]
    fn removes_only_the_lane_menu_style_attribute() {
        let src = r#"<div class="lane-menu" data-lane-menu="x" style="left: 1px;" hidden><b style="color:red">k</b></div>"#;
        let out = strip_lane_menu_style(src);
        assert!(
            !out.contains(r#"style="left: 1px;""#),
            "the menu's own style must go: {out}"
        );
        assert!(
            out.contains(r#"style="color:red""#),
            "a child's style must survive: {out}"
        );
        assert!(
            out.contains("hidden"),
            "other attributes must survive: {out}"
        );
        assert!(out.contains(r#"data-lane-menu="x""#));
    }

    #[test]
    fn leaves_markup_without_a_style_attribute_untouched() {
        let src = r#"<div class="lane-menu" data-lane-menu="x" hidden></div>"#;
        assert_eq!(strip_lane_menu_style(src), src);
    }

    #[test]
    fn handles_several_menus() {
        let src =
            r#"<div class="lane-menu" a style="1"></div><div class="lane-menu" b style="2"></div>"#;
        let out = strip_lane_menu_style(src);
        assert!(!out.contains("style="), "both must be stripped: {out}");
        assert!(out.contains(" a") && out.contains(" b"));
    }
}
