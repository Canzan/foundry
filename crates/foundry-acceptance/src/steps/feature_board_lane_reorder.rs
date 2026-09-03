//! board-lane-reorder step definitions
//! (`tests/features/board-lane-reorder.feature`, 25 scenarios, ALL `@pending` —
//! scaffolded RED per ADR-025; DELIVER un-pends one at a time and never
//! re-authors).
//!
//! The production seams these steps drive are the DESIGN port signatures
//! (feature-delta DDD-5/DDD-8): `foundry_store::lanes::move_lane_before` +
//! `LaneMoveOutcome`, `foundry_services::lanes::move_lane` + `MoveLaneError`,
//! and the mounted `foundry_app::lanes::submit_move_lane` handler (answering a
//! clean 501 until DELIVER). The `lanes` table, the `⋯` menu and the OOB
//! `#board-columns` refresh are all SHIPPED, so lane-seeding Givens succeed.
//! The RED comes from two honest places: the two Move menu items and the drag
//! surface do not exist yet (MISSING_FUNCTIONALITY(markup)), and the write port
//! is a panicking scaffold behind a 501 handler (MISSING_FUNCTIONALITY(port)).
//!
//! THE CONCURRENCY ORACLE ASSERTS ORDER, NEVER "no error was raised" (DDD-4).
//! This is the module's most important rule and it inverts the predecessor's.
//! For INSERT, the unguarded race hands the loser a raw Postgres duplicate-key
//! error, so "neither operator saw a database error" is a real oracle. For
//! MOVE it is worthless: ADR-BOARD-LANE-006 Finding 4 measured that two
//! unlocked concurrent moves raise NO error, keep contiguity, keep uniqueness,
//! keep zero laneless issues — and still leave a lane neither operator
//! mentioned shoved past another. A status-code assertion is GREEN on that
//! case. So is [`assert_contiguous`]. Only the resulting ORDER separates them.
//!
//! The concurrency scenario's expected order is deliberately chosen to be
//! SERIALISATION-INDEPENDENT: "In-Progress before Done" and "Staging before
//! Backlog" commute, so A-then-B and B-then-A both yield
//! Staging, Backlog, In-Progress, Done — while the measured stale-read result
//! is Staging, In-Progress, Backlog, Done. The oracle is therefore
//! deterministic AND discriminating, which a race oracle usually is not.
//!
//! THE LANE-LIST ORACLE RULE: every lane expectation reads lane rows BACK FROM
//! THE DATABASE (`lanes`: slug, label, position). This module holds NO static
//! expected-lane list — one would go green over exactly the static-list
//! consumers the `check-arch` rule exists to forbid. The `Then the board reads
//! …` step maps LABELS from the feature file onto rows read from the store.
//!
//! THE IDENTITY RULE: a move must leave every lane's `slug` AND `label`
//! byte-identical; only `position` may differ. Asserted from the STORE, since a
//! DOM-only assertion would pass over a move that also rewrote a slug — which
//! would silently re-home every issue in the lane under `fk_issues_lane`.
//!
//! LAYER 3 (real adapter + real HTTP, `@real-io`): real Postgres via the shared
//! testcontainer + per-scenario schema; the real tower-sessions store; the real
//! double-submit CSRF middleware; the in-process axum router. Example-based
//! (Mandates 9 + 11). State-mutation assertions follow the state-delta
//! discipline: snapshot the declared universe before the write, snapshot after,
//! assert the delta fail-closed. A move must write ZERO issue rows, ZERO change
//! events and ZERO outbox rows.
//!
//! The eleven `@needs-browser` scenarios drive a REAL headless Chrome
//! (fantoccini, `support::browser_harness`) because the HTTP lane is byte-blind
//! to a pointer drag, to the movement threshold that keeps `⋯` clickable, to
//! `Escape` reaching `keyboard.js::closeTopLayer()` mid-drag, and to the board
//! auto-scrolling under a held pointer.

use crate::support::browser_harness;
use crate::support::harness::{
    establish_session, post_with_cookie, signed_in_get, signed_in_post, InProcHarness, PostOutcome,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::StatusCode;
use secrecy::SecretString;
use sqlx::PgPool;

const TEST_NOW: &str = "2026-03-01T12:00:00Z";
const PRIYA_EMAIL: &str = "priya.blr@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const MARCO_EMAIL: &str = "marco.blr@canzan.test";
const MARCO_PASSWORD: &str = "marco-correct-horse-battery-staple";
const NADIA_EMAIL: &str = "nadia.blr@canzan.test";
const NADIA_PASSWORD: &str = "nadia-correct-horse-battery-staple";

// --- DESIGN-pinned scraper markers. If DELIVER moves these, the template and
// --- this module move in the SAME change.
const MENU_TRIGGER: &str = "data-action=\"toggle-lane-menu\"";
/// The column header becomes the drag surface (D2). Absent until DELIVER.
const DRAG_SURFACE: &str = "data-lane-drag";
/// The drop indicator (US-BLR-03). Must not outlive any drag exit path.
const DROP_INDICATOR: &str = "[data-lane-drop-indicator]";

// ------------------------------------------------------------------- plumbing

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
        .blr_current_project
        .clone()
        .expect("a board Given must have named the project under test")
}

/// STORED slugs, read back at seed time — never re-derived from a name.
fn stored_slugs(world: &FoundryWorld, project_name: &str) -> (String, String) {
    world
        .blr_project_slugs
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded by a Given"))
        .clone()
}

fn project_id_of(world: &FoundryWorld, project_name: &str) -> uuid::Uuid {
    *world
        .blr_project_ids
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded by a Given"))
}

fn board_path(world: &FoundryWorld, project_name: &str) -> String {
    let (team, project) = stored_slugs(world, project_name);
    format!("/team/{team}/project/{project}")
}

fn lane_move_path(team: &str, project: &str, lane: &str) -> String {
    format!("/team/{team}/project/{project}/lanes/{lane}/move")
}

