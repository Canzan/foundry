//! instance-admin-workspace-rename step definitions
//! (`tests/features/instance-admin-workspace-rename.feature`, DISTILL 2026-10-05,
//! every scenario `@pending`).
//!
//! The seams these steps drive are the DESIGN-pinned driving ports, nothing
//! internal: `POST /admin/instance/workspaces/{workspace_id}/rename` (form `name`
//! and `_csrf`), `GET /admin/instance/workspaces` (`[data-workspace-row]`, then
//! `[data-workspace-head]`, then `[data-workspace-name]`, the rename form and the
//! `[data-error-slot]`), any app-shell page (`.sidebar__workspace` and `title`), and
//! the `foundry doctor export-workspace` / `verify-export` CLI. Every step is
//! real: HTTP against the in-process router with real session + CSRF layers, SQL
//! reads of the per-scenario Postgres schema, a real headless Chrome, and the real
//! `foundry` binary. No placeholder is needed at this layer: DELIVER's new API
//! (`Store::rename_workspace_with_audit`, `workspaces::rename_workspace`, the
//! handler) is reached only through those ports.
//!
//! THE RECORD ORACLE (D5, DDD-5): the rename record is read back from
//! `workspace_rename_events` through SQL. Until migration 0018 exists the read
//! panics with a `MISSING:` message, so every scenario that asserts "nothing went
//! on record" is RED today for that reason (classified MISSING_FUNCTIONALITY in
//! the feature delta). That read is also what keeps the refusal scenarios from
//! being vacuous: an unmounted route answers the same uniform 404 they assert.
//!
//! STATE DELTA (Mandate 8): every rename captures the observable universe — every
//! workspace `(id, name)` and every rename record — before the request, and the
//! outcome steps assert it after: an effective rename moves exactly one name and
//! appends exactly one record (bounded change); a no-op, a refusal, or a forged
//! request moves nothing (fail-closed: any undeclared change is a violation).
//!
//! LAYER 3 (real adapter + real HTTP, `@real-io`): example-based only (Mandates 9
//! and 11), no property machinery. The 24/25 boundary is pinned by named examples
//! here; the classifier's property suite is DELIVER's, at the unit layer.
//!
//! Shared vocabulary (cucumber steps are global): the Background, Marco, the
//! never-existed answer, the dashboard in the browser, and the project grouping
//! assertion are the precedent's steps
//! (`feature_instance_admin_project_rename.rs`), reused verbatim. Every step
//! defined here says "workspace" so it cannot collide with a project step.

use crate::support::browser_harness;
use crate::support::harness::{
    ensure_postgres, establish_session, post_with_cookie, signed_in_get, signed_in_post,
    InProcHarness, PostOutcome,
};
use crate::world::FoundryWorld;
use assert_cmd::Command as AssertCommand;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use scraper::{Html, Selector};
use secrecy::SecretString;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use time::OffsetDateTime;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
/// The precedent's Background seeds Priya and Marco with these credentials.
const PRIYA_EMAIL: &str = "priya@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const MARCO_EMAIL: &str = "marco@canzan.test";
const MARCO_PASSWORD: &str = "marco-correct-horse-battery-staple";
const MEMBER_PASSWORD: &str = "member-correct-horse-battery-staple";

/// DESIGN-pinned seams (feature delta, DESIGN [REF] Handoff). If DELIVER moves
/// one, the head partial and this module move in the same change.
fn rename_url(workspace_id: &str) -> String {
    format!("/admin/instance/workspaces/{workspace_id}/rename")
}
const DASHBOARD_PATH: &str = "/admin/instance/workspaces";
/// The refusal fragment's marker in its attribute form. The bare substring
/// `workspace-rename-error` also appears in every head's error-slot id
/// (`workspace-rename-error-{id}`), so only the attribute form tells a refusal
/// apart from a correct head.
const ERROR_FRAGMENT_ATTR: &str = r#"data-hx-fragment="workspace-rename-error""#;
const HEAD_MARKER: &str = "data-workspace-head";

/// The precedent's browser Given plants this page-lifetime marker after load; a
/// full navigation wipes it, so reading it back proves the swap did not reload.
const PAGE_MARKER_GET: &str = "return window.__iapr_page_marker || null;";

/// One rename record as the store holds it (DDD-5 columns):
/// `(workspace_id, actor_id, old_name, new_name, created_at)`.
pub type RenameRecord = (uuid::Uuid, uuid::Uuid, String, String, OffsetDateTime);

/// The observable universe of a workspace rename: every workspace's name by id,
/// and every rename record (`None` while the record does not exist in the schema).
#[derive(Debug, Clone, PartialEq)]
pub struct Universe {
    pub workspaces: BTreeMap<uuid::Uuid, String>,
    pub records: Option<Vec<RenameRecord>>,
}

// ===========================================================================
// Harness plumbing
// ===========================================================================

