//! project-name-rule step definitions
//! (`tests/features/project-name-rule.feature`, DISTILL 2026-10-06, every
//! scenario `@pending`).
//!
//! The seams these steps drive are the DESIGN-pinned driving ports, nothing
//! internal (feature delta, DESIGN [REF] Driving Ports / Handoff):
//!
//! - `POST /team/{team_slug}/projects` (the create door) — form `name`,
//!   `key_prefix`; a refusal is a 422 full create page with one `p.error`
//!   carrying the copy and the `name` / `key_prefix` inputs holding what was
//!   sent, trimmed (D11, OQ-D6); with `HX-Request: true` it is the bare
//!   `[data-hx-fragment="project-create-error"]` fragment; a success is a
//!   redirect whose `Location` is the minted address.
//! - `POST /admin/instance/projects/{project_id}/rename` (the rename door) —
//!   through the iapr steps (`priya_renames_project`, `send_rename_as`, the
//!   `project-rename-error` fragment, the row partial).
//! - `GET /team/{team_slug}/project/{project_slug}[/report]` — the board and the
//!   change report a redirect lands on (KPI-5).
//!
//! Every step is real: HTTP against the in-process router with real session +
//! CSRF layers, SQL reads of the per-scenario Postgres schema, and a real
//! headless Chrome for `@needs-browser`. No placeholder is needed at this layer:
//! DELIVER's new API (`foundry_core::ProjectName`, `mint_project_slug`,
//! `foundry_services::projects::create_project`) is reached only through those
//! ports, so every scenario compiles and runs today and reds on behaviour, never
//! on a missing symbol.
//!
//! STATE DELTA (Mandate 8): every attempt captures the [`ProjectUniverse`] —
//! every project `(team, name, address, key prefix, issue counter)`, the lane
//! count of every project, and the issue count — just before it (after every
//! Given's fixtures), and the outcome steps assert it after: a refusal moves
//! NOTHING (fail-closed, D9); a create adds exactly one project with its lanes;
//! a rename changes exactly one name.
//!
//! THE ADDRESS RULE (ADR-PROJECT-RENAME-001): an address asserted after a
//! rename is the one read from the database before the rename (the iapr
//! `iapr_stored_slugs` map); an address asserted after a create is either the
//! literal the scenario names or the stored slug of the one project the create
//! added. This module derives no slug from a name.
//!
//! LAYER 3 (real adapter + real HTTP, `@real-io`): example-based only
//! (Mandates 9 and 11). The rule's boundaries, the uniqueness arms and the
//! address mint are pinned as properties and exact pairs at the unit layer
//! (`crates/foundry-core/tests/project_name.rs`) and the retry at the service
//! seam (`crates/foundry-services/tests/create_project_use_case.rs`).
//!
//! NAMES: a quoted name in the feature goes through [`expand_name`] — the
//! `[N×c]` repeat mark, then the iwnr [`decode_invisibles`] marks — before it is
//! sent or compared.

use crate::steps::feature_instance_admin_project_rename::{
    creation_slug, ensure_harness, harness, http, pool, priya_renames_project, project_id_of,
    record_outcome, rename_url, row_xpath, seed_team, send_rename_as, stored_slugs_of,
    ERROR_MARKER, MARCO_EMAIL, MARCO_PASSWORD, PRIYA_EMAIL, PRIYA_PASSWORD, ROW_MARKER,
};
use crate::steps::feature_instance_workspace_name_rule::decode_invisibles;
use crate::support::browser_harness;
use crate::support::harness::{establish_session, signed_in_get, signed_in_post, PostOutcome};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::StatusCode;
use scraper::{Html, Selector};
use std::collections::BTreeMap;
use std::time::Duration;

/// DESIGN-pinned seams. If DELIVER moves one, the template and this module move
/// in the same change.
/// The refusal slot is the `p.error` the template renders immediately before
/// the create form (it is a sibling, not a descendant), so the oracle is scoped
/// by that adjacency — a refusal in any other `p.error` on the page does not count.
const CREATE_ERROR_CSS: &str = r#"p.error:has(+ form[action$="/projects"])"#;
const CREATE_NAME_CSS: &str = r#"form[action$="/projects"] input[name="name"]"#;
const CREATE_KEY_CSS: &str = r#"form[action$="/projects"] input[name="key_prefix"]"#;
const CREATE_FRAGMENT_CSS: &str = r#"[data-hx-fragment="project-create-error"]"#;
const RENAME_FRAGMENT_CSS: &str = r#"[data-hx-fragment="project-rename-error"]"#;
const HEADING_CSS: &str = "h1";

/// The team every create targets unless a scenario names another.
const DEFAULT_TEAM: &str = "Backend";
/// The workspace the Background seeds; a second team is added to it.
const WORKSPACE: &str = "Canzan Labs";
/// The rename door's target and the create door's free key in the parity matrix.
const PARITY_RENAME_TARGET: &str = "Sandbox";
const PARITY_KEY: &str = "PAR";

