//! team-lifecycle — the acceptance driver the `feature_team_lifecycle` step file
//! delegates to (DISTILL 2026-10-11).
//!
//! It extends, and never duplicates, the team-membership driver
//! ([`crate::support::team_membership`], `tm` below): the Background, the
//! roster, sessions, CSRF, `open` / `post`, the page readers, the uniform-404
//! oracle and the team-row universe are all `tm`'s. This module adds three
//! things, and nothing else:
//!
//! - **The vocabulary** of this feature ([`RecordKind`], [`Record`],
//!   [`Universe`], [`Change`], [`DbAnswer`], [`TlState`]).
//! - **The doors** DESIGN pinned (DDD-17, DDD-21): `POST /teams` (the create
//!   form on `/teams`), `POST /team/{slug}/rename` (the prefilled Rename form on
//!   the members page), `GET` and `POST /team/{slug}/delete` (the confirm page,
//!   reached by the members page's **Delete team** link; **Cancel** is a link
//!   back), the members page's **New project** link, the dashboard's "create
//!   your first project" link, the shipped project create, and
//!   `POST /workspace/switch`. A write "from the page" submits the page's own
//!   form; a write the caller has no form for is the same request built by hand.
//!   The operator's door is SQL at the database prompt (OQ-7).
//! - **The universe** (Mandate 8): `tm`'s (team rows, workspace rows, accounts,
//!   teams, defaults) plus every team's name, every project, every machine token
//!   and its scope, and every team lifecycle record. A `When` captures it first;
//!   a `Then` compares the after-image with the before-image plus the declared
//!   [`Change`]s, fail-closed. A delete's cascade (its team rows, its scoped
//!   tokens, a default that pointed at it) is derived from the before-image, and
//!   the Gherkin must name the tokens and the default when there are any.
//!
//! Preconditions (Homelab, the instance admin, legacy teams, extra projects, a
//! scoped token, the race resets) are seeded with SQL, never the outcome under
//! test (No Fixture Theater): every team, rename, delete and record the
//! scenarios assert on is written by the product.
//!
//! `team_lifecycle_events` arrives with migration 0023 (slice 02). Until then
//! the universe reads no records, so the slice 01 scenarios run unchanged
//! before and after it.

use crate::steps::feature_project_name_rule::expand_name;
use crate::support::html_assertions::{parse, select_all, text_of};
use crate::support::team_membership::{self as tm, Actor, Answer, TeamRef};
use crate::world::FoundryWorld;
use reqwest::header::COOKIE;
use reqwest::StatusCode;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The workspace provisioned from the instance's workspaces page, with no team.
pub const HOMELAB: &str = "Homelab";
const HOMELAB_CREATED: &str = "2026-10-01T00:00:00Z";
/// The address the authz matrix uses for a team no workspace has.
const UNKNOWN_SLUG: &str = "no-such-team";
/// The address an operator's hand-typed team is stored under.
const HAND_SLUG: &str = "hand-typed";

// ===========================================================================
// Vocabulary
// ===========================================================================

/// What a team lifecycle record says happened (OQ-6, ADR-TEAM-LIFECYCLE-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordKind {
    Renamed,
    Deleted,
}

impl RecordKind {
    fn parse(word: &str) -> Self {
        match word {
            "renamed" => Self::Renamed,
            "deleted" => Self::Deleted,
            other => panic!("a lifecycle record of kind {other:?}; only renamed and deleted exist"),
        }
    }
}

/// One lifecycle record, as the operator reads it with psql.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Record {
    pub workspace: String,
    pub kind: RecordKind,
    pub team_slug: String,
    /// The name before the event.
    pub name: String,
    pub new_name: Option<String>,
    /// The actor's email.
    pub actor: String,
}

/// The observable universe of every write in this feature (Mandate 8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Universe {
    pub base: tm::Universe,
    /// `(workspace, slug) -> name`.
    pub team_names: BTreeMap<(String, String), String>,
    /// `(workspace, team slug, project name, key)`.
    pub projects: BTreeSet<(String, String, String, String)>,
    /// `token id -> (workspace, scope team slug)`.
    pub tokens: BTreeMap<String, (String, Option<String>)>,
    /// Every lifecycle record, sorted (a multiset).
    pub records: Vec<Record>,
}

/// One declared movement of the universe.
#[derive(Debug, Clone)]
pub enum Change {
    /// A new team, its name, and its creator's lead row (D3, INV-TL-7).
    Created {
        workspace: String,
        name: String,
        slug: String,
        lead: String,
    },
    /// A team written by hand at the database prompt (no lead).
    HandAdded { workspace: String, name: String },
    /// The team (by its scenario handle) now has this name; its slug is unchanged.
    Renamed { team: String, new_name: String },
    /// The team (by handle) is gone, with its team rows and scoped tokens, and
    /// any default that pointed at it is cleared.
    Deleted { team: String },
    /// One new lifecycle record.
    Recorded(Record),
}

/// What the database said to a hand-typed statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbAnswer {
    Accepted(u64),
    Refused {
        code: String,
        constraint: Option<String>,
        table: Option<String>,
        detail: Option<String>,
    },
}

/// One run of the delete-versus-project-create race.
#[derive(Debug, Clone)]
pub struct DeleteRace {
    pub delete: Answer,
    pub create: Answer,
    pub team_exists: bool,
    pub project_exists: bool,
}

/// One run of a create race: both answers and the teams it left, by name.
#[derive(Debug, Clone)]
pub struct CreateRace {
    pub first: Answer,
    pub second: Answer,
    /// `name -> slugs` of the raced names after the run.
    pub teams: BTreeMap<String, Vec<String>>,
}

/// The per-scenario state of this feature (`FoundryWorld::tl`).
#[derive(Debug, Default)]
pub struct TlState {
    pub before: Option<Universe>,
    /// The workspace each person's session acts in, when it is not Canzan Labs.
    pub working_in: HashMap<String, String>,
    /// A delete page an actor opened earlier and kept open (actor key -> page).
    pub earlier_delete: HashMap<String, Answer>,
    /// The last create form an actor sent: (action, fields).
    pub last_create: HashMap<String, (String, Vec<(String, String)>)>,
    pub delete_race: Vec<DeleteRace>,
    pub create_race: Vec<CreateRace>,
    pub db: Option<(String, DbAnswer)>,
}

// ===========================================================================
// Name resolution
// ===========================================================================

fn expand(raw: &str) -> String {
    expand_name(raw)
}

fn slug_or_unknown(world: &FoundryWorld, team: &str) -> String {
    match team.trim() {
        "a team that does not exist" => UNKNOWN_SLUG.to_string(),
        name => tm::team_ref(world, name).slug,
    }
}

fn members_path(slug: &str) -> String {
    tm::members_path(slug)
}

fn delete_path(slug: &str) -> String {
    format!("/team/{slug}/delete")
}

fn rename_path(slug: &str) -> String {
    format!("/team/{slug}/rename")
}