fn now_anchor() -> OffsetDateTime {
    OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(
            reqwest::Client::builder()
                .redirect(Policy::none())
                .cookie_store(false)
                .build()
                .expect("build reqwest client"),
        );
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

/// The database clock, read outside any rename transaction.
async fn db_now(world: &FoundryWorld) -> OffsetDateTime {
    let (now,): (OffsetDateTime,) = sqlx::query_as("SELECT clock_timestamp()")
        .fetch_one(&pool(world))
        .await
        .expect("read the database clock");
    now
}

/// A workspace's id by the name the scenario calls it: a name it was renamed to
/// in this scenario first, then its seed name, then a unique stored name.
async fn resolve(world: &FoundryWorld, label: &str) -> uuid::Uuid {
    if let Some(id) = world.iawr_aliases.get(label) {
        return *id;
    }
    if let Some(id) = world.iapr_workspace_ids.get(label) {
        return *id;
    }
    let ids: Vec<(uuid::Uuid,)> = sqlx::query_as("SELECT id FROM workspaces WHERE name = $1")
        .bind(label)
        .fetch_all(&pool(world))
        .await
        .expect("look a workspace up by name");
    match ids.as_slice() {
        [(id,)] => *id,
        _ => panic!("the scenario names workspace {label:?}, which matches {ids:?}"),
    }
}

async fn capture_universe(world: &FoundryWorld) -> Universe {
    let pool = pool(world);
    let workspaces: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, name FROM workspaces")
        .fetch_all(&pool)
        .await
        .expect("read every workspace");
    let (present,): (bool,) =
        sqlx::query_as("SELECT to_regclass('workspace_rename_events') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("probe for the rename record");
    let records = if present {
        Some(
            sqlx::query_as(
                "SELECT workspace_id, actor_id, old_name, new_name, created_at
                   FROM workspace_rename_events ORDER BY created_at, id",
            )
            .fetch_all(&pool)
            .await
            .expect("read the rename record (columns per DDD-5)"),
        )
    } else {
        None
    };
    Universe {
        workspaces: workspaces.into_iter().collect(),
        records,
    }
}

fn records_of(universe: &Universe) -> &[RenameRecord] {
    universe.records.as_deref().unwrap_or_else(|| {
        panic!(
            "MISSING: the workspace rename record does not exist yet \
             (migration 0018_workspace_rename_events, DDD-5), so nothing can be on record"
        )
    })
}

fn before(world: &FoundryWorld) -> Universe {
    world
        .iawr_before
        .clone()
        .expect("the universe was captured before the rename was sent")
}

/// Fail-closed state delta over the rename universe. `change = None`: nothing
/// moves. `change = Some((id, new))`: only workspace `id`'s name moves, to `new`,
/// and exactly one record is appended for it — by `actor`, from the name it had
/// before, to `new`, stamped inside `window`. Earlier records are never altered.
fn assert_universe_delta(
    before: &Universe,
    after: &Universe,
    change: Option<(uuid::Uuid, &str)>,
    actor: Option<uuid::Uuid>,
    window: Option<(OffsetDateTime, OffsetDateTime)>,
) {
    assert_eq!(
        before.workspaces.keys().collect::<Vec<_>>(),
        after.workspaces.keys().collect::<Vec<_>>(),
        "a rename must neither create nor remove a workspace (D8)"
    );
    for (id, old_name) in &before.workspaces {
        let now = &after.workspaces[id];
        match change {
            Some((changed, new_name)) if changed == *id => assert_eq!(
                now, new_name,
                "workspace {id} must now be named {new_name:?}"
            ),
            _ => assert_eq!(
                now, old_name,
                "workspace {id} must be untouched; {old_name:?} -> {now:?}"
            ),
        }
    }
    let (rb, ra) = (records_of(before), records_of(after));
    match change {
        None => assert_eq!(
            ra, rb,
            "nothing new may go on record (a no-op or a refusal records nothing, D4/KPI-2)"
        ),
        Some((changed, new_name)) => {
            assert!(
                rb.iter().all(|r| ra.contains(r)),
                "the record is append-only: an earlier entry changed or vanished; before {rb:?} after {ra:?}"
            );
            let added: Vec<&RenameRecord> = ra.iter().filter(|r| !rb.contains(r)).collect();
            assert_eq!(
                added.len(),
                1,
                "an effective rename appends exactly one record (D5); added {added:?}"
            );
            let (workspace_id, actor_id, old, new, at) = added[0];
            assert_eq!(
                *workspace_id, changed,
                "the record names the renamed workspace"
            );
            assert_eq!(
                old, &before.workspaces[&changed],
                "the record's old name is the name the workspace had"
            );
            assert_eq!(
                new, new_name,
                "the record's new name is the stored new name"
            );
            if let Some(actor) = actor {
                assert_eq!(*actor_id, actor, "the record names who renamed it");
            }
            if let Some((t0, t1)) = window {
                assert!(
                    t0 <= *at && *at <= t1,
                    "the record is stamped at the time of the rename: {at} not within [{t0}, {t1}]"
                );
            }
        }
    }
}

async fn seed_member(world: &mut FoundryWorld, first_name: &str, workspace_id: uuid::Uuid) {
    let pool = pool(world);
    let email = format!("{}@members.test", first_name.to_ascii_lowercase());
    let hash = foundry_auth::hash_password(&SecretString::new(MEMBER_PASSWORD.to_string().into()))
        .await
        .expect("hash password");
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(&email)
    .bind(first_name)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("insert member");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("insert membership");
    world
        .iawr_members
        .insert(first_name.to_string(), (email, user_id, workspace_id));
}

fn member(world: &FoundryWorld, first_name: &str) -> (String, uuid::Uuid, uuid::Uuid) {
    world
        .iawr_members
        .get(first_name)
        .cloned()
        .unwrap_or_else(|| panic!("{first_name} must be seeded as a member first"))
}

async fn member_page(world: &mut FoundryWorld, first_name: &str) -> String {
    let (email, _, _) = member(world, first_name);
    let outcome = signed_in_get(harness(world), &http(world), &email, MEMBER_PASSWORD, "/").await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "{first_name}'s page must render; body = {:?}",
        outcome.body
    );
    world.iawr_member_page = Some(outcome.body.clone());
    outcome.body
}

/// `(name text, monogram text, title attribute)` of the sidebar brand.
fn sidebar_brand(page: &str) -> (String, String, Option<String>) {
    let doc = Html::parse_document(page);
    let name_sel = Selector::parse(".sidebar__workspace").expect("selector");
    let mono_sel = Selector::parse(".sidebar__monogram").expect("selector");
    let name = doc
        .select(&name_sel)
        .next()
        .expect("every app-shell page renders the sidebar brand");
    let mono = doc
        .select(&mono_sel)
        .next()
        .expect("the sidebar brand carries a monogram");
    (
        name.text().collect::<String>().trim().to_string(),
        mono.text().collect::<String>().trim().to_string(),
        name.value().attr("title").map(str::to_string),
    )
}

async fn dashboard_as_priya(world: &FoundryWorld) -> String {
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
    )
    .await;
    assert_eq!(outcome.status, StatusCode::OK, "the dashboard must render");
    outcome.body
}

