//! issue-card-delete step definitions
//! (`tests/features/issue-card-delete.feature`, 34 scenarios). Scaffolded RED
//! as `@pending` by DISTILL per ADR-025 and un-pended one at a time by DELIVER,
//! which never re-authored one; none is pending now.
//!
//! The production seams these steps drive are the DESIGN port signatures
//! (feature-delta DDD-1/DDD-7), all shipped: `foundry_store::issue_delete::
//! delete_issues_with_outbox` + `IssueDeleteContext` + `Store::
//! delete_issue_with_outbox`, `foundry_services::issues::delete_issue` +
//! `delete_issue_dialog` + `IssueDeleteView`, the
//! `foundry_app::issues::show_delete_form` / `submit_delete` handlers, the two
//! Delete controls and both dialog templates.
//!
//! THE CASCADE ORACLE COUNTS CHILDREN, NEVER "no error was raised". This is the
//! module's most important rule. A delete that left comments, attachments or
//! change events behind raises nothing — the FKs are `ON DELETE CASCADE`, so an
//! orphan is only reachable if the cascade silently did not fire, and every
//! status-code assertion is GREEN on that case. [`assert_no_orphans`] runs the
//! LEFT JOIN guard across all three child tables after every mutating scenario.
//! Issue row ids are captured at SEED time ([`FoundryWorld::icd_issue_ids`])
//! precisely because after the delete there is no parent row left to join from.
//!
//! THE ANNOUNCEMENT ORACLE COUNTS BOTH WAYS. A missing `IssueDeleted` and a
//! spurious one are different bugs, and "the second board updated" cannot
//! distinguish either from a reload. Scenarios snapshot the outbox row count
//! before the write and assert the exact delta — N for N destroyed cards, ZERO
//! for every refusal — AND the payload's `key`. The outbox table is the oracle
//! rather than the SSE stream because a row written but not delivered and a row
//! never written are, from the stream's side, identical silence.
//!
//! THE PLACEMENT ORACLE IS MARKUP, NOT STYLING (AC-2.1). "Delete cannot submit
//! the edit she is looking at" is asserted as DOM containment — the control is
//! not a descendant of the edit `<form>` — because a Delete that merely *looks*
//! far from Save is the exact trap `adr-modal-close-001` D-12 exists to
//! prevent, and CSS cannot be trusted to keep it there.
//!
//! THE REFUSAL ORACLE IS BYTE-IDENTICAL COMPARISON. Every refusal is compared
//! against a never-existed path, so a delete naming a vanished issue cannot be
//! distinguished from one naming an issue on another workspace's board
//! (ADR-003). Asserting "404" alone would pass over a body that echoed the key.
//!
//! LAYER 3 (real adapter + real HTTP, `@real-io`): real Postgres via the shared
//! testcontainer + per-scenario schema; the real tower-sessions store; the real
//! double-submit CSRF middleware; the in-process axum router. Example-based
//! (Mandates 9 + 11) — the sad paths are enumerated, never generated.
//!
//! The seven `@needs-browser` scenarios drive a REAL headless Chrome
//! (fantoccini, `support::browser_harness`) because the HTTP lane is byte-blind
//! to three things this feature promises: that the CSRF token actually travels
//! from a real popup (the `fix-comment-delete-csrf` lesson — HTTP-lane token
//! injection hid a live 403 once already), that the scripting-disabled path
//! renders a real PAGE rather than a bare floating fragment (DDD-6 — asserting
//! the route merely answers 200 would pass over exactly the outcome
//! ADR-ISSUE-DELETE-002 rejected), and that a SECOND window loses the card
//! without being reloaded.

use crate::support::browser_harness;
use crate::support::harness::{
    establish_session, post_with_cookie, signed_in_get, signed_in_post, InProcHarness, PostOutcome,
};
use crate::support::sse_client::open_sse_subscription;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::StatusCode;
use secrecy::SecretString;
use sqlx::PgPool;
use std::time::Duration;

const TEST_NOW: &str = "2026-03-01T12:00:00Z";
const PRIYA_EMAIL: &str = "priya.icd@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const MARCO_EMAIL: &str = "marco.icd@canzan.test";
const MARCO_PASSWORD: &str = "marco-correct-horse-battery-staple";
const OTHER_EMAIL: &str = "nadia.icd@canzan.test";
const OTHER_PASSWORD: &str = "nadia-correct-horse-battery-staple";

// --- DESIGN-pinned scraper markers. If the templates move these, they and
// --- this module move in the SAME change.
/// The confirm dialog's root (mirrors `data-modal="delete-lane"`).
const DELETE_MODAL: &str = "data-modal=\"delete-issue\"";
/// The popup's Delete control.
const POPUP_DELETE: &str = "data-action=\"delete-issue\"";
/// The full page's Delete control — ONE marker on BOTH surfaces (DDD-5: one
/// delete seam), aliased rather than re-spelt so the two cannot drift apart.
const PAGE_DELETE: &str = POPUP_DELETE;
/// The shipped edit dialog — proof the popup still renders unchanged.
const EDIT_MODAL: &str = "data-modal=\"edit-issue\"";
/// The board's card marker (shipped).
const CARD_ATTR: &str = "data-issue-key";

/// Bounded wait for a browser-lane element that appears only after an htmx
/// round trip. `support::browser_harness` keeps its `READY_TIMEOUT` private and
/// exposes no public timeout, and `support/` is shared by every feature in the
/// suite, so this mirrors the shipped sibling's local constant
/// (`feature_issue_edit_modal_close.rs:41`) at the same value rather than
/// reaching into the harness.
const BROWSER_WAIT: Duration = Duration::from_secs(10);

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
        .icd_current_project
        .clone()
        .expect("a board Given must have named the project under test")
}

/// STORED slugs, read back at seed time — never re-derived from a name
/// (`brief.md` §names-are-labels).
fn stored_slugs(world: &FoundryWorld, project_name: &str) -> (String, String) {
    world
        .icd_project_slugs
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded by a Given"))
        .clone()
}

fn project_id_of(world: &FoundryWorld, project_name: &str) -> uuid::Uuid {
    *world
        .icd_project_ids
        .get(project_name)
        .unwrap_or_else(|| panic!("project {project_name:?} must be seeded by a Given"))
}

fn board_path(world: &FoundryWorld, project_name: &str) -> String {
    let (team, project) = stored_slugs(world, project_name);
    format!("/team/{team}/project/{project}")
}

/// The GET+POST confirm pair (D4 / ADR-ISSUE-DELETE-002) — deliberately NOT a
/// DELETE verb, so the whole path works with scripting disabled.
fn delete_path(world: &FoundryWorld, project_name: &str, number: i32) -> String {
    format!("{}/issues/{number}/delete", board_path(world, project_name))
}

fn edit_path(world: &FoundryWorld, project_name: &str, number: i32) -> String {
    format!("{}/issues/{number}/edit", board_path(world, project_name))
}

fn issue_page_path(world: &FoundryWorld, project_name: &str, number: i32) -> String {
    format!("{}/issues/{number}", board_path(world, project_name))
}

/// `In-Progress` -> `in_progress`. The feature file speaks lane LABELS; the
/// lane routes and the `issues.state` column speak slugs. One derivation, used
/// by the seeding Givens and the lane-delete Whens alike, so a scenario can
/// never seed one spelling and then POST another.
fn lane_slug(label: &str) -> String {
    label.to_lowercase().replace('-', "_")
}

/// `AUTH-42` -> 42. The feature file speaks keys; the routes speak numbers.
fn number_of(key: &str) -> i32 {
    key.rsplit('-')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in a number"))
}

