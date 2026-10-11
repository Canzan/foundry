//! team-membership — the acceptance driver the `feature_team_membership*` step
//! files delegate to (DISTILL 2026-10-09).
//!
//! Every step body in those files is one call into this module. The module owns
//! three things, and nothing else:
//!
//! - **The vocabulary** ([`TeamRole`], [`TeamRef`], [`PersonRec`], [`Target`],
//!   [`Answer`], [`Universe`], [`Change`]): the domain nouns the Gherkin uses,
//!   typed once.
//! - **The doors**: real HTTP against the in-process router (real session, real
//!   double-submit CSRF), entering only through the DESIGN-pinned routes (DDD-2):
//!   `GET /teams`, `POST /teams/default`, `GET|POST /team/{slug}/members`,
//!   `POST /team/{slug}/members/{user_id}/role|remove`, plus the shipped doors
//!   `GET /`, `POST /workspace/invites`, `GET|POST /invites/accept`,
//!   `POST /workspace/members/remove` and the board `GET /team/{t}/project/{p}`.
//!   A write made "from a page" is the page's own form: its action, its hidden
//!   fields and the option the person picked. A write a caller has no form for
//!   (the authz matrix) is the same POST built by hand, as an attacker would.
//! - **The universe** (Mandate 8, the repo's Rust reading): every team row
//!   `(workspace, team, email) -> role`, every workspace row, every account, every
//!   team and every workspace's default team. A `When` captures it first; a `Then`
//!   compares the after-image with the before-image plus the declared [`Change`]s,
//!   fail-closed: anything that moved and was not declared is a failure.
//!
//! Preconditions (the roster, a leaderless legacy team, a team deleted by hand)
//! are seeded with SQL, never the expected outcome (No Fixture Theater): every
//! membership the scenarios assert on is written by the product.
//!
//! The setting `workspaces.default_team_id` arrives with migration 0022 (slice
//! 05). Until then the universe reads every default as `None`, so the slice 01-04
//! scenarios run unchanged before and after it.

use crate::support::harness::InProcHarness;
use crate::support::html_assertions::{parse, select_all, text_of};
use crate::world::FoundryWorld;
use reqwest::header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
use reqwest::StatusCode;
use secrecy::SecretString;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Everyone in the roster signs in with this one password (hashed once per run).
pub const ROSTER_PASSWORD: &str = "canzan-labs-correct-horse-battery";
/// The newcomer's password when he accepts an invite.
pub const NEWCOMER_PASSWORD: &str = "newcomer-correct-horse-battery";
/// The Keycloak realm role foundry provisions in these scenarios.
pub const PROVISION_ROLE: &str = "foundry-user";
/// The instance's original workspace and the other tenant.
pub const HOME: &str = "Canzan Labs";
pub const FOREIGN: &str = "Globex";
/// When each workspace was created: the original one is the oldest (keycloak-sso
/// provisioning joins it).
const HOME_CREATED: &str = "2025-06-01T00:00:00Z";
const FOREIGN_CREATED: &str = "2026-01-01T00:00:00Z";
const TEST_NOW: &str = "2026-10-09T12:00:00Z";

/// D15 / OQ-9 copy that the shipped dashboard already shows (unchanged string).
pub const NO_TEAM_COPY: &str = "You're not on any team yet — ask a workspace admin to add you.";

// ===========================================================================
// Vocabulary
// ===========================================================================

/// A person's role on a team (`team_memberships.role`), as the pages label it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TeamRole {
    Lead,
    Member,
}

impl TeamRole {
    /// "lead" / "member" (or "Lead" / "Member") as written in a scenario.
    pub fn parse(word: &str) -> Self {
        match word.to_ascii_lowercase().as_str() {
            "lead" => Self::Lead,
            "member" => Self::Member,
            other => panic!("a scenario named the team role {other:?}; only lead and member exist"),
        }
    }

    /// The value the role form posts and the database stores (DDD-2).
    pub fn wire(self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Member => "member",
        }
    }

    /// The label every page shows (D15: "Lead" and "Member").
    pub fn label(self) -> &'static str {
        match self {
            Self::Lead => "Lead",
            Self::Member => "Member",
        }
    }
}

/// A person's role in a workspace (`workspace_memberships.role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceRole {
    Admin,
    Member,
}

impl WorkspaceRole {
    pub fn parse(word: &str) -> Self {
        match word.trim().to_ascii_lowercase().as_str() {
            "admin" => Self::Admin,
            "member" => Self::Member,
            other => panic!("a scenario named the workspace role {other:?}"),
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }
}

/// One team as seeded: its workspace, its address and its board.
#[derive(Debug, Clone)]
pub struct TeamRef {
    pub id: uuid::Uuid,
    pub workspace: String,
    pub name: String,
    pub slug: String,
}

/// One project as seeded: `(team slug, project slug)` is its board address.
#[derive(Debug, Clone)]
pub struct ProjectRef {
    pub team_slug: String,
    pub slug: String,
}

/// One person the scenarios can name.
#[derive(Debug, Clone)]
pub struct PersonRec {
    pub name: String,
    pub email: String,
    /// What the scenarios call them when two accounts share a name ("Jeff",
    /// "Jeff's Keycloak account").
    pub alias: Option<String>,
    pub id: uuid::Uuid,
    /// `None` for an account that has no usable password (Keycloak-provisioned).
    pub password: Option<String>,
}

/// Who a request comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    SignedOut,
    /// Keyed by email.
    Person(String),
}

/// Whom a write names as its target (`{user_id}` or the Add form's `user_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A person in the roster (keyed by email).
    Person(String),
    /// A well-formed id that names nobody.
    Nobody,
    /// A value that is not an id at all.
    Garbled,
}

/// Which team a request names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TeamName {
    Known(String),
    /// A slug no workspace has.
    Unknown,
}

/// One answer from the product, kept as the scenario's evidence.
#[derive(Debug, Clone)]
pub struct Answer {
    pub path: String,
    pub status: StatusCode,
    pub content_type: Option<String>,
    pub location: Option<String>,
    /// The `foundry_session=…` pair the answer set, if it signed someone in.
    pub session: Option<String>,
    pub body: String,
}

/// The observable universe of every write in this feature (Mandate 8).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Universe {
    /// `(workspace, team slug, email) -> role`.
    pub team_rows: BTreeMap<(String, String, String), TeamRole>,
    /// `(workspace, email) -> role`.
    pub workspace_rows: BTreeMap<(String, String), String>,
    /// Every account's email.
    pub accounts: BTreeSet<String>,
    /// Every team, `(workspace, slug)`.
    pub teams: BTreeSet<(String, String)>,
    /// `workspace -> default team slug`; `None` when unset (or before 0022).
    pub defaults: BTreeMap<String, Option<String>>,
}

/// One declared movement of the universe.
#[derive(Debug, Clone)]
pub enum Change {
    /// The person's row on the team is now `Some(role)`, or gone (`None`).
    OnTeam {
        team: String,
        email: String,
        role: Option<TeamRole>,
    },
    /// The person's workspace row is now `Some(role)`, or gone.
    InWorkspace {
        workspace: String,
        email: String,
        role: Option<WorkspaceRole>,
    },
    /// A new account exists for this email.
    NewAccount { email: String },
    /// The workspace's default team is now this team (or none).
    Default {
        workspace: String,
        team: Option<String>,
    },
}

/// The per-scenario state of this feature (`FoundryWorld::tm`).
#[derive(Debug, Default)]
pub struct TmState {
    pub workspaces: HashMap<String, uuid::Uuid>,
    /// Keyed by team NAME ("Backend").
    pub teams: HashMap<String, TeamRef>,
    /// Keyed by project NAME ("Identity Platform").
    pub projects: HashMap<String, ProjectRef>,
    pub people: Vec<PersonRec>,
    /// email -> `foundry_session=…` pair.
    pub sessions: HashMap<String, String>,
    /// The double-submit token every request of this scenario carries.
    pub csrf: String,
    /// The last page each actor has in front of them (email or "" for signed out).
    pub current: HashMap<String, Answer>,
    /// A page an actor opened earlier and keeps open in another tab.
    pub earlier: HashMap<String, Answer>,
    pub last: Option<Answer>,
    pub last_actor: Option<Actor>,
    pub before: Option<Universe>,
    /// Each race iteration's two answers and the lead count it left.
    pub race: Vec<(Answer, Answer, i64)>,
    pub race_team: Option<String>,
}

// ===========================================================================
// Harness access
// ===========================================================================

pub fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("the Background spawned foundry for this scenario")
}

pub fn pool(world: &FoundryWorld) -> PgPool {
    harness(world).app.state.store.pool().clone()
}

pub fn http(world: &FoundryWorld) -> reqwest::Client {
    world
        .http
        .clone()
        .expect("the Background built an HTTP client")
}

pub fn ts(raw: &str) -> time::OffsetDateTime {
    time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
        .expect("timestamp parses")
}

/// One argon2 hash of [`ROSTER_PASSWORD`] for the whole run (it is salted, and
/// reusing it across rows changes nothing a scenario observes).
async fn roster_hash() -> String {
    static HASH: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();
    HASH.get_or_init(|| async {
        foundry_auth::hash_password(&SecretString::new(ROSTER_PASSWORD.to_string().into()))
            .await
            .expect("hash the roster password")
    })
    .await
    .clone()
}