/// The hidden `_csrf` value is per sign-in, not project-row state; blank it so a
/// before/after comparison sees only what the rename could have touched.
fn redact_csrf(html: &str) -> String {
    const NEEDLE: &str = "name=\"_csrf\" value=\"";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(NEEDLE) {
        let value_start = pos + NEEDLE.len();
        out.push_str(&rest[..value_start]);
        let value_len = rest[value_start..].find('"').unwrap_or(0);
        out.push_str("CSRF");
        rest = &rest[value_start + value_len..];
    }
    out.push_str(rest);
    out
}

/// The project rows under one workspace, AS SERVED (byte slices of the page, not
/// a re-serialization: an HTML parser does not keep attribute order). The span is
/// from the workspace's own row marker to the next workspace row (or the end of
/// the list); each project row runs from `<li data-project-row` to its `</li>`
/// (a project row holds no nested list item).
fn project_rows_under(page: &str, workspace_id: uuid::Uuid) -> Vec<String> {
    let anchor = format!("data-workspace-id=\"{workspace_id}\"");
    let start = page
        .find(&anchor)
        .unwrap_or_else(|| panic!("the dashboard must list workspace {workspace_id}"));
    let span = &page[start + anchor.len()..];
    let end = span
        .find("data-workspace-row")
        .or_else(|| span.find("</section>"))
        .unwrap_or(span.len());
    let mut span = &span[..end];
    let mut rows = Vec::new();
    while let Some(open) = span.find("<li data-project-row") {
        let rest = &span[open..];
        let close = rest.find("</li>").expect("a project row closes") + "</li>".len();
        rows.push(redact_csrf(&rest[..close]));
        span = &rest[close..];
    }
    rows
}

// ===========================================================================
// Given
// ===========================================================================

#[given(regex = r#"^(\w+) is a member of workspace "([^"]+)"$"#)]
async fn is_a_member_of_workspace(world: &mut FoundryWorld, first_name: String, label: String) {
    ensure_harness(world).await;
    let workspace_id = resolve(world, &label).await;
    seed_member(world, &first_name, workspace_id).await;
}

#[given(regex = r#"^Priya has renamed workspace "([^"]+)" to "([^"]*)"$"#)]
async fn priya_has_renamed(world: &mut FoundryWorld, label: String, new_name: String) {
    priya_renames(world, &label, &new_name).await;
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "Priya's rename of {label:?} to {new_name:?} must succeed; body = {:?}",
        world.last_body
    );
    let target = world.iawr_target.expect("a rename was aimed");
    let after = capture_universe(world).await;
    let stored = new_name.trim();
    assert_universe_delta(
        &before(world),
        &after,
        Some((target, stored)),
        world.iapr_priya_id,
        world.iawr_window,
    );
    world.iawr_aliases.insert(stored.to_string(), target);
}

#[given(regex = r#"^Priya has noted the project rows listed under workspace "([^"]+)"$"#)]
async fn priya_noted_project_rows(world: &mut FoundryWorld, label: String) {
    let workspace_id = resolve(world, &label).await;
    let page = dashboard_as_priya(world).await;
    let rows = project_rows_under(&page, workspace_id);
    assert!(
        !rows.is_empty(),
        "{label:?} must list its projects before the rename; page = {page:?}"
    );
    world.iawr_noted_rows = Some(rows);
}

// ===========================================================================
// When — renames over HTTP
// ===========================================================================

/// Capture the universe, then send the rename from Priya's real session with a
/// fresh double-submit `_csrf`, bracketing it with the database clock.
async fn priya_renames(world: &mut FoundryWorld, label: &str, new_name: &str) {
    send_rename_as(world, PRIYA_EMAIL, PRIYA_PASSWORD, label, new_name).await;
}

async fn send_rename_as(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
    label: &str,
    new_name: &str,
) {
    let target = resolve(world, label).await;
    world.iawr_target = Some(target);
    // Later steps may call the workspace by the name it was asked to take; a
    // refused request's name is never looked up, so recording it is harmless.
    let asked = new_name.trim();
    if !asked.is_empty() {
        world.iawr_aliases.insert(asked.to_string(), target);
    }
    world.iawr_before = Some(capture_universe(world).await);
    let t0 = db_now(world).await;
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        email,
        password,
        &rename_url(&target.to_string()),
        &[("name", new_name)],
    )
    .await;
    let t1 = db_now(world).await;
    world.iawr_window = Some((t0, t1));
    record_outcome(world, outcome);
}

#[when(regex = r#"^Priya renames workspace "([^"]+)" to "([^"]*)"$"#)]
async fn when_priya_renames(world: &mut FoundryWorld, label: String, new_name: String) {
    priya_renames(world, &label, &new_name).await;
}

#[when(regex = r#"^Marco sends the rename for workspace "([^"]+)" to "([^"]+)"$"#)]
async fn marco_sends_rename(world: &mut FoundryWorld, label: String, new_name: String) {
    send_rename_as(world, MARCO_EMAIL, MARCO_PASSWORD, &label, &new_name).await;
}