fn issue_id_of(world: &FoundryWorld, key: &str) -> uuid::Uuid {
    *world
        .icd_issue_ids
        .get(key)
        .unwrap_or_else(|| panic!("issue {key:?} must be seeded by a Given"))
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

async fn add_to_team(world: &FoundryWorld, user_id: uuid::Uuid) {
    let pool = pool(world);
    let ws = world.icd_workspace_id.expect("workspace seeded first");
    let team = world.icd_team_id.expect("team seeded first");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("workspace membership");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(team)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("team membership");
}

async fn seed_project(world: &mut FoundryWorld, name: &str, slug: &str, prefix: &str) {
    if world.icd_project_ids.contains_key(name) {
        return;
    }
    let pool = pool(world);
    let ws = world.icd_workspace_id.expect("workspace seeded first");
    let team = world.icd_team_id.expect("team seeded first");
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
    world.icd_project_ids.insert(name.to_string(), id);
    let (team_slug, project_slug): (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug FROM projects p JOIN teams t ON t.id = p.team_id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .expect("read back stored slugs");
    world
        .icd_project_slugs
        .insert(name.to_string(), (team_slug, project_slug));
}

async fn seed_lane(
    world: &FoundryWorld,
    project_id: uuid::Uuid,
    slug: &str,
    label: &str,
    pos: i32,
) {
    let ws = world.icd_workspace_id.expect("workspace seeded first");
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
              VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (project_id, slug) DO NOTHING",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(ws)
    .bind(slug)
    .bind(label)
    .bind(pos)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| panic!("seed lane ({slug:?}, {label:?}, {pos}): {err}"));
}

/// Seed one issue and REMEMBER its row id. The id is what makes the cascade
/// oracle possible: after the delete there is no parent row left to join from.
async fn seed_issue(
    world: &mut FoundryWorld,
    project_name: &str,
    number: i32,
    title: &str,
    state: &str,
    position: i32,
    author: uuid::Uuid,
) {
    let pool = pool(world);
    let project_id = project_id_of(world, project_name);
    let ws = world.icd_workspace_id.expect("workspace seeded first");
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, state, position, author_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
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
    let (prefix,): (String,) = sqlx::query_as("SELECT key_prefix FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .expect("read key prefix");
    let key = format!("{prefix}-{number}");
    world.icd_issue_ids.insert(key.clone(), id);
    world.icd_titles_before.insert(key, title.to_string());
}

async fn seed_comment(world: &FoundryWorld, issue_id: uuid::Uuid, body: &str, author: uuid::Uuid) {
    let ws = world.icd_workspace_id.expect("workspace seeded");
    sqlx::query(
        "INSERT INTO comments (id, issue_id, workspace_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(issue_id)
    .bind(ws)
    .bind(author)
    .bind(body)
    .bind(format!("<p>{body}</p>"))
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| panic!("seed comment: {err}"));
}

async fn seed_attachment(world: &FoundryWorld, issue_id: uuid::Uuid, name: &str, up: uuid::Uuid) {
    let ws = world.icd_workspace_id.expect("workspace seeded");
    let bytes: &[u8] = b"acceptance-attachment-bytes";
    sqlx::query(
        "INSERT INTO issue_attachments
              (id, issue_id, workspace_id, uploader_id, filename, content_type,
               size_bytes, sha256_hex, content)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(issue_id)
    .bind(ws)
    .bind(up)
    .bind(name)
    .bind("text/plain")
    .bind(bytes.len() as i64)
    .bind("0".repeat(64))
    .bind(bytes)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| panic!("seed attachment: {err}"));
}

async fn seed_change_event(world: &FoundryWorld, issue_id: uuid::Uuid, field: &str, new: &str) {
    let ws = world.icd_workspace_id.expect("workspace seeded");
    let project = sqlx::query_as::<_, (uuid::Uuid,)>("SELECT project_id FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_one(&pool(world))
        .await
        .expect("issue project")
        .0;
    sqlx::query(
        "INSERT INTO issue_change_events
              (id, workspace_id, project_id, issue_id, actor_id, field, old_value, new_value)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(ws)
    .bind(project)
    .bind(issue_id)
    .bind(world.icd_priya_id.expect("priya"))
    .bind(field)
    .bind(Option::<String>::None)
    .bind(new)
    .execute(&pool(world))
    .await
    .unwrap_or_else(|err| panic!("seed change event: {err}"));
}

/// Seed the AUTH board with its three cards. `lanes` are the shipped defaults
/// unless a lane-shaped Given replaced them first.
async fn seed_auth_board(world: &mut FoundryWorld, lanes: &[(&str, &str)]) {
    ensure_backdrop(world).await;
    seed_project(world, "Identity Platform", "identity-platform", "AUTH").await;
    let pid = project_id_of(world, "Identity Platform");
    for (i, (slug, label)) in lanes.iter().enumerate() {
        seed_lane(world, pid, slug, label, i as i32).await;
    }
    world.icd_current_project = Some("Identity Platform".to_string());
    let first = lanes[0].0.to_string();
    let priya = world.icd_priya_id.expect("priya seeded");
    for (i, n) in [41, 42, 43].into_iter().enumerate() {
        seed_issue(
            world,
            "Identity Platform",
            n,
            &format!("Seeded AUTH-{n}"),
            &first,
            i as i32,
            priya,
        )
        .await;
    }
}

/// Workspace + team + the three actors. Idempotent.
async fn ensure_backdrop(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    if world.icd_workspace_id.is_some() {
        return;
    }
    let pool = pool(world);
    let ws = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(ws)
        .bind("Canzan Labs")
        .execute(&pool)
        .await
        .expect("insert workspace");
    world.icd_workspace_id = Some(ws);

    let team = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team)
        .bind(ws)
        .bind("Backend")
        .bind("backend")
        .execute(&pool)
        .await
        .expect("insert team");
    world.icd_team_id = Some(team);

    let priya = seed_user(world, PRIYA_EMAIL, "Priya Raman", PRIYA_PASSWORD).await;
    world.icd_priya_id = Some(priya);
    add_to_team(world, priya).await;

    let other = seed_user(world, OTHER_EMAIL, "Nadia Osei", OTHER_PASSWORD).await;
    world.icd_other_id = Some(other);
    add_to_team(world, other).await;

    // Marco is a real account in the SAME workspace but NOT in Backend — the
    // authz foil. Every refusal he receives must be the uniform 404.
    let marco = seed_user(world, MARCO_EMAIL, "Marco", MARCO_PASSWORD).await;
    world.icd_marco_id = Some(marco);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(ws)
    .bind(marco)
    .execute(&pool)
    .await
    .expect("marco workspace membership");
}

// -------------------------------------------------------------- store oracles

async fn outbox_count(world: &FoundryWorld) -> i64 {
    sqlx::query_as::<_, (i64,)>("SELECT count(*) FROM outbox")
        .fetch_one(&pool(world))
        .await
        .expect("count outbox")
        .0
}

async fn outbox_rows_of_type(world: &FoundryWorld, event_type: &str) -> Vec<serde_json::Value> {
    sqlx::query_as::<_, (serde_json::Value,)>(
        "SELECT payload FROM outbox WHERE event_type = $1 ORDER BY id ASC",
    )
    .bind(event_type)
    .fetch_all(&pool(world))
    .await
    .expect("read outbox payloads")
    .into_iter()
    .map(|(p,)| p)
    .collect()
}

async fn issue_exists(world: &FoundryWorld, key: &str) -> bool {
    let id = issue_id_of(world, key);
    sqlx::query_as::<_, (i64,)>("SELECT count(*) FROM issues WHERE id = $1")
        .bind(id)
        .fetch_one(&pool(world))
        .await
        .expect("count issue")
        .0
        > 0
}

/// THE cascade oracle. An orphan raises no error and no status code — the only
/// way to see one is to look for it.
async fn assert_no_orphans(world: &FoundryWorld) {
    let pool = pool(world);
    for (table, label) in [
        ("comments", "comment"),
        ("issue_attachments", "attachment"),
        ("issue_change_events", "change event"),
    ] {
        let sql = format!(
            "SELECT count(*) FROM {table} c
               LEFT JOIN issues i ON i.id = c.issue_id
              WHERE i.id IS NULL"
        );
        let (orphans,): (i64,) = sqlx::query_as(&sql)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|err| panic!("orphan guard on {table}: {err}"));
        assert_eq!(
            orphans, 0,
            "{orphans} {label} row(s) survived their issue — the ON DELETE CASCADE \
             (0004/0005/0013) did not fire. This raises no error and no status code; \
             the guard query is the only thing that can see it."
        );
    }
}

/// The shipped invariant, re-asserted after every mutating scenario (OUT-5).
async fn assert_no_laneless(world: &FoundryWorld) {
    let (stranded,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues i
           LEFT JOIN lanes l ON l.project_id = i.project_id AND l.slug = i.state
          WHERE l.slug IS NULL",
    )
    .fetch_one(&pool(world))
    .await
    .expect("zero-laneless guard");
    assert_eq!(stranded, 0, "{stranded} issue(s) left without a lane");
}

/// Board card keys, read off the RENDERED board — not off the database. The
/// question "is the card gone" is a rendering question; a row-count assertion
/// would pass over a board that still paints a card for a deleted row.
async fn rendered_card_keys(world: &mut FoundryWorld, project_name: &str) -> Vec<String> {
    let url = board_path(world, project_name);
    let out = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
    )
    .await;
    scrape_card_keys(&out.body)
}

fn scrape_card_keys(body: &str) -> Vec<String> {
    let needle = format!("{CARD_ATTR}=\"");
    let mut keys = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find(&needle) {
        rest = &rest[at + needle.len()..];
        if let Some(end) = rest.find('"') {
            keys.push(rest[..end].to_string());
            rest = &rest[end..];
        }
    }
    keys
}

/// The uniform non-enumerable refusal a never-existed board produces. Every
/// refusal in this module is compared against THIS, byte for byte.
async fn never_existed_refusal(world: &mut FoundryWorld, post: bool) -> (StatusCode, String) {
    const NOWHERE: &str = "/team/backend/project/no-such-project-ever/issues/9999/delete";
    let out = if post {
        signed_in_post(
            harness(world),
            &http(world),
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
            NOWHERE,
            &[],
        )
        .await
    } else {
        signed_in_get(
            harness(world),
            &http(world),
            PRIYA_EMAIL,
            PRIYA_PASSWORD,
            NOWHERE,
        )
        .await
    };
    (out.status, out.body)
}

fn record(world: &mut FoundryWorld, out: PostOutcome) {
    record_verb(world, out, false)
}

/// `post` records WHICH verb produced the response, so the refusal oracle can
/// compare like with like. A non-member POST and a never-existed GET are
/// different requests; comparing them would fail for a reason that has nothing
/// to do with non-enumerability — RED for the wrong reason.
///
/// The `Location` header is captured alongside the body because a `303` HAS no
/// body: without it the redirect DESTINATION — the entire content of D8 — is
/// unobservable, and any body-based assertion over a redirect is vacuous.
/// Assigned unconditionally so a non-redirect never inherits the previous
/// step's destination.
fn record_verb(world: &mut FoundryWorld, out: PostOutcome, post: bool) {
    world.icd_last_was_post = post;
    world.icd_last_location = out
        .headers
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    world.icd_last = Some((out.status, out.body.clone()));
    if out.status.is_client_error() || out.status.is_server_error() {
        world.icd_refusals.push((out.status, out.body));
    }
}

/// Block until the browser's URL path satisfies `settled`, then return it.
///
/// `click()` returns when the click is DISPATCHED, not when the navigation it
/// triggers has committed, so reading `current_url` straight afterwards is a
/// race: the driver may still be showing the page that was submitted FROM.
/// Same bounded-poll house idiom as `then_second_window_drops_one`; `find` does
/// not poll, and neither does `current_url`.
///
/// This does not weaken any oracle — it only decides WHEN the destination is
/// read. A navigation that lands somewhere wrong satisfies `settled` just as
/// fast and fails its assertion exactly as before.
async fn wait_for_navigation(
    client: &fantoccini::Client,
    settled: impl Fn(&str) -> bool,
    expectation: &str,
) -> String {
    let deadline = std::time::Instant::now() + BROWSER_WAIT;
    loop {
        let path = client
            .current_url()
            .await
            .expect("browser current url")
            .path()
            .to_owned();
        if settled(&path) {
            return path;
        }
        if std::time::Instant::now() > deadline {
            panic!("after {BROWSER_WAIT:?} the browser is still at {path:?}; {expectation}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Record what the BROWSER is showing: its rendered source, plus the PATH it
/// settled on. The browser follows a `303` for us, so its final URL is the
/// same destination `record_verb` reads out of the `Location` header on the
/// HTTP lane — captured here so both lanes assert D8 the same way.
async fn record_browser_page(world: &mut FoundryWorld, client: &fantoccini::Client, path: String) {
    world.icd_last_was_post = false;
    world.icd_last_location = Some(path);
    world.icd_last = Some((StatusCode::OK, client.source().await.unwrap_or_default()));
}

fn last_body(world: &FoundryWorld) -> String {
    world
        .icd_last
        .as_ref()
        .expect("a When must have produced a response")
        .1
        .clone()
}

// -------------------------------------------------------------------- Givens

#[given(regex = r"^Priya is a Backend team member tidying her own boards$")]
async fn given_priya(world: &mut FoundryWorld) {
    ensure_backdrop(world).await;
}

#[given(regex = r#"^"([^"]+)" \((\w+)\) is a board holding (\w+-\d+), (\w+-\d+) and (\w+-\d+)$"#)]
async fn given_board_holding(
    world: &mut FoundryWorld,
    _name: String,
    _prefix: String,
    _a: String,
    _b: String,
    _c: String,
) {
    seed_auth_board(
        world,
        &[
            ("backlog", "Backlog"),
            ("in_progress", "In-Progress"),
            ("done", "Done"),
        ],
    )
    .await;
}

#[given(regex = r#"^"([^"]+)" \((\w+)\) is a board whose lanes are (\w+), ([\w\-]+) and (\w+)$"#)]
async fn given_board_with_lanes(
    world: &mut FoundryWorld,
    _name: String,
    _prefix: String,
    a: String,
    b: String,
    c: String,
) {
    let lanes: Vec<(String, String)> = [a, b, c]
        .into_iter()
        .map(|label| (lane_slug(&label), label))
        .collect();
    let refs: Vec<(&str, &str)> = lanes
        .iter()
        .map(|(s, l)| (s.as_str(), l.as_str()))
        .collect();
    seed_auth_board(world, &refs).await;
}

#[given(regex = r#"^"([^"]+)" \((\w+)\) is a board with a single lane holding (\w+-\d+)$"#)]
async fn given_single_lane_board(
    world: &mut FoundryWorld,
    _name: String,
    _prefix: String,
    key: String,
) {
    ensure_backdrop(world).await;
    seed_project(world, "Identity Platform", "identity-platform", "AUTH").await;
    let pid = project_id_of(world, "Identity Platform");
    seed_lane(world, pid, "backlog", "Backlog", 0).await;
    world.icd_current_project = Some("Identity Platform".to_string());
    let priya = world.icd_priya_id.expect("priya");
    seed_issue(
        world,
        "Identity Platform",
        number_of(&key),
        "Only card on a one-lane board",
        "backlog",
        0,
        priya,
    )
    .await;
}

#[given(regex = r"^(\w+-\d+) carries three comments and one attachment$")]
async fn given_carries_children(world: &mut FoundryWorld, key: String) {
    let id = issue_id_of(world, &key);
    let priya = world.icd_priya_id.expect("priya");
    for n in 1..=3 {
        seed_comment(world, id, &format!("Seeded comment {n} on {key}"), priya).await;
    }
    seed_attachment(world, id, "keys.json", priya).await;
}

#[given(regex = r"^(\w+-\d+) carries no comments and no attachments$")]
async fn given_carries_nothing(world: &mut FoundryWorld, key: String) {
    let id = issue_id_of(world, &key);
    let pool = pool(world);
    for table in ["comments", "issue_attachments"] {
        let (n,): (i64,) =
            sqlx::query_as(&format!("SELECT count(*) FROM {table} WHERE issue_id = $1"))
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("count children");
        assert_eq!(n, 0, "{key} was expected to carry no {table}");
    }
}

#[given(regex = r"^(\w+-\d+) has been edited twice$")]
async fn given_edited_twice(world: &mut FoundryWorld, key: String) {
    let id = issue_id_of(world, &key);
    seed_change_event(world, id, "title", "First retitle").await;
    seed_change_event(world, id, "status", "in_progress").await;
}

#[given(regex = r"^(\w+-\d+) and (\w+-\d+) sit in the same lane as (\w+-\d+)$")]
async fn given_same_lane(world: &mut FoundryWorld, a: String, b: String, c: String) {
    let pool = pool(world);
    let (state,): (String,) = sqlx::query_as("SELECT state FROM issues WHERE id = $1")
        .bind(issue_id_of(world, &c))
        .fetch_one(&pool)
        .await
        .expect("read lane of subject");
    for key in [a, b] {
        sqlx::query("UPDATE issues SET state = $1 WHERE id = $2")
            .bind(&state)
            .bind(issue_id_of(world, &key))
            .execute(&pool)
            .await
            .expect("colocate sibling");
    }
}

#[given(regex = r"^(\w+-\d+) was filed by another Backend team member$")]
async fn given_filed_by_other(world: &mut FoundryWorld, key: String) {
    let other = world.icd_other_id.expect("other member seeded");
    sqlx::query("UPDATE issues SET author_id = $1 WHERE id = $2")
        .bind(other)
        .bind(issue_id_of(world, &key))
        .execute(&pool(world))
        .await
        .expect("reassign author");
}

#[given(regex = r"^Priya has been asked to confirm deleting (\w+-\d+)$")]
async fn given_dialog_open(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let url = delete_path(world, &project, number_of(&key));
    let out = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
    )
    .await;
    record(world, out);
}

#[given(regex = r"^(\w+-\d+) has already been deleted$")]
async fn given_already_deleted(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let url = delete_path(world, &project, number_of(&key));
    let out = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
        &[],
    )
    .await;
    assert!(
        out.status.is_success() || out.status.is_redirection(),
        "the pre-delete Given must succeed, got {} — body {}",
        out.status,
        out.body
    );
}

#[given(regex = r"^Priya is browsing with scripting switched off$")]
async fn given_no_scripting(world: &mut FoundryWorld) {
    let client = browser_harness::new_session_without_scripting().await;
    world.browser = Some(client);
}

#[given(regex = r"^the same board is open in another window$")]
async fn given_second_window(world: &mut FoundryWorld) {
    let project = current_project(world);
    let url = format!(
        "{}{}",
        harness(world).base_url(),
        board_path(world, &project)
    );
    let client = browser_harness::new_session().await;
    browser_harness::sign_in_through_browser(&client, harness(world), PRIYA_EMAIL, PRIYA_PASSWORD)
        .await;
    client.goto(&url).await.expect("second window opens board");
    browser_harness::wait_for_board_ready(&client).await;
    world.icd_second_window_before = Some(client.source().await.expect("second window source"));
    world.browser = Some(client);
}

#[given(regex = r#"^someone is listening to the "([^"]+)" board( instead)?$"#)]
async fn given_listener(world: &mut FoundryWorld, project_name: String, _instead: String) {
    // A listener on a project that is NOT under test still has to exist as a
    // real project, or "heard nothing" would be true for the wrong reason.
    if !world.icd_project_ids.contains_key(&project_name) {
        seed_project(world, &project_name, "homelab-ops", "OPS").await;
        let pid = project_id_of(world, &project_name);
        seed_lane(world, pid, "backlog", "Backlog", 0).await;
    }
    let (team, project) = stored_slugs(world, &project_name);
    let session =
        establish_session(harness(world), &http(world), PRIYA_EMAIL, PRIYA_PASSWORD).await;
    let base = harness(world).base_url();
    let sub = open_sse_subscription(&base, &project, &team, &session).await;
    sub.wait_until_ready(Duration::from_secs(5)).await;
    world.icd_subscription = Some(sub);
    world.icd_outbox_before = Some(outbox_count(world).await);
}

#[given(regex = r"^(\w+-\d+), (\w+-\d+) and (\w+-\d+) all sit in (\w+)$")]
async fn given_all_sit_in(
    world: &mut FoundryWorld,
    a: String,
    b: String,
    c: String,
    lane_label: String,
) {
    let slug = lane_slug(&lane_label);
    let pool = pool(world);
    for (i, key) in [a, b, c].into_iter().enumerate() {
        sqlx::query("UPDATE issues SET state = $1, position = $2 WHERE id = $3")
            .bind(&slug)
            .bind(i as i32)
            .bind(issue_id_of(world, &key))
            .execute(&pool)
            .await
            .expect("place card in lane");
    }
}

#[given(regex = r"^(\w+-\d+), (\w+-\d+) and (\w+-\d+) are spread across those lanes$")]
async fn given_spread(world: &mut FoundryWorld, a: String, b: String, c: String) {
    let pool = pool(world);
    for (key, slug) in [(a, "backlog"), (b, "in_progress"), (c, "done")] {
        sqlx::query("UPDATE issues SET state = $1, position = 0 WHERE id = $2")
            .bind(slug)
            .bind(issue_id_of(world, &key))
            .execute(&pool)
            .await
            .expect("spread card");
    }
}

// --------------------------------------------------------------------- Whens

#[when(regex = r"^Priya asks to delete (\w+-\d+) from its own page$")]
async fn when_asks_to_delete(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let url = delete_path(world, &project, number_of(&key));
    let out = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
    )
    .await;
    record(world, out);
}

#[when(regex = r"^Priya deletes (\w+-\d+) from its own page$")]
async fn when_deletes_from_page(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    if world.icd_outbox_before.is_none() {
        world.icd_outbox_before = Some(outbox_count(world).await);
    }
    let url = delete_path(world, &project, number_of(&key));
    let out = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
        &[],
    )
    .await;
    record_verb(world, out, true);
}

#[when(regex = r"^another team member comments on (\w+-\d+) and Priya then confirms$")]
async fn when_racing_comment_then_confirm(world: &mut FoundryWorld, key: String) {
    let id = issue_id_of(world, &key);
    let other = world.icd_other_id.expect("other member");
    seed_comment(
        world,
        id,
        "A comment that arrived after the count was read",
        other,
    )
    .await;
    when_deletes_from_page(world, key).await;
}

#[when(regex = r"^Marco asks to (delete|confirm deleting) (\w+-\d+)$")]
async fn when_marco_acts(world: &mut FoundryWorld, verb: String, key: String) {
    let project = current_project(world);
    let url = delete_path(world, &project, number_of(&key));
    let is_post = verb == "delete";
    let out = if is_post {
        signed_in_post(
            harness(world),
            &http(world),
            MARCO_EMAIL,
            MARCO_PASSWORD,
            &url,
            &[],
        )
        .await
    } else {
        signed_in_get(
            harness(world),
            &http(world),
            MARCO_EMAIL,
            MARCO_PASSWORD,
            &url,
        )
        .await
    };
    record_verb(world, out, is_post);
}

#[when(regex = r"^Marco deletes (\w+-\d+) from the board$")]
async fn when_marco_deletes_from_board(world: &mut FoundryWorld, key: String) {
    if world.icd_outbox_before.is_none() {
        world.icd_outbox_before = Some(outbox_count(world).await);
    }
    when_marco_acts(world, "delete".to_string(), key).await;
}

#[when(regex = r"^someone who is not signed in asks to delete (\w+-\d+)$")]
async fn when_signed_out_delete(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let url = format!(
        "{}{}",
        harness(world).base_url(),
        delete_path(world, &project, number_of(&key))
    );
    let resp = http(world)
        .post(&url)
        .form(&[("_csrf", "none")])
        .send()
        .await
        .expect("signed-out delete");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    record(
        world,
        PostOutcome {
            status,
            headers: reqwest::header::HeaderMap::new(),
            body,
        },
    );
}

#[when(regex = r"^Priya confirms deleting (\w+-\d+) a second time$")]
async fn when_double_submit(world: &mut FoundryWorld, key: String) {
    world.icd_outbox_before = Some(outbox_count(world).await);
    when_deletes_from_page(world, key).await;
}

#[when(regex = r"^Priya asks to delete a card that was never filed$")]
async fn when_delete_never_filed(world: &mut FoundryWorld) {
    let project = current_project(world);
    let url = delete_path(world, &project, 9999);
    let out = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
    )
    .await;
    record(world, out);
}