/// Lower-case kebab of a name, as the product's create path mints a slug.
pub fn slug_of(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ===========================================================================
// Background: foundry, the two workspaces, their teams and people
// ===========================================================================

/// Spawn foundry (real router, real Postgres, Keycloak sign-in pointed at the
/// in-process identity-provider double with provisioning on), then seed Canzan
/// Labs as the instance's ORIGINAL workspace with these teams and boards.
pub async fn spawn_with_home_teams(world: &mut FoundryWorld, teams: &[(String, String, String)]) {
    use crate::steps::feature_keycloak_sso_provisioning::CALLBACK_PATH;
    use crate::support::oidc_issuer::OidcIssuerDouble;

    let double = OidcIssuerDouble::start("foundry").await;
    let config = foundry_oidc::OidcConfig::from_parts(
        Some(double.issuer()),
        Some("foundry".to_string()),
        Some("test-client-secret".to_string()),
        Some(format!("http://127.0.0.1{CALLBACK_PATH}")),
    )
    .expect("complete oidc config")
    .expect("oidc config present")
    .with_provision_role(Some(PROVISION_ROLE.to_string()));
    world.harness = Some(InProcHarness::spawn_with_oidc(ts(TEST_NOW), config).await);
    world.kc_issuer = Some(double);
    world.http = Some(
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .cookie_store(false)
            .build()
            .expect("build reqwest client"),
    );
    world.tm.csrf = format!("tm-{}", uuid::Uuid::new_v4().simple());

    seed_workspace(world, HOME, HOME_CREATED).await;
    for (team, project, key) in teams {
        seed_team(world, HOME, team).await;
        if !project.trim().is_empty() {
            seed_project(world, team, project, key).await;
        }
    }
}

pub async fn seed_workspace(world: &mut FoundryWorld, name: &str, created: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(ts(created))
        .execute(&pool(world))
        .await
        .expect("seed a workspace");
    world.tm.workspaces.insert(name.to_string(), id);
    id
}

async fn seed_team(world: &mut FoundryWorld, workspace: &str, name: &str) -> TeamRef {
    let ws = workspace_id(world, workspace);
    let team = TeamRef {
        id: uuid::Uuid::now_v7(),
        workspace: workspace.to_string(),
        name: name.to_string(),
        slug: slug_of(name),
    };
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team.id)
        .bind(ws)
        .bind(&team.name)
        .bind(&team.slug)
        .execute(&pool(world))
        .await
        .expect("seed a team");
    world.tm.teams.insert(name.to_string(), team.clone());
    team
}

pub async fn seed_project(world: &mut FoundryWorld, team: &str, name: &str, key: &str) {
    let team = team_ref(world, team);
    let ws = workspace_id(world, &team.workspace);
    let id = uuid::Uuid::now_v7();
    let slug = slug_of(name);
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team.id)
    .bind(ws)
    .bind(name)
    .bind(&slug)
    .bind(key.trim())
    .execute(&pool(world))
    .await
    .expect("seed a project");
    crate::support::harness::seed_lanes_for_project(&pool(world), id).await;
    world.tm.projects.insert(
        name.to_string(),
        ProjectRef {
            team_slug: team.slug.clone(),
            slug,
        },
    );
}

/// Seed one person with an account, a workspace role and their team places.
/// `teams` is "Backend as Lead, Ops as Member", or empty for no team.
pub async fn seed_person(
    world: &mut FoundryWorld,
    workspace: &str,
    name: &str,
    email: &str,
    role: WorkspaceRole,
    teams: &str,
    alias: Option<String>,
) {
    let id = uuid::Uuid::now_v7();
    let hash = roster_hash().await;
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(email.to_ascii_lowercase())
    .bind(email)
    .bind(name)
    .bind(&hash)
    .execute(&pool(world))
    .await
    .expect("seed a person");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(workspace_id(world, workspace))
    .bind(id)
    .bind(role.wire())
    .execute(&pool(world))
    .await
    .expect("seed a workspace membership");
    world.tm.people.push(PersonRec {
        name: name.to_string(),
        email: email.to_ascii_lowercase(),
        alias,
        id,
        password: Some(ROSTER_PASSWORD.to_string()),
    });
    for place in teams.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (team, team_role) = place
            .split_once(" as ")
            .unwrap_or_else(|| panic!("a team place reads \"Team as Role\"; got {place:?}"));
        seed_team_row(world, team.trim(), email, TeamRole::parse(team_role.trim())).await;
    }
}

/// A team place as a precondition (never the outcome under test).
pub async fn seed_team_row(world: &FoundryWorld, team: &str, email: &str, role: TeamRole) {
    let team = team_ref(world, team);
    let user = person(world, email).id;
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, $3)
             ON CONFLICT (team_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(team.id)
    .bind(user)
    .bind(role.wire())
    .execute(&pool(world))
    .await
    .expect("seed a team place");
}

/// Globex: another tenant, with one team led by its own admin.
pub async fn seed_foreign_workspace(world: &mut FoundryWorld, team: &str, name: &str, email: &str) {
    seed_workspace(world, FOREIGN, FOREIGN_CREATED).await;
    seed_team(world, FOREIGN, team).await;
    seed_person(
        world,
        FOREIGN,
        name,
        email,
        WorkspaceRole::Admin,
        &format!("{team} as Lead"),
        None,
    )
    .await;
}

/// A team that predates this feature and has no lead (INV-1's exemption).
pub async fn seed_leaderless_team(world: &mut FoundryWorld, team: &str, members: &[String]) {
    seed_team(world, HOME, team).await;
    for who in members {
        let email = resolve_person(world, who).email;
        seed_team_row(world, team, &email, TeamRole::Member).await;
    }
}

/// Put everyone in Canzan Labs on the team (as Member, unless already on it).
pub async fn seed_everyone_on(world: &FoundryWorld, team: &str) {
    let team = team_ref(world, team);
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
         SELECT $1, wm.user_id, 'member' FROM workspace_memberships wm WHERE wm.workspace_id = $2
             ON CONFLICT (team_id, user_id) DO NOTHING",
    )
    .bind(team.id)
    .bind(workspace_id(world, HOME))
    .execute(&pool(world))
    .await
    .expect("seed everyone on the team");
}

/// Make exactly these people the team's leads (promotion only; never a demote,
/// so it is valid under the 0021 lead guard).
pub async fn seed_only_leads(world: &FoundryWorld, team: &str, leads: &[String]) {
    for who in leads {
        let email = resolve_person(world, who).email;
        seed_team_row(world, team, &email, TeamRole::Lead).await;
    }
    let u = universe(world).await;
    let tr = team_ref(world, team);
    let actual: BTreeSet<String> = u
        .team_rows
        .iter()
        .filter(|((ws, slug, _), role)| {
            *ws == tr.workspace && *slug == tr.slug && **role == TeamRole::Lead
        })
        .map(|((_, _, email), _)| email.clone())
        .collect();
    let wanted: BTreeSet<String> = leads
        .iter()
        .map(|w| resolve_person(world, w).email)
        .collect();
    assert_eq!(
        actual, wanted,
        "precondition: {team}'s leads must be exactly {wanted:?}"
    );
}

/// Delete a team by hand, as psql would (the cascade the 0021 trigger exempts and
/// the 0022 foreign key nulls the default for).
pub async fn delete_team_by_hand(world: &FoundryWorld, team: &str) {
    let team = team_ref(world, team);
    sqlx::query("DELETE FROM teams WHERE id = $1")
        .bind(team.id)
        .execute(&pool(world))
        .await
        .expect("delete the team by hand");
}

// ===========================================================================
// Name resolution
// ===========================================================================

pub fn workspace_id(world: &FoundryWorld, name: &str) -> uuid::Uuid {
    *world
        .tm
        .workspaces
        .get(name)
        .unwrap_or_else(|| panic!("workspace {name:?} is seeded by the Background"))
}

pub fn team_ref(world: &FoundryWorld, name: &str) -> TeamRef {
    world
        .tm
        .teams
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("team {name:?} is seeded by the Background"))
}

pub fn project_ref(world: &FoundryWorld, name: &str) -> ProjectRef {
    world
        .tm
        .projects
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("project {name:?} is seeded by the Background"))
}

pub fn person(world: &FoundryWorld, email: &str) -> PersonRec {
    let email = email.to_ascii_lowercase();
    world
        .tm
        .people
        .iter()
        .find(|p| p.email == email)
        .cloned()
        .unwrap_or_else(|| panic!("nobody with the address {email:?} is in the roster"))
}

/// Resolve how a scenario names someone: an alias ("Jeff", "Jeff's Keycloak
/// account"), a full name, a first name, an address, or the option text
/// "Name — address" the Add list shows.
pub fn resolve_person(world: &FoundryWorld, words: &str) -> PersonRec {
    let words = words.trim().trim_matches('"');
    if let Some((_, email)) = words.split_once(" — ") {
        return person(world, email.trim());
    }
    if words.contains('@') {
        return person(world, words);
    }
    let people = &world.tm.people;
    if let Some(p) = people.iter().find(|p| p.alias.as_deref() == Some(words)) {
        return p.clone();
    }
    let by_full: Vec<&PersonRec> = people.iter().filter(|p| p.name == words).collect();
    if by_full.len() == 1 {
        return by_full[0].clone();
    }
    let by_first: Vec<&PersonRec> = people
        .iter()
        .filter(|p| p.name.split(' ').next() == Some(words))
        .collect();
    match by_first.as_slice() {
        [one] => (*one).clone(),
        [] => panic!("nobody called {words:?} is in the roster"),
        _ => panic!("{words:?} names more than one person; the scenario must say which account"),
    }
}