/// The names a caller who may not create is asked with (D10 gate scenario), and
/// the acceptable name whose answer each must equal byte for byte.
const UNFIT_NAMES: [&str; 4] = [
    "[300×a]",
    "Homelab[TAB]Ops",
    "Homelab[NUL]Ops",
    "[SPACE][SPACE]",
];
const ACCEPTABLE_NAME: &str = "Homelab Ops";
const GATE_KEY: &str = "OPS";

// ===========================================================================
// Domain types — the vocabulary the steps share
// ===========================================================================

/// One project as the doors can observe it (port-exposed columns, read back
/// through SQL; never an internal struct).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRecord {
    pub team_id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub key_prefix: String,
    pub next_issue_number: i32,
}

/// The observable universe of naming a project.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProjectUniverse {
    pub projects: BTreeMap<uuid::Uuid, ProjectRecord>,
    pub lanes_by_project: BTreeMap<uuid::Uuid, i64>,
    pub issues: i64,
}

/// A create as it was sent: which team (by address), the name exactly as
/// submitted, and the key prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAttempt {
    pub team_slug: String,
    pub name: String,
    pub key: String,
}

/// Who sends a create in the authorization-gate scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caller {
    /// A workspace member who is on no team (the non-member foil).
    Marco,
    /// No session at all.
    SignedOut,
    /// A member of Backend, here aimed at a team that does not exist.
    Priya,
}

impl Caller {
    fn parse(text: &str) -> Self {
        match text {
            "Marco" => Self::Marco,
            "a signed-out visitor" => Self::SignedOut,
            "Priya" => Self::Priya,
            other => panic!("no caller registered for {other:?}"),
        }
    }
}

/// The two doors, in the order the parity matrix visits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    Create,
    Rename,
}

/// One door's verdict on one name, reduced to what the person sees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Refused with this copy (the text of the door's refusal slot).
    Refused(String),
    /// Accepted; the project is now stored under this name.
    Accepted(String),
    /// Anything else (a 500, a 404, a second message) — always a failure.
    Other(String),
}

/// Expand the feature's name marks (header of the feature file): first every
/// `[N×c]` (the single character `c`, `N` times), then the iwnr invisible marks.
/// Unknown bracketed text is kept literally.
pub(crate) fn expand_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open..];
        let Some(close) = after.find(']') else {
            out.push_str(after);
            rest = "";
            break;
        };
        match repeat_mark(&after[1..close]) {
            Some((count, c)) => out.extend(std::iter::repeat_n(c, count)),
            None => out.push_str(&after[..=close]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    decode_invisibles(&out)
}

fn repeat_mark(token: &str) -> Option<(usize, char)> {
    let (count, glyph) = token.split_once('×')?;
    let count = count.parse::<usize>().ok()?;
    let mut chars = glyph.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some((count, c))
}

/// The HTML escaping the templates apply to a name (askama's default set).
fn escaped(name: &str) -> String {
    name.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn snippet(body: &str) -> String {
    body.chars().take(300).collect()
}

fn texts(html: &str, css: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css).expect("valid selector");
    doc.select(&sel).map(|e| e.text().collect()).collect()
}

fn input_value(html: &str, css: &str) -> Option<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(css).expect("valid selector");
    doc.select(&sel)
        .next()
        .map(|e| e.value().attr("value").unwrap_or_default().to_string())
}

// ===========================================================================
// The universe (Mandate 8)
// ===========================================================================

pub(crate) async fn capture_project_universe(world: &FoundryWorld) -> ProjectUniverse {
    let pool = pool(world);
    type Row = (uuid::Uuid, uuid::Uuid, String, String, String, i32);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, team_id, name, slug, key_prefix, next_issue_number FROM projects",
    )
    .fetch_all(&pool)
    .await
    .expect("read every project");
    let lanes: Vec<(uuid::Uuid, i64)> =
        sqlx::query_as("SELECT project_id, count(*) FROM lanes GROUP BY project_id")
            .fetch_all(&pool)
            .await
            .expect("count lanes per project");
    let (issues,): (i64,) = sqlx::query_as("SELECT count(*) FROM issues")
        .fetch_one(&pool)
        .await
        .expect("count issues");
    ProjectUniverse {
        projects: rows
            .into_iter()
            .map(|(id, team_id, name, slug, key_prefix, next_issue_number)| {
                (
                    id,
                    ProjectRecord {
                        team_id,
                        name,
                        slug,
                        key_prefix,
                        next_issue_number,
                    },
                )
            })
            .collect(),
        lanes_by_project: lanes.into_iter().collect(),
        issues,
    }
}

async fn snapshot_before(world: &mut FoundryWorld) {
    world.pnr_before = Some(capture_project_universe(world).await);
}

fn universe_before(world: &FoundryWorld) -> ProjectUniverse {
    world
        .pnr_before
        .clone()
        .expect("the project universe was captured before the attempt")
}

/// Fail-closed: a refusal changes no project and adds no project, lane or issue.
fn assert_nothing_moved(before: &ProjectUniverse, after: &ProjectUniverse) {
    assert_eq!(
        after.projects, before.projects,
        "a refusal must create, rename or re-address no project (D9)"
    );
    assert_eq!(
        after.lanes_by_project, before.lanes_by_project,
        "a refusal must leave no lane behind (D9)"
    );
    assert_eq!(after.issues, before.issues, "a refusal must add no issue");
}