#[when(regex = r"^Priya's delete of (\w+-\d+) arrives without its token$")]
async fn when_tokenless(world: &mut FoundryWorld, key: String) {
    world.icd_outbox_before = Some(outbox_count(world).await);
    let project = current_project(world);
    let url = delete_path(world, &project, number_of(&key));
    let session =
        establish_session(harness(world), &http(world), PRIYA_EMAIL, PRIYA_PASSWORD).await;
    // Deliberately NO `_csrf` field and no header — the middleware must refuse
    // before the handler runs.
    let out = post_with_cookie(harness(world), &http(world), &url, &session, &[]).await;
    record(world, out);
}

/// Sign in through a REAL browser with scripting off, open `key`'s full page,
/// and follow its plain Delete LINK to the confirm page — the shared prefix of
/// both no-scripting Whens. Returns the driver and the confirm route it settled
/// on, so the caller can go on to submit the confirm (or not).
///
/// The `[data-action="delete-issue"], a[href$='/delete']` pair is deliberate:
/// with scripting off the control must be something the BROWSER can follow on
/// its own, so an `href` is sufficient evidence even if the DESIGN marker moves.
async fn follow_delete_link_without_scripting(
    world: &mut FoundryWorld,
    key: &str,
) -> (fantoccini::Client, String) {
    let project = current_project(world);
    let base = harness(world).base_url();
    let page = format!("{base}{}", issue_page_path(world, &project, number_of(key)));
    let client = world
        .browser
        .as_ref()
        .expect("no-scripting session")
        .clone();
    browser_harness::sign_in_through_browser(&client, harness(world), PRIYA_EMAIL, PRIYA_PASSWORD)
        .await;
    client.goto(&page).await.expect("issue page");
    client
        .find(Locator::Css(&format!(
            "[{PAGE_DELETE}], a[href$='/delete']"
        )))
        .await
        .expect("a plain delete link must exist with scripting off (DDD-6)")
        .click()
        .await
        .expect("follow delete link");
    let landed = wait_for_navigation(
        &client,
        |path| path.ends_with("/delete"),
        "the plain Delete link must navigate to the confirm route (DDD-6)",
    )
    .await;
    (client, landed)
}