/// Priya's real session but WITHOUT the `_csrf` field/cookie pair: the
/// double-submit middleware must refuse it before the handler runs (D2).
#[when(
    regex = r#"^a rename for workspace "([^"]+)" is submitted without the dashboard's matching token$"#
)]
async fn rename_without_token(world: &mut FoundryWorld, label: String) {
    let target = resolve(world, &label).await;
    world.iawr_target = Some(target);
    world.iawr_before = Some(capture_universe(world).await);
    let http = http(world);
    let session = establish_session(harness(world), &http, PRIYA_EMAIL, PRIYA_PASSWORD).await;
    let outcome = post_with_cookie(
        harness(world),
        &http,
        &rename_url(&target.to_string()),
        &session,
        &[("name", "Household")],
    )
    .await;
    record_outcome(world, outcome);
}

/// A VALID double-submit pair but no session: judged by the authz gate, which
/// must answer with the uniform non-enumerable 404 (D1).
#[when(regex = r#"^a signed-out visitor sends a rename for workspace "([^"]+)"$"#)]
async fn signed_out_rename(world: &mut FoundryWorld, label: String) {
    let target = resolve(world, &label).await;
    world.iawr_target = Some(target);
    world.iawr_before = Some(capture_universe(world).await);
    let http = http(world);
    let base = harness(world).base_url();
    let signin = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for a csrf pair");
    let token = signin
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("/sign-in must mint a foundry_csrf cookie")
        .to_string();
    let outcome = post_with_cookie(
        harness(world),
        &http,
        &rename_url(&target.to_string()),
        &format!("foundry_csrf={token}"),
        &[("name", "Household"), ("_csrf", &token)],
    )
    .await;
    record_outcome(world, outcome);
}

async fn priya_posts_to_raw_id(world: &mut FoundryWorld, raw_id: &str) {
    world.iawr_before = Some(capture_universe(world).await);
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &rename_url(raw_id),
        &[("name", "Household")],
    )
    .await;
    record_outcome(world, outcome);
}

#[when(regex = r#"^Priya sends a workspace rename aimed at the workspace id "([^"]+)"$"#)]
async fn rename_garbled_id(world: &mut FoundryWorld, raw_id: String) {
    priya_posts_to_raw_id(world, &raw_id).await;
}

#[when(regex = r"^Priya sends a workspace rename aimed at a workspace id that matches nothing$")]
async fn rename_unknown_id(world: &mut FoundryWorld) {
    let unknown = uuid::Uuid::now_v7().to_string();
    priya_posts_to_raw_id(world, &unknown).await;
}

// ===========================================================================
// When — members, the backup
// ===========================================================================

#[when(regex = r"^(\w+) opens a page in (?:her|his) workspace$")]
async fn member_opens_a_page(world: &mut FoundryWorld, first_name: String) {
    member_page(world, &first_name).await;
}

#[when(regex = r#"^Priya exports workspace "([^"]+)" to a backup$"#)]
async fn priya_exports_workspace(world: &mut FoundryWorld, label: String) {
    let workspace_id = resolve(world, &label).await;
    let base = ensure_postgres().await;
    let schema = harness(world).schema.clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");
    let dir = tempfile::TempDir::new().expect("backup tempdir");
    let out = dir.path().join("workspace.dump");
    world.iawr_backup_dir = Some(dir);
    world.iawr_backup_path = Some(out.clone());
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "export-workspace"])
            .arg(workspace_id.to_string())
            .arg(&out)
            .output()
            .expect("invoke foundry doctor export-workspace")
    })
    .await
    .expect("join blocking cli");
    world.iawr_cli = Some((
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ));
}

// ===========================================================================
// Then — what Priya gets back, and what the dashboard shows
// ===========================================================================

/// The 200 answer is the BARE workspace head (DDD-7/8): `[data-workspace-head]`
/// whose `[data-workspace-name]` reads exactly `name`, with no page wrapper.
fn assert_head_fragment(world: &FoundryWorld, name: &str) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the rename must answer with the re-rendered workspace row; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("answer captured");
    assert!(
        body.contains(HEAD_MARKER),
        "the answer must be the workspace head ([{HEAD_MARKER}]); got {body:?}"
    );
    assert!(
        !body.contains("<html"),
        "the answer must be BARE (no page wrapper); got {body:?}"
    );
    let doc = Html::parse_fragment(body);
    let sel = Selector::parse("[data-workspace-name]").expect("selector");
    let shown: Vec<String> = doc
        .select(&sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();
    assert_eq!(
        shown,
        vec![name.to_string()],
        "the workspace row must show exactly {name:?}; got {body:?}"
    );
}

#[then(regex = r#"^the workspace row she gets back shows "([^"]+)"$"#)]
async fn row_shows(world: &mut FoundryWorld, name: String) {
    assert_head_fragment(world, &name);
    let target = world.iawr_target.expect("a rename was aimed");
    let after = capture_universe(world).await;
    assert_universe_delta(
        &before(world),
        &after,
        Some((target, &name)),
        world.iapr_priya_id,
        world.iawr_window,
    );
    world.iawr_aliases.insert(name, target);
}

#[then(regex = r#"^the workspace row she gets back shows "([^"]+)" and carries no error$"#)]
async fn row_shows_no_error(world: &mut FoundryWorld, name: String) {
    assert_head_fragment(world, &name);
    let body = world.last_body.as_deref().expect("answer captured");
    assert!(
        !body.contains(ERROR_FRAGMENT_ATTR),
        "a quiet success carries no refusal fragment ({ERROR_FRAGMENT_ATTR}); got {body:?}"
    );
    let after = capture_universe(world).await;
    assert_universe_delta(&before(world), &after, None, None, None);
}