/// A rename changes exactly one project's name; its address, key prefix and
/// issue counter, every other project, and every lane stay as they were.
fn assert_only_renamed(
    before: &ProjectUniverse,
    after: &ProjectUniverse,
    id: uuid::Uuid,
    new_name: &str,
) {
    let old = before
        .projects
        .get(&id)
        .expect("the renamed project existed");
    let mut expected = before.projects.clone();
    expected.insert(
        id,
        ProjectRecord {
            name: new_name.to_string(),
            ..old.clone()
        },
    );
    assert_eq!(
        after.projects, expected,
        "only the name of the renamed project may change, to {new_name:?} (D1, D7)"
    );
    assert_eq!(after.lanes_by_project, before.lanes_by_project);
    assert_eq!(after.issues, before.issues);
}

/// A create adds exactly one project, with at least one lane of its own, and
/// changes nothing else. Returns the new project.
fn assert_created_exactly_one(
    before: &ProjectUniverse,
    after: &ProjectUniverse,
) -> (uuid::Uuid, ProjectRecord) {
    for (id, old) in &before.projects {
        assert_eq!(
            after.projects.get(id),
            Some(old),
            "an existing project must be untouched by a create"
        );
    }
    let added: Vec<(&uuid::Uuid, &ProjectRecord)> = after
        .projects
        .iter()
        .filter(|(id, _)| !before.projects.contains_key(id))
        .collect();
    assert_eq!(
        added.len(),
        1,
        "a create must add exactly one project; added {added:?}"
    );
    let (id, record) = (*added[0].0, added[0].1.clone());
    for (pid, n) in &before.lanes_by_project {
        assert_eq!(
            after.lanes_by_project.get(pid),
            Some(n),
            "a create must not touch another project's lanes"
        );
    }
    let new_lanes = after.lanes_by_project.get(&id).copied().unwrap_or(0);
    assert!(
        new_lanes > 0,
        "the new project must be created with its lanes"
    );
    assert_eq!(
        after.lanes_by_project.len(),
        before.lanes_by_project.len() + 1,
        "only the new project may gain lanes"
    );
    assert_eq!(after.issues, before.issues);
    (id, record)
}

// ===========================================================================
// Driving the doors
// ===========================================================================

fn team_slug(team_name: &str) -> String {
    creation_slug(team_name)
}

/// A fresh double-submit pair from the public sign-in page: `(token, cookie)`.
async fn fresh_csrf(world: &FoundryWorld) -> (String, String) {
    let resp = http(world)
        .get(format!("{}/sign-in", harness(world).base_url()))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let token = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("/sign-in must mint foundry_csrf")
        .to_string();
    let cookie = format!("foundry_csrf={token}");
    (token, cookie)
}

async fn raw_post(
    world: &FoundryWorld,
    url: &str,
    cookie: &str,
    form: &[(&str, &str)],
    live_form: bool,
) -> PostOutcome {
    let mut req = http(world)
        .post(format!("{}{url}", harness(world).base_url()))
        .header(reqwest::header::COOKIE, cookie.to_string())
        .form(form);
    if live_form {
        req = req.header("HX-Request", "true");
    }
    let resp = req.send().await.expect("post create");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    PostOutcome {
        status,
        headers,
        body,
    }
}

/// Send one create as `caller` (no snapshot, no recording).
async fn send_create(
    world: &FoundryWorld,
    caller: Caller,
    team_slug: &str,
    name: &str,
    key: &str,
    live_form: bool,
) -> PostOutcome {
    let url = format!("/team/{team_slug}/projects");
    match caller {
        Caller::SignedOut => {
            let (token, cookie) = fresh_csrf(world).await;
            raw_post(
                world,
                &url,
                &cookie,
                &[("name", name), ("key_prefix", key), ("_csrf", &token)],
                live_form,
            )
            .await
        }
        Caller::Marco | Caller::Priya => {
            let (email, password) = match caller {
                Caller::Marco => (MARCO_EMAIL, MARCO_PASSWORD),
                _ => (PRIYA_EMAIL, PRIYA_PASSWORD),
            };
            if !live_form {
                return signed_in_post(
                    harness(world),
                    &http(world),
                    email,
                    password,
                    &url,
                    &[("name", name), ("key_prefix", key)],
                )
                .await;
            }
            let session = establish_session(harness(world), &http(world), email, password).await;
            let (token, csrf_cookie) = fresh_csrf(world).await;
            raw_post(
                world,
                &url,
                &format!("{session}; {csrf_cookie}"),
                &[("name", name), ("key_prefix", key), ("_csrf", &token)],
                live_form,
            )
            .await
        }
    }
}

/// Priya creates a project: snapshot the universe, send, record the outcome.
async fn priya_creates(
    world: &mut FoundryWorld,
    team_name: &str,
    raw_name: &str,
    key: &str,
    live_form: bool,
) {
    let slug = team_slug(team_name);
    let name = expand_name(raw_name);
    snapshot_before(world).await;
    world.pnr_attempt = Some(CreateAttempt {
        team_slug: slug.clone(),
        name: name.clone(),
        key: key.to_string(),
    });
    let outcome = send_create(world, Caller::Priya, &slug, &name, key, live_form).await;
    record_outcome(world, outcome).await;
}