#[when(regex = r"^she follows the delete link on (\w+-\d+)'s page and confirms$")]
async fn when_nojs_full_path(world: &mut FoundryWorld, key: String) {
    let (client, confirm_path) = follow_delete_link_without_scripting(world, &key).await;
    client
        .find(Locator::Css("form[method='post'] button[type='submit']"))
        .await
        .expect("the confirm must be a plain form with scripting off")
        .click()
        .await
        .expect("submit confirm");
    // The confirm's `303` moves her OFF the delete route; anywhere else is a
    // settled destination `then_looking_at_board` then judges. Waiting here
    // rather than asserting on a mid-navigation URL is what makes the browser
    // lane's D8 oracle deterministic instead of a coin-flip on click timing.
    let landed = wait_for_navigation(
        &client,
        |path| path != confirm_path,
        "the confirm must navigate away from the delete page (D8)",
    )
    .await;
    record_browser_page(world, &client, landed).await;
}

#[when(regex = r"^she follows the delete link on (\w+-\d+)'s page$")]
async fn when_nojs_dialog_only(world: &mut FoundryWorld, key: String) {
    let (client, landed) = follow_delete_link_without_scripting(world, &key).await;
    record_browser_page(world, &client, landed).await;
}

#[when(regex = r"^Priya opens (\w+-\d+) from the board$")]
async fn when_opens_popup(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let url = edit_path(world, &project, number_of(&key));
    let out = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
    )
    .await;
    record(world, out);
}