#[then(
    regex = r#"^reopening the instance dashboard shows workspace "([^"]+)" and no longer "([^"]+)"$"#
)]
async fn reopened_dashboard_shows(world: &mut FoundryWorld, new_name: String, old_name: String) {
    let target = world.iawr_target.expect("a rename was aimed");
    let page = dashboard_as_priya(world).await;
    let doc = Html::parse_document(&page);
    let name_sel = Selector::parse(&format!(
        r#"[data-workspace-row][data-workspace-id="{target}"] [data-workspace-name]"#
    ))
    .expect("selector");
    let shown: Vec<String> = doc
        .select(&name_sel)
        .map(|el| el.text().collect::<String>().trim().to_string())
        .collect();
    assert_eq!(
        shown,
        vec![new_name.clone()],
        "the reloaded dashboard must show the workspace as {new_name:?}"
    );
    let list_sel = Selector::parse("[data-workspace-list]").expect("selector");
    let list: String = doc
        .select(&list_sel)
        .flat_map(|el| el.text())
        .collect::<String>();
    assert!(
        !list.contains(&old_name),
        "the reloaded dashboard must no longer list {old_name:?}"
    );
}

#[then(regex = r"^no other workspace's name changed$")]
async fn no_other_workspace_changed(world: &mut FoundryWorld) {
    let target = world.iawr_target.expect("a rename was aimed");
    let before = before(world);
    let after = capture_universe(world).await;
    for (id, name) in before.workspaces.iter().filter(|(id, _)| **id != target) {
        assert_eq!(
            after.workspaces.get(id),
            Some(name),
            "workspace {id} ({name:?}) must be untouched by another workspace's rename"
        );
    }
}

// ===========================================================================
// Then — the record (D5, KPI-2)
// ===========================================================================

#[then(regex = r#"^workspace "([^"]+)" has exactly (\d+) renames? on record$"#)]
async fn has_n_renames_on_record(world: &mut FoundryWorld, label: String, count: usize) {
    let workspace_id = resolve(world, &label).await;
    let now = capture_universe(world).await;
    let mine = records_of(&now)
        .iter()
        .filter(|r| r.0 == workspace_id)
        .count();
    assert_eq!(
        mine, count,
        "workspace {label:?} must have exactly {count} rename(s) on record"
    );
}

#[then(
    regex = r#"^the latest rename on record for workspace "([^"]+)" names Priya, from "([^"]+)" to "([^"]+)", at the time of the rename$"#
)]
async fn latest_rename_on_record(
    world: &mut FoundryWorld,
    label: String,
    old: String,
    new: String,
) {
    let workspace_id = resolve(world, &label).await;
    let now = capture_universe(world).await;
    let latest = records_of(&now)
        .iter()
        .rev()
        .find(|r| r.0 == workspace_id)
        .unwrap_or_else(|| panic!("workspace {label:?} must have a rename on record"));
    let priya = world.iapr_priya_id.expect("Priya seeded by the Background");
    assert_eq!(
        latest.1, priya,
        "the record must name Priya as the one who renamed it"
    );
    assert_eq!(latest.2, old, "the record's old name");
    assert_eq!(latest.3, new, "the record's new name");
    let (t0, t1) = world.iawr_window.expect("the rename was timed");
    assert!(
        t0 <= latest.4 && latest.4 <= t1,
        "the record must carry the time of the rename: {} not within [{t0}, {t1}]",
        latest.4
    );
}

// ===========================================================================
// Then — refusals change nothing
// ===========================================================================

#[then(regex = r#"^the workspace rename is refused saying "([^"]+)"$"#)]
async fn refused_saying(world: &mut FoundryWorld, message: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "a validation refusal must be 422 (D3); body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("fragment captured");
    assert!(
        body.contains(&message),
        "the refusal must state {message:?}; got {body:?}"
    );
    assert!(
        body.contains(ERROR_FRAGMENT_ATTR),
        "the refusal must be the {ERROR_FRAGMENT_ATTR} fragment; got {body:?}"
    );
    assert!(
        !body.contains("<html"),
        "the refusal must be BARE (no page wrapper); got {body:?}"
    );
}

#[then(regex = r#"^workspace "([^"]+)" is unchanged with no rename on record$"#)]
async fn unchanged_with_no_record(world: &mut FoundryWorld, label: String) {
    let workspace_id = resolve(world, &label).await;
    let after = capture_universe(world).await;
    assert_universe_delta(&before(world), &after, None, None, None);
    assert_eq!(
        after.workspaces.get(&workspace_id),
        Some(&label),
        "workspace {label:?} must keep its name"
    );
    assert!(
        records_of(&after).iter().all(|r| r.0 != workspace_id),
        "workspace {label:?} must have no rename on record"
    );
}

#[then(regex = r"^no workspace changed and nothing new went on record$")]
async fn nothing_changed(world: &mut FoundryWorld) {
    let after = capture_universe(world).await;
    assert_universe_delta(&before(world), &after, None, None, None);
}

#[then(regex = r"^the workspace rename is refused before any change is made$")]
async fn refused_before_any_change(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::FORBIDDEN),
        "a rename without its double-submit pair is refused by the middleware (403); body = {:?}",
        world.last_body
    );
    let after = capture_universe(world).await;
    let before = before(world);
    assert_eq!(
        after.workspaces, before.workspaces,
        "the refused rename must have changed no workspace"
    );
}

// ===========================================================================
// Then — members' sidebar (D6, D8)
// ===========================================================================

#[then(regex = r#"^(\w+)'s sidebar shows the workspace "([^"]+)" with monogram "([^"]+)"$"#)]
async fn sidebar_shows(world: &mut FoundryWorld, first_name: String, name: String, mono: String) {
    let page = member_page(world, &first_name).await;
    let (shown, monogram, _) = sidebar_brand(&page);
    assert_eq!(shown, name, "{first_name}'s sidebar must read {name:?}");
    assert_eq!(monogram, mono, "the monogram follows the name (D8)");
}