fn location(world: &FoundryWorld) -> Option<String> {
    world
        .last_headers
        .as_ref()?
        .get(reqwest::header::LOCATION)?
        .to_str()
        .ok()
        .map(str::to_string)
}

/// The h1 texts of a page Priya opens.
async fn headings_at(world: &FoundryWorld, path: &str) -> (StatusCode, Vec<String>, String) {
    let outcome = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        path,
    )
    .await;
    let headings = texts(&outcome.body, HEADING_CSS);
    (outcome.status, headings, outcome.body)
}

async fn assert_board_headed(world: &FoundryWorld, path: &str, name: &str) {
    let (status, headings, body) = headings_at(world, path).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the board at {path} must open; body = {:?}",
        snippet(&body)
    );
    assert!(
        headings.iter().any(|h| h == name),
        "the board at {path} must be headed {name:?}; headings = {headings:?}"
    );
}

/// The last create landed on a working board: a redirect to the address of the
/// one project it added (stored, non-empty), which opens headed with its name.
/// Returns the new project and registers it under its stored name for later
/// steps (the address rule: captured, never re-derived).
async fn assert_landed(
    world: &mut FoundryWorld,
    expected_path: Option<&str>,
    heading: &str,
) -> (uuid::Uuid, ProjectRecord) {
    let status = world.last_status.expect("a create was sent");
    assert!(
        status.is_redirection(),
        "the create must redirect to the new board; got {status} with {:?}",
        snippet(world.last_body.as_deref().unwrap_or(""))
    );
    let landed = location(world).expect("the redirect names where it goes");
    let after = capture_project_universe(world).await;
    let (id, record) = assert_created_exactly_one(&universe_before(world), &after);
    let attempt = world.pnr_attempt.clone().expect("the create attempt");
    assert_eq!(
        record.name, heading,
        "the new project is stored as {heading:?}"
    );
    assert_eq!(
        record.key_prefix, attempt.key,
        "the new project keeps its key"
    );
    assert!(
        !record.slug.is_empty(),
        "every new project must have a non-empty address (D15, KPI-5)"
    );
    assert_eq!(
        landed,
        format!("/team/{}/project/{}", attempt.team_slug, record.slug),
        "the redirect must go to the address minted for the new project"
    );
    if let Some(path) = expected_path {
        assert_eq!(landed, path, "the new project's address");
    }
    assert_board_headed(world, &landed, heading).await;
    world.iapr_project_ids.insert(record.name.clone(), id);
    world.iapr_stored_slugs.insert(
        record.name.clone(),
        (attempt.team_slug, record.slug.clone()),
    );
    (id, record)
}

/// Seed a project the way it was before this feature (raw SQL, as a restore or
/// an old replica would leave it) in Backend, with its lanes, and register it.
async fn seed_old_project(world: &mut FoundryWorld, name: &str, key: &str, slug: &str) {
    let workspace = *world
        .iapr_workspace_ids
        .get(WORKSPACE)
        .expect("the Background seeds Canzan Labs");
    let team = *world
        .iapr_team_ids
        .get(DEFAULT_TEAM)
        .expect("the Background seeds Backend");
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team)
    .bind(workspace)
    .bind(name)
    .bind(slug)
    .bind(key)
    .execute(&pool(world))
    .await
    .expect("insert a project from before the rule");
    crate::support::harness::seed_lanes_for_project(&pool(world), id).await;
    world.iapr_project_ids.insert(name.to_string(), id);
    world.iapr_stored_slugs.insert(
        name.to_string(),
        (team_slug(DEFAULT_TEAM), slug.to_string()),
    );
}

/// The create door's verdict, read from the last outcome (plain POST).
fn create_verdict(
    outcome: &PostOutcome,
    after: &ProjectUniverse,
    before: &ProjectUniverse,
) -> Verdict {
    match outcome.status {
        StatusCode::UNPROCESSABLE_ENTITY => match texts(&outcome.body, CREATE_ERROR_CSS).as_slice()
        {
            [one] => Verdict::Refused(one.clone()),
            many => Verdict::Other(format!("422 with refusal slots {many:?}")),
        },
        s if s.is_redirection() => {
            let added: Vec<&ProjectRecord> = after
                .projects
                .iter()
                .filter(|(id, _)| !before.projects.contains_key(id))
                .map(|(_, r)| r)
                .collect();
            match added.as_slice() {
                [one] => Verdict::Accepted(one.name.clone()),
                other => Verdict::Other(format!("{s} adding {} projects", other.len())),
            }
        }
        s => Verdict::Other(format!("{s}: {}", snippet(&outcome.body))),
    }
}