fn workspace_of_actor(world: &FoundryWorld, actor: &Actor) -> String {
    // The acting workspace is the session's; scenarios name it explicitly only
    // for Homelab (`… is working in Homelab`).
    match actor {
        Actor::Person(email) if world.tl.working_in.contains_key(email) => {
            world.tl.working_in[email].clone()
        }
        _ => tm::HOME.to_string(),
    }
}

/// Register a team the product wrote, so later steps can name it.
async fn register_team(world: &mut FoundryWorld, workspace: &str, name: &str) -> Option<TeamRef> {
    let ws = tm::workspace_id(world, workspace);
    let row: Option<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, slug FROM teams WHERE workspace_id = $1 AND name = $2")
            .bind(ws)
            .bind(name)
            .fetch_optional(&tm::pool(world))
            .await
            .expect("look the new team up");
    let (id, slug) = row?;
    let team = TeamRef {
        id,
        workspace: workspace.to_string(),
        name: name.to_string(),
        slug,
    };
    world.tm.teams.insert(name.to_string(), team.clone());
    Some(team)
}

/// Register a project the product wrote.
async fn register_project(world: &mut FoundryWorld, team: &str, name: &str) {
    let tr = tm::team_ref(world, team);
    let row: Option<(String,)> =
        sqlx::query_as("SELECT slug FROM projects WHERE team_id = $1 AND name = $2")
            .bind(tr.id)
            .bind(name)
            .fetch_optional(&tm::pool(world))
            .await
            .expect("look the new project up");
    if let Some((slug,)) = row {
        world.tm.projects.insert(
            name.to_string(),
            tm::ProjectRef {
                team_slug: tr.slug,
                slug,
            },
        );
    }
}

// ===========================================================================
// The universe
// ===========================================================================