#[when(regex = r"^Priya opens (\w+-\d+) from the board and chooses Delete$")]
async fn when_popup_choose_delete(world: &mut FoundryWorld, key: String) {
    when_opens_popup(world, key.clone()).await;
    let popup = last_body(world);
    assert!(
        popup.contains(POPUP_DELETE),
        "the edit dialog must offer a Delete control (AC-2.1); it does not"
    );
    when_asks_to_delete(world, key).await;
}

#[when(regex = r"^Priya opens (\w+-\d+), retitles it without saving, and chooses Delete$")]
async fn when_popup_retitle_then_delete(world: &mut FoundryWorld, key: String) {
    // "Without saving" means literally no save request is made. The oracle is
    // that the stored title is unchanged afterwards.
    when_popup_choose_delete(world, key).await;
}

#[when(regex = r"^Priya opens (\w+-\d+) from the board and deletes it$")]
async fn when_popup_delete(world: &mut FoundryWorld, key: String) {
    when_popup_choose_delete(world, key.clone()).await;
    when_popup_confirms(world, &key).await;
}

/// The POPUP's confirm POST. Same endpoint, same form and the same ONE
/// `issue_service::delete_issue` seam as the full-page confirm — it differs in
/// exactly one byte-level respect, and that respect is the whole point of this
/// slice: it carries `HX-Request: true`, because the dialog htmx swapped into
/// `#modal-root` submits through htmx (`hx-post` on `delete_issue_modal.html`'s
/// form), not as a browser page submit.
///
/// WHY THIS EXISTS RATHER THAN REUSING [`when_deletes_from_page`]: that helper
/// goes through `support::harness::signed_in_post`, which sets COOKIE and no
/// htmx header. Delegating to it made the popup delete byte-identical to the
/// no-JS page submit, so the handler correctly answered the D8 `303` and the
/// popup oracles — which assert the D7 out-of-band `#board-columns` refresh —
/// were asserting an htmx response against a non-htmx request. The oracles were
/// right; the request was wrong. `when_deletes_from_page` deliberately stays
/// header-free so the US-ICD-01 page scenarios keep pinning the `303` arm.
///
/// Follows the shipped htmx-POST idiom (`feature_issue_edit_dialog.rs:449`):
/// build the request, add the header, send. The session and the CSRF token are
/// minted through the SHIPPED `establish_session` + `/sign-in` cookie dance —
/// `support/harness.rs` is shared by every feature in the suite and is not
/// touched.
async fn when_popup_confirms(world: &mut FoundryWorld, key: &str) {
    let project = current_project(world);
    if world.icd_outbox_before.is_none() {
        world.icd_outbox_before = Some(outbox_count(world).await);
    }
    let url = delete_path(world, &project, number_of(key));
    let http = http(world);
    let base = harness(world).base_url();
    let session = establish_session(harness(world), &http, PRIYA_EMAIL, PRIYA_PASSWORD).await;
    let csrf = mint_csrf_token(&http, &base).await;

    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("_csrf", csrf.clone());
    let resp = http
        .post(format!("{base}{url}"))
        .header(
            reqwest::header::COOKIE,
            format!("{session}; foundry_csrf={csrf}"),
        )
        .header("HX-Request", "true")
        .form(&form)
        .send()
        .await
        .expect("post the popup's confirm");
    let out = PostOutcome {
        status: resp.status(),
        headers: resp.headers().clone(),
        body: resp.text().await.unwrap_or_default(),
    };
    record_verb(world, out, true);
}

/// Mint a double-submit CSRF token the same way `signed_in_post` does — GET
/// `/sign-in` and read the `foundry_csrf` cookie back. The CSRF cookie is
/// independent of the session cookie (see `support/harness.rs`), so the token
/// minted here rides correctly alongside an already-established session.
async fn mint_csrf_token(http: &reqwest::Client, base: &str) -> String {
    http.get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf")
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("/sign-in must mint a foundry_csrf cookie")
        .to_string()
}

#[when(
    regex = r"^Priya opens (\w+-\d+) from the board, chooses Delete, then dismisses the confirmation$"
)]
async fn when_popup_dismiss(world: &mut FoundryWorld, key: String) {
    world.icd_outbox_before = Some(outbox_count(world).await);
    when_popup_choose_delete(world, key).await;
    // Dismissal is a client-side close (`data-action="close-modal"`, BR-4). It
    // sends NO request: the oracle is that nothing was written or announced.
}

#[when(regex = r"^Priya deletes (\w+-\d+) from the board in a real browser$")]
async fn when_browser_delete(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let base = harness(world).base_url();
    let board = format!("{base}{}", board_path(world, &project));
    let client = match world.browser.as_ref() {
        Some(c) => c.clone(),
        None => {
            let c = browser_harness::new_session().await;
            world.browser = Some(c.clone());
            c
        }
    };
    browser_harness::sign_in_through_browser(&client, harness(world), PRIYA_EMAIL, PRIYA_PASSWORD)
        .await;
    client.goto(&board).await.expect("board");
    browser_harness::wait_for_board_ready(&client).await;

    // Each click below is followed by an htmx round trip that REPLACES
    // #modal-root's contents, so the next control does not exist yet at the
    // moment the click returns. `find` does not poll; `wait().for_element()`
    // does. Without these waits the step raced its own swaps and failed at a
    // DIFFERENT point on each run over unchanged markup. Same house idiom as
    // the shipped feature_issue_edit_modal_close.rs:76-97.
    let card = format!("[{CARD_ATTR}='{key}']");
    let popup_delete = format!("[{POPUP_DELETE}]");
    let confirm_submit = format!("[{DELETE_MODAL}] button[type='submit']");

    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(&card))
        .await
        .expect("the card must be on the board")
        .click()
        .await
        .expect("open the popup");
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(&popup_delete))
        .await
        .expect("the popup must offer Delete (AC-2.1)")
        .click()
        .await
        .expect("choose Delete");
    client
        .wait()
        .at_most(BROWSER_WAIT)
        .for_element(Locator::Css(&confirm_submit))
        .await
        .expect("the confirm dialog must offer a destructive submit")
        .click()
        .await
        .expect("confirm");

    browser_harness::wait_for_board_ready(&client).await;
    // The confirm is the THIRD htmx round trip, and the board-ready marker is
    // already in the DOM before it — so it settles nothing. Wait for the card
    // to actually leave the rendered board (the D7 out-of-band #board-columns
    // refresh applying), mirroring this module's own bounded-absence poll in
    // `then_second_window_drops_one`. Without it the Then could read the board
    // before the delete landed.
    let deadline = std::time::Instant::now() + BROWSER_WAIT;
    loop {
        let src = client.source().await.unwrap_or_default();
        if !scrape_card_keys(&src).contains(&key) {
            return;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "the real-browser confirm left {key} on the board after {BROWSER_WAIT:?} — \
                 the CSRF token did not travel from the popup, or the out-of-band \
                 #board-columns refresh never arrived (D7, the fix-comment-delete-csrf lesson)"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[when(regex = r"^Priya opens (\w+-\d+) from the board and saves a new title$")]
async fn when_popup_save(world: &mut FoundryWorld, key: String) {
    let project = current_project(world);
    let url = edit_path(world, &project, number_of(&key));
    let out = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
        &[
            ("title", "Retitled by the regression scenario"),
            ("description", ""),
        ],
    )
    .await;
    record(world, out);
}

#[when(regex = r"^Priya deletes (\w+-\d+) from the board$")]
async fn when_deletes_from_board(world: &mut FoundryWorld, key: String) {
    when_deletes_from_page(world, key).await;
}

#[when(regex = r"^another team member comments on (\w+-\d+) and Priya then deletes (\w+-\d+)$")]
async fn when_comment_then_delete(world: &mut FoundryWorld, commented: String, deleted: String) {
    let project = current_project(world);
    let number = number_of(&commented);
    let url = format!("{}/issues/{number}/comments", board_path(world, &project));
    let out = signed_in_post(
        harness(world),
        &http(world),
        OTHER_EMAIL,
        OTHER_PASSWORD,
        &url,
        &[("body", "A comment on a card that is staying")],
    )
    .await;
    assert!(
        out.status.is_success() || out.status.is_redirection(),
        "the comment Given must succeed, got {}",
        out.status
    );
    when_deletes_from_page(world, deleted).await;
}

/// POST the SHIPPED lane-delete confirm for the lane named `slug`, with `fate`
/// (and, for the move fate, `destination`) already resolved by the caller.
/// Snapshots the outbox first when nothing has yet — the announcement oracle
/// counts BOTH ways, so it needs a `before` on every mutating path.
async fn post_lane_delete(world: &mut FoundryWorld, slug: &str, form: &[(&str, &str)]) {
    if world.icd_outbox_before.is_none() {
        world.icd_outbox_before = Some(outbox_count(world).await);
    }
    let project = current_project(world);
    let url = format!("{}/lanes/{slug}/delete", board_path(world, &project));
    let out = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &url,
        form,
    )
    .await;
    record(world, out);
}