/// An actor phrase: "a signed-out visitor", a pronoun (the last actor), or a person.
pub fn resolve_actor(world: &FoundryWorld, words: &str) -> Actor {
    match words.trim() {
        "a signed-out visitor" => Actor::SignedOut,
        "she" | "he" | "it" => world
            .tm
            .last_actor
            .clone()
            .expect("a pronoun refers to the previous step's actor"),
        other => Actor::Person(resolve_person(world, other).email),
    }
}

/// A target phrase: a person, "herself"/"himself" (the actor), "someone who never
/// existed", or "a garbled name".
pub fn resolve_target(world: &FoundryWorld, actor: &Actor, words: &str) -> Target {
    match words.trim() {
        "someone who never existed" => Target::Nobody,
        "a garbled name" => Target::Garbled,
        "herself" | "himself" => match actor {
            Actor::Person(email) => Target::Person(email.clone()),
            Actor::SignedOut => panic!("a signed-out visitor has no self"),
        },
        other => Target::Person(resolve_person(world, other).email),
    }
}

pub fn resolve_team(words: &str) -> TeamName {
    match words.trim() {
        "a team that does not exist" => TeamName::Unknown,
        other => TeamName::Known(other.to_string()),
    }
}

pub fn team_slug(world: &FoundryWorld, team: &TeamName) -> String {
    match team {
        TeamName::Known(name) => team_ref(world, name).slug,
        TeamName::Unknown => "no-such-team".to_string(),
    }
}

fn target_value(world: &FoundryWorld, target: &Target) -> String {
    match target {
        Target::Person(email) => person(world, email).id.to_string(),
        Target::Nobody => uuid::Uuid::now_v7().to_string(),
        Target::Garbled => "not-a-person".to_string(),
    }
}

pub fn actor_key(actor: &Actor) -> String {
    match actor {
        Actor::SignedOut => String::new(),
        Actor::Person(email) => email.clone(),
    }
}

// ===========================================================================
// Sessions and requests
// ===========================================================================

/// Sign the actor in (once per scenario) and return the Cookie header every
/// request of theirs carries: their session plus the scenario's CSRF cookie.
pub async fn cookie_for(world: &mut FoundryWorld, actor: &Actor) -> String {
    let csrf = format!("foundry_csrf={}", world.tm.csrf);
    let Actor::Person(email) = actor else {
        return csrf;
    };
    if !world.tm.sessions.contains_key(email) {
        let who = person(world, email);
        let password = who.password.clone().unwrap_or_else(|| {
            panic!("{email:?} has no password; it signs in through Keycloak first")
        });
        let session = crate::support::harness::establish_session(
            harness(world),
            &http(world),
            &who.email,
            &password,
        )
        .await;
        world.tm.sessions.insert(email.clone(), session);
    }
    format!("{}; {csrf}", world.tm.sessions[email])
}

/// Remember a session established elsewhere (an invite accept, a Keycloak sign-in).
pub fn adopt_session(world: &mut FoundryWorld, email: &str, session_pair: String) {
    world
        .tm
        .sessions
        .insert(email.to_ascii_lowercase(), session_pair);
}

pub async fn into_answer(path: &str, resp: reqwest::Response) -> Answer {
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let location = resp
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let session = session_set_by(resp.headers());
    Answer {
        path: path.to_string(),
        status,
        content_type,
        location,
        session,
        body: resp.text().await.unwrap_or_default(),
    }
}

/// GET `path` as the actor; it becomes the page in front of them.
pub async fn open(world: &mut FoundryWorld, actor: &Actor, path: &str) -> Answer {
    let cookie = cookie_for(world, actor).await;
    let base = harness(world).base_url();
    let resp = http(world)
        .get(format!("{base}{path}"))
        .header(COOKIE, cookie)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {path}: {e}"));
    let answer = into_answer(path, resp).await;
    world.tm.current.insert(actor_key(actor), answer.clone());
    answer
}

/// POST `fields` to `path` as the actor, with the double-submit token unless
/// `with_token` is false.
pub async fn post(
    world: &mut FoundryWorld,
    actor: &Actor,
    path: &str,
    fields: &[(String, String)],
    with_token: bool,
) -> Answer {
    let mut cookie = cookie_for(world, actor).await;
    let mut form: Vec<(String, String)> = fields
        .iter()
        .filter(|(k, _)| k != "_csrf")
        .cloned()
        .collect();
    if with_token {
        form.push(("_csrf".to_string(), world.tm.csrf.clone()));
    } else {
        cookie = cookie
            .split("; ")
            .filter(|c| !c.starts_with("foundry_csrf="))
            .collect::<Vec<_>>()
            .join("; ");
    }
    let base = harness(world).base_url();
    let resp = http(world)
        .post(format!("{base}{path}"))
        .header(COOKIE, cookie)
        .form(&form)
        .send()
        .await
        .unwrap_or_else(|e| panic!("POST {path}: {e}"));
    let answer = into_answer(path, resp).await;
    world.tm.current.insert(actor_key(actor), answer.clone());
    answer
}

/// Keep an answer as the scenario's latest evidence.
pub fn record(world: &mut FoundryWorld, actor: &Actor, answer: Answer) {
    world.last_status = Some(answer.status);
    world.last_body = Some(answer.body.clone());
    world.tm.last = Some(answer);
    world.tm.last_actor = Some(actor.clone());
}

pub fn last(world: &FoundryWorld) -> &Answer {
    world
        .tm
        .last
        .as_ref()
        .expect("a When step produced an answer")
}

/// The canonical never-existed answer: a signed-out GET of a path no route serves.
pub async fn never_existed(world: &mut FoundryWorld) -> Answer {
    let path = format!("/never-existed-{}", uuid::Uuid::now_v7());
    open(world, &Actor::SignedOut, &path).await
}

// ===========================================================================
// Reading pages
// ===========================================================================