pub async fn universe(world: &FoundryWorld) -> Universe {
    let pool = tm::pool(world);
    let base = tm::universe(world).await;
    let names: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT w.name, t.slug, t.name FROM teams t JOIN workspaces w ON w.id = t.workspace_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read team names");
    let projects: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT w.name, t.slug, p.name, p.key_prefix
           FROM projects p JOIN teams t ON t.id = p.team_id JOIN workspaces w ON w.id = t.workspace_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read projects");
    let tokens: Vec<(uuid::Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT mt.jti, w.name, t.slug
           FROM machine_tokens mt JOIN workspaces w ON w.id = mt.workspace_id
           LEFT JOIN teams t ON t.id = mt.scope_team_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read machine tokens");
    let (has_records,): (bool,) = sqlx::query_as(
        "SELECT to_regclass(format('%I.team_lifecycle_events', current_schema())) IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("look the lifecycle records up");
    let mut records: Vec<Record> = if has_records {
        let rows: Vec<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT w.name, e.kind, e.team_slug, e.name, e.new_name, u.email_lower
               FROM team_lifecycle_events e
               JOIN workspaces w ON w.id = e.workspace_id
               JOIN users u ON u.id = e.actor_id",
        )
        .fetch_all(&pool)
        .await
        .expect("read the lifecycle records");
        rows.into_iter()
            .map(
                |(workspace, kind, team_slug, name, new_name, actor)| Record {
                    workspace,
                    kind: RecordKind::parse(&kind),
                    team_slug,
                    name,
                    new_name,
                    actor,
                },
            )
            .collect()
    } else {
        Vec::new()
    };
    records.sort();
    Universe {
        base,
        team_names: names.into_iter().map(|(w, s, n)| ((w, s), n)).collect(),
        projects: projects.into_iter().collect(),
        tokens: tokens
            .into_iter()
            .map(|(id, w, scope)| (id.to_string(), (w, scope)))
            .collect(),
        records,
    }
}

/// Capture the before-image a `When` is judged against (both `tm`'s and ours).
pub async fn capture_before(world: &mut FoundryWorld) {
    tm::capture_before(world).await;
    let u = universe(world).await;
    world.tl.before = Some(u);
}

fn before(world: &FoundryWorld) -> Universe {
    world
        .tl
        .before
        .clone()
        .expect("the When captured the universe before acting")
}

/// `assert_state_delta(before, after, universe, expected)`: the after-image
/// must equal the before-image with exactly `changes` applied. Fail-closed.
pub async fn assert_state_delta(world: &FoundryWorld, changes: &[Change]) {
    let before = before(world);
    let after = universe(world).await;
    let mut expected = before.clone();
    for change in changes {
        match change {
            Change::Created {
                workspace,
                name,
                slug,
                lead,
            } => {
                let key = (workspace.clone(), slug.clone());
                expected.base.teams.insert(key.clone());
                expected.team_names.insert(key, name.clone());
                expected.base.team_rows.insert(
                    (workspace.clone(), slug.clone(), lead.clone()),
                    tm::TeamRole::Lead,
                );
            }
            Change::HandAdded { workspace, name } => {
                let key = (workspace.clone(), HAND_SLUG.to_string());
                expected.base.teams.insert(key.clone());
                expected.team_names.insert(key, name.clone());
            }
            Change::Renamed { team, new_name } => {
                let tr = tm::team_ref(world, team);
                expected
                    .team_names
                    .insert((tr.workspace, tr.slug), new_name.clone());
            }
            Change::Deleted { team } => {
                let tr = tm::team_ref(world, team);
                let key = (tr.workspace.clone(), tr.slug.clone());
                expected.base.teams.remove(&key);
                expected.team_names.remove(&key);
                expected
                    .base
                    .team_rows
                    .retain(|(w, s, _), _| !(*w == tr.workspace && *s == tr.slug));
                expected.tokens.retain(|_, (w, scope)| {
                    !(*w == tr.workspace && scope.as_deref() == Some(&tr.slug))
                });
                if let Some(default) = expected.base.defaults.get_mut(&tr.workspace) {
                    if default.as_deref() == Some(tr.slug.as_str()) {
                        *default = None;
                    }
                }
            }
            Change::Recorded(record) => {
                expected.records.push(record.clone());
                expected.records.sort();
            }
        }
    }
    assert_eq!(
        after, expected,
        "state delta: the universe must equal the before-image plus exactly {changes:?}"
    );
}

// ===========================================================================
// Reading pages
// ===========================================================================

fn has_form(answer: &Answer, action: &str) -> bool {
    tm::forms(&answer.body).iter().any(|f| f.action == action)
}

fn has_link(answer: &Answer, text: &str) -> bool {
    let doc = parse(&answer.body);
    select_all(&doc, "a[href]")
        .into_iter()
        .any(|a| text_of(&a).split_whitespace().collect::<Vec<_>>().join(" ") == text)
}

fn has_control(answer: &Answer, text: &str) -> bool {
    let doc = parse(&answer.body);
    select_all(&doc, "a, button")
        .into_iter()
        .any(|a| text_of(&a).split_whitespace().collect::<Vec<_>>().join(" ") == text)
}

fn heading(answer: &Answer) -> Option<String> {
    let doc = parse(&answer.body);
    select_all(&doc, "h1")
        .first()
        .map(|h| text_of(h).split_whitespace().collect::<Vec<_>>().join(" "))
}

fn field_values(answer: &Answer, field: &str) -> Vec<String> {
    tm::forms(&answer.body)
        .into_iter()
        .flat_map(|f| f.fields.into_iter())
        .filter(|(k, _)| k == field)
        .map(|(_, v)| v)
        .collect()
}

fn require_status(answer: &Answer, status: StatusCode, what: &str) {
    assert_eq!(answer.status, status, "{what}; {}", tm::describe(answer));
}

// ===========================================================================
// Givens
// ===========================================================================

/// Homelab, exactly as `/admin/instance/workspaces` leaves it (DISCUSS
/// inventory): the workspace and its admin's membership, and no team.
pub async fn given_homelab(world: &mut FoundryWorld, admin: &str) {
    let ws = tm::seed_workspace(world, HOMELAB, HOMELAB_CREATED).await;
    let who = tm::resolve_person(world, admin);
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(ws)
    .bind(who.id)
    .execute(&tm::pool(world))
    .await
    .expect("seed Homelab's admin");
}

/// Switch the actor's session to the workspace through the shipped door.
pub async fn given_working_in(world: &mut FoundryWorld, who: &str, workspace: &str) {
    let actor = tm::resolve_actor(world, who);
    let ws = tm::workspace_id(world, workspace);
    let answer = tm::post(
        world,
        &actor,
        "/workspace/switch",
        &[("workspace_id".to_string(), ws.to_string())],
        true,
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::SEE_OTHER,
        "precondition: {who} switches to {workspace}; {}",
        tm::describe(&answer)
    );
    if let Actor::Person(email) = &actor {
        world
            .tl
            .working_in
            .insert(email.clone(), workspace.to_string());
    }
    world.tm.last_actor = Some(actor);
}

pub async fn given_instance_admin(world: &mut FoundryWorld, who: &str) {
    let p = tm::resolve_person(world, who);
    let u = tm::universe(world).await;
    assert_eq!(
        u.workspace_rows
            .get(&(tm::HOME.to_string(), p.email.clone()))
            .map(String::as_str),
        Some("member"),
        "precondition: {who} is a member, not an admin, of Canzan Labs"
    );
    sqlx::query("INSERT INTO instance_admins (user_id) VALUES ($1)")
        .bind(p.id)
        .execute(&tm::pool(world))
        .await
        .expect("seed an instance admin");
}

/// Teams written by hand before this feature, each with its stored address.
pub async fn given_legacy_teams(world: &mut FoundryWorld, step: &cucumber::gherkin::Step) {
    let ws = tm::workspace_id(world, tm::HOME);
    for r in tm::table_rows(step) {
        let team = TeamRef {
            id: uuid::Uuid::now_v7(),
            workspace: tm::HOME.to_string(),
            name: r[0].clone(),
            slug: r[1].clone(),
        };
        sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
            .bind(team.id)
            .bind(ws)
            .bind(&team.name)
            .bind(&team.slug)
            .execute(&tm::pool(world))
            .await
            .expect("seed a legacy team");
        world.tm.teams.insert(team.name.clone(), team);
    }
}

pub async fn given_projects(world: &mut FoundryWorld, team: &str, step: &cucumber::gherkin::Step) {
    for r in tm::table_rows(step) {
        tm::seed_project(world, team, &r[0], &r[1]).await;
    }
}

pub async fn given_scoped_tokens(world: &mut FoundryWorld, team: &str, count: usize) {
    let tr = tm::team_ref(world, team);
    let ws = tm::workspace_id(world, &tr.workspace);
    let priya = tm::resolve_person(world, "Priya");
    for n in 1..=count {
        sqlx::query(
            "INSERT INTO machine_tokens (jti, user_id, workspace_id, scope_team_id, expires_at, label, created_by)
                  VALUES ($1, $2, $3, $4, now() + interval '30 days', $5, $2)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(priya.id)
        .bind(ws)
        .bind(tr.id)
        .bind(format!("{team} deploy bot {n}"))
        .execute(&tm::pool(world))
        .await
        .expect("seed a scoped machine token");
    }
}

/// A Given that reuses the create When and requires it to go through (Pillar 2).
pub async fn given_has_created(world: &mut FoundryWorld, who: &str, name: &str) {
    when_creates_from_page(world, who, name).await;
    tm::require_ok(world, &format!("{who} creates the team {name:?}"));
}

/// Open the team's delete page from its members page and keep it open.
pub async fn given_opened_delete_earlier(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = follow_delete_link(world, &actor, team).await;
    let slug = tm::team_ref(world, team).slug;
    assert!(
        page.status == StatusCode::OK && has_form(&page, &delete_path(&slug)),
        "precondition: {who} opens {team}'s delete page and it offers Delete team; {}",
        tm::describe(&page)
    );
    world.tl.earlier_delete.insert(tm::actor_key(&actor), page);
    world.tm.last_actor = Some(actor);
}

/// The delete as a precondition (Pillar 2). It ends by capturing this
/// feature's before-image of the post-delete world, so a reused
/// team-membership `When` that captures only its own image (e.g. "adds … from
/// the page she opened earlier", scenario 24) is still judged fail-closed by
/// "no team, project or membership changed". A `When` of this feature captures
/// again, so nothing else changes.
pub async fn given_has_deleted(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = follow_delete_link(world, &actor, team).await;
    let answer = confirm_from(world, &actor, team, &page).await;
    assert!(
        answer.status == StatusCode::OK && tm::says(&answer, &format!("Deleted {team}.")),
        "precondition: {who} deletes {team}; {}",
        tm::describe(&answer)
    );
    tm::record(world, &actor, answer);
    capture_before(world).await;
}

pub async fn given_has_renamed(world: &mut FoundryWorld, who: &str, team: &str, new_name: &str) {
    when_renames(world, who, team, new_name, false).await;
    tm::require_ok(world, &format!("{who} renames {team} to {new_name:?}"));
}

/// A project created through the shipped door (the race partner of INV-TL-1).
pub async fn given_has_created_project(
    world: &mut FoundryWorld,
    who: &str,
    project: &str,
    key: &str,
    team: &str,
) {
    let actor = tm::resolve_actor(world, who);
    let answer = create_project_from_form(world, &actor, team, project, key).await;
    assert_eq!(
        answer.status,
        StatusCode::SEE_OTHER,
        "precondition: {who} creates {project} on {team}; {}",
        tm::describe(&answer)
    );
    register_project(world, team, project).await;
}

// ===========================================================================
// The doors
// ===========================================================================

/// The page the actor has in front of them if it is `path` (200) AND holds what
/// the write needs (`holds`), else a fresh GET of `path`.
///
/// The path alone is not enough: writes answer with a 200 re-render at the POST
/// path, no redirect (DDD-22). A create's answer is the NEW TEAM's members page
/// served at `/teams`, so a later "creates … from the teams page" must not take
/// it for the teams page (tl-03 scenario 31).
async fn page_at(
    world: &mut FoundryWorld,
    actor: &Actor,
    path: &str,
    holds: impl Fn(&Answer) -> bool,
) -> Answer {
    match world.tm.current.get(&tm::actor_key(actor)) {
        Some(page) if page.path == path && page.status == StatusCode::OK && holds(page) => {
            page.clone()
        }
        _ => tm::open(world, actor, path).await,
    }
}

async fn send_create(world: &mut FoundryWorld, actor: &Actor, typed: &str) -> Answer {
    let page = page_at(world, actor, "/teams", |p| has_form(p, "/teams")).await;
    let form = tm::form_at(&page, "/teams");
    let mut fields: Vec<(String, String)> = form
        .fields
        .into_iter()
        .filter(|(k, _)| k != "name")
        .collect();
    fields.push(("name".to_string(), typed.to_string()));
    world
        .tl
        .last_create
        .insert(tm::actor_key(actor), (form.action.clone(), fields.clone()));
    tm::post(world, actor, &form.action, &fields, true).await
}

async fn follow_delete_link(world: &mut FoundryWorld, actor: &Actor, team: &str) -> Answer {
    let slug = tm::team_ref(world, team).slug;
    let members = tm::open(world, actor, &members_path(&slug)).await;
    let href = tm::link_to(&members, "Delete team");
    tm::open(world, actor, &href).await
}

async fn confirm_from(
    world: &mut FoundryWorld,
    actor: &Actor,
    team: &str,
    page: &Answer,
) -> Answer {
    let slug = tm::team_ref(world, team).slug;
    let form = tm::form_at(page, &delete_path(&slug));
    tm::post(world, actor, &form.action, &form.fields, true).await
}

async fn rename_from(
    world: &mut FoundryWorld,
    actor: &Actor,
    team: &str,
    typed: Option<&str>,
    earlier: bool,
) -> Answer {
    let slug = tm::team_ref(world, team).slug;
    let page = if earlier {
        world
            .tm
            .earlier
            .get(&tm::actor_key(actor))
            .cloned()
            .expect("a Given opened that page earlier")
    } else {
        let rename = rename_path(&slug);
        page_at(world, actor, &members_path(&slug), |p| has_form(p, &rename)).await
    };
    let form = tm::form_at(&page, &rename_path(&slug));
    let fields: Vec<(String, String)> = match typed {
        Some(name) => {
            let mut f: Vec<(String, String)> = form
                .fields
                .into_iter()
                .filter(|(k, _)| k != "name")
                .collect();
            f.push(("name".to_string(), name.to_string()));
            f
        }
        None => form.fields,
    };
    tm::post(world, actor, &form.action, &fields, true).await
}

async fn create_project_from_form(
    world: &mut FoundryWorld,
    actor: &Actor,
    team: &str,
    project: &str,
    key: &str,
) -> Answer {
    let slug = tm::team_ref(world, team).slug;
    let page = tm::open(world, actor, &format!("/team/{slug}/projects/new")).await;
    let form = tm::form_at(&page, &format!("/team/{slug}/projects"));
    let mut fields: Vec<(String, String)> = form
        .fields
        .into_iter()
        .filter(|(k, _)| k != "name" && k != "key_prefix")
        .collect();
    fields.push(("name".to_string(), project.to_string()));
    fields.push(("key_prefix".to_string(), key.to_string()));
    tm::post(world, actor, &form.action, &fields, true).await
}

// ===========================================================================
// Whens
// ===========================================================================

pub async fn when_creates_from_page(world: &mut FoundryWorld, who: &str, raw: &str) {
    let actor = tm::resolve_actor(world, who);
    let typed = expand(raw);
    capture_before(world).await;
    let answer = send_create(world, &actor, &typed).await;
    if answer.status == StatusCode::OK {
        let ws = workspace_of_actor(world, &actor);
        register_team(world, &ws, typed.trim()).await;
    }
    tm::record(world, &actor, answer);
}

pub async fn when_sends_create_again(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let (action, fields) = world
        .tl
        .last_create
        .get(&tm::actor_key(&actor))
        .cloned()
        .expect("a create form was sent before");
    capture_before(world).await;
    let answer = tm::post(world, &actor, &action, &fields, true).await;
    tm::record(world, &actor, answer);
}

pub async fn when_submits_create(world: &mut FoundryWorld, who: &str, raw: &str, with_token: bool) {
    let actor = tm::resolve_actor(world, who);
    capture_before(world).await;
    let answer = tm::post(
        world,
        &actor,
        "/teams",
        &[("name".to_string(), expand(raw))],
        with_token,
    )
    .await;
    tm::record(world, &actor, answer);
}

pub async fn when_follows_first_project_link(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    capture_before(world).await;
    let dash = tm::open(world, &actor, "/").await;
    let href = tm::link_to(&dash, "create your first project");
    let page = tm::open(world, &actor, &href).await;
    tm::record(world, &actor, page);
}

pub async fn when_new_project_from_members_page(
    world: &mut FoundryWorld,
    who: &str,
    team: &str,
    project: &str,
    key: &str,
) {
    let actor = tm::resolve_actor(world, who);
    let slug = tm::team_ref(world, team).slug;
    capture_before(world).await;
    let members = page_at(world, &actor, &members_path(&slug), |p| {
        has_link(p, "New project")
    })
    .await;
    let href = tm::link_to(&members, "New project");
    let form_page = tm::open(world, &actor, &href).await;
    let form = tm::form_at(&form_page, &format!("/team/{slug}/projects"));
    let mut fields: Vec<(String, String)> = form
        .fields
        .into_iter()
        .filter(|(k, _)| k != "name" && k != "key_prefix")
        .collect();
    fields.push(("name".to_string(), project.to_string()));
    fields.push(("key_prefix".to_string(), key.to_string()));
    let created = tm::post(world, &actor, &form.action, &fields, true).await;
    let board = match (created.status, created.location.clone()) {
        (StatusCode::SEE_OTHER, Some(location)) => tm::open(world, &actor, &location).await,
        _ => created,
    };
    register_project(world, team, project).await;
    tm::record(world, &actor, board);
}

pub async fn when_follows_delete(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    capture_before(world).await;
    let page = follow_delete_link(world, &actor, team).await;
    tm::record(world, &actor, page);
}

pub async fn when_confirms(world: &mut FoundryWorld, who: &str, earlier: bool) {
    let actor = tm::resolve_actor(world, who);
    let page = if earlier {
        world
            .tl
            .earlier_delete
            .get(&tm::actor_key(&actor))
            .cloned()
            .expect("a Given opened the delete page earlier")
    } else {
        world
            .tm
            .current
            .get(&tm::actor_key(&actor))
            .cloned()
            .expect("the delete page is in front of them")
    };
    let team = world
        .tm
        .teams
        .values()
        .find(|t| page.path == delete_path(&t.slug))
        .map(|t| t.name.clone())
        .unwrap_or_else(|| panic!("the page at {} is a team's delete page", page.path));
    capture_before(world).await;
    let answer = confirm_from(world, &actor, &team, &page).await;
    tm::record(world, &actor, answer);
}

pub async fn when_follows_cancel(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = world
        .tm
        .current
        .get(&tm::actor_key(&actor))
        .cloned()
        .expect("the delete page is in front of them");
    let href = tm::link_to(&page, "Cancel");
    let answer = tm::open(world, &actor, &href).await;
    tm::record(world, &actor, answer);
}

pub async fn when_asks_delete_page(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let slug = slug_or_unknown(world, team);
    capture_before(world).await;
    let answer = tm::open(world, &actor, &delete_path(&slug)).await;
    tm::record(world, &actor, answer);
}

pub async fn when_submits_delete(
    world: &mut FoundryWorld,
    who: &str,
    team: &str,
    with_token: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let slug = slug_or_unknown(world, team);
    capture_before(world).await;
    let answer = tm::post(world, &actor, &delete_path(&slug), &[], with_token).await;
    tm::record(world, &actor, answer);
}

pub async fn when_renames(
    world: &mut FoundryWorld,
    who: &str,
    team: &str,
    raw: &str,
    earlier: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let typed = expand(raw);
    capture_before(world).await;
    let answer = rename_from(world, &actor, team, Some(&typed), earlier).await;
    tm::record(world, &actor, answer);
}

pub async fn when_submits_rename_unchanged(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    capture_before(world).await;
    let answer = rename_from(world, &actor, team, None, false).await;
    tm::record(world, &actor, answer);
}

pub async fn when_submits_rename(
    world: &mut FoundryWorld,
    who: &str,
    team: &str,
    raw: &str,
    with_token: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let slug = slug_or_unknown(world, team);
    capture_before(world).await;
    let answer = tm::post(
        world,
        &actor,
        &rename_path(&slug),
        &[("name".to_string(), expand(raw))],
        with_token,
    )
    .await;
    tm::record(world, &actor, answer);
}

pub async fn when_operator_adds_team(world: &mut FoundryWorld, raw: &str) {
    let typed = expand(raw);
    let ws = tm::workspace_id(world, tm::HOME);
    capture_before(world).await;
    let result =
        sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
            .bind(uuid::Uuid::now_v7())
            .bind(ws)
            .bind(&typed)
            .bind(HAND_SLUG)
            .execute(&tm::pool(world))
            .await;
    world.tl.db = Some((typed, db_answer(result)));
}

pub async fn when_operator_renames_team(world: &mut FoundryWorld, team: &str, raw: &str) {
    let typed = expand(raw);
    let tr = tm::team_ref(world, team);
    capture_before(world).await;
    let result = sqlx::query("UPDATE teams SET name = $1 WHERE id = $2")
        .bind(&typed)
        .bind(tr.id)
        .execute(&tm::pool(world))
        .await;
    world.tl.db = Some((typed, db_answer(result)));
}

fn db_answer(result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>) -> DbAnswer {
    match result {
        Ok(done) => DbAnswer::Accepted(done.rows_affected()),
        Err(err) => {
            let db = err
                .as_database_error()
                .unwrap_or_else(|| panic!("expected the database's answer, got {err:?}"));
            let pg = db.downcast_ref::<sqlx::postgres::PgDatabaseError>();
            DbAnswer::Refused {
                code: pg.code().to_string(),
                constraint: pg.constraint().map(str::to_string),
                table: pg.table().map(str::to_string),
                detail: pg.detail().map(str::to_string),
            }
        }
    }
}

/// INV-TL-1 over HTTP: before each run Design exists again with Lena as its
/// lead and no project (a reset, never an outcome); then Priya's delete and
/// Lena's project create are sent at the same moment.
pub async fn when_delete_races_create(
    world: &mut FoundryWorld,
    admin: &str,
    team: &str,
    creator: &str,
    project: &str,
    key: &str,
    times: usize,
) {
    let priya = tm::resolve_actor(world, admin);
    let lena = tm::resolve_actor(world, creator);
    let lena_email = tm::resolve_person(world, creator).email;
    let priya_cookie = tm::cookie_for(world, &priya).await;
    let lena_cookie = tm::cookie_for(world, &lena).await;
    let base = tm::harness(world).base_url();
    let client = tm::http(world);
    let csrf = world.tm.csrf.clone();
    let ws = tm::workspace_id(world, tm::HOME);
    world.tl.delete_race.clear();
    for _ in 0..times {
        reset_team(world, ws, team, &lena_email).await;
        let slug = tm::team_ref(world, team).slug;
        let send = |path: String, cookie: String, fields: Vec<(String, String)>| {
            let client = client.clone();
            let base = base.clone();
            let csrf = csrf.clone();
            async move {
                let mut form = vec![("_csrf".to_string(), csrf)];
                form.extend(fields);
                let resp = client
                    .post(format!("{base}{path}"))
                    .header(COOKIE, cookie)
                    .form(&form)
                    .send()
                    .await
                    .expect("race POST");
                tm::into_answer(&path, resp).await
            }
        };
        let (delete, create) = tokio::join!(
            send(delete_path(&slug), priya_cookie.clone(), vec![]),
            send(
                format!("/team/{slug}/projects"),
                lena_cookie.clone(),
                vec![
                    ("name".to_string(), project.to_string()),
                    ("key_prefix".to_string(), key.to_string()),
                ],
            )
        );
        let (team_exists, project_exists): (bool, bool) = sqlx::query_as(
            "SELECT EXISTS (SELECT 1 FROM teams WHERE workspace_id = $1 AND slug = $2),
                    EXISTS (SELECT 1 FROM projects WHERE workspace_id = $1 AND name = $3)",
        )
        .bind(ws)
        .bind(&slug)
        .bind(project)
        .fetch_one(&tm::pool(world))
        .await
        .expect("read the race's outcome");
        world.tl.delete_race.push(DeleteRace {
            delete,
            create,
            team_exists,
            project_exists,
        });
    }
}

/// The race reset: the team exists with `lead` as its lead and no project.
async fn reset_team(world: &mut FoundryWorld, ws: uuid::Uuid, team: &str, lead: &str) {
    let pool = tm::pool(world);
    let tr = tm::team_ref(world, team);
    let (exists,): (bool,) = sqlx::query_as("SELECT EXISTS (SELECT 1 FROM teams WHERE id = $1)")
        .bind(tr.id)
        .fetch_one(&pool)
        .await
        .expect("does the team exist");
    let tr = if exists {
        sqlx::query("DELETE FROM projects WHERE team_id = $1")
            .bind(tr.id)
            .execute(&pool)
            .await
            .expect("reset: remove the team's projects");
        tr
    } else {
        let fresh = TeamRef {
            id: uuid::Uuid::now_v7(),
            ..tr
        };
        sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
            .bind(fresh.id)
            .bind(ws)
            .bind(&fresh.name)
            .bind(&fresh.slug)
            .execute(&pool)
            .await
            .expect("reset: the team exists again");
        world.tm.teams.insert(team.to_string(), fresh.clone());
        fresh
    };
    tm::seed_team_row(world, &tr.name, lead, tm::TeamRole::Lead).await;
}

/// INV-TL-4 over HTTP: before each run the raced names are free again (a
/// reset); then both admins send their create at the same moment.
pub async fn when_create_race(
    world: &mut FoundryWorld,
    first: (&str, &str),
    second: (&str, &str),
    times: usize,
) {
    let a = tm::resolve_actor(world, first.0);
    let b = tm::resolve_actor(world, second.0);
    let a_cookie = tm::cookie_for(world, &a).await;
    let b_cookie = tm::cookie_for(world, &b).await;
    let base = tm::harness(world).base_url();
    let client = tm::http(world);
    let csrf = world.tm.csrf.clone();
    let ws = tm::workspace_id(world, tm::HOME);
    let names: BTreeSet<String> = [first.1.to_string(), second.1.to_string()].into();
    world.tl.create_race.clear();
    for _ in 0..times {
        for name in &names {
            sqlx::query("DELETE FROM teams WHERE workspace_id = $1 AND name = $2")
                .bind(ws)
                .bind(name)
                .execute(&tm::pool(world))
                .await
                .expect("reset: the raced name is free");
        }
        let send = |cookie: String, name: String| {
            let client = client.clone();
            let base = base.clone();
            let csrf = csrf.clone();
            async move {
                let form = vec![("_csrf".to_string(), csrf), ("name".to_string(), name)];
                let resp = client
                    .post(format!("{base}/teams"))
                    .header(COOKIE, cookie)
                    .form(&form)
                    .send()
                    .await
                    .expect("race POST");
                tm::into_answer("/teams", resp).await
            }
        };
        let (one, two) = tokio::join!(
            send(a_cookie.clone(), first.1.to_string()),
            send(b_cookie.clone(), second.1.to_string())
        );
        let mut teams: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for name in &names {
            let slugs: Vec<(String,)> = sqlx::query_as(
                "SELECT slug FROM teams WHERE workspace_id = $1 AND name = $2 ORDER BY slug",
            )
            .bind(ws)
            .bind(name)
            .fetch_all(&tm::pool(world))
            .await
            .expect("read the raced teams");
            teams.insert(name.clone(), slugs.into_iter().map(|(s,)| s).collect());
        }
        world.tl.create_race.push(CreateRace {
            first: one,
            second: two,
            teams,
        });
    }
}

// ===========================================================================
// Thens
// ===========================================================================

pub fn then_refused_with(world: &FoundryWorld, copy: &str, raw: &str) {
    let answer = tm::last(world);
    require_status(
        answer,
        StatusCode::UNPROCESSABLE_ENTITY,
        &format!("a name refusal is a 422 saying {copy:?} (DDD-18)"),
    );
    assert!(
        tm::says(answer, copy),
        "the refusal must say {copy:?}; {}",
        tm::describe(answer)
    );
    // An HTML parser turns U+0000 (raw or `&#0;`) into U+FFFD, so a kept NUL is
    // read back as U+FFFD (tl-03 scenario 33, the `[NUL]` row); every other
    // character is compared as typed.
    let typed = expand(raw).replace('\0', "\u{FFFD}");
    assert!(
        field_values(answer, "name").contains(&typed),
        "the form must keep the text {typed:?}; name fields: {:?}",
        field_values(answer, "name")
    );
}

/// "she sees the team "…" created, with her as its lead": the create's 200
/// page with "Created {name}. You're its lead.", the name expanded from the
/// feature's marks (`[64×a]`), which team-membership's "she sees" does not do.
pub fn then_sees_created(world: &FoundryWorld, raw: &str) {
    tm::then_sees(world, &format!("Created {}. You're its lead.", expand(raw)));
}

pub async fn then_nothing_changed(world: &FoundryWorld) {
    assert_state_delta(world, &[]).await;
}

pub async fn then_now_has_team(
    world: &mut FoundryWorld,
    workspace: &str,
    raw_name: &str,
    raw_slug: &str,
    lead: &str,
) {
    let name = expand(raw_name);
    let slug = expand(raw_slug);
    let lead = tm::resolve_person(world, lead).email;
    assert_state_delta(
        world,
        &[Change::Created {
            workspace: workspace.to_string(),
            name: name.trim().to_string(),
            slug,
            lead,
        }],
    )
    .await;
    if !world.tm.teams.contains_key(name.trim()) {
        register_team(world, workspace, name.trim()).await;
    }
}

/// The delete's declared delta; the Gherkin must name the scoped tokens and the
/// cleared default exactly when the before-image has them.
pub async fn then_deleted(
    world: &FoundryWorld,
    team: &str,
    tokens: Option<usize>,
    clears_default: bool,
    actor: &str,
) {
    let before = before(world);
    let tr = tm::team_ref(world, team);
    let scoped = before
        .tokens
        .values()
        .filter(|(w, s)| *w == tr.workspace && s.as_deref() == Some(tr.slug.as_str()))
        .count();
    assert_eq!(
        scoped,
        tokens.unwrap_or(0),
        "the scenario must name the {scoped} machine token(s) scoped to {team}"
    );
    let was_default = before.base.defaults.get(&tr.workspace) == Some(&Some(tr.slug.clone()));
    assert_eq!(
        was_default, clears_default,
        "the scenario must say whether {team} was the default team"
    );
    let name = before
        .team_names
        .get(&(tr.workspace.clone(), tr.slug.clone()))
        .cloned()
        .expect("the team existed before");
    let actor = tm::resolve_person(world, actor).email;
    assert_state_delta(
        world,
        &[
            Change::Deleted {
                team: team.to_string(),
            },
            Change::Recorded(Record {
                workspace: tr.workspace.clone(),
                kind: RecordKind::Deleted,
                team_slug: tr.slug.clone(),
                name,
                new_name: None,
                actor,
            }),
        ],
    )
    .await;
}

pub async fn then_renamed(world: &FoundryWorld, team: &str, raw_new: &str, actor: &str) {
    let new_name = expand(raw_new);
    let before = before(world);
    let tr = tm::team_ref(world, team);
    let old = before
        .team_names
        .get(&(tr.workspace.clone(), tr.slug.clone()))
        .cloned()
        .expect("the team existed before");
    let actor = tm::resolve_person(world, actor).email;
    assert_state_delta(
        world,
        &[
            Change::Renamed {
                team: team.to_string(),
                new_name: new_name.clone(),
            },
            Change::Recorded(Record {
                workspace: tr.workspace.clone(),
                kind: RecordKind::Renamed,
                team_slug: tr.slug.clone(),
                name: old,
                new_name: Some(new_name),
                actor,
            }),
        ],
    )
    .await;
}

pub async fn then_board_opens_and_dashboard_lists(
    world: &mut FoundryWorld,
    project: &str,
    entry: &str,
) {
    let answer = tm::last(world).clone();
    let actor = world.tm.last_actor.clone().expect("the When's actor");
    let p = tm::project_ref(world, project);
    let board = format!("/team/{}/project/{}", p.team_slug, p.slug);
    assert!(
        answer.status == StatusCode::OK && answer.path == board,
        "the {project} board opens after the create; {}",
        tm::describe(&answer)
    );
    let section = tm::dashboard_projects(world, &actor).await;
    assert!(
        tm::page_text(&section).contains(entry),
        "the dashboard lists {entry:?}; Your projects: {}",
        tm::page_text(&section)
    );
}

pub fn then_new_project_form(world: &FoundryWorld, team: &str) {
    let answer = tm::last(world);
    let slug = tm::team_ref(world, team).slug;
    assert!(
        answer.status == StatusCode::OK
            && answer.path == format!("/team/{slug}/projects/new")
            && has_form(answer, &format!("/team/{slug}/projects")),
        "the link must lead to {team}'s new project form (D14, DDD-21); {}",
        tm::describe(answer)
    );
}

/// What the members page in front of the actor offers (DDD-21, ADR-004).
pub fn then_members_page_offers(world: &FoundryWorld, offers: bool, control: &str) {
    let answer = tm::last(world);
    require_status(answer, StatusCode::OK, "the members page renders");
    let slug = answer
        .path
        .strip_prefix("/team/")
        .and_then(|rest| rest.strip_suffix("/members"))
        .unwrap_or_else(|| panic!("the last page is a members page, not {}", answer.path))
        .to_string();
    let present = match control {
        "New project" => has_link(answer, "New project"),
        "Rename" => has_form(answer, &rename_path(&slug)),
        "Delete team" => has_link(answer, "Delete team"),
        other => panic!("the members page has no control called {other:?}"),
    };
    assert_eq!(
        present,
        offers,
        "the members page must {} {control}; {}",
        if offers { "offer" } else { "not offer" },
        tm::describe(answer)
    );
}

pub fn then_teams_page_offers_create(world: &FoundryWorld, offers: bool) {
    let answer = tm::last(world);
    require_status(answer, StatusCode::OK, "the teams page renders");
    assert_eq!(
        has_form(answer, "/teams"),
        offers,
        "the teams page must {} the create form; {}",
        if offers { "offer" } else { "not offer" },
        tm::describe(answer)
    );
}

pub async fn then_still_offers_create(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = tm::open(world, &actor, "/teams").await;
    assert!(
        page.status == StatusCode::OK
            && has_form(&page, "/teams")
            && field_values(&page, "name").len() == 1,
        "control: {who}'s teams page offers the create form, so the refusal hid a door that exists; {}",
        tm::describe(&page)
    );
}

pub async fn then_teams_page_lists(world: &mut FoundryWorld, who: &str, entry: &str, lists: bool) {
    let actor = tm::resolve_actor(world, who);
    let page = tm::open(world, &actor, "/teams").await;
    require_status(&page, StatusCode::OK, "the teams page renders");
    let text = tm::page_text(&page.body);
    let found = if entry.contains(" · ") {
        text.contains(entry)
    } else {
        text.contains(&format!("{entry} · "))
    };
    assert_eq!(
        found,
        lists,
        "{who}'s teams page must {}list {entry:?}; {}",
        if lists { "" } else { "not " },
        tm::describe(&page)
    );
}

pub fn then_delete_page_asks(world: &FoundryWorld, question: &str) {
    let answer = tm::last(world);
    require_status(answer, StatusCode::OK, "the delete page renders");
    assert_eq!(
        heading(answer).as_deref(),
        Some(question),
        "the delete page's heading (D12); {}",
        tm::describe(answer)
    );
}

pub fn then_delete_page_says(world: &FoundryWorld, copy: &str) {
    let answer = tm::last(world);
    require_status(
        answer,
        StatusCode::OK,
        "the delete page renders (DDD-18: 200)",
    );
    assert!(
        tm::says(answer, copy),
        "the delete page must say {copy:?}; {}",
        tm::describe(answer)
    );
}

pub fn then_delete_page_silent_on_default_and_tokens(world: &FoundryWorld) {
    let answer = tm::last(world);
    let text = tm::page_text(&answer.body);
    for copy in [
        "is the default team for new members",
        "machine token(s) scoped to",
    ] {
        assert!(
            !text.contains(copy),
            "the page must not mention {copy:?} for a team that is neither; {}",
            tm::describe(answer)
        );
    }
}

pub fn then_delete_page_offers(world: &FoundryWorld, offers: bool) {
    let answer = tm::last(world);
    require_status(answer, StatusCode::OK, "the delete page renders");
    let slug = answer
        .path
        .strip_prefix("/team/")
        .and_then(|rest| rest.strip_suffix("/delete"))
        .unwrap_or_else(|| panic!("the last page is a delete page, not {}", answer.path))
        .to_string();
    let delete = has_form(answer, &delete_path(&slug)) && has_control(answer, "Delete team");
    assert_eq!(
        delete,
        offers,
        "the delete page must {} Delete team (D8, OQ-4); {}",
        if offers { "offer" } else { "not offer" },
        tm::describe(answer)
    );
    if offers {
        assert!(
            has_link(answer, "Cancel"),
            "the delete page offers Cancel, a link back to the team; {}",
            tm::describe(answer)
        );
    }
}

pub fn then_back_on_members_page(world: &FoundryWorld, team: &str) {
    let answer = tm::last(world);
    let slug = tm::team_ref(world, team).slug;
    assert!(
        answer.status == StatusCode::OK && answer.path == members_path(&slug),
        "Cancel leads back to {team}'s members page; {}",
        tm::describe(answer)
    );
}

pub fn then_no_notice(world: &FoundryWorld, team: &str) {
    let answer = tm::last(world);
    require_status(
        answer,
        StatusCode::OK,
        "a byte-equal rename re-renders the members page (D15)",
    );
    assert!(
        heading(answer).as_deref() == Some(team),
        "the answer is {team}'s members page; {}",
        tm::describe(answer)
    );
    let doc = parse(&answer.body);
    assert!(
        select_all(&doc, ".notice").is_empty() && !tm::page_text(&answer.body).contains("Renamed "),
        "a no-op rename shows no notice (D15); {}",
        tm::describe(answer)
    );
}

pub async fn then_still_finds_delete_page(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let slug = tm::team_ref(world, team).slug;
    let page = tm::open(world, &actor, &delete_path(&slug)).await;
    let text = tm::page_text(&page.body);
    assert!(
        page.status == StatusCode::OK
            && (text.contains(&format!("Delete {team}?"))
                || text.contains(&format!("{team} still has"))),
        "control: {who} finds {team}'s delete page, so the refusal hid a team that exists; {}",
        tm::describe(&page)
    );
}

pub async fn then_still_finds_rename_form(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let slug = tm::team_ref(world, team).slug;
    let page = tm::open(world, &actor, &members_path(&slug)).await;
    assert!(
        page.status == StatusCode::OK && has_form(&page, &rename_path(&slug)),
        "control: {who} finds {team}'s rename form, so the refusal hid a team that exists; {}",
        tm::describe(&page)
    );
}

pub async fn then_members_page_at_opens(world: &mut FoundryWorld, slug: &str, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = tm::open(world, &actor, &members_path(&expand(slug))).await;
    assert!(
        page.status == StatusCode::OK && tm::row_count(&page) >= 1,
        "the members page at {slug:?} opens; {}",
        tm::describe(&page)
    );
}

pub async fn then_board_still_opens(world: &mut FoundryWorld, project: &str, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let board = tm::open_board(world, &actor, project).await;
    require_status(
        &board,
        StatusCode::OK,
        &format!("the {project} board still opens"),
    );
}

pub async fn then_same_create_with_token(world: &mut FoundryWorld, name: &str) {
    let actor = world.tm.last_actor.clone().expect("the When's actor");
    let Actor::Person(email) = actor.clone() else {
        panic!("the control is sent by the person who was turned away");
    };
    capture_before(world).await;
    let answer = tm::post(
        world,
        &actor,
        "/teams",
        &[("name".to_string(), name.to_string())],
        true,
    )
    .await;
    require_status(
        &answer,
        StatusCode::OK,
        "control: the same create with the token goes through",
    );
    assert_state_delta(
        world,
        &[Change::Created {
            workspace: tm::HOME.to_string(),
            name: name.to_string(),
            slug: tm::slug_of(name),
            lead: email,
        }],
    )
    .await;
}

pub async fn then_same_delete_with_token(world: &mut FoundryWorld, team: &str) {
    let actor = world.tm.last_actor.clone().expect("the When's actor");
    let Actor::Person(email) = actor.clone() else {
        panic!("the control is sent by the person who was turned away");
    };
    let slug = tm::team_ref(world, team).slug;
    capture_before(world).await;
    let answer = tm::post(world, &actor, &delete_path(&slug), &[], true).await;
    require_status(
        &answer,
        StatusCode::OK,
        "control: the same delete with the token goes through",
    );
    let who = tm::person(world, &email).name;
    then_deleted(world, team, None, false, &who).await;
}

pub async fn then_same_rename_with_token(world: &mut FoundryWorld, team: &str, new_name: &str) {
    let actor = world.tm.last_actor.clone().expect("the When's actor");
    let Actor::Person(email) = actor.clone() else {
        panic!("the control is sent by the person who was turned away");
    };
    let slug = tm::team_ref(world, team).slug;
    capture_before(world).await;
    let answer = tm::post(
        world,
        &actor,
        &rename_path(&slug),
        &[("name".to_string(), new_name.to_string())],
        true,
    )
    .await;
    require_status(
        &answer,
        StatusCode::OK,
        "control: the same rename with the token goes through",
    );
    let who = tm::person(world, &email).name;
    then_renamed(world, team, new_name, &who).await;
}

pub fn then_delete_race_outcomes(world: &FoundryWorld) {
    assert!(!world.tl.delete_race.is_empty(), "the race ran");
    for (i, run) in world.tl.delete_race.iter().enumerate() {
        let kept = run.create.status == StatusCode::SEE_OTHER
            && run.delete.status == StatusCode::CONFLICT
            && run.team_exists
            && run.project_exists;
        let deleted = run.delete.status == StatusCode::OK
            && run.create.status == StatusCode::NOT_FOUND
            && !run.team_exists
            && !run.project_exists;
        assert!(
            kept || deleted,
            "run {}: either the project was kept and the delete refused (409), or the team was \
             deleted and the create found no team (404); delete {}, create {}, team exists {}, \
             project exists {}",
            i + 1,
            tm::describe(&run.delete),
            tm::describe(&run.create),
            run.team_exists,
            run.project_exists
        );
    }
}

pub fn then_no_server_error(world: &FoundryWorld) {
    let answers: Vec<&Answer> = world
        .tl
        .delete_race
        .iter()
        .flat_map(|r| [&r.delete, &r.create])
        .chain(
            world
                .tl
                .create_race
                .iter()
                .flat_map(|r| [&r.first, &r.second]),
        )
        .collect();
    assert!(!answers.is_empty(), "a race ran");
    for a in answers {
        assert!(
            !a.status.is_server_error(),
            "no racing write may answer a server error (INV-TL-2, INV-TL-4); {}",
            tm::describe(a)
        );
    }
}

pub fn then_no_project_lost(world: &FoundryWorld) {
    for (i, run) in world.tl.delete_race.iter().enumerate() {
        if run.create.status == StatusCode::SEE_OTHER {
            assert!(
                run.project_exists,
                "run {}: the create went through, so the project must still exist (INV-TL-1)",
                i + 1
            );
        }
    }
}

pub fn then_one_team_each_time(world: &FoundryWorld, raw: &str, copy: &str) {
    let name = expand(raw);
    assert!(!world.tl.create_race.is_empty(), "the race ran");
    for (i, run) in world.tl.create_race.iter().enumerate() {
        let slugs = run.teams.get(&name).cloned().unwrap_or_default();
        assert_eq!(
            slugs.len(),
            1,
            "run {}: exactly one {name:?} team; got {slugs:?}",
            i + 1
        );
        let (won, lost) = match (run.first.status, run.second.status) {
            (StatusCode::OK, StatusCode::UNPROCESSABLE_ENTITY) => (&run.first, &run.second),
            (StatusCode::UNPROCESSABLE_ENTITY, StatusCode::OK) => (&run.second, &run.first),
            _ => panic!(
                "run {}: one 200 and one 422; got {} and {}",
                i + 1,
                tm::describe(&run.first),
                tm::describe(&run.second)
            ),
        };
        assert!(
            tm::says(won, &format!("Created {name}. You're its lead.")) && tm::says(lost, copy),
            "run {}: the winner is told it was created and the other {copy:?}; {} / {}",
            i + 1,
            tm::describe(won),
            tm::describe(lost)
        );
    }
}

pub fn then_both_created(world: &FoundryWorld, a: &str, b: &str) {
    assert!(!world.tl.create_race.is_empty(), "the race ran");
    let wanted: BTreeSet<String> = [a.to_string(), b.to_string()].into();
    for (i, run) in world.tl.create_race.iter().enumerate() {
        assert!(
            run.first.status == StatusCode::OK && run.second.status == StatusCode::OK,
            "run {}: both creates go through (never a false duplicate); {} / {}",
            i + 1,
            tm::describe(&run.first),
            tm::describe(&run.second)
        );
        let got: BTreeSet<String> = run.teams.values().flatten().cloned().collect();
        let counts: Vec<usize> = run.teams.values().map(Vec::len).collect();
        assert!(
            got == wanted && counts.iter().all(|n| *n == 1),
            "run {}: one team each, at {wanted:?}; got {:?}",
            i + 1,
            run.teams
        );
    }
}

pub fn then_db_refuses(world: &FoundryWorld, arm: &str) {
    let (typed, answer) = world.tl.db.clone().expect("the operator typed a statement");
    match answer {
        DbAnswer::Refused {
            code,
            constraint,
            table,
            detail,
        } => {
            assert_eq!(code, "23514", "a check violation (OQ-7, ADR-NAME-DB-001) for {typed:?}");
            assert_eq!(constraint.as_deref(), Some(arm), "the refusal names the rule");
            assert_eq!(table.as_deref(), Some("teams"), "the refusal names the table");
            assert!(detail.is_none(), "no row dump in DETAIL");
        }
        DbAnswer::Accepted(n) => panic!(
            "the database must refuse {typed:?} under {arm:?} (OQ-7: migration 0024); it stored {n} row(s)"
        ),
    }
}

pub async fn then_db_stores_exactly(world: &FoundryWorld) {
    let (typed, answer) = world.tl.db.clone().expect("the operator typed a statement");
    assert_eq!(
        answer,
        DbAnswer::Accepted(1),
        "a name the app would accept is accepted by the database (never stricter)"
    );
    assert_state_delta(
        world,
        &[Change::HandAdded {
            workspace: tm::HOME.to_string(),
            name: typed,
        }],
    )
    .await;
}