fn lane_edit_path(team: &str, project: &str, lane: &str) -> String {
    format!("/team/{team}/project/{project}/lanes/{lane}/edit")
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
    let ws = world.blr_workspace_id.expect("workspace seeded first");
    let team = world.blr_team_id.expect("team seeded first");
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
    world.blr_project_ids.insert(name.to_string(), id);
    let (team_slug, project_slug): (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug FROM projects p JOIN teams t ON t.id = p.team_id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("read back stored slugs");
    world
        .blr_project_slugs
        .insert(name.to_string(), (team_slug, project_slug));
}

async fn seed_lane(
    world: &FoundryWorld,
    project_id: uuid::Uuid,
    slug: &str,
    label: &str,
    position: i32,
) {
    let ws = world.blr_workspace_id.expect("workspace seeded first");
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
    let ws = world.blr_workspace_id.expect("workspace seeded first");
    let author = world.blr_priya_id.expect("Priya seeded");
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

/// Seed a Backend member other than Priya, for the concurrency scenario's
/// "another member" arm.
async fn ensure_nadia(world: &mut FoundryWorld) {
    if world.blr_team_id.is_none() {
        panic!("team seeded first");
    }
    let id = seed_user(world, NADIA_EMAIL, "Nadia Osei", NADIA_PASSWORD).await;
    let ws = world.blr_workspace_id.expect("workspace seeded");
    let team = world.blr_team_id.expect("team seeded");
    let pool = pool(world);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(id)
    .execute(&pool)
    .await
    .expect("nadia workspace membership");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(team)
    .bind(id)
    .execute(&pool)
    .await
    .expect("nadia team membership");
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

async fn lane_labels_in_order(world: &FoundryWorld) -> Vec<String> {
    let project_id = project_id_of(world, &current_project(world));
    lanes_of(&pool(world), project_id)
        .await
        .into_iter()
        .map(|(_, label, _)| label)
        .collect()
}

async fn lane_slug_for_label(world: &FoundryWorld, label: &str) -> String {
    let project_id = project_id_of(world, &current_project(world));
    lanes_of(&pool(world), project_id)
        .await
        .into_iter()
        .find(|(_, l, _)| l == label)
        .map(|(slug, _, _)| slug)
        .unwrap_or_else(|| panic!("no lane labelled {label:?} on this board"))
}

async fn issues_of(world: &FoundryWorld, project_name: &str) -> Vec<(String, String, i32)> {
    let project_id = project_id_of(world, project_name);
    let rows: Vec<(i32, String, i32)> = sqlx::query_as(
        "SELECT number, state, position FROM issues WHERE project_id = $1 ORDER BY number",
    )
    .bind(project_id)
    .fetch_all(&pool(world))
    .await
    .expect("read issue rows");
    rows.into_iter()
        .map(|(n, state, pos)| (format!("OPS-{n}"), state, pos))
        .collect()
}

async fn count_of(pool: &PgPool, table: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as(&format!("SELECT count(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("count {table}: {err}"));
    n
}

/// Snapshot the declared universe before a write (state-delta discipline).
async fn capture_universe(world: &mut FoundryWorld) {
    let p = current_project(world);
    let project_id = project_id_of(world, &p);
    let pool = pool(world);
    let lanes = lanes_of(&pool, project_id).await;
    world.blr_identity_before = Some(
        lanes
            .iter()
            .map(|(s, l, _)| (s.clone(), l.clone()))
            .collect(),
    );
    world.blr_lanes_before = Some(lanes);
    world.blr_issues_before = Some(issues_of(world, &p).await);
    world.blr_events_before = Some(count_of(&pool, "issue_change_events").await);
    world.blr_outbox_before = Some(count_of(&pool, "outbox").await);
}

/// Positions must be contiguous from zero AND unique. Postgres enforces only
/// the uniqueness half.
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
async fn never_existed(world: &mut FoundryWorld) -> PostOutcome {
    let (team, project) = stored_slugs(world, &current_project(world));
    let path = lane_edit_path(&team, &project, "no_such_lane_at_all");
    priya_get(world, &path).await
}

/// A `foundry_csrf` cookie + matching token WITHOUT a session, so a signed-out
/// POST clears the CSRF middleware and is judged by the handler's authz arm.
async fn csrf_pair_without_session(world: &mut FoundryWorld) -> (String, String) {
    ensure_harness(world).await;
    let base = harness(world).base_url();
    let resp = http(world)
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for a csrf pair");
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with("foundry_csrf="))
        .map(|v| v.split(';').next().unwrap_or("").to_string())
        .expect("sign-in must mint a foundry_csrf cookie");
    let token = cookie
        .strip_prefix("foundry_csrf=")
        .expect("csrf cookie shape")
        .to_string();
    (cookie, token)
}

/// The neighbour slug a one-step move should name, or `None` for "place last".
/// Resolved from the STORE, never from a static list.
async fn one_step_target(world: &FoundryWorld, mover_label: &str, dir: &str) -> Option<String> {
    let project_id = project_id_of(world, &current_project(world));
    let lanes = lanes_of(&pool(world), project_id).await;
    let idx = lanes
        .iter()
        .position(|(_, l, _)| l == mover_label)
        .unwrap_or_else(|| panic!("no lane labelled {mover_label:?} on this board"));
    match dir {
        "left" => {
            assert!(idx > 0, "{mover_label:?} is already the leftmost lane");
            Some(lanes[idx - 1].0.clone())
        }
        "right" => {
            assert!(
                idx + 1 < lanes.len(),
                "{mover_label:?} is already the rightmost lane"
            );
            // Landing at idx+1 means landing immediately BEFORE whatever is at
            // idx+2 — or last, when idx+1 IS the last slot.
            lanes.get(idx + 2).map(|(s, _, _)| s.clone())
        }
        other => panic!("unknown direction {other:?}"),
    }
}

async fn do_move(
    world: &mut FoundryWorld,
    mover_slug: &str,
    before: Option<String>,
) -> PostOutcome {
    let (team, project) = stored_slugs(world, &current_project(world));
    let path = lane_move_path(&team, &project, mover_slug);
    let before_value = before.unwrap_or_default();
    priya_post(world, &path, &[("before", &before_value)]).await
}

// ------------------------------------------------------------- HTML scraping

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

// ------------------------------------------------------------------- browser

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

/// Column labels in on-screen order, read from the live DOM.
async fn screen_order(world: &FoundryWorld) -> Vec<String> {
    let script = "return Array.prototype.map.call(\
                    document.querySelectorAll('section.column > h3'), \
                    function (h) { return h.textContent.trim(); });";
    let value = browser(world)
        .execute(script, vec![])
        .await
        .expect("read on-screen column order");
    value
        .as_array()
        .expect("column-order probe must return an array")
        .iter()
        .map(|v| v.as_str().expect("column label is a string").to_string())
        .collect()
}

/// The drag surface for one lane. Absent until DELIVER builds it — this
/// suite's honest MISSING_FUNCTIONALITY(markup) RED.
async fn drag_surface(world: &FoundryWorld, lane: &str) -> fantoccini::elements::Element {
    let selector = format!("[{DRAG_SURFACE}=\"{lane}\"]");
    browser(world)
        .find(Locator::Css(&selector))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "MISSING_FUNCTIONALITY(markup): the {lane:?} column header is not a drag surface \
                 ({selector}, feature-delta DDD-11 / D2). DELIVER slice 02 renders it. \
                 Underlying error: {err}"
            )
        })
}

async fn menu_trigger(world: &FoundryWorld, lane: &str) -> fantoccini::elements::Element {
    let selector = format!("button[{MENU_TRIGGER}][data-lane=\"{lane}\"]");
    browser(world)
        .find(Locator::Css(&selector))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "MISSING_FUNCTIONALITY(markup): the {lane:?} column has no ⋯ menu trigger \
                 ({selector}). Underlying error: {err}"
            )
        })
}