/// The rename door's verdict on its target.
fn rename_verdict(outcome: &PostOutcome, after: &ProjectUniverse, target: uuid::Uuid) -> Verdict {
    match outcome.status {
        StatusCode::UNPROCESSABLE_ENTITY => {
            match texts(&outcome.body, RENAME_FRAGMENT_CSS).as_slice() {
                [one] => Verdict::Refused(one.clone()),
                many => Verdict::Other(format!("422 with refusal fragments {many:?}")),
            }
        }
        StatusCode::OK => Verdict::Accepted(
            after
                .projects
                .get(&target)
                .map(|r| r.name.clone())
                .unwrap_or_default(),
        ),
        s => Verdict::Other(format!("{s}: {}", snippet(&outcome.body))),
    }
}

/// Offer one name at both doors: create in `create_team` with the parity key,
/// then rename the parity target. Records both verdicts.
async fn offer_at_both_doors(world: &mut FoundryWorld, raw: &str, create_team: &str) {
    let name = expand_name(raw);
    snapshot_before(world).await;
    let before = universe_before(world);
    world.pnr_verdicts.clear();

    let created = send_create(
        world,
        Caller::Priya,
        &team_slug(create_team),
        &name,
        PARITY_KEY,
        false,
    )
    .await;
    let mid = capture_project_universe(world).await;
    let create = create_verdict(&created, &mid, &before);
    world.pnr_verdicts.push((Door::Create, create));

    let target = project_id_of(world, PARITY_RENAME_TARGET);
    let renamed = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &rename_url(&target.to_string()),
        &[("name", &name)],
    )
    .await;
    let after = capture_project_universe(world).await;
    let rename = rename_verdict(&renamed, &after, target);
    world.pnr_verdicts.push((Door::Rename, rename));
}

// ===========================================================================
// Given
// ===========================================================================

#[given(regex = r#"^project "([^"]+)" \(([A-Z]+)\) was named before the rule existed$"#)]
async fn project_named_before_the_rule(world: &mut FoundryWorld, raw: String, key: String) {
    let slug = format!("legacy-{}", key.to_ascii_lowercase());
    seed_old_project(world, &expand_name(&raw), &key, &slug).await;
}

#[given(
    regex = r#"^project "([^"]+)" \(([A-Z]+)\) was created before this fix and has no board address$"#
)]
async fn project_without_address(world: &mut FoundryWorld, raw: String, key: String) {
    seed_old_project(world, &expand_name(&raw), &key, "").await;
}

#[given(regex = r#"^Priya has renamed project "([^"]+)" to "([^"]+)"$"#)]
async fn priya_has_renamed(world: &mut FoundryWorld, from: String, to: String) {
    let id = project_id_of(world, &from);
    let stored = stored_slugs_of(world, &from);
    priya_renames_project(world, &from, &to).await;
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "renaming {from:?} to {to:?} must succeed; body = {:?}",
        world.last_body
    );
    world.iapr_project_ids.insert(to.clone(), id);
    world.iapr_stored_slugs.insert(to, stored);
}

#[given(regex = r#"^workspace "([^"]+)" also has a team "([^"]+)" with no projects$"#)]
async fn workspace_also_has_team(world: &mut FoundryWorld, workspace: String, team: String) {
    let ws = *world
        .iapr_workspace_ids
        .get(&workspace)
        .unwrap_or_else(|| panic!("workspace {workspace:?} is seeded by the Background"));
    // seed_team also makes Priya a member of the new team.
    seed_team(world, ws, &team, &team_slug(&team)).await;
}

#[given(regex = r#"^Priya has created a project named "([^"]+)" with key prefix "([^"]+)"$"#)]
async fn priya_has_created(world: &mut FoundryWorld, raw: String, key: String) {
    priya_creates(world, DEFAULT_TEAM, &raw, &key, false).await;
    let name = expand_name(&raw);
    assert_landed(world, None, &name).await;
}

#[given(
    regex = r#"^Priya's create of "([^"]+)" with key prefix "([^"]+)" was refused for its name$"#
)]
async fn priyas_create_was_refused(world: &mut FoundryWorld, raw: String, key: String) {
    priya_creates(world, DEFAULT_TEAM, &raw, &key, false).await;
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the create of {raw:?} must be refused for its name; got {:?} with {:?}",
        world.last_status,
        snippet(world.last_body.as_deref().unwrap_or(""))
    );
    let after = capture_project_universe(world).await;
    assert_nothing_moved(&universe_before(world), &after);
}

#[given(regex = r#"^Priya has the new-project form for team "([^"]+)" open in her browser$"#)]
async fn create_form_open_in_browser(world: &mut FoundryWorld, team: String) {
    ensure_harness(world).await;
    let browser = browser_harness::new_session().await;
    {
        let harness = harness(world);
        browser_harness::sign_in_through_browser(&browser, harness, PRIYA_EMAIL, PRIYA_PASSWORD)
            .await;
        browser
            .goto(&format!(
                "{}/team/{}/projects/new",
                harness.base_url(),
                team_slug(&team)
            ))
            .await
            .expect("open the new-project form in the browser");
    }
    world.browser = Some(browser);
}

// ===========================================================================
// When — the rename door
// ===========================================================================

#[when(regex = r#"^Priya renames project "([^"]+)" to the pasted name "([^"]*)"$"#)]
async fn priya_renames_pasted(world: &mut FoundryWorld, raw_project: String, pasted: String) {
    let project = expand_name(&raw_project);
    snapshot_before(world).await;
    priya_renames_project(world, &project, &expand_name(&pasted)).await;
}