/// The page's visible text, entity-decoded, whitespace collapsed.
pub fn page_text(body: &str) -> String {
    let doc = scraper::Html::parse_document(body);
    let raw: String = doc.root_element().text().collect::<Vec<_>>().join(" ");
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A form on the page: its action and its fields (hidden inputs, the first
/// option of each select, named buttons).
#[derive(Debug, Clone)]
pub struct PageForm {
    pub action: String,
    pub fields: Vec<(String, String)>,
    /// `select[name] -> [(option text, option value)]`.
    pub options: HashMap<String, Vec<(String, String)>>,
}

pub fn forms(body: &str) -> Vec<PageForm> {
    let doc = parse(body);
    let mut out = Vec::new();
    for form in select_all(&doc, "form") {
        let action = form.value().attr("action").unwrap_or("").to_string();
        let mut fields = Vec::new();
        let mut options = HashMap::new();
        let inputs = scraper::Selector::parse("input[name]").expect("selector");
        for input in form.select(&inputs) {
            let kind = input.value().attr("type").unwrap_or("text");
            if matches!(kind, "hidden" | "text") {
                fields.push((
                    input.value().attr("name").unwrap_or("").to_string(),
                    input.value().attr("value").unwrap_or("").to_string(),
                ));
            }
        }
        let selects = scraper::Selector::parse("select[name]").expect("selector");
        let option_sel = scraper::Selector::parse("option").expect("selector");
        for select in form.select(&selects) {
            let name = select.value().attr("name").unwrap_or("").to_string();
            let opts: Vec<(String, String)> = select
                .select(&option_sel)
                .map(|o| {
                    let text = text_of(&o).split_whitespace().collect::<Vec<_>>().join(" ");
                    let value = o
                        .value()
                        .attr("value")
                        .map(str::to_string)
                        .unwrap_or_else(|| text.clone());
                    (text, value)
                })
                .collect();
            options.insert(name, opts);
        }
        out.push(PageForm {
            action,
            fields,
            options,
        });
    }
    out
}

pub fn form_at(answer: &Answer, action: &str) -> PageForm {
    forms(&answer.body)
        .into_iter()
        .find(|f| f.action == action)
        .unwrap_or_else(|| {
            panic!(
                "the page at {} (status {}) offers no form that posts to {action}; page text: {}",
                answer.path,
                answer.status,
                page_text(&answer.body)
            )
        })
}

/// The Add list (`select[name=user_id]` options with a value), as "Name — email".
pub fn offered(answer: &Answer, team_slug: &str) -> Vec<String> {
    form_at(answer, &format!("/team/{team_slug}/members"))
        .options
        .get("user_id")
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(text, _)| text)
        .collect()
}

/// Which management controls the page renders (D11): Add, role change, Remove.
pub fn controls(answer: &Answer) -> (bool, bool, bool) {
    let actions: Vec<String> = forms(&answer.body).into_iter().map(|f| f.action).collect();
    let add = actions
        .iter()
        .any(|a| a.starts_with("/team/") && a.ends_with("/members"));
    let role = actions
        .iter()
        .any(|a| a.starts_with("/team/") && a.ends_with("/role"));
    let remove = actions
        .iter()
        .any(|a| a.starts_with("/team/") && a.ends_with("/remove"));
    (add, role, remove)
}

/// The href of the first link whose text is exactly `text`.
pub fn link_to(answer: &Answer, text: &str) -> String {
    let doc = parse(&answer.body);
    select_all(&doc, "a[href]")
        .into_iter()
        .find(|a| text_of(a).split_whitespace().collect::<Vec<_>>().join(" ") == text)
        .and_then(|a| a.value().attr("href").map(str::to_string))
        .unwrap_or_else(|| {
            panic!(
                "the page at {} (status {}) has no link reading {text:?}; page text: {}",
                answer.path,
                answer.status,
                page_text(&answer.body)
            )
        })
}

/// The href of the first link whose text starts with `text` (a team row on /teams).
pub fn link_starting(answer: &Answer, text: &str) -> String {
    let doc = parse(&answer.body);
    select_all(&doc, "a[href]")
        .into_iter()
        .find(|a| {
            text_of(a)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .starts_with(text)
        })
        .and_then(|a| a.value().attr("href").map(str::to_string))
        .unwrap_or_else(|| {
            panic!(
                "the page at {} (status {}) has no link starting {text:?}; page text: {}",
                answer.path,
                answer.status,
                page_text(&answer.body)
            )
        })
}

/// How many member rows the page renders ("… · Lead" / "… · Member", DDD-17).
pub fn row_count(answer: &Answer) -> usize {
    let text = page_text(&answer.body);
    text.matches(" · Lead").count() + text.matches(" · Member").count()
}

/// Where each expected row first appears in the page's text (`None` if absent).
pub fn row_positions(answer: &Answer, rows: &[String]) -> Vec<Option<usize>> {
    let text = page_text(&answer.body);
    rows.iter().map(|r| text.find(r.as_str())).collect()
}

// ===========================================================================
// The universe
// ===========================================================================

pub async fn universe(world: &FoundryWorld) -> Universe {
    let pool = pool(world);
    let team_rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT w.name, t.slug, u.email_lower, tm.role
           FROM team_memberships tm
           JOIN teams t ON t.id = tm.team_id
           JOIN workspaces w ON w.id = t.workspace_id
           JOIN users u ON u.id = tm.user_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read team rows");
    let workspace_rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT w.name, u.email_lower, wm.role
           FROM workspace_memberships wm
           JOIN workspaces w ON w.id = wm.workspace_id
           JOIN users u ON u.id = wm.user_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read workspace rows");
    let accounts: Vec<(String,)> = sqlx::query_as("SELECT email_lower FROM users")
        .fetch_all(&pool)
        .await
        .expect("read accounts");
    let teams: Vec<(String, String)> = sqlx::query_as(
        "SELECT w.name, t.slug FROM teams t JOIN workspaces w ON w.id = t.workspace_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read teams");
    let (has_default,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                         WHERE table_schema = current_schema()
                           AND table_name = 'workspaces' AND column_name = 'default_team_id')",
    )
    .fetch_one(&pool)
    .await
    .expect("look the default-team setting up");
    let defaults: Vec<(String, Option<String>)> = if has_default {
        sqlx::query_as(
            "SELECT w.name, t.slug FROM workspaces w LEFT JOIN teams t ON t.id = w.default_team_id",
        )
        .fetch_all(&pool)
        .await
        .expect("read default teams")
    } else {
        sqlx::query_as("SELECT name, NULL::text FROM workspaces")
            .fetch_all(&pool)
            .await
            .expect("read workspaces")
    };
    Universe {
        team_rows: team_rows
            .into_iter()
            .map(|(w, t, e, r)| ((w, t, e), TeamRole::parse(&r)))
            .collect(),
        workspace_rows: workspace_rows
            .into_iter()
            .map(|(w, e, r)| ((w, e), r))
            .collect(),
        accounts: accounts.into_iter().map(|(e,)| e).collect(),
        teams: teams.into_iter().collect(),
        defaults: defaults.into_iter().collect(),
    }
}

/// Capture the before-image a `When` is judged against.
pub async fn capture_before(world: &mut FoundryWorld) {
    let u = universe(world).await;
    world.tm.before = Some(u);
}

/// `assert_state_delta(before, after, universe, expected)`: the after-image must
/// equal the before-image with exactly `changes` applied. Fail-closed.
pub async fn assert_state_delta(world: &FoundryWorld, changes: &[Change]) {
    let before = world
        .tm
        .before
        .clone()
        .expect("the When captured the universe before acting");
    let after = universe(world).await;
    let mut expected = before.clone();
    for change in changes {
        match change {
            Change::OnTeam { team, email, role } => {
                let tr = team_ref(world, team);
                let key = (tr.workspace.clone(), tr.slug.clone(), email.clone());
                match role {
                    Some(r) => {
                        expected.team_rows.insert(key, *r);
                    }
                    None => {
                        expected.team_rows.remove(&key);
                    }
                }
            }
            Change::InWorkspace {
                workspace,
                email,
                role,
            } => {
                let key = (workspace.clone(), email.clone());
                match role {
                    Some(r) => {
                        expected.workspace_rows.insert(key, r.wire().to_string());
                    }
                    None => {
                        expected.workspace_rows.remove(&key);
                    }
                }
            }
            Change::NewAccount { email } => {
                expected.accounts.insert(email.clone());
            }
            Change::Default { workspace, team } => {
                let slug = team.as_ref().map(|t| team_ref(world, t).slug);
                expected.defaults.insert(workspace.clone(), slug);
            }
        }
    }
    assert_eq!(
        after, expected,
        "state delta: the universe must equal the before-image plus exactly {changes:?}"
    );
}

// ===========================================================================
// The feature's doors
// ===========================================================================

pub fn members_path(slug: &str) -> String {
    format!("/team/{slug}/members")
}

/// The page the actor writes from: the one they already have in front of them
/// if it is this team's members page, else a fresh one.
async fn members_page(world: &mut FoundryWorld, actor: &Actor, slug: &str) -> Answer {
    let path = members_path(slug);
    match world.tm.current.get(&actor_key(actor)) {
        Some(page) if page.path == path && page.status == StatusCode::OK => page.clone(),
        _ => open(world, actor, &path).await,
    }
}

/// Which page a write is made from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FromPage {
    /// The page in front of them now (opened fresh if needed).
    Current,
    /// The page they opened earlier and kept open.
    Earlier,
}

async fn source_page(
    world: &mut FoundryWorld,
    actor: &Actor,
    slug: &str,
    from: FromPage,
) -> Answer {
    match from {
        FromPage::Current => members_page(world, actor, slug).await,
        FromPage::Earlier => world
            .tm
            .earlier
            .get(&actor_key(actor))
            .cloned()
            .expect("a Given opened that page earlier"),
    }
}

/// Keep the actor's members page open "in another tab" for later.
pub async fn open_earlier(world: &mut FoundryWorld, actor: &Actor, team: &str) -> Answer {
    let slug = team_ref(world, team).slug;
    let page = open(world, actor, &members_path(&slug)).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "precondition: {actor:?} opens {team}'s members page; page text: {}",
        page_text(&page.body)
    );
    world.tm.earlier.insert(actor_key(actor), page.clone());
    page
}

/// Add from the page: the Add form, with the option naming the person.
pub async fn add_from_page(
    world: &mut FoundryWorld,
    actor: &Actor,
    who: &str,
    team: &str,
    from: FromPage,
) -> Answer {
    let slug = team_ref(world, team).slug;
    let page = source_page(world, actor, &slug, from).await;
    let form = form_at(&page, &members_path(&slug));
    let email = person(world, who).email;
    let (_, value) = form
        .options
        .get("user_id")
        .and_then(|opts| {
            opts.iter()
                .find(|(text, _)| text.ends_with(&email))
                .cloned()
        })
        .unwrap_or_else(|| {
            panic!(
                "{team}'s Add a member does not offer {email:?}; offered: {:?}",
                form.options.get("user_id")
            )
        });
    let mut fields: Vec<(String, String)> = form
        .fields
        .into_iter()
        .filter(|(k, _)| k != "user_id")
        .collect();
    fields.push(("user_id".to_string(), value));
    post(world, actor, &form.action, &fields, true).await
}

/// Change a role from the page: the row's role form, set to `role`.
pub async fn role_from_page(
    world: &mut FoundryWorld,
    actor: &Actor,
    who: &str,
    role: TeamRole,
    team: &str,
    from: FromPage,
) -> Answer {
    let slug = team_ref(world, team).slug;
    let page = source_page(world, actor, &slug, from).await;
    let id = person(world, who).id;
    let form = form_at(&page, &format!("/team/{slug}/members/{id}/role"));
    let mut fields: Vec<(String, String)> = form
        .fields
        .into_iter()
        .filter(|(k, _)| k != "role")
        .collect();
    fields.push(("role".to_string(), role.wire().to_string()));
    post(world, actor, &form.action, &fields, true).await
}

/// Remove from the page: the row's Remove form.
pub async fn remove_from_page(
    world: &mut FoundryWorld,
    actor: &Actor,
    who: &str,
    team: &str,
    from: FromPage,
) -> Answer {
    let slug = team_ref(world, team).slug;
    let page = source_page(world, actor, &slug, from).await;
    let id = person(world, who).id;
    let form = form_at(&page, &format!("/team/{slug}/members/{id}/remove"));
    post(world, actor, &form.action, &form.fields, true).await
}

/// The authz matrix's hand-built requests (no form is offered to these callers).
#[derive(Debug, Clone)]
pub enum Forged {
    Add(Target),
    /// The role as posted: "lead", "member", or a word that is neither.
    Role(Target, String),
    Remove(Target),
}