/// Move keyboard focus to a selector. `fantoccini::Element` exposes no
/// `focus()`, and the WebDriver click path would not prove the KEYBOARD route
/// D4 exists for — so focus is set explicitly and the activation is a real key
/// press.
async fn focus_by_selector(world: &FoundryWorld, selector: &str) {
    let script = format!(
        "var el = document.querySelector('{}'); if (!el) {{ return false; }} el.focus(); \
         return document.activeElement === el;",
        selector.replace('\'', "\\'")
    );
    let focused = browser(world)
        .execute(&script, vec![])
        .await
        .expect("focus probe")
        .as_bool()
        .unwrap_or(false);
    assert!(
        focused,
        "could not place keyboard focus on {selector:?} — a menu item that cannot take focus is \
         not keyboard-reachable (D4)"
    );
}

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

/// A pointer drag across the header band, in steps, so a threshold-gated
/// implementation actually sees movement. Panics with MISSING_FUNCTIONALITY
/// when the drag surface is absent.
async fn drag_column(world: &mut FoundryWorld, from_label: &str, to_label: &str, touch: bool) {
    let from_slug = lane_slug_for_label(world, from_label).await;
    let to_slug = lane_slug_for_label(world, to_label).await;
    let _ = drag_surface(world, &from_slug).await;
    let pointer = if touch { "touch" } else { "mouse" };
    let script = format!(
        "var src = document.querySelector('[{DRAG_SURFACE}=\"{from_slug}\"]'); \
         var dst = document.querySelector('[data-column=\"{to_slug}\"]'); \
         if (!src || !dst) {{ return false; }} \
         var a = src.getBoundingClientRect(), b = dst.getBoundingClientRect(); \
         var opts = {{ pointerType: '{pointer}', bubbles: true, cancelable: true, isPrimary: true }}; \
         function at(x, y, type) {{ \
           var e = new PointerEvent(type, Object.assign({{}}, opts, \
             {{ clientX: x, clientY: y, pointerId: 1 }})); \
           src.dispatchEvent(e); \
         }} \
         var rightward = b.left >= a.left; \
         /* Release DECISIVELY past the destination's midpoint, not exactly on \
            it. The drop indicator takes 3px of layout the moment the drag \
            begins, so a coordinate computed from the pre-drag snapshot sits on \
            a knife edge: measured, an exact-midpoint release landed one slot \
            short and posted a no-op. */ \
         var dropX = rightward ? b.left + b.width * 0.9 : b.left + b.width * 0.1; \
         at(a.left + a.width / 2, a.top + a.height / 2, 'pointerdown'); \
         for (var i = 1; i <= 8; i++) {{ \
           at(a.left + (dropX - a.left) * i / 8, a.top + a.height / 2, 'pointermove'); \
         }} \
         at(dropX, a.top + a.height / 2, 'pointerup'); \
         return true;"
    );
    let ok = browser(world)
        .execute(&script, vec![])
        .await
        .expect("dispatch pointer drag")
        .as_bool()
        .unwrap_or(false);
    assert!(
        ok,
        "MISSING_FUNCTIONALITY(markup): could not drag {from_label:?} onto {to_label:?} — the \
         drag surface or the destination column was not in the DOM"
    );
}

// =========================================================================
// Given
// =========================================================================

#[given(regex = r"^Priya is a Backend team member ordering her own board lanes$")]
async fn priya_backend_member(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let pool = pool(world);
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(ws)
        .bind("Canzan Labs")
        .execute(&pool)
        .await
        .expect("insert workspace");
    world.blr_workspace_id = Some(ws);

    let team = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team)
        .bind(ws)
        .bind("Backend")
        .bind("backend")
        .execute(&pool)
        .await
        .expect("insert team");
    world.blr_team_id = Some(team);

    let priya = seed_user(world, PRIYA_EMAIL, "Priya Raman", PRIYA_PASSWORD).await;
    world.blr_priya_id = Some(priya);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(priya)
    .execute(&pool)
    .await
    .expect("priya workspace membership");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
              VALUES ($1, $2, 'lead') ON CONFLICT DO NOTHING",
    )
    .bind(team)
    .bind(priya)
    .execute(&pool)
    .await
    .expect("priya team membership");
}

#[given(regex = r"^Marco is signed in without membership of team Backend$")]
async fn marco_not_a_member(world: &mut FoundryWorld) {
    let ws = world.blr_workspace_id.expect("workspace seeded");
    let marco = seed_user(world, MARCO_EMAIL, "Marco Silva", MARCO_PASSWORD).await;
    world.blr_marco_id = Some(marco);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(marco)
    .execute(&pool(world))
    .await
    .expect("marco workspace membership (but NOT team Backend)");
    // Prove the foil is actually a foil — a Marco who silently WAS a member
    // would make every authz scenario vacuous.
    let (is_member,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM team_memberships tm
           JOIN teams t ON t.id = tm.team_id WHERE tm.user_id = $1 AND t.slug = 'backend')",
    )
    .bind(marco)
    .fetch_one(&pool(world))
    .await
    .expect("probe team membership");
    assert!(!is_member, "Marco must NOT be a member of team Backend");
}