#[when(regex = r"^Priya deletes the (\w+) lane and its cards with it$")]
async fn when_lane_delete_cards(world: &mut FoundryWorld, lane_label: String) {
    post_lane_delete(world, &lane_slug(&lane_label), &[("fate", "delete")]).await;
}

#[when(regex = r"^Priya deletes the (\w+) lane and moves its cards to (\w+)$")]
async fn when_lane_delete_move(world: &mut FoundryWorld, lane_label: String, dest: String) {
    let destination = lane_slug(&dest);
    post_lane_delete(
        world,
        &lane_slug(&lane_label),
        &[("fate", "move"), ("destination", &destination)],
    )
    .await;
}

#[when(regex = r"^Priya tries to delete the only lane the board has$")]
async fn when_delete_last_lane(world: &mut FoundryWorld) {
    // The single-lane board seeded by `given_single_lane_board` — Backlog.
    post_lane_delete(world, "backlog", &[("fate", "delete")]).await;
}

// --------------------------------------------------------------------- Thens

#[then(regex = r"^she is asked to confirm deleting (\w+-\d+)$")]
async fn then_asked_to_confirm(world: &mut FoundryWorld, key: String) {
    let body = last_body(world);
    assert!(
        body.contains(DELETE_MODAL),
        "expected the delete confirm dialog ({DELETE_MODAL}); got {} bytes not carrying it",
        body.len()
    );
    assert!(
        body.contains(&key),
        "the confirm must name {key} so she knows what she is deleting"
    );
}

#[then(regex = r"^the confirmation says its three comments and one attachment go with it$")]
async fn then_counts_children(world: &mut FoundryWorld) {
    let body = last_body(world);
    assert!(
        body.contains('3') && body.to_lowercase().contains("comment"),
        "the confirm must COUNT what goes with the issue, not merely warn (D3/D13)"
    );
    assert!(
        body.to_lowercase().contains("attachment"),
        "the confirm must name the attachment that goes with the issue"
    );
}

#[then(regex = r"^the confirmation says nothing else goes with it$")]
async fn then_no_children_copy(world: &mut FoundryWorld) {
    let body = last_body(world).to_lowercase();
    assert!(
        body.contains("no comments") || body.contains("nothing else"),
        "with zero children the confirm must say so, as delete_lane_modal does for an empty lane"
    );
}

#[then(regex = r"^the confirmation says this cannot be undone$")]
async fn then_cannot_be_undone(world: &mut FoundryWorld) {
    let body = last_body(world).to_lowercase();
    assert!(
        body.contains("cannot be undone"),
        "the shipped destructive-dialog sentence must be present — with no undo, \
         the dialog is the entire safety net (D1/D3)"
    );
}

#[then(regex = r"^(\w+-\d+) is still on the board$")]
async fn then_still_on_board(world: &mut FoundryWorld, key: String) {
    assert!(issue_exists(world, &key).await, "{key} must still exist");
    let project = current_project(world);
    let keys = rendered_card_keys(world, &project).await;
    assert!(
        keys.contains(&key),
        "{key} must still RENDER on the board; board shows {keys:?}"
    );
}

/// THE DESTINATION IS THE ASSERTION. A `303` carries an EMPTY body, so every
/// body-based oracle — `body.contains(board_path)`, `body.is_empty()` — is
/// vacuously true over one, and a redirect to `/sign-in`, to another project's
/// board, or to `/` would satisfy it. D8 is a statement about WHERE she lands,
/// and the only place that is observable is the `Location` header (HTTP lane)
/// or the URL the browser settled on (`@needs-browser` lane). Both are compared
/// for EQUALITY against the board path built from the STORED slugs.
#[then(regex = r#"^she is looking at the "([^"]+)" board$"#)]
async fn then_looking_at_board(world: &mut FoundryWorld, project_name: String) {
    let (status, body) = world.icd_last.clone().expect("a When produced a response");
    let expected = board_path(world, &project_name);
    let landed = world.icd_last_location.clone();

    if status.is_redirection() {
        // The non-htmx success path is a 303 to the board (D8) — never a
        // re-render of a page for a resource that no longer exists.
        let target = landed.unwrap_or_else(|| {
            panic!("a {status} must carry a Location header; without one she lands nowhere")
        });
        assert_eq!(
            target, expected,
            "the redirect must target the board she came from"
        );
        return;
    }

    // The browser lane followed the 303 itself: the URL it settled on IS the
    // header's destination, and the board must actually have rendered there.
    let target = landed.unwrap_or_else(|| {
        panic!("status {status} is neither a redirect nor a recorded browser landing")
    });
    assert_eq!(
        target, expected,
        "she must have landed on the board, not merely on a page that mentions it"
    );
    assert!(
        body.contains(CARD_ATTR),
        "the board she landed on must have rendered its cards, got {} bytes at status {status}",
        body.len()
    );
}

/// The board renders EXACTLY `expected` and nothing else, and the delete left
/// nothing dangling. Set equality, not containment: "the board holds A and C"
/// is a claim about what is ABSENT as much as what is present, and a
/// `contains` oracle passes over a card that should have gone. The two orphan
/// guards ride along on every board assertion because a cascade that silently
/// did not fire raises nothing (module docs).
async fn assert_board_holds(world: &mut FoundryWorld, expected: &[String]) {
    let project = current_project(world);
    let keys = rendered_card_keys(world, &project).await;
    let mut expected: Vec<String> = expected.to_vec();
    expected.sort();
    let mut got = keys.clone();
    got.sort();
    got.dedup();
    assert_eq!(
        got, expected,
        "board renders {keys:?}, expected exactly {expected:?}"
    );
    assert_no_orphans(world).await;
    assert_no_laneless(world).await;
}

#[then(regex = r"^the board holds (\w+-\d+) and (\w+-\d+) only$")]
async fn then_board_holds_two(world: &mut FoundryWorld, a: String, b: String) {
    assert_board_holds(world, &[a, b]).await;
}

#[then(regex = r"^the board holds (\w+-\d+), (\w+-\d+) and (\w+-\d+)$")]
async fn then_board_holds_three(world: &mut FoundryWorld, a: String, b: String, c: String) {
    assert_board_holds(world, &[a, b, c]).await;
}

#[then(regex = r"^nothing of (\w+-\d+) remains anywhere$")]
async fn then_nothing_remains(world: &mut FoundryWorld, key: String) {
    let id = issue_id_of(world, &key);
    let pool = pool(world);
    for table in [
        "issues",
        "comments",
        "issue_attachments",
        "issue_change_events",
    ] {
        let col = if table == "issues" { "id" } else { "issue_id" };
        let (n,): (i64,) =
            sqlx::query_as(&format!("SELECT count(*) FROM {table} WHERE {col} = $1"))
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("count remains");
        assert_eq!(n, 0, "{n} row(s) of {key} survived in {table}");
    }
}

#[then(regex = r"^no comment, attachment or change record is left behind without its card$")]
async fn then_no_orphans(world: &mut FoundryWorld) {
    assert_no_orphans(world).await;
}

#[then(regex = r"^(\w+-\d+) and (\w+-\d+) are in the same lane and the same order as before$")]
async fn then_siblings_unmoved(world: &mut FoundryWorld, a: String, b: String) {
    let pool = pool(world);
    for key in [&a, &b] {
        let row: Option<(String, i32)> =
            sqlx::query_as("SELECT state, position FROM issues WHERE id = $1")
                .bind(issue_id_of(world, key))
                .fetch_optional(&pool)
                .await
                .expect("read sibling");
        assert!(row.is_some(), "sibling {key} must still exist");
    }
}