pub async fn forge(
    world: &mut FoundryWorld,
    actor: &Actor,
    team: &TeamName,
    request: &Forged,
    with_token: bool,
) -> Answer {
    let slug = team_slug(world, team);
    let (path, fields) = match request {
        Forged::Add(target) => (
            members_path(&slug),
            vec![("user_id".to_string(), target_value(world, target))],
        ),
        Forged::Role(target, role) => (
            format!("/team/{slug}/members/{}/role", target_value(world, target)),
            vec![("role".to_string(), role.to_ascii_lowercase())],
        ),
        Forged::Remove(target) => (
            format!(
                "/team/{slug}/members/{}/remove",
                target_value(world, target)
            ),
            vec![],
        ),
    };
    post(world, actor, &path, &fields, with_token).await
}

/// GET a team's members page by name or as an unknown slug.
pub async fn ask_members_page(world: &mut FoundryWorld, actor: &Actor, team: &TeamName) -> Answer {
    let slug = team_slug(world, team);
    open(world, actor, &members_path(&slug)).await
}

/// Set the default team from `/teams` (the select's option reading `choice`).
pub async fn set_default_from_page(
    world: &mut FoundryWorld,
    actor: &Actor,
    choice: &str,
) -> Answer {
    let page = open(world, actor, "/teams").await;
    let form = form_at(&page, "/teams/default");
    let (_, value) = form
        .options
        .get("team")
        .and_then(|opts| opts.iter().find(|(text, _)| text == choice).cloned())
        .unwrap_or_else(|| {
            panic!(
                "the default-team select does not offer {choice:?}; offered: {:?}",
                form.options.get("team")
            )
        });
    let mut fields: Vec<(String, String)> = form
        .fields
        .into_iter()
        .filter(|(k, _)| k != "team")
        .collect();
    fields.push(("team".to_string(), value));
    post(world, actor, "/teams/default", &fields, true).await
}

/// The hand-built default-team POST (DDD-14: a team slug, or empty).
pub async fn forge_default(world: &mut FoundryWorld, actor: &Actor, team: &TeamName) -> Answer {
    let slug = team_slug(world, team);
    post(
        world,
        actor,
        "/teams/default",
        &[("team".to_string(), slug)],
        true,
    )
    .await
}

/// The default the teams page shows as selected (as Priya, a workspace admin).
pub fn selected_default(answer: &Answer) -> Option<String> {
    let doc = parse(&answer.body);
    let css = "form[action=\"/teams/default\"] select[name=\"team\"] option";
    let options = select_all(&doc, css);
    // A select with no `selected` option shows its first one, as a browser does.
    options
        .iter()
        .find(|o| o.value().attr("selected").is_some())
        .or_else(|| options.first())
        .map(|o| text_of(o).split_whitespace().collect::<Vec<_>>().join(" "))
}

/// The shipped workspace-removal door (scripted; no UI form).
pub async fn remove_from_workspace(world: &mut FoundryWorld, actor: &Actor, who: &str) -> Answer {
    let email = person(world, who).email;
    post(
        world,
        actor,
        "/workspace/members/remove",
        &[("email".to_string(), email)],
        true,
    )
    .await
}

/// A workspace admin invites `email`; the invitee accepts through the emailed
/// link and sets a password. The newcomer joins the roster under `name`.
pub async fn invite_and_accept(
    world: &mut FoundryWorld,
    inviter: &Actor,
    name: &str,
    email: &str,
) -> Answer {
    let sent = post(
        world,
        inviter,
        "/workspace/invites",
        &[("email".to_string(), email.to_string())],
        true,
    )
    .await;
    assert_eq!(
        sent.status,
        StatusCode::OK,
        "the shipped invite form must issue the invite; page text: {}",
        page_text(&sent.body)
    );
    let link_at = sent
        .body
        .find("/invites/accept?id=")
        .expect("the invite-sent page carries the accept link");
    let link: String = sent.body[link_at..]
        .chars()
        .take_while(|c| !matches!(c, '"' | '<' | ' ' | '\''))
        .collect::<String>()
        .replace("&amp;", "&");
    let url = reqwest::Url::parse(&format!("http://x{link}")).expect("accept link parses");
    let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
    let (id, sig) = (query["id"].clone(), query["sig"].clone());

    let form_page = open(world, &Actor::SignedOut, &link).await;
    assert_eq!(
        form_page.status,
        StatusCode::OK,
        "the accept page renders the set-password form"
    );
    let accepted = post(
        world,
        &Actor::SignedOut,
        "/invites/accept",
        &[
            ("id".to_string(), id),
            ("sig".to_string(), sig),
            ("password".to_string(), NEWCOMER_PASSWORD.to_string()),
            ("confirm".to_string(), NEWCOMER_PASSWORD.to_string()),
        ],
        true,
    )
    .await;
    world.tm.people.push(PersonRec {
        name: name.to_string(),
        email: email.to_ascii_lowercase(),
        alias: None,
        id: account_id(world, email)
            .await
            .unwrap_or_else(uuid::Uuid::nil),
        password: Some(NEWCOMER_PASSWORD.to_string()),
    });
    accepted
}

/// The id of the account holding `email`, if any.
pub async fn account_id(world: &FoundryWorld, email: &str) -> Option<uuid::Uuid> {
    sqlx::query_as::<_, (uuid::Uuid,)>("SELECT id FROM users WHERE email_lower = $1")
        .bind(email.to_ascii_lowercase())
        .fetch_optional(&pool(world))
        .await
        .expect("look an account up")
        .map(|(id,)| id)
}

/// Remember an account the product created (a Keycloak first sign-in).
pub async fn adopt_person(
    world: &mut FoundryWorld,
    name: &str,
    email: &str,
    alias: Option<String>,
) {
    let id = account_id(world, email)
        .await
        .unwrap_or_else(|| panic!("no account was created for {email:?}"));
    world
        .tm
        .people
        .retain(|p| p.email != email.to_ascii_lowercase());
    world.tm.people.push(PersonRec {
        name: name.to_string(),
        email: email.to_ascii_lowercase(),
        alias,
        id,
        password: None,
    });
}

/// The board of `project` as the actor sees it.
pub async fn open_board(world: &mut FoundryWorld, actor: &Actor, project: &str) -> Answer {
    let p = project_ref(world, project);
    open(
        world,
        actor,
        &format!("/team/{}/project/{}", p.team_slug, p.slug),
    )
    .await
}

/// The "Your projects" section of the actor's dashboard.
pub async fn dashboard_projects(world: &mut FoundryWorld, actor: &Actor) -> String {
    let dash = open(world, actor, "/").await;
    assert_eq!(dash.status, StatusCode::OK, "the dashboard renders");
    let start = dash
        .body
        .find("<h2>Your projects</h2>")
        .expect("the dashboard has a Your projects section");
    let rest = &dash.body[start..];
    let end = rest[1..].find("<h2>").map_or(rest.len(), |i| i + 1);
    rest[..end].to_string()
}

/// How many leads the team has now.
pub async fn lead_count(world: &FoundryWorld, team: &str) -> i64 {
    let tr = team_ref(world, team);
    let (n,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM team_memberships WHERE team_id = $1 AND role = 'lead'",
    )
    .bind(tr.id)
    .fetch_one(&pool(world))
    .await
    .expect("count leads");
    n
}

/// Which race the scenario runs (OQ-D3: a few HTTP repeats; the 100-run proof is
/// the store's).
#[derive(Debug, Clone, Copy)]
pub enum Race {
    DemoteBoth,
    RemoveFirstDemoteSecond,
}

/// Run the race `times` times. Before each run both people are the team's only
/// leads again (promotion and re-adding only, so the reset is valid under 0021).
pub async fn race(
    world: &mut FoundryWorld,
    actor: &Actor,
    kind: Race,
    first: &str,
    second: &str,
    team: &str,
    times: usize,
) {
    let tr = team_ref(world, team);
    let a = person(world, first);
    let b = person(world, second);
    let cookie = cookie_for(world, actor).await;
    let base = harness(world).base_url();
    let client = http(world);
    let csrf = world.tm.csrf.clone();
    world.tm.race.clear();
    world.tm.race_team = Some(team.to_string());
    for _ in 0..times {
        for p in [&a, &b] {
            seed_team_row(world, team, &p.email, TeamRole::Lead).await;
        }
        let first_path = match kind {
            Race::DemoteBoth => format!("/team/{}/members/{}/role", tr.slug, a.id),
            Race::RemoveFirstDemoteSecond => format!("/team/{}/members/{}/remove", tr.slug, a.id),
        };
        let second_path = format!("/team/{}/members/{}/role", tr.slug, b.id);
        let send = |path: String, role: Option<&'static str>| {
            let client = client.clone();
            let cookie = cookie.clone();
            let csrf = csrf.clone();
            let base = base.clone();
            async move {
                let mut form = vec![("_csrf".to_string(), csrf)];
                if let Some(r) = role {
                    form.push(("role".to_string(), r.to_string()));
                }
                let resp = client
                    .post(format!("{base}{path}"))
                    .header(COOKIE, cookie)
                    .form(&form)
                    .send()
                    .await
                    .expect("race POST");
                into_answer(&path, resp).await
            }
        };
        let first_role = match kind {
            Race::DemoteBoth => Some("member"),
            Race::RemoveFirstDemoteSecond => None,
        };
        let (one, two) = tokio::join!(
            send(first_path, first_role),
            send(second_path, Some("member"))
        );
        let leads = lead_count(world, team).await;
        world.tm.race.push((one, two, leads));
    }
}