#[given(
    regex = r#"^"([^"]+)" \(([A-Z]+)\) is a board with lanes Backlog, Done, Staging and In-Progress$"#
)]
async fn wrong_order_board(world: &mut FoundryWorld, name: String, prefix: String) {
    seed_project(world, &name, "homelab-ops", &prefix).await;
    let id = project_id_of(world, &name);
    // The journey's exact wrong-order board: Staging was inserted late and
    // landed after Done.
    seed_lane(world, id, "backlog", "Backlog", 0).await;
    seed_lane(world, id, "done", "Done", 1).await;
    seed_lane(world, id, "staging", "Staging", 2).await;
    seed_lane(world, id, "in_progress", "In-Progress", 3).await;
    world.blr_current_project = Some(name);
    capture_universe(world).await;
}

#[given(regex = r#"^"([^"]+)" \(([A-Z]+)\) is a board with eight lanes on a narrow screen$"#)]
async fn eight_lane_board(world: &mut FoundryWorld, name: String, prefix: String) {
    seed_project(world, &name, "homelab-ops", &prefix).await;
    let id = project_id_of(world, &name);
    for (i, (slug, label)) in [
        ("backlog", "Backlog"),
        ("triage", "Triage"),
        ("done", "Done"),
        ("staging", "Staging"),
        ("in_progress", "In-Progress"),
        ("review", "Review"),
        ("blocked", "Blocked"),
        ("shipped", "Shipped"),
    ]
    .iter()
    .enumerate()
    {
        seed_lane(world, id, slug, label, i as i32).await;
    }
    world.blr_current_project = Some(name);
    // A narrow viewport is what makes `.board{overflow-x:auto}` actually
    // scroll — the whole premise of US-BLR-03.
    ensure_harness(world).await;
    if world.browser.is_none() {
        let client = browser_harness::open_mobile_session().await;
        browser_harness::sign_in_through_browser(
            &client,
            harness(world),
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
        )
        .await;
        world.browser = Some(client);
    }
    capture_universe(world).await;
}

#[given(regex = r"^OPS-3, OPS-7 and OPS-9 sit in Done$")]
async fn three_cards_in_done(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 3, "Rotate the backup key", "done", 0).await;
    seed_issue(world, &p, 7, "Pin the chart version", "done", 1).await;
    seed_issue(world, &p, 9, "Drain the stale queue", "done", 2).await;
    capture_universe(world).await;
}

#[given(regex = r"^OPS-3 sits in Done$")]
async fn one_card_in_done(world: &mut FoundryWorld) {
    let p = current_project(world);
    seed_issue(world, &p, 3, "Rotate the backup key", "done", 0).await;
    capture_universe(world).await;
}

#[given(regex = r"^Priya has begun dragging the Done column$")]
async fn drag_in_flight(world: &mut FoundryWorld) {
    open_board_in_browser(world).await;
    world.blr_screen_before = Some(screen_order(world).await);
    let slug = lane_slug_for_label(world, "Done").await;
    let _ = drag_surface(world, &slug).await;
    let script = format!(
        "var src = document.querySelector('[{DRAG_SURFACE}=\"{slug}\"]'); \
         if (!src) {{ return false; }} \
         var a = src.getBoundingClientRect(); \
         var opts = {{ pointerType: 'mouse', bubbles: true, cancelable: true, isPrimary: true, pointerId: 1 }}; \
         src.dispatchEvent(new PointerEvent('pointerdown', \
           Object.assign({{}}, opts, {{ clientX: a.left + 5, clientY: a.top + 5 }}))); \
         src.dispatchEvent(new PointerEvent('pointermove', \
           Object.assign({{}}, opts, {{ clientX: a.left + 90, clientY: a.top + 5 }}))); \
         return true;"
    );
    let ok = browser(world)
        .execute(&script, vec![])
        .await
        .expect("begin drag")
        .as_bool()
        .unwrap_or(false);
    assert!(
        ok,
        "MISSING_FUNCTIONALITY(markup): could not begin a drag on the Done column header"
    );
}

#[given(regex = r"^the next move will be refused$")]
async fn next_move_refused(world: &mut FoundryWorld) {
    world.blr_force_refusal = true;
}

/// Make the NEXT drop be refused by the server, so the revert path is actually
/// exercised. The column's move URL is repointed at a lane that does not exist,
/// which is the honest shape of the race this scenario stands for: the lane
/// vanished between the drag starting and the pointer lifting, so the POST comes
/// back as the uniform 404 (D9) and the optimistic move must be undone.
///
/// Without this the Given was an inert flag nobody read — the drop succeeded and
/// the scenario asserted the revert of a move that never needed reverting.
async fn sabotage_next_move(world: &mut FoundryWorld) {
    if !world.blr_force_refusal {
        return;
    }
    let (team, project) = stored_slugs(world, &current_project(world));
    let doomed = lane_move_path(&team, &project, "no_such_lane_at_all");
    let script = format!(
        "var cols = document.querySelectorAll('[data-lane-move-url]'); \
         for (var i = 0; i < cols.length; i++) {{ \
           cols[i].setAttribute('data-lane-move-url', '{doomed}'); \
         }} \
         return cols.length;"
    );
    let touched = browser(world)
        .execute(&script, vec![])
        .await
        .expect("repoint the move URL")
        .as_f64()
        .unwrap_or(0.0);
    assert!(
        touched > 0.0,
        "could not arrange a refused move — no element carried data-lane-move-url"
    );
}

// =========================================================================
// When
// =========================================================================

#[when(regex = r#"^Priya opens the "([^"]+)" board to reorder it$"#)]
async fn priya_opens_board(world: &mut FoundryWorld, name: String) {
    let path = board_path(world, &name);
    let outcome = priya_get(world, &path).await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
}