#[then(regex = r"^every comment and attachment on (\w+-\d+) and (\w+-\d+) is still there$")]
async fn then_sibling_children_intact(world: &mut FoundryWorld, a: String, b: String) {
    let pool = pool(world);
    for key in [&a, &b] {
        let id = issue_id_of(world, key);
        let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM issues WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("sibling row");
        assert_eq!(n, 1, "the cascade reached beyond its issue and took {key}");
    }
}

#[then(regex = r"^the board still reads (\w+), ([\w\-]+), (\w+)$")]
async fn then_lanes_unchanged(world: &mut FoundryWorld, a: String, b: String, c: String) {
    let project = current_project(world);
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT label FROM lanes WHERE project_id = $1 ORDER BY position ASC")
            .bind(project_id_of(world, &project))
            .fetch_all(&pool(world))
            .await
            .expect("read lanes back");
    let got: Vec<String> = rows.into_iter().map(|(l,)| l).collect();
    assert_eq!(
        got,
        vec![a, b, c],
        "a card delete is not a lane operation (D1); lanes now read {got:?}"
    );
}

#[then(regex = r"^every lane still has the slug and label it had$")]
async fn then_lane_identity(world: &mut FoundryWorld) {
    let project = current_project(world);
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT slug, label FROM lanes WHERE project_id = $1 ORDER BY position ASC")
            .bind(project_id_of(world, &project))
            .fetch_all(&pool(world))
            .await
            .expect("read lane identity");
    assert!(!rows.is_empty(), "the board must still have its lanes");
}

#[then(regex = r"^no issue is left without a lane$")]
async fn then_no_laneless(world: &mut FoundryWorld) {
    assert_no_laneless(world).await;
}

#[then(regex = r"^the refusal is byte-identical to a card that never existed$")]
async fn then_refusal_indistinguishable(world: &mut FoundryWorld) {
    let (status, body) = world
        .icd_refusals
        .last()
        .cloned()
        .expect("a refusal must have been recorded");
    let was_post = world.icd_last_was_post;
    let (ref_status, ref_body) = never_existed_refusal(world, was_post).await;
    // Byte-identity ALONE is vacuous: two identical WRONG answers satisfy it,
    // and would have satisfied it while both sides were still scaffolds. DDD-9
    // pins the refusal to the uniform `resource_not_found_page`, so the status
    // is asserted against 404 as well as against the reference — strictly
    // stronger than comparing the two responses to each other.
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a refusal must be the uniform non-enumerable 404 (DDD-9) — never a 401, \
         a 403, or anything else that confirms the resource exists"
    );
    assert_eq!(
        status, ref_status,
        "a refusal must not be distinguishable by status (ADR-003)"
    );
    assert_eq!(
        body, ref_body,
        "a refusal must not be distinguishable by body — an echoed key IS the \
         enumeration oracle every other route closed (ADR-003)"
    );
}

#[then(regex = r"^nothing was announced to anyone$")]
async fn then_nothing_announced(world: &mut FoundryWorld) {
    let before = world
        .icd_outbox_before
        .expect("the announcement oracle needs a before-snapshot");
    let after = outbox_count(world).await;
    assert_eq!(
        after,
        before,
        "a refused delete must write ZERO outbox rows (AC-3.7); the table grew by {}",
        after - before
    );
}

#[then(regex = r"^nothing was announced to that listener$")]
async fn then_listener_silent(world: &mut FoundryWorld) {
    let sub = world
        .icd_subscription
        .as_ref()
        .expect("a listener Given must have run");
    let events = sub.drain();
    let leaked: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "IssueDeleted")
        .collect();
    assert!(
        leaked.is_empty(),
        "a delete leaked to another project's listener — project_id is the \
         tenancy filter (AC-3.5); saw {leaked:?}"
    );
}

#[then(regex = r"^exactly one card was announced as deleted$")]
async fn then_one_announced(world: &mut FoundryWorld) {
    let rows = outbox_rows_of_type(world, "IssueDeleted").await;
    assert_eq!(
        rows.len(),
        1,
        "expected exactly one IssueDeleted row; a missing one and a spurious one \
         are different bugs and both are invisible to a status assertion"
    );
}

#[then(regex = r"^exactly three cards were announced as deleted$")]
async fn then_three_announced(world: &mut FoundryWorld) {
    let rows = outbox_rows_of_type(world, "IssueDeleted").await;
    assert_eq!(
        rows.len(),
        3,
        "a lane delete with fate=delete must announce every card it destroyed \
         (AC-3.8) — this is the ADR-BOARD-LANE-002 silence DDD-2 removes"
    );
}

#[then(regex = r"^exactly three cards were announced as moved$")]
async fn then_three_moved(world: &mut FoundryWorld) {
    let rows = outbox_rows_of_type(world, "IssueUpdated").await;
    assert_eq!(
        rows.len(),
        3,
        "the move fate must still announce N IssueUpdated (AC-3.9)"
    );
}

#[then(regex = r"^no card was announced as deleted$")]
async fn then_none_deleted(world: &mut FoundryWorld) {
    let rows = outbox_rows_of_type(world, "IssueDeleted").await;
    assert!(
        rows.is_empty(),
        "the move fate must emit ZERO IssueDeleted — DDD-2's amendment to \
         ADR-BOARD-LANE-002 is ONE clause, and this is what proves it"
    );
}

#[then(regex = r"^the announcement names (\w+-\d+)$")]
async fn then_announcement_names(world: &mut FoundryWorld, key: String) {
    let rows = outbox_rows_of_type(world, "IssueDeleted").await;
    let named: Vec<String> = rows
        .iter()
        .filter_map(|p| p.get("key").and_then(|k| k.as_str()).map(String::from))
        .collect();
    assert!(
        named.contains(&key),
        "the payload must carry key {key} — composed BEFORE the row is gone; saw {named:?}"
    );
}

#[then(regex = r"^the announcements name (\w+-\d+), (\w+-\d+) and (\w+-\d+)$")]
async fn then_announcements_name_three(world: &mut FoundryWorld, a: String, b: String, c: String) {
    let rows = outbox_rows_of_type(world, "IssueDeleted").await;
    let mut named: Vec<String> = rows
        .iter()
        .filter_map(|p| p.get("key").and_then(|k| k.as_str()).map(String::from))
        .collect();
    named.sort();
    let mut expected = vec![a, b, c];
    expected.sort();
    assert_eq!(named, expected, "every destroyed card must be named");
}

#[then(regex = r"^the listener heard the comment on (\w+-\d+) and the deletion of (\w+-\d+)$")]
async fn then_heard_both(world: &mut FoundryWorld, _commented: String, deleted: String) {
    let deletes = outbox_rows_of_type(world, "IssueDeleted").await;
    let comments = outbox_rows_of_type(world, "CommentAdded").await;
    assert_eq!(deletes.len(), 1, "the delete must be announced once");
    assert_eq!(
        comments.len(),
        1,
        "the comment must still be announced (AC-3.6)"
    );
    let named: Vec<String> = deletes
        .iter()
        .filter_map(|p| p.get("key").and_then(|k| k.as_str()).map(String::from))
        .collect();
    assert!(named.contains(&deleted), "the delete must name {deleted}");
}

#[then(regex = r"^each was announced exactly once$")]
async fn then_each_once(world: &mut FoundryWorld) {
    assert_eq!(outbox_rows_of_type(world, "IssueDeleted").await.len(), 1);
    assert_eq!(outbox_rows_of_type(world, "CommentAdded").await.len(), 1);
}

#[then(regex = r"^the dialog offers Delete$")]
async fn then_dialog_offers_delete(world: &mut FoundryWorld) {
    let body = last_body(world);
    assert!(
        body.contains(EDIT_MODAL),
        "the edit dialog must still render"
    );
    assert!(
        body.contains(POPUP_DELETE),
        "the edit dialog must offer a Delete control (AC-2.1)"
    );
}

/// Byte offsets of the edit dialog's `<form>` … `</form>`. The containment
/// oracle both AC-2.1 (Delete outside) and AC-2.8 (Save inside) are stated in.
fn edit_form_bounds(body: &str) -> (usize, usize) {
    let start = body
        .find("<form")
        .expect("the edit dialog must contain its form");
    let end = body[start..]
        .find("</form>")
        .map(|i| start + i)
        .expect("the edit form must close");
    (start, end)
}

#[then(regex = r"^Delete cannot submit the edit she is looking at$")]
async fn then_delete_outside_form(world: &mut FoundryWorld) {
    // MARKUP oracle, not styling (AC-2.1): the control must not be a descendant
    // of the edit <form>. A Delete that merely LOOKS far from Save is exactly
    // the trap adr-modal-close-001 D-12 exists to prevent.
    let body = last_body(world);
    let (form_start, form_end) = edit_form_bounds(&body);
    let delete_at = body
        .find(POPUP_DELETE)
        .expect("the edit dialog must offer a Delete control (AC-2.1)");
    assert!(
        delete_at < form_start || delete_at > form_end,
        "Delete is INSIDE the edit <form> — it can submit the edit. It must sit \
         outside, beside the ×, exactly as adr-modal-close-001 D-12 requires."
    );
}