/// The copy a page or plain-text answer carries, decoded.
pub fn says(answer: &Answer, copy: &str) -> bool {
    let is_html = answer
        .content_type
        .as_deref()
        .is_some_and(|c| c.starts_with("text/html"));
    if is_html {
        page_text(&answer.body).contains(copy)
    } else {
        answer.body.contains(copy)
    }
}

/// The page's identity for diagnostics: path, status and visible text.
pub fn describe(answer: &Answer) -> String {
    format!(
        "{} answered {} ({:?}): {}",
        answer.path,
        answer.status,
        answer.content_type,
        page_text(&answer.body)
    )
}

/// The session cookie a response set, if any.
pub fn session_set_by(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|c| c.starts_with("foundry_session="))
        .and_then(|c| c.split(';').next())
        .map(str::to_string)
}

// ===========================================================================
// Step services: one per Gherkin sentence, so every step body is one call
// ===========================================================================

/// Parse a Gherkin table into its body rows (the first row is the header).
pub fn table_rows(step: &cucumber::gherkin::Step) -> Vec<Vec<String>> {
    step.table
        .as_ref()
        .expect("this step carries a table")
        .rows
        .iter()
        .skip(1)
        .map(|r| r.iter().map(|c| c.trim().to_string()).collect())
        .collect()
}

/// A one-column table whose every row is data (no header).
pub fn table_column(step: &cucumber::gherkin::Step) -> Vec<String> {
    step.table
        .as_ref()
        .expect("this step carries a table")
        .rows
        .iter()
        .map(|r| r[0].trim().to_string())
        .collect()
}

pub async fn given_home_teams(world: &mut FoundryWorld, step: &cucumber::gherkin::Step) {
    let teams: Vec<(String, String, String)> = table_rows(step)
        .into_iter()
        .map(|r| (r[0].clone(), r[1].clone(), r[2].clone()))
        .collect();
    spawn_with_home_teams(world, &teams).await;
}

pub async fn given_roster(world: &mut FoundryWorld, step: &cucumber::gherkin::Step) {
    for r in table_rows(step) {
        let alias = r.get(4).filter(|a| !a.is_empty()).cloned();
        seed_person(
            world,
            HOME,
            &r[0],
            &r[1],
            WorkspaceRole::parse(&r[2]),
            &r[3],
            alias,
        )
        .await;
    }
}

pub async fn when_follows_teams(world: &mut FoundryWorld, who: &str) {
    let actor = resolve_actor(world, who);
    capture_before(world).await;
    let dash = open(world, &actor, "/").await;
    let href = link_to(&dash, "Teams");
    let page = open(world, &actor, &href).await;
    record(world, &actor, page);
}

pub async fn when_opens_teams_page(world: &mut FoundryWorld, who: &str) {
    let actor = resolve_actor(world, who);
    capture_before(world).await;
    let page = open(world, &actor, "/teams").await;
    record(world, &actor, page);
}

pub async fn when_opens_team_from_teams_page(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = resolve_actor(world, who);
    let teams_page = world
        .tm
        .current
        .get(&actor_key(&actor))
        .cloned()
        .expect("the teams page is in front of them");
    let href = link_starting(&teams_page, team);
    let page = open(world, &actor, &href).await;
    record(world, &actor, page);
}

pub async fn when_opens_members_page(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = resolve_actor(world, who);
    capture_before(world).await;
    let page = ask_members_page(world, &actor, &resolve_team(team)).await;
    record(world, &actor, page);
}

pub async fn given_opened_earlier(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = resolve_actor(world, who);
    open_earlier(world, &actor, team).await;
}

fn from_page(earlier: &Option<String>) -> FromPage {
    if earlier.as_deref().is_some_and(|s| !s.is_empty()) {
        FromPage::Earlier
    } else {
        FromPage::Current
    }
}

pub async fn when_adds(
    world: &mut FoundryWorld,
    who: &str,
    target: &str,
    team: &str,
    earlier: Option<String>,
) {
    let actor = resolve_actor(world, who);
    let email = resolve_person(world, target).email;
    capture_before(world).await;
    let answer = add_from_page(world, &actor, &email, team, from_page(&earlier)).await;
    record(world, &actor, answer);
}

pub async fn when_makes(
    world: &mut FoundryWorld,
    who: &str,
    target: &str,
    role: &str,
    team: &str,
    earlier: Option<String>,
) {
    let actor = resolve_actor(world, who);
    let Target::Person(email) = resolve_target(world, &actor, target) else {
        panic!("a role change from the page names someone on it");
    };
    capture_before(world).await;
    let answer = role_from_page(
        world,
        &actor,
        &email,
        TeamRole::parse(role),
        team,
        from_page(&earlier),
    )
    .await;
    record(world, &actor, answer);
}

pub async fn when_removes(
    world: &mut FoundryWorld,
    who: &str,
    target: &str,
    team: &str,
    earlier: Option<String>,
) {
    let actor = resolve_actor(world, who);
    let Target::Person(email) = resolve_target(world, &actor, target) else {
        panic!("a removal from the page names someone on it");
    };
    capture_before(world).await;
    let answer = remove_from_page(world, &actor, &email, team, from_page(&earlier)).await;
    record(world, &actor, answer);
}

/// A Given that reuses a When and requires it to have gone through (Pillar 2).
pub fn require_ok(world: &FoundryWorld, what: &str) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "precondition: {what} must go through; {}",
        describe(answer)
    );
}

pub async fn given_has_added(world: &mut FoundryWorld, who: &str, target: &str, team: &str) {
    when_adds(world, who, target, team, None).await;
    require_ok(world, &format!("{who} adds {target} to {team}"));
}

pub async fn given_has_made(
    world: &mut FoundryWorld,
    who: &str,
    target: &str,
    role: &str,
    team: &str,
) {
    when_makes(world, who, target, role, team, None).await;
    require_ok(world, &format!("{who} makes {target} a {role} of {team}"));
}

pub async fn given_has_removed(world: &mut FoundryWorld, who: &str, target: &str, team: &str) {
    when_removes(world, who, target, team, None).await;
    require_ok(world, &format!("{who} removes {target} from {team}"));
}

pub enum Submission {
    Add,
    Role(String),
    Remove,
}

pub async fn when_submits(
    world: &mut FoundryWorld,
    who: &str,
    kind: Submission,
    target: &str,
    team: &str,
    with_token: bool,
) {
    let actor = resolve_actor(world, who);
    let target = resolve_target(world, &actor, target);
    let request = match kind {
        Submission::Add => Forged::Add(target),
        Submission::Role(role) => Forged::Role(target, role),
        Submission::Remove => Forged::Remove(target),
    };
    capture_before(world).await;
    let answer = forge(world, &actor, &resolve_team(team), &request, with_token).await;
    record(world, &actor, answer);
}

pub async fn when_asks_members_page(world: &mut FoundryWorld, who: &str, team: &str) {
    when_opens_members_page(world, who, team).await;
}

pub async fn given_left_workspace(world: &mut FoundryWorld, who: &str) {
    let admin = Actor::Person(resolve_person(world, "Priya").email);
    let email = resolve_person(world, who).email;
    let answer = remove_from_workspace(world, &admin, &email).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "precondition: {who} leaves Canzan Labs; {}",
        describe(&answer)
    );
}

pub async fn when_removes_from_workspace(world: &mut FoundryWorld, who: &str, target: &str) {
    let actor = resolve_actor(world, who);
    let email = resolve_person(world, target).email;
    capture_before(world).await;
    let answer = remove_from_workspace(world, &actor, &email).await;
    record(world, &actor, answer);
}

pub async fn given_everyone_on(world: &mut FoundryWorld, team: &str) {
    seed_everyone_on(world, team).await;
}

pub async fn given_only_leads(world: &mut FoundryWorld, leads: &[String], team: &str) {
    seed_only_leads(world, team, leads).await;
    world.tm.race_team = Some(team.to_string());
}

pub async fn given_leaderless_team(world: &mut FoundryWorld, team: &str, members: &[String]) {
    seed_leaderless_team(world, team, members).await;
}

pub async fn when_race(
    world: &mut FoundryWorld,
    who: &str,
    kind: Race,
    first: &str,
    second: &str,
    times: usize,
) {
    let team = world
        .tm
        .race_team
        .clone()
        .expect("a Given named the team whose only leads race");
    let team = team.as_str();
    let actor = resolve_actor(world, who);
    let a = resolve_person(world, first).email;
    let b = resolve_person(world, second).email;
    race(world, &actor, kind, &a, &b, team, times).await;
}

// ----------------------------------------------------------------- Thens

pub fn then_sees(world: &FoundryWorld, copy: &str) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "expected a 200 page saying {copy:?}; {}",
        describe(answer)
    );
    assert!(
        says(answer, copy),
        "expected the page to say {copy:?}; {}",
        describe(answer)
    );
}

pub fn then_is_told(world: &FoundryWorld, copy: &str) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::CONFLICT,
        "a stated refusal is a 409 (team-membership DDD-8, team-lifecycle DDD-18); {}",
        describe(answer)
    );
    assert!(
        says(answer, copy),
        "expected the refusal to say {copy:?}; {}",
        describe(answer)
    );
}