#[when(regex = r"^Priya moves the ([A-Za-z-]+) lane (left|right)$")]
async fn priya_moves_lane(world: &mut FoundryWorld, label: String, dir: String) {
    capture_universe(world).await;
    let target = one_step_target(world, &label, &dir).await;
    let slug = lane_slug_for_label(world, &label).await;
    let outcome = do_move(world, &slug, target).await;
    world.blr_last_move = Some((outcome.status, outcome.body));
}

#[when(regex = r"^Priya moves the ([A-Za-z-]+) lane to the position it already holds$")]
async fn priya_moves_lane_nowhere(world: &mut FoundryWorld, label: String) {
    capture_universe(world).await;
    // "Before the lane immediately to my right" IS my own position.
    let project_id = project_id_of(world, &current_project(world));
    let lanes = lanes_of(&pool(world), project_id).await;
    let idx = lanes
        .iter()
        .position(|(_, l, _)| *l == label)
        .expect("lane exists");
    let target = lanes.get(idx + 1).map(|(s, _, _)| s.clone());
    let slug = lane_slug_for_label(world, &label).await;
    let outcome = do_move(world, &slug, target).await;
    world.blr_last_move = Some((outcome.status, outcome.body));
}

#[when(regex = r"^Priya moves a lane that no longer exists$")]
async fn priya_moves_ghost_lane(world: &mut FoundryWorld) {
    capture_universe(world).await;
    let target = lane_slug_for_label(world, "Backlog").await;
    let outcome = do_move(world, "no_such_lane_at_all", Some(target)).await;
    world.blr_last_move = Some((outcome.status, outcome.body.clone()));
    world.blr_refusals.push((outcome.status, outcome.body));
}

#[when(regex = r"^Priya moves the ([A-Za-z-]+) lane beside a lane that no longer exists$")]
async fn priya_moves_beside_ghost(world: &mut FoundryWorld, label: String) {
    capture_universe(world).await;
    let slug = lane_slug_for_label(world, &label).await;
    let outcome = do_move(world, &slug, Some("no_such_lane_at_all".to_string())).await;
    world.blr_last_move = Some((outcome.status, outcome.body.clone()));
    world.blr_refusals.push((outcome.status, outcome.body));
}

#[when(regex = r"^Marco moves the ([A-Za-z-]+) lane (left|right)$")]
async fn marco_moves_lane(world: &mut FoundryWorld, label: String, dir: String) {
    capture_universe(world).await;
    let target = one_step_target(world, &label, &dir)
        .await
        .unwrap_or_default();
    let slug = lane_slug_for_label(world, &label).await;
    let (team, project) = stored_slugs(world, &current_project(world));
    let path = lane_move_path(&team, &project, &slug);
    let outcome = marco_post(world, &path, &[("before", &target)]).await;
    world.blr_last_move = Some((outcome.status, outcome.body.clone()));
    world.blr_refusals.push((outcome.status, outcome.body));
}

#[when(regex = r"^a signed-out visitor moves the ([A-Za-z-]+) lane (left|right)$")]
async fn signed_out_moves_lane(world: &mut FoundryWorld, label: String, dir: String) {
    capture_universe(world).await;
    let target = one_step_target(world, &label, &dir)
        .await
        .unwrap_or_default();
    let slug = lane_slug_for_label(world, &label).await;
    let (team, project) = stored_slugs(world, &current_project(world));
    let path = lane_move_path(&team, &project, &slug);
    ensure_harness(world).await;
    // A CSRF pair but NO session, so the refusal proves the AUTHZ arm rather
    // than the middleware's tokenless arm. Posting with an empty cookie header
    // instead would be stopped at 403 by the middleware and the scenario would
    // assert nothing about authorization at all.
    let (csrf_cookie, csrf_token) = csrf_pair_without_session(world).await;
    let outcome = post_with_cookie(
        harness(world),
        &http(world),
        &path,
        &csrf_cookie,
        &[("before", &target), ("_csrf", &csrf_token)],
    )
    .await;
    world.blr_last_move = Some((outcome.status, outcome.body.clone()));
    world.blr_refusals.push((outcome.status, outcome.body));
}

#[when(regex = r"^Priya moves the ([A-Za-z-]+) lane (left|right) without the request token$")]
async fn priya_moves_tokenless(world: &mut FoundryWorld, label: String, dir: String) {
    capture_universe(world).await;
    let target = one_step_target(world, &label, &dir)
        .await
        .unwrap_or_default();
    let slug = lane_slug_for_label(world, &label).await;
    let (team, project) = stored_slugs(world, &current_project(world));
    let path = lane_move_path(&team, &project, &slug);
    ensure_harness(world).await;
    let session =
        establish_session(harness(world), &http(world), PRIYA_EMAIL, PRIYA_PASSWORD).await;
    // Session present, `_csrf` deliberately absent — the middleware must refuse
    // BEFORE the handler runs.
    let outcome = post_with_cookie(
        harness(world),
        &http(world),
        &path,
        &session,
        &[("before", &target)],
    )
    .await;
    world.blr_last_move = Some((outcome.status, outcome.body));
}

#[when(
    regex = r"^Priya moves In-Progress before Done while another member moves Staging before Backlog$"
)]
async fn two_operators_move(world: &mut FoundryWorld) {
    capture_universe(world).await;
    ensure_nadia(world).await;
    let (team, project) = stored_slugs(world, &current_project(world));
    let in_progress = lane_slug_for_label(world, "In-Progress").await;
    let done = lane_slug_for_label(world, "Done").await;
    let staging = lane_slug_for_label(world, "Staging").await;
    let backlog = lane_slug_for_label(world, "Backlog").await;
    let path_a = lane_move_path(&team, &project, &in_progress);
    let path_b = lane_move_path(&team, &project, &staging);
    ensure_harness(world).await;
    let h = harness(world);
    let c = http(world);
    // Bound before the join so the form slices outlive both futures.
    let form_a = [("before", done.as_str())];
    let form_b = [("before", backlog.as_str())];
    // Both requests genuinely in flight at once, against the real adapter.
    let (a, b) = tokio::join!(
        signed_in_post(h, &c, PRIYA_EMAIL, PRIYA_PASSWORD, &path_a, &form_a),
        signed_in_post(h, &c, NADIA_EMAIL, NADIA_PASSWORD, &path_b, &form_b),
    );
    world.blr_concurrent = vec![(a.status, a.body), (b.status, b.body)];
}