#[when(
    regex = r#"^Priya sends a rename with the pasted name "([^"]*)" aimed at a project id that matches nothing$"#
)]
async fn rename_unknown_id_with_name(world: &mut FoundryWorld, pasted: String) {
    snapshot_before(world).await;
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &rename_url(&uuid::Uuid::now_v7().to_string()),
        &[("name", &expand_name(&pasted))],
    )
    .await;
    record_outcome(world, outcome).await;
}

#[when(regex = r#"^Marco sends the rename for "([^"]+)" with the pasted name "([^"]*)"$"#)]
async fn marco_renames_pasted(world: &mut FoundryWorld, project: String, pasted: String) {
    snapshot_before(world).await;
    send_rename_as(
        world,
        MARCO_EMAIL,
        MARCO_PASSWORD,
        &project,
        &expand_name(&pasted),
    )
    .await;
}

/// A browser cannot TYPE a tab into a text input (the key moves focus), so the
/// paste is simulated the way a clipboard paste lands: the value is set, then the
/// row's own submit button is pressed.
#[when(
    regex = r#"^she pastes "([^"]+)" over the "([^"]+)" project name in her browser and submits it$"#
)]
async fn pastes_project_name_in_browser(world: &mut FoundryWorld, pasted: String, project: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let input_xpath = format!("{}//input[@name='name']", row_xpath(&project));
    browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::XPath(&input_xpath))
        .await
        .unwrap_or_else(|e| panic!("the {project:?} row must carry a rename input: {e}"));
    browser
        .execute(
            "document.evaluate(arguments[0], document, null, \
             XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue.value = arguments[1];",
            vec![
                serde_json::Value::String(input_xpath),
                serde_json::Value::String(expand_name(&pasted)),
            ],
        )
        .await
        .expect("paste the name into the rename input");
    browser
        .find(Locator::XPath(&format!(
            "{}//button[@type='submit']",
            row_xpath(&project)
        )))
        .await
        .expect("the row must carry a submit button")
        .click()
        .await
        .expect("submit the rename");
}

// ===========================================================================
// When — the create door
// ===========================================================================

#[when(regex = r#"^Priya creates a project named "([^"]*)" with key prefix "([^"]*)"$"#)]
async fn priya_creates_in_backend(world: &mut FoundryWorld, raw: String, key: String) {
    priya_creates(world, DEFAULT_TEAM, &raw, &key, false).await;
}

#[when(
    regex = r#"^Priya creates a project named "([^"]*)" with key prefix "([^"]*)" in team "([^"]+)"$"#
)]
async fn priya_creates_in_team(world: &mut FoundryWorld, raw: String, key: String, team: String) {
    priya_creates(world, &team, &raw, &key, false).await;
}

#[when(
    regex = r#"^Priya creates a project named "([^"]*)" with key prefix "([^"]*)" from the page without reloading it$"#
)]
async fn priya_creates_live(world: &mut FoundryWorld, raw: String, key: String) {
    priya_creates(world, DEFAULT_TEAM, &raw, &key, true).await;
}

#[when(
    regex = r#"^(Marco|a signed-out visitor|Priya) sends creates to team "([^"]+)" with unfit names and with an acceptable name$"#
)]
async fn gated_creates(world: &mut FoundryWorld, caller: String, team: String) {
    let caller = Caller::parse(&caller);
    let slug = team_slug(&team);
    snapshot_before(world).await;
    let answer = |o: &PostOutcome| {
        (
            o.status,
            o.headers
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string),
            o.body.clone(),
        )
    };
    let baseline =
        answer(&send_create(world, caller, &slug, ACCEPTABLE_NAME, GATE_KEY, false).await);
    let mut differing = Vec::new();
    for raw in UNFIT_NAMES {
        let got =
            answer(&send_create(world, caller, &slug, &expand_name(raw), GATE_KEY, false).await);
        if got != baseline {
            differing.push(format!(
                "{raw}: {} {:?} {:?} vs baseline {} {:?} {:?}",
                got.0,
                got.1,
                snippet(&got.2),
                baseline.0,
                baseline.1,
                snippet(&baseline.2)
            ));
        }
    }
    assert!(
        !baseline.0.is_success(),
        "the gate scenario's acceptable name must itself be refused for this caller; got {}",
        baseline.0
    );
    world.pnr_gate_report = Some((UNFIT_NAMES.len(), differing));
}

#[when(
    regex = r#"^she types the name "([^"]+)" and the key prefix "([^"]+)" into the form in her browser and submits it$"#
)]
async fn types_into_create_form(world: &mut FoundryWorld, raw: String, key: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let name_input = browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::Css(CREATE_NAME_CSS))
        .await
        .unwrap_or_else(|e| panic!("the page must carry the create form: {e}"));
    name_input.clear().await.expect("clear the name input");
    name_input
        .send_keys(&expand_name(&raw))
        .await
        .expect("type the project name");
    let key_input = browser
        .find(Locator::Css(CREATE_KEY_CSS))
        .await
        .expect("the create form must carry a key prefix input");
    key_input.clear().await.expect("clear the key input");
    key_input
        .send_keys(&key)
        .await
        .expect("type the key prefix");
    browser
        .find(Locator::Css(
            r#"form[action$="/projects"] button[type="submit"]"#,
        ))
        .await
        .expect("the create form must carry a submit button")
        .click()
        .await
        .expect("submit the create form");
}