pub async fn then_never_existed(world: &mut FoundryWorld) {
    let canon = never_existed(world).await;
    let answer = last(world).clone();
    assert_eq!(
        canon.status,
        StatusCode::NOT_FOUND,
        "the never-existed answer is a 404"
    );
    assert_eq!(
        answer.status,
        canon.status,
        "a refusal must be the uniform 404 (D5); {}",
        describe(&answer)
    );
    assert_eq!(
        answer.body,
        canon.body,
        "a refusal must be BYTE-IDENTICAL to a page that never existed (D5: no oracle); got {}",
        describe(&answer)
    );
}

pub async fn then_nothing_changed(world: &FoundryWorld) {
    assert_state_delta(world, &[]).await;
}

pub async fn then_now_role(world: &FoundryWorld, who: &str, role: &str, team: &str) {
    let email = resolve_person(world, who).email;
    assert_state_delta(
        world,
        &[Change::OnTeam {
            team: team.to_string(),
            email,
            role: Some(TeamRole::parse(role)),
        }],
    )
    .await;
}

pub async fn then_no_longer_on(world: &FoundryWorld, who: &str, team: &str) {
    let email = resolve_person(world, who).email;
    assert_state_delta(
        world,
        &[Change::OnTeam {
            team: team.to_string(),
            email,
            role: None,
        }],
    )
    .await;
}

pub async fn then_left_workspace_and_teams(world: &FoundryWorld, who: &str) {
    let email = resolve_person(world, who).email;
    let before = world
        .tm
        .before
        .clone()
        .expect("the When captured the universe");
    let mut changes = vec![Change::InWorkspace {
        workspace: HOME.to_string(),
        email: email.clone(),
        role: None,
    }];
    for (ws, slug, e) in before.team_rows.keys() {
        if *ws == HOME && *e == email {
            let team = world
                .tm
                .teams
                .values()
                .find(|t| t.workspace == HOME && t.slug == *slug)
                .expect("a seeded team")
                .name
                .clone();
            changes.push(Change::OnTeam {
                team,
                email: email.clone(),
                role: None,
            });
        }
    }
    assert_state_delta(world, &changes).await;
}

pub fn then_teams_page_lists(world: &FoundryWorld, entries: &[String]) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the teams page renders; {}",
        describe(answer)
    );
    let text = page_text(&answer.body);
    for entry in entries {
        assert!(
            text.contains(entry.as_str()),
            "the teams page must list {entry:?}; {}",
            describe(answer)
        );
    }
    for team in world.tm.teams.values() {
        let listed = entries
            .iter()
            .any(|e| e.starts_with(&format!("{} · ", team.name)));
        if !listed {
            assert!(
                !text.contains(&format!("{} · ", team.name)),
                "the teams page must not list {:?} for this caller; {}",
                team.name,
                describe(answer)
            );
        }
    }
}

async fn members_page_for_then(world: &mut FoundryWorld, team: &str) -> Answer {
    let slug = team_ref(world, team).slug;
    let path = members_path(&slug);
    match world.tm.last.clone() {
        Some(a) if a.path == path && a.status == StatusCode::OK => a,
        _ => {
            let actor = world
                .tm
                .last_actor
                .clone()
                .unwrap_or(Actor::Person(resolve_person(world, "Priya").email));
            let actor = match actor {
                Actor::SignedOut => Actor::Person(resolve_person(world, "Priya").email),
                other => other,
            };
            open(world, &actor, &path).await
        }
    }
}

pub async fn then_members_in_order(world: &mut FoundryWorld, team: &str, rows: &[String]) {
    let page = members_page_for_then(world, team).await;
    let at = row_positions(&page, rows);
    assert!(
        at.iter().all(Option::is_some),
        "{team}'s members page must list every row {rows:?}; {}",
        describe(&page)
    );
    let positions: Vec<usize> = at.into_iter().flatten().collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "{team}'s rows must be leads first, then by name: {rows:?}; {}",
        describe(&page)
    );
    assert_eq!(
        row_count(&page),
        rows.len(),
        "{team}'s members page must list exactly {rows:?}; {}",
        describe(&page)
    );
}

pub async fn then_members_lists(world: &mut FoundryWorld, team: &str, row: &str, once: bool) {
    let page = members_page_for_then(world, team).await;
    let n = page_text(&page.body).matches(row).count();
    assert!(
        n >= 1,
        "{team}'s members page must list {row:?}; {}",
        describe(&page)
    );
    if once {
        assert_eq!(
            n,
            1,
            "{team}'s members page must list {row:?} exactly once; {}",
            describe(&page)
        );
    }
}

pub async fn then_members_not_listing(world: &mut FoundryWorld, team: &str, who: &str) {
    let email = resolve_person(world, who).email;
    let page = members_page_for_then(world, team).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "{team}'s members page renders; {}",
        describe(&page)
    );
    assert!(
        !page_text(&page.body).contains(&format!(" · {email} · ")),
        "{team}'s members page must not list {email}; {}",
        describe(&page)
    );
}

pub fn then_controls(world: &FoundryWorld, expected: bool) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the members page renders; {}",
        describe(answer)
    );
    let (add, role, remove) = controls(answer);
    if expected {
        assert!(add && role && remove, "the page must offer Add, role change and Remove (D11); add={add} role={role} remove={remove}; {}", describe(answer));
    } else {
        assert!(!add && !role && !remove, "the page must offer no management control (D11); add={add} role={role} remove={remove}; {}", describe(answer));
    }
}

pub fn then_offers_exactly(world: &FoundryWorld, team: &str, options: &[String]) {
    let answer = last(world);
    let slug = team_ref(world, team).slug;
    assert_eq!(
        offered(answer, &slug),
        options,
        "Add a member must offer exactly these people, by name then address (D7, D8)"
    );
}

pub fn then_add_replaced_by(world: &FoundryWorld, copy: &str) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the members page renders; {}",
        describe(answer)
    );
    assert!(
        says(answer, copy),
        "the page must say {copy:?}; {}",
        describe(answer)
    );
    assert!(
        !controls(answer).0,
        "with nobody to add, no Add form is rendered; {}",
        describe(answer)
    );
}

pub async fn then_sees_project_and_opens_board(world: &mut FoundryWorld, who: &str, project: &str) {
    let actor = resolve_actor(world, who);
    let p = project_ref(world, project);
    let href = format!("/team/{}/project/{}", p.team_slug, p.slug);
    let section = dashboard_projects(world, &actor).await;
    assert!(
        section.contains(&format!("href=\"{href}\"")),
        "{who}'s dashboard must list {project}; Your projects: {section}"
    );
    let board = open_board(world, &actor, project).await;
    assert_eq!(
        board.status,
        StatusCode::OK,
        "{who} opens the {project} board; {}",
        describe(&board)
    );
}

pub async fn then_board_never_existed(world: &mut FoundryWorld, who: &str, project: &str) {
    let actor = resolve_actor(world, who);
    let board = open_board(world, &actor, project).await;
    let canon = never_existed(world).await;
    assert_eq!(
        (board.status, board.body.clone()),
        (canon.status, canon.body),
        "{who}'s visit to the {project} board must be the page that never existed; {}",
        describe(&board)
    );
}

pub async fn then_dashboard_says(world: &mut FoundryWorld, who: &str, copy: &str) {
    let actor = resolve_actor(world, who);
    let section = dashboard_projects(world, &actor).await;
    assert!(
        page_text(&section).contains(copy),
        "{who}'s dashboard must say {copy:?}; Your projects: {}",
        page_text(&section)
    );
}

pub async fn then_still_finds(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = resolve_actor(world, who);
    let page = ask_members_page(world, &actor, &resolve_team(team)).await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: {who} finds {team}'s members page, so the refusal hid a team that exists; {}",
        describe(&page)
    );
    assert!(
        row_count(&page) >= 1,
        "control: {team}'s members page lists its people; {}",
        describe(&page)
    );
}

pub async fn then_teams_page_still_lists(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = resolve_actor(world, who);
    let page = open(world, &actor, "/teams").await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "control: {who}'s teams page renders; {}",
        describe(&page)
    );
    assert!(
        page_text(&page.body).contains(&format!("{team} · ")),
        "control: {who}'s teams page lists {team}; {}",
        describe(&page)
    );
}

pub fn then_forged_form(world: &FoundryWorld) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::FORBIDDEN,
        "the shipped CSRF layer turns a tokenless write away (OQ-D1); {}",
        describe(answer)
    );
    assert!(
        answer.body.contains("CSRF token missing or mismatched"),
        "the CSRF layer's own refusal; {}",
        describe(answer)
    );
}

pub async fn then_same_add_with_token(world: &mut FoundryWorld, who: &str, team: &str) {
    let admin = world.tm.last_actor.clone().expect("the When's actor");
    let email = resolve_person(world, who).email;
    capture_before(world).await;
    let answer = forge(
        world,
        &admin,
        &TeamName::Known(team.to_string()),
        &Forged::Add(Target::Person(email.clone())),
        true,
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "control: the same add with the token goes through; {}",
        describe(&answer)
    );
    assert_state_delta(
        world,
        &[Change::OnTeam {
            team: team.to_string(),
            email,
            role: Some(TeamRole::Member),
        }],
    )
    .await;
}