#[when(regex = r"^Priya opens the ([A-Za-z-]+) column's menu and chooses Move list (left|right)$")]
async fn menu_choose_move(world: &mut FoundryWorld, label: String, dir: String) {
    open_board_in_browser(world).await;
    let slug = lane_slug_for_label(world, &label).await;
    menu_trigger(world, &slug)
        .await
        .click()
        .await
        .expect("open the lane menu");
    let item = format!("[data-lane-menu=\"{slug}\"] [data-action=\"move-lane-{dir}\"]");
    browser(world)
        .find(Locator::Css(&item))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "MISSING_FUNCTIONALITY(markup): the {label:?} menu has no Move list {dir} item \
                 ({item}, D5). DELIVER slice 01 renders it. Underlying error: {err}"
            )
        })
        .click()
        .await
        .expect("choose the move item");
    browser_harness::wait_for_board_ready(browser(world)).await;
}

#[when(
    regex = r"^Priya reaches the ([A-Za-z-]+) menu by keyboard and activates Move list (left|right)$"
)]
async fn keyboard_move(world: &mut FoundryWorld, label: String, dir: String) {
    open_board_in_browser(world).await;
    let slug = lane_slug_for_label(world, &label).await;
    let _ = menu_trigger(world, &slug).await;
    focus_by_selector(
        world,
        &format!("button[{MENU_TRIGGER}][data-lane=\"{slug}\"]"),
    )
    .await;
    browser_harness::press_key(browser(world), "Enter").await;
    let item = format!("[data-lane-menu=\"{slug}\"] [data-action=\"move-lane-{dir}\"]");
    let _ = browser(world)
        .find(Locator::Css(&item))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "MISSING_FUNCTIONALITY(markup): no keyboard-reachable Move list {dir} item on \
                 {label:?} ({item}, D4). Underlying error: {err}"
            )
        });
    focus_by_selector(world, &item).await;
    browser_harness::press_key(browser(world), "Enter").await;
    browser_harness::wait_for_board_ready(browser(world)).await;
}

#[when(regex = r"^Priya drags the ([A-Za-z-]+) column past ([A-Za-z-]+) and releases$")]
async fn drag_column_past(world: &mut FoundryWorld, from: String, to: String) {
    open_board_in_browser(world).await;
    sabotage_next_move(world).await;
    world.blr_screen_before = Some(screen_order(world).await);
    drag_column(world, &from, &to, false).await;
}

#[when(regex = r"^Priya drags the ([A-Za-z-]+) column past ([A-Za-z-]+) with a touch pointer$")]
async fn drag_column_touch(world: &mut FoundryWorld, from: String, to: String) {
    open_board_in_browser(world).await;
    world.blr_screen_before = Some(screen_order(world).await);
    drag_column(world, &from, &to, true).await;
}

#[when(
    regex = r"^Priya presses the ([A-Za-z-]+) column's menu trigger without moving the pointer$"
)]
async fn press_without_moving(world: &mut FoundryWorld, label: String) {
    open_board_in_browser(world).await;
    let slug = lane_slug_for_label(world, &label).await;
    menu_trigger(world, &slug)
        .await
        .click()
        .await
        .expect("click the menu trigger without moving");
}

#[when(regex = r"^Priya presses Escape to cancel the drag$")]
async fn escape_cancels_drag(world: &mut FoundryWorld) {
    browser_harness::press_key(browser(world), "Escape").await;
}

#[when(regex = r"^Priya drags (OPS-\d+) from ([A-Za-z-]+) into ([A-Za-z-]+)$")]
async fn drag_a_card(world: &mut FoundryWorld, key: String, _from: String, to: String) {
    open_board_in_browser(world).await;
    let to_slug = lane_slug_for_label(world, &to).await;
    // The SHIPPED card drag: native HTML5 DnD (board-dnd.js). Exercised here
    // from the lane side to prove ADR-BOARD-LANE-007's origin boundary — a
    // gesture starting on a card is a card move, never a lane move.
    let script = format!(
        "var card = document.querySelector('[data-issue-key=\"{key}\"]'); \
         var col = document.querySelector('[data-column=\"{to_slug}\"]'); \
         if (!card || !col) {{ return false; }} \
         var dt = new DataTransfer(); \
         card.dispatchEvent(new DragEvent('dragstart', {{ bubbles: true, dataTransfer: dt }})); \
         col.dispatchEvent(new DragEvent('dragover', {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
         col.dispatchEvent(new DragEvent('drop', {{ bubbles: true, cancelable: true, dataTransfer: dt }})); \
         return true;"
    );
    let ok = browser(world)
        .execute(&script, vec![])
        .await
        .expect("dispatch card drag")
        .as_bool()
        .unwrap_or(false);
    assert!(ok, "could not drag card {key} into {to:?}");
}

#[when(
    regex = r"^Priya drags the leftmost column to the right edge and holds until the board scrolls$"
)]
async fn drag_to_edge_and_scroll(world: &mut FoundryWorld) {
    open_board_in_browser(world).await;
    world.blr_scroll_before = Some(board_scroll_left(world).await);
    drag_column(world, "Backlog", "Shipped", true).await;
}

#[when(regex = r"^Priya drags a column to the right edge and holds past the end of the board$")]
async fn drag_past_end(world: &mut FoundryWorld) {
    open_board_in_browser(world).await;
    world.blr_scroll_before = Some(window_scroll_x(world).await);
    drag_column(world, "Backlog", "Shipped", true).await;
}

async fn board_scroll_left(world: &FoundryWorld) -> f64 {
    browser(world)
        .execute(
            "var b = document.querySelector('.board'); return b ? b.scrollLeft : -1;",
            vec![],
        )
        .await
        .expect("read board scrollLeft")
        .as_f64()
        .unwrap_or(-1.0)
}

async fn window_scroll_x(world: &FoundryWorld) -> f64 {
    browser(world)
        .execute("return window.scrollX;", vec![])
        .await
        .expect("read window scrollX")
        .as_f64()
        .unwrap_or(-1.0)
}

// =========================================================================
// Then
// =========================================================================