#[then(regex = r#"^the workspace name's hover title reads "([^"]+)"$"#)]
async fn hover_title_reads(world: &mut FoundryWorld, name: String) {
    let page = world
        .iawr_member_page
        .clone()
        .expect("a member opened a page");
    let (_, _, title) = sidebar_brand(&page);
    assert_eq!(
        title.as_deref(),
        Some(name.as_str()),
        "the sidebar brand must carry the full name as its title (D6)"
    );
}

#[then(regex = r"^(\w+) is still a member of the same workspace$")]
async fn still_a_member(world: &mut FoundryWorld, first_name: String) {
    let (_, user_id, workspace_id) = member(world, &first_name);
    let (still,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(&pool(world))
    .await
    .expect("read membership");
    assert!(still, "{first_name}'s membership survives the rename (D8)");
    assert_eq!(
        world.iawr_target,
        Some(workspace_id),
        "the renamed workspace keeps its id (D8)"
    );
}

// ===========================================================================
// Then — the guards (OQ-D1, ADR-WORKSPACE-RENAME-002)
// ===========================================================================

#[then(
    regex = r#"^the project rows listed under workspace "([^"]+)" are byte-identical to before$"#
)]
async fn project_rows_identical(world: &mut FoundryWorld, label: String) {
    let workspace_id = resolve(world, &label).await;
    let noted = world
        .iawr_noted_rows
        .clone()
        .expect("the project rows were noted before the rename");
    let page = dashboard_as_priya(world).await;
    assert_eq!(
        project_rows_under(&page, workspace_id),
        noted,
        "a workspace rename must leave its project rows byte-identical (OQ-D1, DDD-7)"
    );
}

fn backup_path(world: &FoundryWorld) -> std::path::PathBuf {
    world
        .iawr_backup_path
        .clone()
        .expect("a backup was written")
}

fn backup_manifest(path: &std::path::Path) -> serde_json::Value {
    use std::io::Read;
    let file = std::fs::File::open(path).expect("open backup");
    let mut archive = tar::Archive::new(file);
    for entry in archive.entries().expect("read backup entries") {
        let mut entry = entry.expect("backup entry");
        if entry.path().expect("entry path").to_string_lossy() == "manifest.json" {
            let mut buf = String::new();
            entry.read_to_string(&mut buf).expect("read manifest");
            return serde_json::from_str(&buf).expect("parse manifest");
        }
    }
    panic!("backup at {path:?} has no manifest.json");
}

#[then(regex = r"^the backup holds exactly the ten workspace tables and no rename record$")]
async fn backup_holds_ten_tables(world: &mut FoundryWorld) {
    let (exit, stdout, stderr) = world.iawr_cli.clone().expect("the export ran");
    assert_eq!(
        exit, 0,
        "the export must succeed; stdout={stdout:?} stderr={stderr:?}"
    );
    let path = backup_path(world);
    let file = std::fs::File::open(&path).expect("open backup");
    let entries: BTreeSet<String> = tar::Archive::new(file)
        .entries()
        .expect("read backup entries")
        .map(|e| {
            e.expect("entry")
                .path()
                .expect("entry path")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let mut expected: BTreeSet<String> = foundry_store::TENANT_TABLES
        .iter()
        .map(|t| format!("tables/{t}.jsonl"))
        .collect();
    expected.insert("manifest.json".to_string());
    assert_eq!(expected.len(), 11, "ten workspace tables plus the manifest");
    assert_eq!(
        entries, expected,
        "the backup must hold exactly the ten workspace tables (ADR-WORKSPACE-RENAME-002)"
    );
    let manifest = backup_manifest(&path).to_string();
    assert!(
        !manifest.contains("workspace_rename_events"),
        "the manifest must not mention the rename record; got {manifest}"
    );
}

#[then(regex = r#"^the backup declares the workspace as "([^"]+)"$"#)]
async fn backup_declares_name(world: &mut FoundryWorld, name: String) {
    let manifest = backup_manifest(&backup_path(world));
    assert_eq!(
        manifest["declared_workspace_name"].as_str(),
        Some(name.as_str()),
        "the backup records the name at export time (D8)"
    );
}

#[then(regex = r"^the backup passes verification$")]
async fn backup_passes_verification(world: &mut FoundryWorld) {
    let path = backup_path(world);
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .args(["doctor", "verify-export"])
            .arg(&path)
            .output()
            .expect("invoke foundry doctor verify-export")
    })
    .await
    .expect("join blocking cli");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(0),
        "verify-export must pass; stdout={stdout:?} stderr={stderr:?}"
    );
    assert!(
        stdout.trim_end().ends_with("status: OK"),
        "verify-export must end with status: OK; stdout={stdout:?}"
    );
}

// ===========================================================================
// @needs-browser — the dashboard row (D9) and the sidebar (D6, OQ-D3)
// ===========================================================================