// ===========================================================================
// When — both doors (KPI-2)
// ===========================================================================

#[when(regex = r#"^Priya offers the project name "([^"]*)" at both doors$"#)]
async fn offer_at_both_doors_backend(world: &mut FoundryWorld, raw: String) {
    offer_at_both_doors(world, &raw, DEFAULT_TEAM).await;
}

#[when(
    regex = r#"^Priya offers the project name "([^"]*)" at both doors, creating it in team "([^"]+)"$"#
)]
async fn offer_at_both_doors_in_team(world: &mut FoundryWorld, raw: String, team: String) {
    offer_at_both_doors(world, &raw, &team).await;
}

// ===========================================================================
// Then
// ===========================================================================

#[then(regex = r"^no project changed and nothing was created$")]
async fn no_project_changed(world: &mut FoundryWorld) {
    let after = capture_project_universe(world).await;
    assert_nothing_moved(&universe_before(world), &after);
}

#[then(
    regex = r#"^project "([^"]+)" is now named "([^"]*)", and its board still opens at its original address$"#
)]
async fn project_now_named(world: &mut FoundryWorld, raw_project: String, raw_stored: String) {
    let project = expand_name(&raw_project);
    let stored = expand_name(&raw_stored);
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the rename must answer with the row; body = {:?}",
        snippet(world.last_body.as_deref().unwrap_or(""))
    );
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        body.contains(ROW_MARKER) && !body.contains(ERROR_MARKER),
        "the rename must answer with the row and no error; got {:?}",
        snippet(&body)
    );
    let id = project_id_of(world, &project);
    let after = capture_project_universe(world).await;
    assert_only_renamed(&universe_before(world), &after, id, &stored);
    // The address read from the database before the rename — never re-derived.
    let (team, slug) = stored_slugs_of(world, &project);
    assert_board_headed(world, &format!("/team/{team}/project/{slug}"), &stored).await;
}

#[then(regex = r#"^the row she gets back still shows "([^"]+)" and carries no error$"#)]
async fn row_still_shows(world: &mut FoundryWorld, raw: String) {
    let name = expand_name(&raw);
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "an untouched name must be a quiet success; body = {:?}",
        snippet(world.last_body.as_deref().unwrap_or(""))
    );
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        body.contains(ROW_MARKER),
        "the answer must be the row; got {:?}",
        snippet(&body)
    );
    assert!(!body.contains(ERROR_MARKER), "the row must carry no error");
    assert!(
        body.contains(&escaped(&name)),
        "the row must still show the name exactly; got {:?}",
        snippet(&body)
    );
}

#[then(regex = r#"^the create form is shown again saying "([^"]+)"$"#)]
async fn create_form_again(world: &mut FoundryWorld, copy: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the create must be refused with the form shown again; body = {:?}",
        snippet(&body)
    );
    assert!(
        input_value(&body, CREATE_NAME_CSS).is_some(),
        "the refusal must re-render the create form, not a bare message"
    );
    assert_eq!(
        texts(&body, CREATE_ERROR_CSS),
        vec![copy.clone()],
        "the form must carry exactly one refusal, reading {copy:?}"
    );
}

#[then(regex = r#"^the create form still holds the name "([^"]*)" and the key prefix "([^"]*)"$"#)]
async fn create_form_holds(world: &mut FoundryWorld, raw: String, key: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        input_value(&body, CREATE_NAME_CSS),
        Some(expand_name(&raw)),
        "the name input must hold the trimmed name as sent (OQ-D6)"
    );
    assert_eq!(
        input_value(&body, CREATE_KEY_CSS),
        Some(key),
        "the key prefix input must hold the key as sent"
    );
}

#[then(regex = r#"^the create refusal comes back as the bare message "([^"]+)"$"#)]
async fn create_bare_message(world: &mut FoundryWorld, copy: String) {
    let body = world.last_body.clone().unwrap_or_default();
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "the live-form create must be refused; body = {:?}",
        snippet(&body)
    );
    assert_eq!(
        texts(&body, CREATE_FRAGMENT_CSS),
        vec![copy],
        "the answer must be the one project-create-error fragment with the copy"
    );
    assert!(!body.contains("<html"), "the fragment must be bare");
}

#[then(regex = r#"^she lands on the new project's board, headed "([^"]+)"$"#)]
async fn lands_on_new_board(world: &mut FoundryWorld, raw: String) {
    assert_landed(world, None, &expand_name(&raw)).await;
}

#[then(regex = r#"^she lands on the board at "([^"]+)" headed "([^"]+)"$"#)]
async fn lands_on_board_at(world: &mut FoundryWorld, path: String, raw: String) {
    assert_landed(world, Some(&path), &expand_name(&raw)).await;
}