#[then(regex = r"^the board reads (.+)$")]
async fn board_reads(world: &mut FoundryWorld, expected: String) {
    if expected.starts_with("with the first lane moved") {
        let mut labels = lane_labels_in_order(world).await;
        for _ in 0..30 {
            if labels.last().map(String::as_str) == Some("Backlog") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            labels = lane_labels_in_order(world).await;
        }
        assert_eq!(
            labels.last().map(String::as_str),
            Some("Backlog"),
            "the dragged lane should have landed at the far right; board reads {labels:?}"
        );
        return;
    }
    let want: Vec<String> = expected
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    // Both surfaces commit through an ASYNC fetch, so reading the rows straight
    // after the gesture races the request. Poll rather than sleep — and note
    // this loop cannot mask a FAILURE, only a slow success: a board that never
    // reaches `want` still fails, just later.
    let mut got = lane_labels_in_order(world).await;
    for _ in 0..30 {
        if got == want {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        got = lane_labels_in_order(world).await;
    }
    assert_eq!(
        got, want,
        "the board's lane order (read from the `lanes` rows, never a static list) must be \
         {want:?}; it is {got:?}"
    );
    assert_zero_laneless(world).await;
}

#[then(regex = r"^the board on screen reads (.+)$")]
async fn screen_reads(world: &mut FoundryWorld, expected: String) {
    let want: Vec<String> = expected
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let mut got = screen_order(world).await;
    for _ in 0..30 {
        if got == want {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        got = screen_order(world).await;
    }
    assert_eq!(
        got, want,
        "the rendered column order must be {want:?}; it is {got:?}"
    );
}

#[then(regex = r"^every lane keeps the slug and label it had$")]
async fn identity_untouched(world: &mut FoundryWorld) {
    let before = world
        .blr_identity_before
        .clone()
        .expect("identity captured before the move");
    let project_id = project_id_of(world, &current_project(world));
    let mut after: Vec<(String, String)> = lanes_of(&pool(world), project_id)
        .await
        .into_iter()
        .map(|(s, l, _)| (s, l))
        .collect();
    let mut before_sorted = before;
    before_sorted.sort();
    after.sort();
    assert_eq!(
        before_sorted, after,
        "a move relocates a lane and changes NOTHING else: every (slug, label) pair must survive \
         byte-identical (D1 / KPI 3). A changed slug would silently re-home every issue in the \
         lane under fk_issues_lane."
    );
}

#[then(regex = r"^no card changed lane or order$")]
async fn no_card_moved(world: &mut FoundryWorld) {
    let p = current_project(world);
    let before = world
        .blr_issues_before
        .clone()
        .expect("universe captured before the move");
    let after = issues_of(world, &p).await;
    assert_eq!(
        before, after,
        "a lane move must write ZERO issue rows (D1 / KPI 2); issue (key, lane, position) drifted"
    );
    assert_zero_laneless(world).await;
}

#[then(regex = r"^no change event and no outbox row was written$")]
async fn no_events_written(world: &mut FoundryWorld) {
    let pool = pool(world);
    let events_before = world.blr_events_before.expect("event count captured");
    let events_after = count_of(&pool, "issue_change_events").await;
    assert_eq!(
        events_before, events_after,
        "a lane move writes NO 0013 change event; count moved {events_before} -> {events_after}"
    );
    let outbox_before = world.blr_outbox_before.expect("outbox count captured");
    let outbox_after = count_of(&pool, "outbox").await;
    assert_eq!(
        outbox_before, outbox_after,
        "a lane move writes NO outbox row; count moved {outbox_before} -> {outbox_after}"
    );
}

#[then(regex = r"^the lane positions are contiguous from zero with no duplicates$")]
async fn positions_contiguous(world: &mut FoundryWorld) {
    let project_id = project_id_of(world, &current_project(world));
    let lanes = lanes_of(&pool(world), project_id).await;
    assert_contiguous(&lanes);
}

#[then(
    regex = r"^the ([A-Za-z-]+) column's menu offers exactly Edit list, Insert list before, Insert list after, Move list left, Move list right and Delete list$"
)]
async fn menu_offers_six(world: &mut FoundryWorld, label: String) {
    let body = world.last_body.clone().expect("a board page was fetched");
    let slug = lane_slug_for_label(world, &label).await;
    let slice = column_slice(&body, &slug);
    let want = [
        "Edit list",
        "Insert list before",
        "Insert list after",
        "Move list left",
        "Move list right",
        "Delete list",
    ];
    let mut cursor = 0usize;
    for item in want {
        let at = slice[cursor..].find(item).unwrap_or_else(|| {
            panic!(
                "MISSING_FUNCTIONALITY(markup): the {label:?} column's menu does not offer \
                 {item:?} in contract order (D5 re-pins OUT-6 at six items). Rendered column: \
                 {slice}"
            )
        });
        cursor += at + item.len();
    }
}

#[then(regex = r"^the ([A-Za-z-]+) column offers a disabled Move list (left|right)$")]
async fn move_item_disabled(world: &mut FoundryWorld, label: String, dir: String) {
    let body = world.last_body.clone().expect("a board page was fetched");
    let slug = lane_slug_for_label(world, &label).await;
    let slice = column_slice(&body, &slug);
    let marker = format!("data-action=\"move-lane-{dir}\"");
    let at = slice.find(&marker).unwrap_or_else(|| {
        panic!(
            "MISSING_FUNCTIONALITY(markup): the {label:?} column has no Move list {dir} item at \
             all. D5 requires it RENDERED-BUT-DISABLED at the board's ends, never omitted — an \
             omitted item makes every menu index position-dependent."
        )
    });
    // The disabled attributes must be on that item's own tag.
    let tag_end = slice[at..].find('>').map(|e| at + e).unwrap_or(slice.len());
    let tag = &slice[..tag_end];
    let tag_start = tag.rfind('<').unwrap_or(0);
    let tag = &slice[tag_start..tag_end];
    assert!(
        tag.contains("disabled") && tag.contains("aria-disabled=\"true\""),
        "the end-of-board Move list {dir} item must carry BOTH `disabled` and \
         `aria-disabled=\"true\"` (AC-1.3); its tag is: {tag}"
    );
}