fn row_css(workspace_id: uuid::Uuid) -> String {
    format!(r#"[data-workspace-row][data-workspace-id="{workspace_id}"]"#)
}

fn head_css(workspace_id: uuid::Uuid) -> String {
    format!("{} [data-workspace-head]", row_css(workspace_id))
}

/// Node-identity probe (DDD-7). Before each browser submit, plant a JS expando
/// on the target `<li data-workspace-row>`, on every child of it OTHER than
/// the head (the project list, or the "No projects yet." line), and on the
/// head itself. A server render can never carry an expando, so after a 200:
/// the row and its non-head children must still hold theirs (same DOM nodes —
/// not re-rendered), while the head must NOT (it was swapped). This catches a
/// whole-`<li>` swap even for a workspace with no projects. Returns the number
/// of tagged non-head children so the step can refuse a vacuous plant.
const ROW_IDENTITY_PLANT: &str = "var li = document.querySelector(arguments[0]);
     if (!li) { return -1; }
     li.__iawr_node = 'kept';
     var n = 0;
     for (var i = 0; i < li.children.length; i++) {
       var c = li.children[i];
       if (c.hasAttribute('data-workspace-head')) { c.__iawr_node = 'head'; }
       else { c.__iawr_node = 'kept'; n++; }
     }
     return n;";

/// Reads back the expandos planted by [`ROW_IDENTITY_PLANT`]:
/// `[row kept?, non-head children all kept?, non-head child count, head still the old node?]`.
const ROW_IDENTITY_PROBE: &str = "var li = document.querySelector(arguments[0]);
     if (!li) { return [false, false, 0, true]; }
     var kept = true, n = 0, oldHead = false;
     for (var i = 0; i < li.children.length; i++) {
       var c = li.children[i];
       if (c.hasAttribute('data-workspace-head')) { if (c.__iawr_node === 'head') { oldHead = true; } }
       else { n++; if (c.__iawr_node !== 'kept') { kept = false; } }
     }
     return [li.__iawr_node === 'kept', kept, n, oldHead];";

/// Text of the whole page with the target row cut out (scripts and templates
/// dropped) — where a refusal message must NOT appear.
const TEXT_OUTSIDE_ROW: &str = "var clone = document.body.cloneNode(true);
     var li = clone.querySelector(arguments[0]);
     if (li) { li.remove(); }
     clone.querySelectorAll('script, template').forEach(function (e) { e.remove(); });
     return [li !== null, clone.textContent];";

async fn submit_in_browser(world: &mut FoundryWorld, label: &str, typed: &str) {
    let target = resolve(world, label).await;
    world.iawr_target = Some(target);
    let browser = world.browser.as_ref().expect("browser session");
    let tagged = browser
        .execute(
            ROW_IDENTITY_PLANT,
            vec![serde_json::Value::String(row_css(target))],
        )
        .await
        .expect("plant the row-identity expandos");
    assert!(
        tagged.as_i64().unwrap_or(-1) >= 1,
        "the {label:?} row must hold content beside its head to prove the swap leaves it alone; \
         plant returned {tagged:?}"
    );
    let head = browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css(&head_css(target)))
        .await
        .unwrap_or_else(|err| {
            panic!("the {label:?} row must carry a [{HEAD_MARKER}] with a rename form: {err}")
        });
    let input = head
        .find(Locator::Css("input[name='name']"))
        .await
        .expect("the workspace head must carry a rename input");
    input.clear().await.expect("clear the rename input");
    input.send_keys(typed).await.expect("type the new name");
    head.find(Locator::Css("button[type='submit']"))
        .await
        .expect("the workspace head must carry a submit button")
        .click()
        .await
        .expect("submit the rename");
}

#[when(regex = r#"^she renames the "([^"]+)" workspace to "([^"]+)" in her browser$"#)]
async fn renames_in_browser(world: &mut FoundryWorld, label: String, typed: String) {
    submit_in_browser(world, &label, &typed).await;
}

/// One space: `required` lets htmx POST it and the server trims it to empty —
/// the only empty name a real browser will submit (the precedent's idiom).
#[when(regex = r#"^she blanks the "([^"]+)" workspace name in her browser$"#)]
async fn blanks_in_browser(world: &mut FoundryWorld, label: String) {
    submit_in_browser(world, &label, " ").await;
}