pub fn then_nothing_to_report(world: &FoundryWorld, team: &str) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "a stale target re-renders the members page (DDD-9); {}",
        describe(answer)
    );
    let text = page_text(&answer.body);
    assert!(
        text.contains(team) && row_count(answer) >= 1,
        "the answer is {team}'s members page; {}",
        describe(answer)
    );
    for copy in [
        "Added ",
        " is already on ",
        " is now a ",
        "Removed ",
        " needs at least one lead",
    ] {
        assert!(
            !text.contains(copy),
            "a stale target re-renders with no message (DDD-9), found {copy:?}; {}",
            describe(answer)
        );
    }
}

pub fn then_race_one_each(world: &FoundryWorld, copy: &str) {
    assert!(!world.tm.race.is_empty(), "the race ran");
    for (i, (one, two, _)) in world.tm.race.iter().enumerate() {
        let statuses = [one.status, two.status];
        assert!(
            statuses.contains(&StatusCode::OK) && statuses.contains(&StatusCode::CONFLICT),
            "run {}: exactly one must go through (200) and the other be refused (409), never a 500; got {} and {}",
            i + 1,
            describe(one),
            describe(two)
        );
        let refused = if one.status == StatusCode::CONFLICT {
            one
        } else {
            two
        };
        assert!(
            says(refused, copy),
            "run {}: the refusal must say {copy:?}; {}",
            i + 1,
            describe(refused)
        );
    }
}

pub fn then_race_one_lead(world: &FoundryWorld, team: &str) {
    assert_eq!(
        world.tm.race_team.as_deref(),
        Some(team),
        "the race ran on {team}"
    );
    for (i, (_, _, leads)) in world.tm.race.iter().enumerate() {
        assert_eq!(
            *leads,
            1,
            "run {}: {team} must be left with exactly one lead (INV-1/2/3)",
            i + 1
        );
    }
}

pub fn then_sent_no_removal_notice(world: &FoundryWorld, who: &str) {
    let email = resolve_person(world, who).email;
    assert_eq!(
        harness(world).fake_email.count_to(&email),
        0,
        "a refused workspace removal notifies nobody (DDD-8)"
    );
}

pub async fn given_only_lead(world: &FoundryWorld, who: &str, team: &str) {
    let leads = vec![who.to_string()];
    seed_only_leads(world, team, &leads).await;
}

// ----------------------------------------------- default team and newcomers

fn priya(world: &FoundryWorld) -> Actor {
    Actor::Person(resolve_person(world, "Priya").email)
}

fn default_choice(words: &str) -> Option<String> {
    match words.trim() {
        "No team" => None,
        team => Some(team.to_string()),
    }
}

pub async fn when_sets_default(world: &mut FoundryWorld, who: &str, choice: &str) {
    let actor = resolve_actor(world, who);
    capture_before(world).await;
    let answer = set_default_from_page(world, &actor, choice).await;
    record(world, &actor, answer);
}

pub async fn given_default_set(world: &mut FoundryWorld, who: &str, choice: &str) {
    when_sets_default(world, who, choice).await;
    require_ok(world, &format!("{who} sets {choice} as the default team"));
}

pub async fn given_no_default(world: &FoundryWorld) {
    let u = universe(world).await;
    assert_eq!(
        u.defaults.get(HOME),
        Some(&None),
        "precondition: Canzan Labs has no default team"
    );
}

pub async fn given_default_deleted(world: &mut FoundryWorld, team: &str) {
    let admin = priya(world);
    let answer = set_default_from_page(world, &admin, team).await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "precondition: {team} becomes the default; {}",
        describe(&answer)
    );
    delete_team_by_hand(world, team).await;
}

pub async fn then_default_now(world: &FoundryWorld, choice: &str) {
    assert_state_delta(
        world,
        &[Change::Default {
            workspace: HOME.to_string(),
            team: default_choice(choice),
        }],
    )
    .await;
}

pub async fn then_teams_page_shows_default(world: &mut FoundryWorld, choice: &str) {
    let admin = priya(world);
    let page = open(world, &admin, "/teams").await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "Priya's teams page renders; {}",
        describe(&page)
    );
    assert_eq!(
        selected_default(&page).as_deref(),
        Some(choice),
        "the default-team select shows {choice:?}; {}",
        describe(&page)
    );
}

pub async fn when_submits_default(world: &mut FoundryWorld, who: &str, team: &str) {
    let actor = resolve_actor(world, who);
    capture_before(world).await;
    let answer = forge_default(world, &actor, &resolve_team(team)).await;
    record(world, &actor, answer);
}

pub fn then_no_default_setting(world: &FoundryWorld) {
    let answer = last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the teams page renders; {}",
        describe(answer)
    );
    assert!(
        !forms(&answer.body)
            .iter()
            .any(|f| f.action == "/teams/default"),
        "only a workspace admin is offered the default team setting (OQ-3); {}",
        describe(answer)
    );
}

pub async fn then_offers_default_setting(world: &mut FoundryWorld, who: &str) {
    let actor = resolve_actor(world, who);
    let page = open(world, &actor, "/teams").await;
    assert!(
        forms(&page.body)
            .iter()
            .any(|f| f.action == "/teams/default"),
        "control: {who}'s teams page offers the default team setting; {}",
        describe(&page)
    );
}

pub async fn when_accepts_invite(world: &mut FoundryWorld, name: &str, email: &str) {
    let admin = priya(world);
    capture_before(world).await;
    let accepted = invite_and_accept(world, &admin, name, email).await;
    if let Some(session) = accepted.session.clone() {
        adopt_session(world, email, session);
    }
    let newcomer = Actor::Person(email.to_ascii_lowercase());
    record(world, &newcomer, accepted);
}

pub async fn then_new_account(world: &FoundryWorld, who: &str, team: Option<&str>) {
    let email = resolve_person(world, who).email;
    let mut changes = vec![
        Change::NewAccount {
            email: email.clone(),
        },
        Change::InWorkspace {
            workspace: HOME.to_string(),
            email: email.clone(),
            role: Some(WorkspaceRole::Member),
        },
    ];
    if let Some(team) = team {
        changes.push(Change::OnTeam {
            team: team.to_string(),
            email,
            role: Some(TeamRole::Member),
        });
    }
    assert_state_delta(world, &changes).await;
}

pub async fn then_signed_in(world: &mut FoundryWorld, who: &str) {
    let email = resolve_person(world, who).email;
    assert!(
        world.tm.sessions.contains_key(&email),
        "{who} was given a session"
    );
    let actor = Actor::Person(email);
    let dash = open(world, &actor, "/").await;
    assert_eq!(
        dash.status,
        StatusCode::OK,
        "{who} is signed in; {}",
        describe(&dash)
    );
    assert!(
        page_text(&dash.body).contains("Welcome back"),
        "{who} sees the signed-in dashboard; {}",
        describe(&dash)
    );
}

pub async fn then_still_on_no_team(world: &FoundryWorld, who: &str) {
    let email = resolve_person(world, who).email;
    let u = universe(world).await;
    assert!(
        !u.team_rows.keys().any(|(_, _, e)| *e == email),
        "{who} is still on no team (OQ-8: no backfill)"
    );
}

/// Sign in through Keycloak (the in-process identity provider double), granting
/// the provision role; the person and their session join the roster.
async fn keycloak_sign_in(
    world: &mut FoundryWorld,
    name: &str,
    email: &str,
    alias: Option<String>,
) {
    use crate::steps::feature_keycloak_sso_provisioning::{
        begin_federated_sign_in, finish_federated_sign_in,
    };
    {
        let issuer = world
            .kc_issuer
            .as_ref()
            .expect("the Background started the identity provider");
        issuer.will_vouch_for(email, true);
        issuer.will_name(
            Some(name),
            Some(email.split('@').next().unwrap_or("someone")),
        );
        issuer.will_grant_realm_roles(&[PROVISION_ROLE]);
    }
    begin_federated_sign_in(world).await;
    finish_federated_sign_in(world).await;
    let session = world.kc_session_cookie.clone().unwrap_or_else(|| {
        panic!(
            "the Keycloak sign-in of {email} gave no session; status {:?}, body {:?}",
            world.last_status, world.last_body
        )
    });
    if world
        .tm
        .people
        .iter()
        .any(|p| p.email == email.to_ascii_lowercase() && p.password.is_some())
    {
        if let Some(p) = world
            .tm
            .people
            .iter_mut()
            .find(|p| p.email == email.to_ascii_lowercase())
        {
            if alias.is_some() {
                p.alias = alias;
            }
        }
    } else {
        adopt_person(world, name, email, alias).await;
    }
    adopt_session(world, email, session);
}

pub async fn given_keycloak_account(world: &mut FoundryWorld, email: &str) {
    keycloak_sign_in(
        world,
        "Jeff Bailey",
        email,
        Some("Jeff's Keycloak account".to_string()),
    )
    .await;
}

pub async fn given_on_no_team_and_no_board(world: &mut FoundryWorld, who: &str, project: &str) {
    then_still_on_no_team(world, who).await;
    then_board_never_existed(world, who, project).await;
}

pub async fn when_first_keycloak_sign_in(world: &mut FoundryWorld, name: &str, email: &str) {
    capture_before(world).await;
    keycloak_sign_in(world, name, email, None).await;
    let actor = Actor::Person(email.to_ascii_lowercase());
    world.tm.last_actor = Some(actor);
}

pub async fn when_keycloak_own_address(world: &mut FoundryWorld, who: &str) {
    let p = resolve_person(world, who);
    capture_before(world).await;
    keycloak_sign_in(world, &p.name, &p.email, None).await;
    world.tm.last_actor = Some(Actor::Person(p.email));
}