#[then(regex = r#"^its change report opens at "([^"]+)" headed "([^"]+)"$"#)]
async fn report_opens(world: &mut FoundryWorld, path: String, raw: String) {
    let name = expand_name(&raw);
    let (status, headings, body) = headings_at(world, &path).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the change report at {path} must open; body = {:?}",
        snippet(&body)
    );
    let expected = format!("Change report — {name}");
    assert!(
        headings.contains(&expected),
        "the report must be headed {expected:?}; headings = {headings:?}"
    );
}

#[then(regex = r#"^the board at "([^"]+)" is headed "([^"]+)"$"#)]
async fn board_is_headed(world: &mut FoundryWorld, path: String, raw: String) {
    assert_board_headed(world, &path, &expand_name(&raw)).await;
}

#[then(regex = r#"^team "([^"]+)" has exactly one project named "([^"]+)" in any letter case$"#)]
async fn exactly_one_named(world: &mut FoundryWorld, team: String, name: String) {
    let team_id = *world
        .iapr_team_ids
        .get(&team)
        .unwrap_or_else(|| panic!("team {team:?} is seeded"));
    let after = capture_project_universe(world).await;
    let lowered = name.to_lowercase();
    let matching = after
        .projects
        .values()
        .filter(|p| p.team_id == team_id && p.name.to_lowercase() == lowered)
        .count();
    assert_eq!(
        matching, 1,
        "team {team:?} must hold one {name:?} in any case"
    );
}

#[then(regex = r"^each answer is byte-identical to the answer for the acceptable name$")]
async fn gate_answers_identical(world: &mut FoundryWorld) {
    let (compared, differing) = world
        .pnr_gate_report
        .clone()
        .expect("the gated creates were sent");
    assert_eq!(compared, UNFIT_NAMES.len(), "every unfit name was compared");
    assert!(
        differing.is_empty(),
        "every unfit name must get the byte-identical answer (D10): {differing:#?}"
    );
}

#[then(regex = r#"^both doors refuse it saying "([^"]+)"$"#)]
async fn both_doors_refuse(world: &mut FoundryWorld, copy: String) {
    let expected = vec![
        (Door::Create, Verdict::Refused(copy.clone())),
        (Door::Rename, Verdict::Refused(copy)),
    ];
    assert_eq!(
        world.pnr_verdicts, expected,
        "both doors must give the same refusal (KPI-2)"
    );
}

#[then(regex = r#"^both doors accept it and store "([^"]*)"$"#)]
async fn both_doors_accept(world: &mut FoundryWorld, raw: String) {
    let stored = expand_name(&raw);
    let expected = vec![
        (Door::Create, Verdict::Accepted(stored.clone())),
        (Door::Rename, Verdict::Accepted(stored)),
    ];
    assert_eq!(
        world.pnr_verdicts, expected,
        "both doors must accept and store the same name (KPI-2)"
    );
}

#[then(regex = r#"^"([^"]+)" appears in the create form in her browser$"#)]
async fn create_error_in_browser(world: &mut FoundryWorld, copy: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(slot) = browser.find(Locator::Css(CREATE_ERROR_CSS)).await {
            if slot.text().await.map(|t| t == copy).unwrap_or(false) {
                return;
            }
        }
        if std::time::Instant::now() > deadline {
            let page = browser.source().await.unwrap_or_default();
            panic!(
                "{copy:?} must appear in the create form's message area; page = {:?}",
                snippet(&page)
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(
    regex = r#"^the create form in her browser still holds the name "([^"]+)" and the key prefix "([^"]+)"$"#
)]
async fn create_form_holds_in_browser(world: &mut FoundryWorld, raw: String, key: String) {
    let browser = world.browser.as_ref().expect("browser session");
    assert_eq!(
        browser_value(browser, CREATE_NAME_CSS).await,
        expand_name(&raw)
    );
    assert_eq!(browser_value(browser, CREATE_KEY_CSS).await, key);
}

async fn browser_value(browser: &fantoccini::Client, css: &str) -> String {
    browser
        .find(Locator::Css(css))
        .await
        .unwrap_or_else(|e| panic!("the create form must still be there ({css}): {e}"))
        .prop("value")
        .await
        .expect("read the input value")
        .unwrap_or_default()
}

#[cfg(test)]
mod name_marks {
    use super::expand_name;

    #[test]
    fn a_repeat_mark_writes_the_character_that_many_times() {
        assert_eq!(expand_name("[3×a]"), "aaa");
        assert_eq!(expand_name("A[2×日]"), "A日日");
        assert_eq!(expand_name("[257×a]").chars().count(), 257);
    }

    #[test]
    fn repeat_and_invisible_marks_combine() {
        assert_eq!(expand_name("[2×a][TAB][2×b]"), "aa\tbb");
        assert_eq!(expand_name("[SPACE][2×x][NUL]"), " xx\0");
    }

    #[test]
    fn other_bracketed_text_is_literal() {
        assert_eq!(expand_name("[draft]"), "[draft]");
        assert_eq!(expand_name("[2×ab]"), "[2×ab]");
        assert_eq!(expand_name("[x×a]"), "[x×a]");
        assert_eq!(expand_name("open [ only"), "open [ only");
    }
}