/// Poll a CSS selector's text until `accept` holds, or panic after 10s.
async fn wait_for_text(world: &FoundryWorld, css: &str, what: &str, accept: impl Fn(&str) -> bool) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(el) = browser.find(Locator::Css(css)).await {
            if let Ok(text) = el.text().await {
                if accept(text.trim()) {
                    return;
                }
            }
        }
        if std::time::Instant::now() > deadline {
            let page = browser.source().await.unwrap_or_default();
            panic!("{what}; page = {page:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(regex = r#"^that workspace's row shows "([^"]+)" without the page reloading$"#)]
async fn row_swapped_in_place(world: &mut FoundryWorld, name: String) {
    let target = world.iawr_target.expect("a rename was submitted");
    let css = format!("{} [data-workspace-name]", head_css(target));
    wait_for_text(
        world,
        &css,
        &format!("the workspace row must re-render in place showing {name:?}"),
        |text| text == name,
    )
    .await;
    let browser = world.browser.as_ref().expect("browser session");
    let marker = browser
        .execute(PAGE_MARKER_GET, vec![])
        .await
        .expect("read the page-lifetime marker");
    assert_eq!(
        marker.as_str(),
        Some("alive"),
        "the row swap must NOT be a full reload"
    );
    let probe = browser
        .execute(
            ROW_IDENTITY_PROBE,
            vec![serde_json::Value::String(row_css(target))],
        )
        .await
        .expect("probe the row's node identity");
    let v = probe
        .as_array()
        .expect("the identity probe returns an array");
    let flag = |i: usize| v[i].as_bool().unwrap_or(false);
    assert!(
        flag(0),
        "only the head may be re-rendered: the workspace's <li data-workspace-row> must be \
         the SAME DOM node as before the rename, not a re-rendered one (DDD-7); probe = {v:?}"
    );
    assert!(
        flag(1) && v[2].as_u64().unwrap_or(0) >= 1,
        "only the head may be re-rendered: the row's content beside the head must be the \
         SAME DOM nodes as before the rename (DDD-7); probe = {v:?}"
    );
    assert!(
        !flag(3),
        "the head must be the re-rendered node carrying the new name (DDD-7); probe = {v:?}"
    );
}

#[then(regex = r#"^"([^"]+)" appears inside that workspace row's message area$"#)]
async fn message_in_row_slot(world: &mut FoundryWorld, message: String) {
    let target = world.iawr_target.expect("a rename was submitted");
    let css = format!("{} [data-error-slot]", head_css(target));
    wait_for_text(
        world,
        &css,
        &format!(
            "{message:?} must appear inside the workspace row's own [data-error-slot] (D9), \
             including after an earlier refusal"
        ),
        |text| text.contains(&message),
    )
    .await;
    let browser = world.browser.as_ref().expect("browser session");
    let outside = browser
        .execute(
            TEXT_OUTSIDE_ROW,
            vec![serde_json::Value::String(row_css(target))],
        )
        .await
        .expect("read the page text outside the workspace row");
    let v = outside
        .as_array()
        .expect("the outside-text probe returns an array");
    assert_eq!(
        v[0].as_bool(),
        Some(true),
        "the workspace row must be on the page"
    );
    let text = v[1].as_str().unwrap_or_default();
    assert!(
        !text.contains(&message),
        "{message:?} must appear ONLY inside that workspace row — no other row's message area \
         and no page banner may show it (D9); page text outside the row = {text:?}"
    );
}

#[then(regex = r"^the workspace rename form is still there for her to correct$")]
async fn form_still_there(world: &mut FoundryWorld) {
    let target = world.iawr_target.expect("a rename was submitted");
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css(&format!(
            "{} input[name='name']",
            head_css(target)
        )))
        .await
        .expect("the rename form must stay mounted and resubmittable after a refusal (D9)");
}

#[then(regex = r#"^the projects listed under "([^"]+)" are still on the page$"#)]
async fn projects_still_on_page(world: &mut FoundryWorld, label: String) {
    let workspace_id = resolve(world, &label).await;
    let browser = world.browser.as_ref().expect("browser session");
    let rows = browser
        .find_all(Locator::Css(&format!(
            r#"[data-workspace-row][data-workspace-id="{workspace_id}"] [data-project-row]"#
        )))
        .await
        .expect("query project rows");
    assert_eq!(
        rows.len(),
        2,
        "the swap of one workspace's head must leave {label:?}'s two project rows in place (DDD-7)"
    );
}

#[when(regex = r"^(\w+) opens a page in (?:her|his) workspace on a (desktop|phone-sized) screen$")]
async fn member_opens_page_in_browser(
    world: &mut FoundryWorld,
    first_name: String,
    screen: String,
) {
    ensure_harness(world).await;
    let (email, _, _) = member(world, &first_name);
    let browser = if screen == "desktop" {
        browser_harness::new_session().await
    } else {
        browser_harness::open_mobile_session().await
    };
    {
        let harness = world.harness.as_ref().expect("harness");
        browser_harness::sign_in_through_browser(&browser, harness, &email, MEMBER_PASSWORD).await;
        browser
            .goto(&format!("{}/", harness.base_url()))
            .await
            .expect("open a page in the workspace");
    }
    browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css(".sidebar__workspace"))
        .await
        .expect("every app-shell page renders the sidebar brand");
    world.browser = Some(browser);
}

#[then(regex = r"^the sidebar shows the workspace name on one line ending in an ellipsis$")]
async fn sidebar_one_line_ellipsis(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let probe = browser
        .execute(
            "var el = document.querySelector('.sidebar__workspace');
             var cs = getComputedStyle(el);
             var lh = parseFloat(cs.lineHeight);
             if (!(lh > 0)) { lh = parseFloat(cs.fontSize) * 1.2; }
             var r = el.getBoundingClientRect();
             return [r.height, lh, cs.textOverflow, cs.overflowX, el.scrollWidth,
                     el.clientWidth, r.right, document.documentElement.clientWidth];",
            vec![],
        )
        .await
        .expect("probe the sidebar brand's layout");
    let v = probe.as_array().expect("the probe returns an array");
    let num = |i: usize| v[i].as_f64().unwrap_or(f64::NAN);
    let (height, line, scroll_w, client_w, right, page_w) =
        (num(0), num(1), num(4), num(5), num(6), num(7));
    let (text_overflow, overflow_x) = (v[2].as_str().unwrap_or(""), v[3].as_str().unwrap_or(""));
    assert!(
        height < line * 1.5,
        "the workspace name must sit on ONE line (height {height}px, line {line}px) (D6)"
    );
    assert_eq!(
        text_overflow, "ellipsis",
        "the overflowing name must end in an ellipsis (D6)"
    );
    assert_ne!(
        overflow_x, "visible",
        "an ellipsis only shows when the overflow is clipped"
    );
    assert!(
        scroll_w > client_w,
        "a 52-character name must actually be truncated (scrollWidth {scroll_w} <= clientWidth {client_w})"
    );
    assert!(
        right <= page_w,
        "the workspace name must end inside the page (right {right}px > {page_w}px) (OQ-D3)"
    );
}

#[then(regex = r#"^hovering the workspace name shows "([^"]+)"$"#)]
async fn hovering_shows(world: &mut FoundryWorld, name: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let title = browser
        .find(Locator::Css(".sidebar__workspace"))
        .await
        .expect("sidebar brand")
        .attr("title")
        .await
        .expect("read title");
    assert_eq!(
        title.as_deref(),
        Some(name.as_str()),
        "the full name must be the brand's hover title (D6)"
    );
}

#[then(regex = r"^the page does not scroll sideways$")]
async fn page_does_not_scroll_sideways(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let probe = browser
        .execute(
            "return [document.documentElement.scrollWidth, document.documentElement.clientWidth];",
            vec![],
        )
        .await
        .expect("probe page width");
    let v = probe.as_array().expect("array");
    let (scroll_w, client_w) = (
        v[0].as_f64().unwrap_or(f64::NAN),
        v[1].as_f64().unwrap_or(f64::NAN),
    );
    assert!(
        scroll_w <= client_w,
        "the page must not scroll sideways (scrollWidth {scroll_w} > clientWidth {client_w}) (OQ-D3)"
    );
}