#[then(regex = r"^the ([A-Za-z-]+) column still offers all six operations$")]
async fn still_six_items(world: &mut FoundryWorld, label: String) {
    let body = world.last_body.clone().expect("a board page was fetched");
    let slug = lane_slug_for_label(world, &label).await;
    let slice = column_slice(&body, &slug);
    let count = slice.matches("role=\"menuitem\"").count();
    assert_eq!(
        count, 6,
        "every column's menu must render the SAME six items regardless of the lane's position \
         (D5); the {label:?} column rendered {count}. A varying item count makes the keyboard \
         contract and every acceptance selector position-dependent."
    );
}

#[then(regex = r"^the refusal is byte-identical to a board that never existed$")]
async fn refusal_is_uniform(world: &mut FoundryWorld) {
    let (status, body) = world
        .blr_last_move
        .clone()
        .expect("a move refusal was recorded");
    let control = never_existed(world).await;
    assert_eq!(
        status, control.status,
        "a refused move must carry the SAME status as a never-existed lane ({}), not {status}",
        control.status
    );
    assert_eq!(
        body, control.body,
        "a refused move must be BYTE-IDENTICAL to a never-existed lane, or the pair enumerates \
         which lanes and boards exist"
    );
}

/// GREEN at DISTILL by design, and deliberately NOT vacuous. The CSRF
/// middleware is shipped, so this scenario guards a cross-cutting property of a
/// NEWLY MOUNTED route rather than driving new behaviour. It pins the exact
/// arm: 403 from the middleware, never the 501 the handler would answer — so
/// it would fail if the move route were ever mounted outside the CSRF layer,
/// which is the regression it exists for.
#[then(regex = r"^the move is refused before the handler runs$")]
async fn move_refused_by_middleware(world: &mut FoundryWorld) {
    let (status, _) = world.blr_last_move.clone().expect("a move was attempted");
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a tokenless move must be refused by the CSRF middleware BEFORE the handler runs — a \
         501 here would mean the route was mounted OUTSIDE the CSRF layer, which is the exact \
         defect fix-comment-delete-csrf shipped to close; got {status}"
    );
}

/// The NoOp arm (DDD-7) must be reached, not merely survived. Without this the
/// scenario passes on the RED scaffold's 501 and proves nothing.
#[then(regex = r"^the move was accepted$")]
async fn move_accepted(world: &mut FoundryWorld) {
    let (status, _) = world.blr_last_move.clone().expect("a move was attempted");
    assert!(
        status.is_success(),
        "a move onto a lane's own position must be ACCEPTED and commit nothing (DDD-7); got \
         {status}. A 501 here means the scaffold answered and the NoOp arm was never exercised."
    );
}

/// Guards scenario 18 against vacuity: the ⋯ menu is SHIPPED, so "the menu
/// opened" is true before slice 02 exists and would stay true if the drag
/// surface swallowed every click. Requiring the surface makes the scenario
/// capable of detecting the regression it exists for.
#[then(regex = r"^the ([A-Za-z-]+) column header is a drag surface$")]
async fn header_is_drag_surface(world: &mut FoundryWorld, label: String) {
    let slug = lane_slug_for_label(world, &label).await;
    let _ = drag_surface(world, &slug).await;
}

#[then(regex = r"^the menu is closed$")]
async fn menu_closed(world: &mut FoundryWorld) {
    let slug = lane_slug_for_label(world, "Staging").await;
    assert!(
        !menu_is_open(world, &slug).await,
        "choosing a move item must close the menu"
    );
}

#[then(regex = r"^the ([A-Za-z-]+) column's menu is open$")]
async fn menu_open(world: &mut FoundryWorld, label: String) {
    let slug = lane_slug_for_label(world, &label).await;
    assert!(
        menu_is_open(world, &slug).await,
        "a press that never passed the drag threshold must still deliver its click to the ⋯ \
         trigger (D2); the {label:?} menu did not open"
    );
}

#[then(regex = r"^(OPS-\d+) sits in ([A-Za-z-]+)$")]
async fn card_sits_in(world: &mut FoundryWorld, key: String, label: String) {
    let slug = lane_slug_for_label(world, &label).await;
    let p = current_project(world);
    // board-dnd.js commits through an ASYNC fetch, so reading the row straight
    // after dispatching the drop races the request. Poll instead of sleeping.
    for _ in 0..30 {
        let rows = issues_of(world, &p).await;
        if rows.iter().any(|(k, st, _)| *k == key && *st == slug) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let rows = issues_of(world, &p).await;
    let found = rows
        .iter()
        .find(|(k, _, _)| *k == key)
        .unwrap_or_else(|| panic!("no issue {key} on this board"));
    assert_eq!(
        found.1, slug,
        "{key} must sit in {label:?} ({slug}); the card drag must still move CARDS \
         (ADR-BOARD-LANE-007 origin boundary), it is in {:?}",
        found.1
    );
}

#[then(regex = r"^the board has scrolled no further than its own end$")]
async fn scrolled_to_extent(world: &mut FoundryWorld) {
    let script = "var b = document.querySelector('.board'); \
                  if (!b) { return -1; } \
                  return b.scrollLeft - (b.scrollWidth - b.clientWidth);";
    let overshoot = browser(world)
        .execute(script, vec![])
        .await
        .expect("measure board scroll overshoot")
        .as_f64()
        .unwrap_or(1.0);
    assert!(
        overshoot <= 0.5,
        "auto-scroll must stop at the board's own scroll extent; it overshot by {overshoot}px"
    );
}

#[then(regex = r"^the page itself has not scrolled$")]
async fn page_did_not_scroll(world: &mut FoundryWorld) {
    let before = world.blr_scroll_before.expect("window scrollX captured");
    let after = window_scroll_x(world).await;
    assert_eq!(
        before, after,
        "edge auto-scroll must move the BOARD, never the page (AC-3.2); window.scrollX moved \
         {before} -> {after}"
    );
}

#[then(regex = r"^no drop indicator remains on the board$")]
async fn no_indicator(world: &mut FoundryWorld) {
    let script = format!("return document.querySelectorAll('{DROP_INDICATOR}').length;");
    let n = browser(world)
        .execute(&script, vec![])
        .await
        .expect("count drop indicators")
        .as_f64()
        .unwrap_or(-1.0);
    assert_eq!(
        n, 0.0,
        "no drag exit path — drop, Escape or pointercancel — may leave a drop indicator on \
         screen (AC-3.4); found {n}"
    );
}