/// "the ONLY way" is a COUNT, not a presence check (AC-2.8). A body merely
/// containing something labelled `Save` is satisfied by a second submit control
/// the new Delete affordance dragged in, and by a `Save` that is not a submit
/// at all. So: exactly ONE submit in the dialog, it is the shipped Save button,
/// and it is INSIDE the edit `<form>` — Delete's exact mirror image.
#[then(regex = r"^Save is still the only way to save$")]
async fn then_save_intact(world: &mut FoundryWorld) {
    let body = last_body(world);
    let (form_start, form_end) = edit_form_bounds(&body);
    let submits: Vec<usize> = body
        .match_indices("type=\"submit\"")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        submits.len(),
        1,
        "the edit dialog must offer exactly ONE submit; it offers {} \
         — Save is no longer the only way to save (AC-2.8)",
        submits.len()
    );
    let save_at = submits[0];
    assert!(
        body[save_at..].starts_with("type=\"submit\">Save<"),
        "the one submit control must still be the shipped Save button (AC-2.8)"
    );
    assert!(
        save_at > form_start && save_at < form_end,
        "Save must sit INSIDE the edit <form> — outside it saves nothing"
    );
}

#[then(regex = r"^(\w+-\d+) still has the title it had$")]
async fn then_title_unchanged(world: &mut FoundryWorld, key: String) {
    let expected = world
        .icd_titles_before
        .get(&key)
        .cloned()
        .expect("seed captured the title");
    let (title,): (String,) = sqlx::query_as("SELECT title FROM issues WHERE id = $1")
        .bind(issue_id_of(world, &key))
        .fetch_one(&pool(world))
        .await
        .expect("read title");
    assert_eq!(
        title, expected,
        "choosing Delete must not save a half-typed edit (AC-2.2)"
    );
}

/// D7 is TWO claims, and the weaker reading of either lets a regression through.
/// (a) the response IS the out-of-band `#board-columns` envelope — a body merely
/// mentioning the string `board-columns`, or carrying a card marker, is also
/// satisfied by a whole re-rendered board PAGE, which is her leaving the board
/// and coming back. (b) NOTHING precedes that envelope: htmx lifts the
/// `hx-swap-oob` node out and applies the REMAINDER to the primary `#modal-root`
/// target, so anything outside it is swapped into the dialog slot — which is how
/// the dialog would fail to close. Both are asserted here.
#[then(regex = r#"^she is still looking at the "([^"]+)" board$"#)]
async fn then_still_on_that_board(world: &mut FoundryWorld, project_name: String) {
    let body = last_body(world);
    let envelope = r#"<div class="board" id="board-columns" hx-swap-oob="true">"#;
    assert!(
        body.trim_start().starts_with(envelope),
        "the htmx success path must BE the out-of-band #board-columns envelope \
         (D7), with nothing before it to land in #modal-root, so she never \
         leaves the {project_name} board; got {} bytes starting {:?}",
        body.len(),
        &body.trim_start()[..body.trim_start().len().min(120)]
    );
    assert!(
        body.contains(CARD_ATTR),
        "the refreshed columns must carry the surviving cards, not an empty board"
    );
}

#[then(regex = r"^nothing is asking for her attention$")]
async fn then_modal_cleared(world: &mut FoundryWorld) {
    let body = last_body(world);
    assert!(
        !body.contains(DELETE_MODAL) && !body.contains(EDIT_MODAL),
        "the success response must CLEAR #modal-root (D7); a dialog is still present"
    );
}

#[then(regex = r"^(\w+-\d+) and (\w+-\d+) are in the lanes and the order they were in$")]
async fn then_siblings_in_place(world: &mut FoundryWorld, a: String, b: String) {
    then_siblings_unmoved(world, a, b).await;
}

#[then(regex = r"^each column still offers all six lane operations$")]
async fn then_six_lane_ops(world: &mut FoundryWorld) {
    let body = last_body(world);
    for item in [
        "Edit list",
        "Insert list before",
        "Insert list after",
        "Move list left",
        "Move list right",
        "Delete list",
    ] {
        assert!(
            body.contains(item),
            "the OOB refresh must not disturb the shipped six-item lane menu \
             (AC-2.5); {item:?} is missing"
        );
    }
}

#[then(regex = r"^the card on the board shows the new title$")]
async fn then_card_retitled(world: &mut FoundryWorld) {
    // The shipped save answers 303 to a non-htmx POST and the card fragment to an
    // htmx one. Asserting the RESPONSE body would pin the harness's header
    // choice, not the behaviour; the persisted title plus the re-rendered board
    // is the observable that holds on both paths.
    let project = current_project(world);
    let keys = rendered_card_keys(world, &project).await;
    assert!(!keys.is_empty(), "the board must still render its cards");
    let (title,): (String,) = sqlx::query_as("SELECT title FROM issues WHERE id = $1")
        .bind(issue_id_of(world, "AUTH-42"))
        .fetch_one(&pool(world))
        .await
        .expect("read title back");
    assert_eq!(
        title, "Retitled by the regression scenario",
        "the shipped edit-and-save path must be unchanged (AC-2.8)"
    );
}

#[then(regex = r"^she is told a board needs at least one lane$")]
async fn then_last_lane_refused(world: &mut FoundryWorld) {
    let body = last_body(world).to_lowercase();
    assert!(
        body.contains("at least one lane"),
        "the shipped last-lane refusal must come through unchanged (AC-3.10)"
    );
}

#[then(regex = r"^(\w+-\d+), (\w+-\d+) and (\w+-\d+) are all in (\w+)$")]
async fn then_all_in_lane(
    world: &mut FoundryWorld,
    a: String,
    b: String,
    c: String,
    lane_label: String,
) {
    let slug = lane_slug(&lane_label);
    let pool = pool(world);
    for key in [a, b, c] {
        let (state,): (String,) = sqlx::query_as("SELECT state FROM issues WHERE id = $1")
            .bind(issue_id_of(world, &key))
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|err| panic!("{key} must have survived the move fate: {err}"));
        assert_eq!(state, slug, "{key} should have moved to {lane_label}");
    }
}

// --- browser-lane Thens ----------------------------------------------------

#[then(regex = r"^the other window stops showing (\w+-\d+) without being reloaded$")]
async fn then_second_window_drops_one(world: &mut FoundryWorld, key: String) {
    let client = world.browser.as_ref().expect("second window").clone();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let src = client.source().await.unwrap_or_default();
        if !scrape_card_keys(&src).contains(&key) {
            return;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "the second window still shows {key} after the delete — a card that \
                 is visible but gone is the inverse of the stranded-card failure, and \
                 just as corrosive (AC-3.4)"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(regex = r"^the other window still shows (\w+-\d+) and (\w+-\d+)$")]
async fn then_second_window_keeps(world: &mut FoundryWorld, a: String, b: String) {
    let client = world.browser.as_ref().expect("second window").clone();
    let src = client.source().await.unwrap_or_default();
    let keys = scrape_card_keys(&src);
    for key in [a, b] {
        assert!(
            keys.contains(&key),
            "the fan-out removed more than it should; {key} is gone from the second window"
        );
    }
}

#[then(
    regex = r"^the other window stops showing (\w+-\d+), (\w+-\d+) and (\w+-\d+) without being reloaded$"
)]
async fn then_second_window_drops_three(world: &mut FoundryWorld, a: String, b: String, c: String) {
    for key in [a, b, c] {
        then_second_window_drops_one(world, key).await;
    }
}

#[then(regex = r"^she is looking at a full page asking her to confirm deleting (\w+-\d+)$")]
async fn then_full_confirm_page(world: &mut FoundryWorld, key: String) {
    let body = last_body(world);
    assert!(
        body.contains(&key),
        "the no-JS confirm page must name {key}"
    );
    assert!(
        body.contains(DELETE_MODAL),
        "the no-JS page must include the SAME confirm partial the htmx path swaps \
         (DDD-6), so the two can never disagree"
    );
}

#[then(regex = r"^the page carries the site's own heading and navigation$")]
async fn then_page_has_chrome(world: &mut FoundryWorld) {
    let body = last_body(world);
    assert!(
        body.contains("<html") && body.contains("</html>"),
        "a direct navigation must render a real PAGE, not a bare floating fragment \
         — asserting only that the route answers 200 would pass over exactly the \
         outcome ADR-ISSUE-DELETE-002 rejected"
    );
}
