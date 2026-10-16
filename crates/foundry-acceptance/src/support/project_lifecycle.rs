//! project-lifecycle — the acceptance driver the `feature_project_lifecycle`
//! step file delegates to (DISTILL 2026-10-13, every scenario `@pending`).
//!
//! It extends, and never duplicates, the team-lifecycle driver
//! ([`crate::support::team_lifecycle`], `tl`) and through it the
//! team-membership driver ([`crate::support::team_membership`], `tm`): the
//! Background, the roster, sessions, CSRF, `open` / `post`, the page readers,
//! the uniform-404 oracle, the team delete pages and the team universe are
//! theirs. This module adds:
//!
//! - **The vocabulary** of this feature ([`RecordKind`], [`ProjectRecord`],
//!   [`ProjectFacts`], [`Holdings`], [`Change`], [`Door`], [`ApiDoor`],
//!   [`Answered`], [`CliRun`], [`PlState`]) — the house's typed domain module
//!   (Mandate 12, criterion 1).
//! - **The doors** DESIGN pinned (DDD-23, DDD-25, DDD-10): the members page's
//!   Projects section (a per-row rename form posting `name` to
//!   `/team/{t}/project/{p}/rename`, a per-row **Delete project** link to
//!   `/team/{t}/project/{p}/delete`), the confirm page (its **Delete project**
//!   form and **Cancel** link), the instance dashboard's rename row, every
//!   shipped project route of the web and the API (the route inventory, for
//!   the differential outline), and the operator's `foundry doctor`
//!   subcommands through the real, prebuilt binary. A write "from the page"
//!   submits the page's own form; a write the caller has no form for is the
//!   same request built by hand.
//! - **The universe** (Mandate 8): team-lifecycle's (team rows, workspace rows,
//!   teams, names, tokens, team records; its `projects` join is blanked, this
//!   module's view of projects is the complete one) plus every project row
//!   (team, name, address, key, next issue number, deleted marker, detached),
//!   a count-and-checksum of everything under each project ([`Holdings`]:
//!   issues, comments live and awaiting cleanup, attachments, history, lanes,
//!   queued notifications) and every project lifecycle record. A `When`
//!   captures it first (through `tl::capture_before`, which calls
//!   [`capture_projects_before`]); a `Then` compares the after-image with the
//!   before-image plus the declared [`Change`]s, fail-closed.
//!
//! Base-table reads are fixture SQL (DESIGN OQ-D6: "nothing erased" reads
//! `projects` and its children; check-arch rule 1 exempts this crate). Every
//! "hidden" assertion goes through a product door, never SQL. Preconditions
//! (projects with their stored address, the race's fresh projects and teams)
//! are seeded with SQL; issues, comments, attachments, deletes, renames and
//! restores the scenarios assert on are written by the product (No Fixture
//! Theater).
//!
//! The schema this feature adds (0025, 0026, 0027) is read only when present,
//! so the universe works before and after each migration lands.
//!
//! The project's states (AT-completeness C2a) and the events the scenarios
//! drive through them:
//!
//! ```text
//!   live ──rename (lead/admin, either door)──▶ live            (one renamed record; byte-equal: no-op)
//!   live ──delete (confirm page POST)───────▶ deleted          (marker + one deleted record)
//!   deleted ──restore-project──────────────▶ live             (marker cleared + one restored record)
//!   deleted ──team delete (no live project)─▶ detached        (kept, team gone; terminal here)
//!   illegal: live ─restore→ exit 4 · deleted ─rename/delete/any write/any read→ "never existed"
//!            deleted ─restore twice→ exit 4 · detached ─restore→ exit 4
//!            live team ─team delete→ refused naming live projects only
//! ```

use crate::steps::feature_project_name_rule::expand_name;
use crate::support::html_assertions::{parse, select_all, text_of};
use crate::support::team_lifecycle as tl;
use crate::support::team_membership::{self as tm, Actor, Answer};
use crate::world::FoundryWorld;
use reqwest::header::{CONTENT_TYPE, COOKIE};
use reqwest::StatusCode;
use secrecy::ExposeSecret;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

/// The address the scenarios use for a project no team ever had.
pub const UNKNOWN_PROJECT: &str = "no-such-project";
/// The lane every lane door names (seeded with every project).
const LANE: &str = "todo";
/// The attachment every seeded project carries on its commented issue.
const ATTACHMENT_NAME: &str = "spike-notes.png";
/// A 1×1 PNG.
const ATTACHMENT_BYTES: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];
/// A database address nothing listens on (the operator's unreachable database).
const UNREACHABLE_DATABASE: &str = "postgres://foundry:foundry@127.0.0.1:1/foundry";

// ===========================================================================
// Vocabulary
// ===========================================================================

/// What a project lifecycle record says happened (OQ-3, ADR-PROJECT-LIFECYCLE-003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordKind {
    Renamed,
    Deleted,
    Restored,
}

impl RecordKind {
    fn parse(word: &str) -> Self {
        match word {
            "renamed" => Self::Renamed,
            "deleted" => Self::Deleted,
            "restored" => Self::Restored,
            other => panic!("a project lifecycle record of kind {other:?}; only renamed, deleted and restored exist"),
        }
    }
}

/// One project lifecycle record, as the operator reads it with psql.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectRecord {
    pub workspace: String,
    pub kind: RecordKind,
    pub project_slug: String,
    pub key: String,
    /// The name before the event (or at it).
    pub name: String,
    pub new_name: Option<String>,
    /// The actor's email; `None` is the operator (`foundry doctor`).
    pub actor: Option<String>,
}

/// One project row as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFacts {
    pub workspace: String,
    /// The team's address; `None` once the team is gone (detached).
    pub team: Option<String>,
    pub name: String,
    pub slug: String,
    pub key: String,
    pub next_number: i64,
    pub deleted: bool,
    /// The deleter's email.
    pub deleted_by: Option<String>,
    pub detached: bool,
}

/// Everything under one project, by count and checksum (INV-PL-1, PL-KPI-2).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Holdings {
    pub issues: (i64, String),
    pub comments_live: i64,
    pub comments_awaiting_cleanup: i64,
    pub comments: String,
    pub attachments: (i64, String),
    pub history: (i64, String),
    pub lanes: (i64, String),
    pub notifications: (i64, String),
}

/// This feature's part of the universe (team-lifecycle's is captured beside it).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Projects {
    pub facts: BTreeMap<uuid::Uuid, ProjectFacts>,
    pub holdings: BTreeMap<uuid::Uuid, Holdings>,
    /// Every project lifecycle record, sorted (a multiset).
    pub records: Vec<ProjectRecord>,
}

/// One declared movement of the universe.
#[derive(Debug, Clone)]
pub enum Change {
    /// A team-lifecycle movement (a team delete and its record).
    Team(tl::Change),
    /// The project (by scenario handle) now has this name; address and key unchanged.
    Renamed { project: String, new_name: String },
    /// The project carries the deleted marker, naming this deleter.
    Deleted { project: String, by: String },
    /// The project's deleted marker is cleared.
    Restored { project: String },
    /// The project's team is gone; the project is kept, detached.
    Detached { project: String },
    /// One new project lifecycle record.
    Recorded(ProjectRecord),
}

/// A project the scenarios can name (by every name it has had).
#[derive(Debug, Clone)]
pub struct PlProject {
    pub id: uuid::Uuid,
    /// The team's scenario handle ("Backend").
    pub team: String,
    pub team_slug: String,
    pub slug: String,
    pub key: String,
    /// The name it was seeded with.
    pub name: String,
}

/// A web door of the route inventory (DISCUSS [REF] Lifecycle inventory, plus
/// attachment upload and download, plus this feature's three new doors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Door {
    Board,
    Report,
    ReportSpreadsheet,
    FileIssue,
    NewIssueForm,
    Search,
    MoveIssue,
    IssuePage,
    IssueEditForm,
    EditIssue,
    IssueDeletePage,
    DeleteIssue,
    CommentOnIssue,
    CommentPage,
    CommentEditForm,
    EditComment,
    DeleteComment,
    AttachFile,
    DownloadAttachment,
    LiveUpdates,
    LaneDeleteDialog,
    DeleteLane,
    LaneEditDialog,
    RenameLane,
    LaneInsertDialog,
    InsertLane,
    MoveLane,
    RenameProject,
    ProjectDeletePage,
    DeleteProject,
}

impl Door {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "the board" => Self::Board,
            "the report" => Self::Report,
            "the report as a spreadsheet" => Self::ReportSpreadsheet,
            "filing an issue" => Self::FileIssue,
            "the new-issue form" => Self::NewIssueForm,
            "searching the issues" => Self::Search,
            "moving an issue to another state" => Self::MoveIssue,
            "an issue's page" => Self::IssuePage,
            "an issue's edit form" => Self::IssueEditForm,
            "editing an issue" => Self::EditIssue,
            "an issue's delete page" => Self::IssueDeletePage,
            "deleting an issue" => Self::DeleteIssue,
            "commenting on an issue" => Self::CommentOnIssue,
            "a comment" => Self::CommentPage,
            "a comment's edit form" => Self::CommentEditForm,
            "editing a comment" => Self::EditComment,
            "deleting a comment" => Self::DeleteComment,
            "attaching a file to an issue" => Self::AttachFile,
            "downloading an attachment" => Self::DownloadAttachment,
            "the live updates" => Self::LiveUpdates,
            "a lane's delete dialog" => Self::LaneDeleteDialog,
            "deleting a lane" => Self::DeleteLane,
            "a lane's edit dialog" => Self::LaneEditDialog,
            "renaming a lane" => Self::RenameLane,
            "a lane's insert dialog" => Self::LaneInsertDialog,
            "inserting a lane" => Self::InsertLane,
            "moving a lane" => Self::MoveLane,
            "renaming the project" => Self::RenameProject,
            "the project's delete page" => Self::ProjectDeletePage,
            "deleting the project" => Self::DeleteProject,
            other => panic!("{other:?} is not a door of the project route inventory"),
        }
    }
}

/// An API door of the route inventory (`/api/v1/teams/{t}/projects/{p}/…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiDoor {
    ListIssues,
    FileIssue,
    MoveIssue,
    History,
    CommentOnIssue,
    EditComment,
    ListTokens,
    RevokeToken,
}

impl ApiDoor {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "listing the issues" => Self::ListIssues,
            "filing an issue" => Self::FileIssue,
            "moving an issue to another state" => Self::MoveIssue,
            "reading an issue's history" => Self::History,
            "commenting on an issue" => Self::CommentOnIssue,
            "editing a comment" => Self::EditComment,
            "listing the machine tokens" => Self::ListTokens,
            "revoking a machine token" => Self::RevokeToken,
            other => panic!("{other:?} is not a door of the API route inventory"),
        }
    }
}

/// What the differential outline's answer column names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered {
    /// `resource_not_found_page()`, byte for byte — the live updates included
    /// (Changed Assumption 1, amended 2026-10-15).
    UniformNotFound,
    /// The API's shipped not-found answer.
    ApiNotFound,
}

impl Answered {
    pub fn parse(words: &str) -> Self {
        match words.trim() {
            "the uniform not-found page" => Self::UniformNotFound,
            "the scripts' not-found answer" => Self::ApiNotFound,
            other => panic!("{other:?} is not a known answer"),
        }
    }
}

/// The real things a door names inside one project.
#[derive(Debug, Clone)]
struct Address {
    team_slug: String,
    project_slug: String,
    issue: i64,
    comment: uuid::Uuid,
    attachment: uuid::Uuid,
}

/// One differential row's evidence.
#[derive(Debug, Clone)]
pub struct Differential {
    pub door: String,
    pub deleted: Answer,
    pub never_existed: Answer,
    pub live: Answer,
    pub held_before: (Option<ProjectFacts>, Holdings),
    pub held_after: (Option<ProjectFacts>, Holdings),
    /// The whole universe (teams, people, tokens, team records, every project,
    /// its holdings and records) before and after the two compared requests.
    pub everything_before: (tl::Universe, Projects),
    pub everything_after: (tl::Universe, Projects),
}

/// The two revokes of the spare token (under the deleted and a never-existed address).
#[derive(Debug, Clone)]
pub struct RevokePair {
    pub first: Answer,
    pub second: Answer,
    pub before: (tl::Universe, Projects),
    pub after_first: (tl::Universe, Projects),
    pub after_second: (tl::Universe, Projects),
    pub revoked_before: bool,
    pub revoked_after: bool,
}

/// One run of the operator's CLI.
#[derive(Debug, Clone)]
pub struct CliRun {
    pub args: Vec<String>,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// One run of the delete-versus-issue-create race.
#[derive(Debug, Clone)]
pub struct DeleteRace {
    pub project: String,
    pub delete: Answer,
    pub create: Answer,
    pub board_for_sam: Answer,
    pub issues_kept: i64,
}

/// One run of the restore-versus-team-delete race.
#[derive(Debug, Clone)]
pub struct RestoreRace {
    pub project: String,
    pub key: String,
    pub restore: CliRun,
    pub team_delete: Answer,
    pub project_row_kept: bool,
    pub issues_kept: i64,
}

/// The per-scenario state of this feature (`FoundryWorld::pl`).
#[derive(Debug, Default)]
pub struct PlState {
    /// This feature's part of the before-image the last `When` captured.
    pub before: Option<Projects>,
    /// Keyed by every name a project has been called by in the scenario.
    pub projects: HashMap<String, PlProject>,
    /// Facts and holdings noted just before each product delete (INV-PL-4).
    pub before_delete: HashMap<uuid::Uuid, (ProjectFacts, Holdings)>,
    /// Pages kept open in another tab: `"{actor}|{what}"` -> page.
    pub earlier: HashMap<String, Answer>,
    /// `(actor key, issue key "SPIKE-2") -> comment id` of the actor's own comment.
    pub comments: HashMap<(String, String), uuid::Uuid>,
    /// The comment deleted and awaiting cleanup.
    pub awaiting_cleanup: Option<uuid::Uuid>,
    /// A well-formed id no project has, fixed per scenario.
    pub nobody: Option<uuid::Uuid>,
    /// The script's bearer and the spare token it may revoke.
    pub script: Option<(String, uuid::Uuid)>,
    pub cli: Option<CliRun>,
    pub diff: Option<Differential>,
    pub answers: Vec<Answer>,
    pub watch: Option<crate::support::sse_client::SseSubscription>,
    pub heard: usize,
    pub delete_race: Vec<DeleteRace>,
    pub restore_race: Vec<RestoreRace>,
    /// The races' fresh projects (seeded by a Given; the When only races).
    pub race_projects: Vec<PlProject>,
    /// How every page of a project answered someone, noted by a Given:
    /// `project name -> [(door, status)]`.
    pub noted: HashMap<String, Vec<(String, StatusCode)>>,
    /// The token revoke pair: (first, second, universe before, after first, after second, spare revoked).
    pub revoke: Option<RevokePair>,
}

// ===========================================================================
// Name resolution
// ===========================================================================

fn expand(raw: &str) -> String {
    expand_name(raw)
}

pub fn project(world: &FoundryWorld, name: &str) -> PlProject {
    world
        .pl
        .projects
        .get(name.trim())
        .cloned()
        .unwrap_or_else(|| panic!("project {name:?} is seeded by the Background"))
}

/// The project an issue key names ("SPIKE-2" -> Auth v2 Spike, 2).
fn issue_of(world: &FoundryWorld, key: &str) -> (PlProject, i64) {
    let (prefix, number) = key
        .rsplit_once('-')
        .unwrap_or_else(|| panic!("{key:?} is an issue key"));
    let number: i64 = number.parse().expect("an issue number");
    let p = world
        .pl
        .projects
        .values()
        .find(|p| p.key == prefix)
        .cloned()
        .unwrap_or_else(|| panic!("no seeded project has the key {prefix}"));
    (p, number)
}

fn members_path(team_slug: &str) -> String {
    tm::members_path(team_slug)
}

fn project_path(p: &PlProject) -> String {
    format!("/team/{}/project/{}", p.team_slug, p.slug)
}

fn rename_path(team_slug: &str, project_slug: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}/rename")
}

fn delete_path(team_slug: &str, project_slug: &str) -> String {
    format!("/team/{team_slug}/project/{project_slug}/delete")
}

/// "a project that does not exist" is a slug under Backend no project has.
fn project_slugs(world: &FoundryWorld, words: &str) -> (String, String) {
    match words.trim() {
        "a project that does not exist" => (
            tm::team_ref(world, "Backend").slug,
            UNKNOWN_PROJECT.to_string(),
        ),
        name => {
            let p = project(world, name);
            (p.team_slug, p.slug)
        }
    }
}

fn nobody(world: &mut FoundryWorld) -> uuid::Uuid {
    *world.pl.nobody.get_or_insert_with(uuid::Uuid::now_v7)
}

/// Resolve `{…}` placeholders: `{X's id}`, `{an id no project has}`,
/// `{that comment's id}`.
fn resolve(world: &mut FoundryWorld, text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}').expect("a closed placeholder");
        let token = &after[..close];
        let value = match token {
            "an id no project has" => nobody(world).to_string(),
            "that comment's id" => world
                .pl
                .awaiting_cleanup
                .expect("a comment was deleted and awaits cleanup")
                .to_string(),
            t if t.ends_with("'s id") => project(world, t.trim_end_matches("'s id")).id.to_string(),
            other => panic!("unknown placeholder {{{other}}}"),
        };
        out.push_str(&value);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

// ===========================================================================
// The universe
// ===========================================================================

async fn has_column(world: &FoundryWorld, table: &str, column: &str) -> bool {
    let (present,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                         WHERE table_schema = current_schema() AND table_name = $1 AND column_name = $2)",
    )
    .bind(table)
    .bind(column)
    .fetch_one(&tm::pool(world))
    .await
    .expect("look a column up");
    present
}

async fn has_table(world: &FoundryWorld, table: &str) -> bool {
    let (present,): (bool,) = sqlx::query_as(
        "SELECT to_regclass(format('%I.%I', current_schema(), $1::text)) IS NOT NULL",
    )
    .bind(table)
    .fetch_one(&tm::pool(world))
    .await
    .expect("look a table up");
    present
}

type FactRow = (
    uuid::Uuid,
    String,
    Option<String>,
    String,
    String,
    String,
    i64,
    bool,
    Option<String>,
    bool,
);

/// One project lifecycle record as read: workspace, kind, slug, key, name,
/// new name, actor email.
type RecordRow = (
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);

/// Read every project row, everything under each, and every project record.
pub async fn read_projects(world: &FoundryWorld) -> Projects {
    let pool = tm::pool(world);
    let marker = has_column(world, "projects", "deleted_at").await;
    let detach = has_column(world, "projects", "detached_from_team_id").await;
    let facts_sql =
        format!(
        "SELECT p.id, w.name, t.slug, p.name, p.slug, p.key_prefix, p.next_issue_number::bigint,
                {deleted}, {deleted_by}, {detached}
           FROM projects p
           JOIN workspaces w ON w.id = p.workspace_id
           LEFT JOIN teams t ON t.id = p.team_id",
        deleted = if marker { "p.deleted_at IS NOT NULL" } else { "false" },
        deleted_by = if marker {
            "(SELECT u.email_lower FROM users u WHERE u.id = p.deleted_by)"
        } else {
            "NULL::text"
        },
        detached = if detach { "p.detached_from_team_id IS NOT NULL" } else { "false" },
    );
    let rows: Vec<FactRow> = sqlx::query_as(&facts_sql)
        .fetch_all(&pool)
        .await
        .expect("read the projects");
    let facts: BTreeMap<uuid::Uuid, ProjectFacts> = rows
        .into_iter()
        .map(
            |(id, workspace, team, name, slug, key, next_number, deleted, deleted_by, detached)| {
                (
                    id,
                    ProjectFacts {
                        workspace,
                        team,
                        name,
                        slug,
                        key,
                        next_number,
                        deleted,
                        deleted_by,
                        detached,
                    },
                )
            },
        )
        .collect();

    let mut holdings: BTreeMap<uuid::Uuid, Holdings> =
        facts.keys().map(|id| (*id, Holdings::default())).collect();
    let sum = |table: &str, key: &str, alias: &str| {
        format!(
            "SELECT {key}::text, count(*), md5(string_agg(row_to_json({alias})::text, '|' ORDER BY {alias}.id::text))
               FROM {table} {alias} GROUP BY 1"
        )
    };
    for (what, sql) in [
        ("issues", sum("issues", "i.project_id", "i")),
        ("history", sum("issue_change_events", "e.project_id", "e")),
        ("lanes", sum("lanes", "l.project_id", "l")),
    ] {
        let rows: Vec<(String, i64, String)> = sqlx::query_as(&sql)
            .fetch_all(&pool)
            .await
            .unwrap_or_else(|e| panic!("read the {what}: {e}"));
        for (id, n, digest) in rows {
            let id: uuid::Uuid = id.parse().expect("a project id");
            let h = holdings.entry(id).or_default();
            match what {
                "issues" => h.issues = (n, digest),
                "history" => h.history = (n, digest),
                _ => h.lanes = (n, digest),
            }
        }
    }
    let comments: Vec<(uuid::Uuid, i64, i64, String)> = sqlx::query_as(
        "SELECT i.project_id,
                count(*) FILTER (WHERE c.deleted_at IS NULL),
                count(*) FILTER (WHERE c.deleted_at IS NOT NULL),
                md5(string_agg(row_to_json(c)::text, '|' ORDER BY c.id::text))
           FROM comments c JOIN issues i ON i.id = c.issue_id GROUP BY i.project_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read the comments");
    for (id, live, awaiting, digest) in comments {
        let h = holdings.entry(id).or_default();
        h.comments_live = live;
        h.comments_awaiting_cleanup = awaiting;
        h.comments = digest;
    }
    let attachments: Vec<(uuid::Uuid, i64, String)> = sqlx::query_as(
        "SELECT i.project_id, count(*), md5(string_agg(row_to_json(a)::text, '|' ORDER BY a.id::text))
           FROM issue_attachments a JOIN issues i ON i.id = a.issue_id GROUP BY i.project_id",
    )
    .fetch_all(&pool)
    .await
    .expect("read the attachments");
    for (id, n, digest) in attachments {
        holdings.entry(id).or_default().attachments = (n, digest);
    }
    // Queued notifications: their identity and payload, not their delivery
    // stamp (the notifier stamps `notified_at` on its own schedule, D17).
    let queued: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT payload->>'project_id', count(*),
                md5(string_agg(id::text || ':' || event_type || ':' || payload::text, '|' ORDER BY id))
           FROM outbox WHERE payload ? 'project_id' GROUP BY 1",
    )
    .fetch_all(&pool)
    .await
    .expect("read the queued notifications");
    for (id, n, digest) in queued {
        if let Ok(id) = id.parse::<uuid::Uuid>() {
            holdings.entry(id).or_default().notifications = (n, digest);
        }
    }

    let mut records = Vec::new();
    if has_table(world, "project_lifecycle_events").await {
        let rows: Vec<RecordRow> = sqlx::query_as(
            "SELECT w.name, e.kind, e.project_slug, e.key_prefix, e.name, e.new_name, u.email_lower
                   FROM project_lifecycle_events e
                   JOIN workspaces w ON w.id = e.workspace_id
                   LEFT JOIN users u ON u.id = e.actor_id",
        )
        .fetch_all(&pool)
        .await
        .expect("read the project lifecycle records");
        records = rows
            .into_iter()
            .map(
                |(workspace, kind, project_slug, key, name, new_name, actor)| ProjectRecord {
                    workspace,
                    kind: RecordKind::parse(&kind),
                    project_slug,
                    key,
                    name,
                    new_name,
                    actor,
                },
            )
            .collect();
        records.sort();
    }
    Projects {
        facts,
        holdings,
        records,
    }
}

/// Called by `tl::capture_before`, so every `When` of the three drivers
/// captures this feature's before-image too.
pub async fn capture_projects_before(world: &mut FoundryWorld) {
    let projects = read_projects(world).await;
    world.pl.before = Some(projects);
}

/// The full before-image: team-membership's, team-lifecycle's and ours.
pub async fn capture_before(world: &mut FoundryWorld) {
    tl::capture_before(world).await;
}

fn blank_projects(mut u: tl::Universe) -> tl::Universe {
    u.projects.clear();
    u
}

/// `assert_state_delta(before, after, universe, expected)`: the after-image
/// must equal the before-image with exactly `changes` applied. Fail-closed,
/// over team-lifecycle's universe and this feature's.
pub async fn assert_state_delta(world: &FoundryWorld, changes: &[Change]) {
    let team_changes: Vec<tl::Change> = changes
        .iter()
        .filter_map(|c| match c {
            Change::Team(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    let mut expected_tl = blank_projects(tl::captured_before(world));
    tl::apply_changes(world, &mut expected_tl, &team_changes);
    let after_tl = blank_projects(tl::universe(world).await);
    assert_eq!(
        after_tl, expected_tl,
        "state delta (teams, people, tokens, team records): the universe must equal the before-image plus exactly {changes:?}"
    );

    let before = world
        .pl
        .before
        .clone()
        .expect("the When captured the projects before acting");
    let after = read_projects(world).await;
    let mut expected = before.clone();
    for change in changes {
        match change {
            Change::Team(_) => {}
            Change::Renamed {
                project: p,
                new_name,
            } => {
                let id = project(world, p).id;
                expected.facts.get_mut(&id).expect("a known project").name = new_name.clone();
            }
            Change::Deleted { project: p, by } => {
                let id = project(world, p).id;
                let f = expected.facts.get_mut(&id).expect("a known project");
                f.deleted = true;
                f.deleted_by = Some(by.clone());
            }
            Change::Restored { project: p } => {
                let id = project(world, p).id;
                let f = expected.facts.get_mut(&id).expect("a known project");
                f.deleted = false;
                f.deleted_by = None;
            }
            Change::Detached { project: p } => {
                let id = project(world, p).id;
                let f = expected.facts.get_mut(&id).expect("a known project");
                f.team = None;
                f.detached = true;
            }
            Change::Recorded(record) => {
                expected.records.push(record.clone());
                expected.records.sort();
            }
        }
    }
    assert_eq!(
        after.facts, expected.facts,
        "state delta (project rows): the projects must equal the before-image plus exactly {changes:?}"
    );
    assert_eq!(
        after.holdings, expected.holdings,
        "state delta (INV-PL-1): nothing under any project may change; declared {changes:?}"
    );
    assert_eq!(
        after.records, expected.records,
        "state delta (project lifecycle records, INV-PL-7): declared {changes:?}"
    );
}

async fn holdings_of(world: &FoundryWorld, id: uuid::Uuid) -> (Option<ProjectFacts>, Holdings) {
    let all = read_projects(world).await;
    (
        all.facts.get(&id).cloned(),
        all.holdings.get(&id).cloned().unwrap_or_default(),
    )
}

// ===========================================================================
// Reading pages
// ===========================================================================

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The Projects section's rows: each `li` of the first list after the h2
/// "Projects", as (visible text, the li's inner HTML).
fn project_rows(answer: &Answer) -> Vec<(String, String)> {
    let doc = parse(&answer.body);
    let Some(h2) = select_all(&doc, "h2")
        .into_iter()
        .find(|h| squash(&text_of(h)) == "Projects")
    else {
        panic!(
            "the page has no Projects section (an h2 \"Projects\"); {}",
            tm::describe(answer)
        );
    };
    let Some(list) = h2
        .next_siblings()
        .filter_map(scraper::ElementRef::wrap)
        .find(|e| matches!(e.value().name(), "ul" | "ol"))
    else {
        return Vec::new();
    };
    let li = scraper::Selector::parse("li").expect("selector");
    list.select(&li)
        .map(|row| {
            (
                squash(&row.text().collect::<Vec<_>>().join(" ")),
                row.html(),
            )
        })
        .collect()
}

/// The row whose text starts "{name} · {KEY}".
fn row_of(answer: &Answer, name: &str, key: &str) -> String {
    let label = format!("{name} · {key}");
    project_rows(answer)
        .into_iter()
        .find(|(text, _)| text.starts_with(&label))
        .map(|(_, html)| html)
        .unwrap_or_else(|| {
            panic!(
                "the Projects section has no row \"{label}\"; {}",
                tm::describe(answer)
            )
        })
}

fn row_answer(answer: &Answer, html: String) -> Answer {
    Answer {
        body: format!("<html><body>{html}</body></html>"),
        ..answer.clone()
    }
}

/// The current name of a project as the store holds it.
async fn current_name(world: &FoundryWorld, p: &PlProject) -> String {
    let (name,): (String,) = sqlx::query_as("SELECT name FROM projects WHERE id = $1")
        .bind(p.id)
        .fetch_one(&tm::pool(world))
        .await
        .expect("read the project's name");
    name
}

// ===========================================================================
// Requests
// ===========================================================================

async fn send(
    world: &mut FoundryWorld,
    actor: &Actor,
    method: reqwest::Method,
    path: &str,
    body: Body,
) -> Answer {
    let cookie = tm::cookie_for(world, actor).await;
    let base = tm::harness(world).base_url();
    let csrf = world.tm.csrf.clone();
    let mut req = tm::http(world)
        .request(method.clone(), format!("{base}{path}"))
        .header(COOKIE, cookie);
    if method != reqwest::Method::GET {
        req = req.header("x-csrf-token", csrf.clone());
    }
    req = match body {
        Body::None => req,
        Body::Form(mut fields) => {
            fields.push(("_csrf".to_string(), csrf));
            req.form(&fields)
        }
        Body::File => {
            let part = reqwest::multipart::Part::bytes(ATTACHMENT_BYTES.to_vec())
                .file_name(ATTACHMENT_NAME)
                .mime_str("image/png")
                .expect("a png part");
            req.multipart(reqwest::multipart::Form::new().part("file", part))
        }
        Body::Json(_) => panic!("JSON goes to the API"),
    };
    let resp = req
        .send()
        .await
        .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
    answer_of(path, resp).await
}

/// Read an answer; a live stream's body is not read (it never ends).
async fn answer_of(path: &str, resp: reqwest::Response) -> Answer {
    let streaming = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|c| c.starts_with("text/event-stream"));
    if streaming {
        Answer {
            path: path.to_string(),
            status: resp.status(),
            content_type: Some("text/event-stream".to_string()),
            location: None,
            session: None,
            body: String::new(),
        }
    } else {
        tm::into_answer(path, resp).await
    }
}

#[derive(Debug, Clone)]
enum Body {
    None,
    Form(Vec<(String, String)>),
    File,
    Json(serde_json::Value),
}

fn form(fields: &[(&str, &str)]) -> Body {
    Body::Form(
        fields
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect(),
    )
}

async fn post_form(
    world: &mut FoundryWorld,
    actor: &Actor,
    path: &str,
    fields: &[(&str, &str)],
) -> Answer {
    send(world, actor, reqwest::Method::POST, path, form(fields)).await
}

async fn api(world: &mut FoundryWorld, method: reqwest::Method, path: &str, body: Body) -> Answer {
    let (bearer, _) = world
        .pl
        .script
        .clone()
        .expect("a script holds a machine token");
    let base = tm::harness(world).base_url();
    let mut req = tm::http(world)
        .request(method.clone(), format!("{base}{path}"))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {bearer}"));
    if let Body::Json(v) = body {
        req = req.json(&v);
    }
    let resp = req
        .send()
        .await
        .unwrap_or_else(|e| panic!("{method} {path}: {e}"));
    tm::into_answer(path, resp).await
}

/// The first person on the project's team, a lead first (the work's author).
async fn author_of(world: &FoundryWorld, p: &PlProject) -> Actor {
    let team = tm::team_ref(world, &p.team);
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT u.email_lower FROM team_memberships m JOIN users u ON u.id = m.user_id
          WHERE m.team_id = $1 ORDER BY (m.role = 'lead') DESC, u.email_lower LIMIT 1",
    )
    .bind(team.id)
    .fetch_optional(&tm::pool(world))
    .await
    .expect("find someone on the team");
    let (email,) = row.unwrap_or_else(|| panic!("precondition: someone is on {}", p.team));
    Actor::Person(email)
}

fn require_written(answer: &Answer, what: &str) {
    assert!(
        answer.status.is_success() || answer.status.is_redirection(),
        "precondition: {what} went through the product; {}",
        tm::describe(answer)
    );
}

async fn file_issue(world: &mut FoundryWorld, actor: &Actor, p: &PlProject, title: &str) -> Answer {
    let path = format!("{}/issues", project_path(p));
    post_form(
        world,
        actor,
        &path,
        &[("title", title), ("description", "")],
    )
    .await
}

async fn comment(
    world: &mut FoundryWorld,
    actor: &Actor,
    p: &PlProject,
    n: i64,
    body: &str,
) -> Answer {
    let path = format!("{}/issues/{n}/comments", project_path(p));
    post_form(world, actor, &path, &[("body", body)]).await
}

async fn latest_comment(world: &FoundryWorld, actor: &Actor, p: &PlProject, n: i64) -> uuid::Uuid {
    let email = tm::actor_key(actor);
    let (id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT c.id FROM comments c JOIN issues i ON i.id = c.issue_id JOIN users u ON u.id = c.author_id
          WHERE i.project_id = $1 AND i.number = $2 AND u.email_lower = $3
          ORDER BY c.created_at DESC, c.id DESC LIMIT 1",
    )
    .bind(p.id)
    .bind(n as i32)
    .bind(email)
    .fetch_one(&tm::pool(world))
    .await
    .expect("find the comment just written");
    id
}

async fn attachment_of(world: &FoundryWorld, p: &PlProject, n: i64) -> uuid::Uuid {
    let (id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT a.id FROM issue_attachments a JOIN issues i ON i.id = a.issue_id
          WHERE i.project_id = $1 AND i.number = $2 ORDER BY a.id LIMIT 1",
    )
    .bind(p.id)
    .bind(n as i32)
    .fetch_one(&tm::pool(world))
    .await
    .unwrap_or_else(|e| panic!("precondition: {}-{n} has an attachment: {e}", p.key));
    id
}

/// The issue that carries a project's comments and attachment.
fn commented_issue(issues: usize) -> i64 {
    if issues >= 2 {
        2
    } else {
        1
    }
}

// ===========================================================================
// Givens
// ===========================================================================

/// Projects with their stored address (the slug is minted once and never
/// re-derived, so "Identity Platform" lives at `auth`).
pub async fn given_projects_live(world: &mut FoundryWorld, step: &cucumber::gherkin::Step) {
    for r in tm::table_rows(step) {
        let (team, name, key, slug) = (&r[0], &r[1], &r[2], &r[3]);
        let tr = tm::team_ref(world, team);
        let ws = tm::workspace_id(world, &tr.workspace);
        let id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(tr.id)
        .bind(ws)
        .bind(name)
        .bind(slug)
        .bind(key)
        .execute(&tm::pool(world))
        .await
        .expect("seed a project");
        crate::support::harness::seed_lanes_for_project(&tm::pool(world), id).await;
        world.tm.projects.insert(
            name.clone(),
            tm::ProjectRef {
                team_slug: tr.slug.clone(),
                slug: slug.clone(),
            },
        );
        world.pl.projects.insert(
            name.clone(),
            PlProject {
                id,
                team: team.clone(),
                team_slug: tr.slug.clone(),
                slug: slug.clone(),
                key: key.clone(),
                name: name.clone(),
            },
        );
    }
}

/// "X has N issues[, M comments and K attachment(s)]", filed through the
/// product by someone on the project's team; comments and attachment go on
/// issue 2 (or 1 when there is one issue).
pub async fn given_work(
    world: &mut FoundryWorld,
    name: &str,
    issues: usize,
    comments: usize,
    attachments: usize,
) {
    let p = project(world, name);
    let author = author_of(world, &p).await;
    for i in 1..=issues {
        let answer = file_issue(world, &author, &p, &format!("{} issue {i}", p.key)).await;
        require_written(&answer, &format!("filing {}-{i}", p.key));
    }
    let n = commented_issue(issues);
    for c in 1..=comments {
        let answer = comment(
            world,
            &author,
            &p,
            n,
            &format!("Comment {c} on {}-{n}", p.key),
        )
        .await;
        require_written(&answer, "a comment");
    }
    for _ in 0..attachments {
        let path = format!("{}/issues/{n}/attachments", project_path(&p));
        let answer = send(world, &author, reqwest::Method::POST, &path, Body::File).await;
        require_written(&answer, "an attachment");
    }
}

/// "one more comment on SPIKE-2 was deleted and awaits cleanup".
pub async fn given_comment_awaiting_cleanup(world: &mut FoundryWorld, issue_key: &str) {
    let (p, n) = issue_of(world, issue_key);
    let author = author_of(world, &p).await;
    let answer = comment(world, &author, &p, n, "A comment soon deleted").await;
    require_written(&answer, "a comment");
    let id = latest_comment(world, &author, &p, n).await;
    let path = format!("{}/issues/{n}/comments/{id}", project_path(&p));
    let answer = send(world, &author, reqwest::Method::DELETE, &path, Body::None).await;
    require_written(&answer, "deleting the comment");
    world.pl.awaiting_cleanup = Some(id);
}

/// "<who> has commented on SPIKE-2 and on AUTH-1" — the actor's own comments,
/// which the comment doors name.
pub async fn given_has_commented(world: &mut FoundryWorld, who: &str, keys: &[&str]) {
    let actor = tm::resolve_actor(world, who);
    for key in keys {
        let (p, n) = issue_of(world, key);
        let answer = comment(world, &actor, &p, n, &format!("{who} on {key}")).await;
        require_written(&answer, &format!("{who}'s comment on {key}"));
        let id = latest_comment(world, &actor, &p, n).await;
        world
            .pl
            .comments
            .insert((tm::actor_key(&actor), (*key).to_string()), id);
    }
}

/// A product delete, from the project's row on its team's members page:
/// follow **Delete project**, then confirm. The holdings just before are noted.
async fn delete_through_product(world: &mut FoundryWorld, actor: &Actor, p: &PlProject) -> Answer {
    let held = holdings_of(world, p.id).await;
    if let (Some(facts), holdings) = held {
        world.pl.before_delete.insert(p.id, (facts, holdings));
    }
    let page = follow_delete_link(world, actor, p).await;
    confirm_on(world, actor, &page).await
}

pub async fn given_has_deleted(world: &mut FoundryWorld, who: &str, name: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let shown = current_name(world, &p).await;
    let answer = delete_through_product(world, &actor, &p).await;
    assert!(
        answer.status == StatusCode::OK && tm::says(&answer, &format!("Deleted {shown}.")),
        "precondition: {who} deleted {name} through the product; {}",
        tm::describe(&answer)
    );
    tm::record(world, &actor, answer);
}

pub async fn given_opened_delete_page_earlier(world: &mut FoundryWorld, who: &str, name: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let page = follow_delete_link(world, &actor, &p).await;
    tl::require_status(
        &page,
        StatusCode::OK,
        "precondition: the delete page opened",
    );
    world
        .pl
        .earlier
        .insert(format!("{}|delete", tm::actor_key(&actor)), page);
}

pub async fn given_opened_edit_form_earlier(world: &mut FoundryWorld, who: &str, key: &str) {
    let actor = tm::resolve_actor(world, who);
    let (p, n) = issue_of(world, key);
    let page = tm::open(
        world,
        &actor,
        &format!("{}/issues/{n}/edit", project_path(&p)),
    )
    .await;
    tl::require_status(&page, StatusCode::OK, "precondition: the edit form opened");
    world
        .pl
        .earlier
        .insert(format!("{}|edit", tm::actor_key(&actor)), page);
}

pub async fn given_opened_instance_dashboard_earlier(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = tm::open(world, &actor, "/admin/instance/workspaces").await;
    tl::require_status(
        &page,
        StatusCode::OK,
        "precondition: the instance dashboard opened",
    );
    world
        .pl
        .earlier
        .insert(format!("{}|instance", tm::actor_key(&actor)), page);
}

/// A machine token of the person's for the team (registered, so it is live),
/// plus a spare registered token the revoke door can name.
pub async fn given_script_token(world: &mut FoundryWorld, who: &str, team: &str) {
    let person = tm::resolve_person(world, who);
    let tr = tm::team_ref(world, team);
    let ws = tm::workspace_id(world, &tr.workspace);
    let store = tm::harness(world).app.state.store.clone();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(3600);
    let jti = uuid::Uuid::now_v7();
    let spare = uuid::Uuid::now_v7();
    for (id, label) in [(jti, "script"), (spare, "spare")] {
        store
            .insert_machine_token(id, person.id, ws, Some(tr.id), exp, label, person.id)
            .await
            .expect("register a machine token");
    }
    let claims = foundry_auth::MachineTokenClaims {
        sub: person.id,
        scope: Some(tr.id),
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let bearer = foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint a machine token")
        .expose_secret()
        .to_string();
    world.pl.script = Some((bearer, spare));
}

/// "Sam is watching X's live updates, and has heard of an issue Lena filed".
pub async fn given_watching(world: &mut FoundryWorld, who: &str, name: &str, filer: &str) {
    let watcher = tm::resolve_actor(world, who);
    let p = project(world, name);
    let cookie = tm::cookie_for(world, &watcher).await;
    let base = tm::harness(world).base_url();
    let sub =
        crate::support::sse_client::open_sse_subscription(&base, &p.slug, &p.team_slug, &cookie)
            .await;
    assert_eq!(
        sub.open_status,
        StatusCode::OK,
        "precondition: {who} opened {name}'s live updates"
    );
    assert!(
        sub.wait_until_ready(Duration::from_secs(5)).await,
        "precondition: the live view is ready"
    );
    let started = std::time::Instant::now();
    let filer = tm::resolve_actor(world, filer);
    let answer = file_issue(world, &filer, &p, "Heard live").await;
    require_written(&answer, "the live control's issue");
    let heard = sub
        .wait_for(Duration::from_secs(5), started, |_| true)
        .await;
    assert!(
        heard.is_some(),
        "precondition: {who}'s live view heard of the issue filed while {name} was live"
    );
    world.pl.heard = sub.drain().len();
    world.pl.watch = Some(sub);
}

pub async fn given_operator_restored(world: &mut FoundryWorld, target: &str) {
    let id = resolve(world, target);
    let run = cli(world, &["doctor", "restore-project", &id], true).await;
    assert_eq!(
        run.code, 0,
        "precondition: the operator restored {target}; {run:?}"
    );
}

/// "Sam's Board link opens Auth v2 Spike" (a precondition here, an outcome in a Then).
pub async fn board_link_opens(world: &mut FoundryWorld, who: &str, name: &str) {
    let actor = tm::resolve_actor(world, who);
    let dash = tm::open(world, &actor, "/").await;
    tl::require_status(&dash, StatusCode::OK, "the dashboard renders");
    let href = tm::link_to(&dash, "Board");
    let p = project(world, name);
    assert_eq!(
        href,
        project_path(&p),
        "{who}'s Board link opens the workspace's first project board, {name}; {}",
        tm::describe(&dash)
    );
}

// ===========================================================================
// Doors on the members page and the confirm page
// ===========================================================================

async fn members_page(world: &mut FoundryWorld, actor: &Actor, team: &str) -> Answer {
    let slug = tm::team_ref(world, team).slug;
    tm::open(world, actor, &members_path(&slug)).await
}

async fn follow_delete_link(world: &mut FoundryWorld, actor: &Actor, p: &PlProject) -> Answer {
    let page = members_page(world, actor, &p.team).await;
    let name = current_name(world, p).await;
    let row = row_answer(&page, row_of(&page, &name, &p.key));
    let href = tm::link_to(&row, "Delete project");
    assert_eq!(
        href,
        delete_path(&p.team_slug, &p.slug),
        "the row's Delete project link opens the project's delete page (DDD-23)"
    );
    tm::open(world, actor, &href).await
}

async fn confirm_on(world: &mut FoundryWorld, actor: &Actor, page: &Answer) -> Answer {
    let form = tm::forms(&page.body)
        .into_iter()
        .find(|f| f.action.starts_with("/team/") && f.action.ends_with("/delete"))
        .unwrap_or_else(|| {
            panic!(
                "the page offers no Delete project form; {}",
                tm::describe(page)
            )
        });
    tm::post(world, actor, &form.action, &form.fields, true).await
}

async fn rename_on(
    world: &mut FoundryWorld,
    actor: &Actor,
    page: &Answer,
    p: &PlProject,
    typed: Option<&str>,
) -> Answer {
    let name = current_name(world, p).await;
    let row = row_answer(page, row_of(page, &name, &p.key));
    let action = rename_path(&p.team_slug, &p.slug);
    let form = tm::form_at(&row, &action);
    let fields: Vec<(String, String)> = match typed {
        Some(typed) => {
            let mut f: Vec<(String, String)> = form
                .fields
                .into_iter()
                .filter(|(k, _)| k != "name")
                .collect();
            f.push(("name".to_string(), typed.to_string()));
            f
        }
        None => form.fields,
    };
    tm::post(world, actor, &action, &fields, true).await
}

// ===========================================================================
// Whens
// ===========================================================================

pub async fn when_renames(
    world: &mut FoundryWorld,
    who: &str,
    name: &str,
    raw: &str,
    earlier: bool,
    team: Option<&str>,
) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    require_team(&p, team);
    let typed = expand(raw);
    let page = if earlier {
        world
            .tm
            .earlier
            .get(&tm::actor_key(&actor))
            .cloned()
            .expect("a Given opened the members page earlier")
    } else {
        members_page(world, &actor, &p.team).await
    };
    capture_before(world).await;
    let answer = rename_on(world, &actor, &page, &p, Some(&typed)).await;
    if answer.status == StatusCode::OK {
        register_alias(world, name, typed.trim());
    }
    tm::record(world, &actor, answer);
}

/// The Gherkin names the members page of the project's own team.
fn require_team(p: &PlProject, team: Option<&str>) {
    if let Some(team) = team {
        assert_eq!(p.team, team, "{} belongs to {}, not {team}", p.name, p.team);
    }
}

fn register_alias(world: &mut FoundryWorld, handle: &str, new_name: &str) {
    let p = project(world, handle);
    world.pl.projects.insert(new_name.to_string(), p);
}

pub async fn when_submits_rename_unchanged(
    world: &mut FoundryWorld,
    who: &str,
    name: &str,
    team: &str,
) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    require_team(&p, Some(team));
    let page = members_page(world, &actor, &p.team).await;
    capture_before(world).await;
    let answer = rename_on(world, &actor, &page, &p, None).await;
    tm::record(world, &actor, answer);
}

pub async fn when_submits_rename(
    world: &mut FoundryWorld,
    who: &str,
    target: &str,
    raw: &str,
    with_token: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let (team_slug, slug) = project_slugs(world, target);
    capture_before(world).await;
    let answer = tm::post(
        world,
        &actor,
        &rename_path(&team_slug, &slug),
        &[("name".to_string(), expand(raw))],
        with_token,
    )
    .await;
    tm::record(world, &actor, answer);
}

/// The instance dashboard's rename row (the shipped door, now recording).
pub async fn when_instance_renames(
    world: &mut FoundryWorld,
    who: &str,
    name: &str,
    raw: &str,
    earlier: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let page = if earlier {
        world
            .pl
            .earlier
            .get(&format!("{}|instance", tm::actor_key(&actor)))
            .cloned()
            .expect("a Given opened the instance dashboard earlier")
    } else {
        tm::open(world, &actor, "/admin/instance/workspaces").await
    };
    assert!(
        page.body.contains(&format!("data-project-id=\"{}\"", p.id)),
        "the instance dashboard {who} has open lists {name}; {}",
        tm::describe(&page)
    );
    capture_before(world).await;
    let answer = post_form(
        world,
        &actor,
        &format!("/admin/instance/projects/{}/rename", p.id),
        &[("name", &expand(raw))],
    )
    .await;
    if answer.status == StatusCode::OK {
        register_alias(world, name, expand(raw).trim());
    }
    tm::record(world, &actor, answer);
}

pub async fn when_follows_delete(world: &mut FoundryWorld, who: &str, name: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    require_team(&p, Some(team));
    capture_before(world).await;
    let page = follow_delete_link(world, &actor, &p).await;
    tm::record(world, &actor, page);
}

pub async fn when_confirms(world: &mut FoundryWorld, who: &str, earlier: bool) {
    let actor = tm::resolve_actor(world, who);
    let page = if earlier {
        world
            .pl
            .earlier
            .get(&format!("{}|delete", tm::actor_key(&actor)))
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
    capture_before(world).await;
    let answer = confirm_on(world, &actor, &page).await;
    tm::record(world, &actor, answer);
}

pub async fn when_deletes(world: &mut FoundryWorld, who: &str, name: &str, team: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    require_team(&p, Some(team));
    capture_before(world).await;
    let answer = delete_through_product(world, &actor, &p).await;
    tm::record(world, &actor, answer);
}

pub async fn when_asks_delete_page(world: &mut FoundryWorld, who: &str, target: &str) {
    let actor = tm::resolve_actor(world, who);
    let (team_slug, slug) = project_slugs(world, target);
    capture_before(world).await;
    let answer = tm::open(world, &actor, &delete_path(&team_slug, &slug)).await;
    tm::record(world, &actor, answer);
}

pub async fn when_submits_delete(
    world: &mut FoundryWorld,
    who: &str,
    target: &str,
    with_token: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let (team_slug, slug) = project_slugs(world, target);
    capture_before(world).await;
    let answer = tm::post(
        world,
        &actor,
        &delete_path(&team_slug, &slug),
        &[],
        with_token,
    )
    .await;
    tm::record(world, &actor, answer);
}

/// The team delete confirm, with this feature's before-image (team-lifecycle's door).
pub async fn when_confirms_team_delete(world: &mut FoundryWorld, who: &str, earlier: bool) {
    capture_before(world).await;
    tl::when_confirms(world, who, earlier).await;
}

/// "Lena cancels the delete page she opened earlier": follow its Cancel link.
pub async fn when_cancels(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = world
        .pl
        .earlier
        .get(&format!("{}|delete", tm::actor_key(&actor)))
        .cloned()
        .expect("a Given opened the delete page earlier");
    capture_before(world).await;
    let href = tm::link_to(&page, "Cancel");
    let answer = tm::open(world, &actor, &href).await;
    tm::record(world, &actor, answer);
}

/// "Sam opens his dashboard".
pub async fn when_opens_dashboard(world: &mut FoundryWorld, who: &str) {
    let actor = tm::resolve_actor(world, who);
    capture_before(world).await;
    let answer = tm::open(world, &actor, "/").await;
    tm::record(world, &actor, answer);
}

/// "Lena tries to file an issue on X" / "Lena tries to comment on SPIKE-2".
pub async fn when_tries_one_write(world: &mut FoundryWorld, who: &str, write: &str) {
    let actor = tm::resolve_actor(world, who);
    capture_before(world).await;
    let answer = if let Some(name) = write.strip_prefix("file an issue on ") {
        let p = project(world, name);
        file_issue(world, &actor, &p, "Filed after the delete").await
    } else if let Some(key) = write.strip_prefix("comment on ") {
        let (q, n) = issue_of(world, key);
        comment(world, &actor, &q, n, "Commented after the delete").await
    } else {
        panic!("{write:?} is not a write this step knows")
    };
    tm::record(world, &actor, answer);
}

pub async fn when_saves_title_from_earlier(world: &mut FoundryWorld, who: &str, title: &str) {
    let actor = tm::resolve_actor(world, who);
    let page = world
        .pl
        .earlier
        .get(&format!("{}|edit", tm::actor_key(&actor)))
        .cloned()
        .expect("a Given opened the edit form earlier");
    let f = tm::forms(&page.body)
        .into_iter()
        .find(|f| f.action.ends_with("/edit"))
        .unwrap_or_else(|| panic!("the edit form posts to …/edit; {}", tm::describe(&page)));
    let mut fields: Vec<(String, String)> =
        f.fields.into_iter().filter(|(k, _)| k != "title").collect();
    fields.push(("title".to_string(), title.to_string()));
    if !fields.iter().any(|(k, _)| k == "state") {
        fields.push(("state".to_string(), "backlog".to_string()));
    }
    capture_before(world).await;
    let answer = tm::post(world, &actor, &f.action, &fields, true).await;
    tm::record(world, &actor, answer);
}

/// "Lena tries to file an issue on X and to comment on SPIKE-2".
pub async fn when_tries_to_write(world: &mut FoundryWorld, who: &str, name: &str, key: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let (q, n) = issue_of(world, key);
    capture_before(world).await;
    let filed = file_issue(world, &actor, &p, "Filed after the delete").await;
    let commented = comment(world, &actor, &q, n, "Commented after the delete").await;
    world.pl.answers = vec![filed, commented];
}

pub async fn when_each_downloads(world: &mut FoundryWorld, who: &[&str], name: &str) {
    capture_before(world).await;
    let p = project(world, name);
    let n = 2;
    let id = attachment_of(world, &p, n).await;
    world.pl.answers.clear();
    for w in who {
        let actor = tm::resolve_actor(world, w);
        let a = tm::open(
            world,
            &actor,
            &format!("{}/issues/{n}/attachments/{id}", project_path(&p)),
        )
        .await;
        world.pl.answers.push(a);
    }
}

// ------------------------------------------------------------------ the page sweep (PL-KPI-5)

/// The read-only doors of a project, every one a GET.
const PAGES: [Door; 15] = [
    Door::Board,
    Door::Report,
    Door::ReportSpreadsheet,
    Door::NewIssueForm,
    Door::Search,
    Door::IssuePage,
    Door::IssueEditForm,
    Door::IssueDeletePage,
    Door::CommentPage,
    Door::CommentEditForm,
    Door::DownloadAttachment,
    Door::LiveUpdates,
    Door::LaneDeleteDialog,
    Door::LaneEditDialog,
    Door::LaneInsertDialog,
];

/// Every page of the project, at its stored address, on its first commented issue.
async fn sweep_pages(world: &mut FoundryWorld, who: &str, name: &str) -> Vec<(String, StatusCode)> {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    // The sweeper's own comment when there is one (a comment's edit form is
    // its author's), else the first comment of the first commented issue.
    let row: Option<(i32, uuid::Uuid)> = sqlx::query_as(
        "SELECT i.number, c.id FROM comments c JOIN issues i ON i.id = c.issue_id
           JOIN users u ON u.id = c.author_id
          WHERE i.project_id = $1
          ORDER BY (u.email_lower = $2) DESC, i.number, c.created_at LIMIT 1",
    )
    .bind(p.id)
    .bind(tm::actor_key(&actor))
    .fetch_optional(&tm::pool(world))
    .await
    .expect("find a commented issue");
    let (n, c) = row.unwrap_or_else(|| panic!("precondition: {name} has a commented issue"));
    let attachment = attachment_of(world, &p, i64::from(n)).await;
    let at = Address {
        team_slug: p.team_slug.clone(),
        project_slug: p.slug.clone(),
        issue: i64::from(n),
        comment: c,
        attachment,
    };
    let mut out = Vec::new();
    for door in PAGES {
        let a = knock(world, &actor, door, &at).await;
        out.push((format!("{door:?}"), a.status));
    }
    out
}

pub async fn given_noted_pages(world: &mut FoundryWorld, who: &str, name: &str) {
    let noted = sweep_pages(world, who, name).await;
    // Someone else's comment's edit form is refused to them (403) as shipped;
    // it is still compared before and after, but not required to open.
    assert!(
        noted
            .iter()
            .all(|(door, s)| s.is_success() || door == "CommentEditForm"),
        "precondition: every page of {name} opens for {who}; {noted:?}"
    );
    world.pl.noted.insert(name.to_string(), noted);
}

pub async fn then_pages_as_before(world: &mut FoundryWorld, who: &str, name: &str) {
    let before = world
        .pl
        .noted
        .get(name)
        .cloned()
        .expect("a Given noted how every page answered");
    let now = sweep_pages(world, who, name).await;
    assert_eq!(
        now, before,
        "PL-KPI-5: every page of {name} answers {who} as it did before"
    );
}

// ------------------------------------------------------------------ the differential

fn address_in(
    world: &FoundryWorld,
    actor: &Actor,
    p: &PlProject,
    n: i64,
    attachment: uuid::Uuid,
) -> Address {
    let key = format!("{}-{n}", p.key);
    Address {
        team_slug: p.team_slug.clone(),
        project_slug: p.slug.clone(),
        issue: n,
        comment: world
            .pl
            .comments
            .get(&(tm::actor_key(actor), key.clone()))
            .copied()
            .or_else(|| {
                world
                    .pl
                    .comments
                    .iter()
                    .find(|((_, k), _)| *k == key)
                    .map(|(_, id)| *id)
            })
            .unwrap_or_else(|| panic!("precondition: someone has commented on {key}")),
        attachment,
    }
}

async fn knock(world: &mut FoundryWorld, actor: &Actor, door: Door, at: &Address) -> Answer {
    use reqwest::Method as M;
    let p = format!("/team/{}/project/{}", at.team_slug, at.project_slug);
    let n = at.issue;
    let c = at.comment;
    let lane = format!("{p}/lanes/{LANE}");
    let (method, path, body) = match door {
        Door::Board => (M::GET, p.clone(), Body::None),
        Door::Report => (M::GET, format!("{p}/report"), Body::None),
        Door::ReportSpreadsheet => (M::GET, format!("{p}/report?format=csv"), Body::None),
        Door::FileIssue => (
            M::POST,
            format!("{p}/issues"),
            form(&[("title", "Knocking"), ("description", "")]),
        ),
        Door::NewIssueForm => (M::GET, format!("{p}/issues/new"), Body::None),
        Door::Search => (M::GET, format!("{p}/search?q=issue"), Body::None),
        Door::MoveIssue => (
            M::POST,
            format!("{p}/issues/{n}/state"),
            form(&[("state", "todo")]),
        ),
        Door::IssuePage => (M::GET, format!("{p}/issues/{n}"), Body::None),
        Door::IssueEditForm => (M::GET, format!("{p}/issues/{n}/edit"), Body::None),
        Door::EditIssue => (
            M::POST,
            format!("{p}/issues/{n}/edit"),
            form(&[
                ("title", "Knocking"),
                ("description", ""),
                ("state", "backlog"),
            ]),
        ),
        Door::IssueDeletePage => (M::GET, format!("{p}/issues/{n}/delete"), Body::None),
        Door::DeleteIssue => (M::POST, format!("{p}/issues/{n}/delete"), form(&[])),
        Door::CommentOnIssue => (
            M::POST,
            format!("{p}/issues/{n}/comments"),
            form(&[("body", "Knocking")]),
        ),
        Door::CommentPage => (M::GET, format!("{p}/issues/{n}/comments/{c}"), Body::None),
        Door::CommentEditForm => (
            M::GET,
            format!("{p}/issues/{n}/comments/{c}/edit"),
            Body::None,
        ),
        Door::EditComment => (
            M::PATCH,
            format!("{p}/issues/{n}/comments/{c}"),
            form(&[("body_markdown", "Knocking")]),
        ),
        Door::DeleteComment => (
            M::DELETE,
            format!("{p}/issues/{n}/comments/{c}"),
            Body::None,
        ),
        Door::AttachFile => (M::POST, format!("{p}/issues/{n}/attachments"), Body::File),
        Door::DownloadAttachment => (
            M::GET,
            format!("{p}/issues/{n}/attachments/{}", at.attachment),
            Body::None,
        ),
        Door::LiveUpdates => (M::GET, format!("{p}/events"), Body::None),
        Door::LaneDeleteDialog => (M::GET, format!("{lane}/delete"), Body::None),
        Door::DeleteLane => (
            M::POST,
            format!("{lane}/delete"),
            form(&[("fate", "delete")]),
        ),
        Door::LaneEditDialog => (M::GET, format!("{lane}/edit"), Body::None),
        Door::RenameLane => (M::POST, format!("{lane}/edit"), form(&[("label", "To do")])),
        Door::LaneInsertDialog => (M::GET, format!("{lane}/insert/after"), Body::None),
        Door::InsertLane => (
            M::POST,
            format!("{lane}/insert/after"),
            form(&[("label", "Review")]),
        ),
        Door::MoveLane => (
            M::POST,
            format!("{lane}/move"),
            form(&[("before", "backlog")]),
        ),
        Door::RenameProject => (
            M::POST,
            format!("{p}/rename"),
            form(&[("name", "Knocking")]),
        ),
        Door::ProjectDeletePage => (M::GET, format!("{p}/delete"), Body::None),
        Door::DeleteProject => (M::POST, format!("{p}/delete"), form(&[])),
    };
    send(world, actor, method, &path, body).await
}

async fn knock_api(world: &mut FoundryWorld, door: ApiDoor, at: &Address) -> Answer {
    use reqwest::Method as M;
    let p = format!(
        "/api/v1/teams/{}/projects/{}",
        at.team_slug, at.project_slug
    );
    let n = at.issue;
    let spare = world.pl.script.as_ref().expect("a script token").1;
    let (method, path, body) = match door {
        ApiDoor::ListIssues => (M::GET, format!("{p}/issues"), Body::None),
        ApiDoor::FileIssue => (
            M::POST,
            format!("{p}/issues"),
            Body::Json(serde_json::json!({ "title": "Knocking" })),
        ),
        ApiDoor::MoveIssue => (
            M::PATCH,
            format!("{p}/issues/{n}"),
            Body::Json(serde_json::json!({ "state": "todo" })),
        ),
        ApiDoor::History => (M::GET, format!("{p}/issues/{n}/history"), Body::None),
        ApiDoor::CommentOnIssue => (
            M::POST,
            format!("{p}/issues/{n}/comments"),
            Body::Json(serde_json::json!({ "body": "Knocking" })),
        ),
        ApiDoor::EditComment => (
            M::PATCH,
            format!("{p}/issues/{n}/comments/{}", at.comment),
            Body::Json(serde_json::json!({ "body": "Knocking" })),
        ),
        ApiDoor::ListTokens => (M::GET, format!("{p}/tokens"), Body::None),
        ApiDoor::RevokeToken => (M::DELETE, format!("{p}/tokens/{spare}"), Body::None),
    };
    api(world, method, &path, body).await
}

/// The three addresses of one differential row: the deleted project, the same
/// door on a slug no project had, and the live control (Identity Platform).
async fn row_addresses(
    world: &FoundryWorld,
    actor: &Actor,
) -> (PlProject, Address, Address, Address) {
    let deleted = project(world, "Auth v2 Spike");
    let live = project(world, "Identity Platform");
    let spike_attachment = attachment_of(world, &deleted, 2).await;
    let live_attachment = attachment_of(world, &live, 1).await;
    let at_deleted = address_in(world, actor, &deleted, 2, spike_attachment);
    let at_never = Address {
        project_slug: UNKNOWN_PROJECT.to_string(),
        ..at_deleted.clone()
    };
    let at_live = address_in(world, actor, &live, 1, live_attachment);
    (deleted, at_deleted, at_never, at_live)
}

pub async fn when_tries_door(world: &mut FoundryWorld, who: &str, door: &str, name: &str) {
    assert_eq!(
        name, "Auth v2 Spike",
        "the differential rows name Auth v2 Spike"
    );
    let actor = tm::resolve_actor(world, who);
    let d = Door::parse(door);
    let (deleted, at_deleted, at_never, at_live) = row_addresses(world, &actor).await;
    let everything_before = everything(world).await;
    let held_before = holdings_of(world, deleted.id).await;
    let deleted_answer = knock(world, &actor, d, &at_deleted).await;
    let never_answer = knock(world, &actor, d, &at_never).await;
    let held_after = holdings_of(world, deleted.id).await;
    let everything_after = everything(world).await;
    let live_answer = knock(world, &actor, d, &at_live).await;
    world.pl.diff = Some(Differential {
        door: door.to_string(),
        deleted: deleted_answer,
        never_existed: never_answer,
        live: live_answer,
        held_before,
        held_after,
        everything_before,
        everything_after,
    });
}

pub async fn when_script_tries_door(world: &mut FoundryWorld, door: &str, name: &str) {
    assert_eq!(
        name, "Auth v2 Spike",
        "the differential rows name Auth v2 Spike"
    );
    let owner = Actor::Person(tm::resolve_person(world, "Priya").email);
    let d = ApiDoor::parse(door);
    let (deleted, at_deleted, at_never, at_live) = row_addresses(world, &owner).await;
    let everything_before = everything(world).await;
    let held_before = holdings_of(world, deleted.id).await;
    let deleted_answer = knock_api(world, d, &at_deleted).await;
    let never_answer = knock_api(world, d, &at_never).await;
    let held_after = holdings_of(world, deleted.id).await;
    let everything_after = everything(world).await;
    let live_answer = knock_api(world, d, &at_live).await;
    world.pl.diff = Some(Differential {
        door: door.to_string(),
        deleted: deleted_answer,
        never_existed: never_answer,
        live: live_answer,
        held_before,
        held_after,
        everything_before,
        everything_after,
    });
}

// ------------------------------------------------------------------ the operator

async fn cli(world: &mut FoundryWorld, args: &[&str], reachable: bool) -> CliRun {
    let database_url = if reachable {
        let base = crate::support::harness::ensure_postgres().await;
        let schema = tm::harness(world).schema.clone();
        format!("{base}?options=-csearch_path%3D{schema}")
    } else {
        UNREACHABLE_DATABASE.to_string()
    };
    let owned: Vec<String> = args.iter().map(|a| (*a).to_string()).collect();
    let run_args = owned.clone();
    let output = tokio::task::spawn_blocking(move || {
        assert_cmd::Command::cargo_bin("foundry")
            .expect("the prebuilt foundry binary (rebuild it before the lane)")
            .env("DATABASE_URL", database_url)
            .args(&run_args)
            .output()
            .expect("run foundry doctor")
    })
    .await
    .expect("join the operator's command");
    let run = CliRun {
        args: owned,
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    };
    world.pl.cli = Some(run.clone());
    run
}

pub async fn when_operator_lists(world: &mut FoundryWorld) {
    capture_before(world).await;
    cli(world, &["doctor", "list-deleted-projects"], true).await;
}

pub async fn when_operator_restores(world: &mut FoundryWorld, target: &str, reachable: bool) {
    let id = resolve(world, target);
    capture_before(world).await;
    cli(world, &["doctor", "restore-project", &id], reachable).await;
}

pub async fn when_operator_restores_comment(world: &mut FoundryWorld) {
    let id = world
        .pl
        .awaiting_cleanup
        .expect("a comment was deleted and awaits cleanup")
        .to_string();
    capture_before(world).await;
    cli(world, &["doctor", "restore-comment", &id], true).await;
}

// ------------------------------------------------------------------ races

async fn seed_raw_project(
    world: &mut FoundryWorld,
    team: &str,
    name: &str,
    key: &str,
    slug: &str,
) -> PlProject {
    let tr = tm::team_ref(world, team);
    let ws = tm::workspace_id(world, &tr.workspace);
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(tr.id)
    .bind(ws)
    .bind(name)
    .bind(slug)
    .bind(key)
    .execute(&tm::pool(world))
    .await
    .expect("seed a race project");
    crate::support::harness::seed_lanes_for_project(&tm::pool(world), id).await;
    let p = PlProject {
        id,
        team: team.to_string(),
        team_slug: tr.slug,
        slug: slug.to_string(),
        key: key.to_string(),
        name: name.to_string(),
    };
    world.pl.projects.insert(name.to_string(), p.clone());
    p
}

async fn issues_in(world: &FoundryWorld, id: uuid::Uuid) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM issues WHERE project_id = $1")
        .bind(id)
        .fetch_one(&tm::pool(world))
        .await
        .expect("count the project's issues");
    n
}

fn letter(i: usize) -> char {
    (b'A' + i as u8) as char
}

/// "Backend has 5 fresh projects, Race 1 to Race 5" (seeded preconditions).
pub async fn given_race_projects(world: &mut FoundryWorld, team: &str, times: usize) {
    world.pl.race_projects.clear();
    for i in 0..times {
        let p = seed_raw_project(
            world,
            team,
            &format!("Race {}", i + 1),
            &format!("RACE{}", letter(i)),
            &format!("race-{}", i + 1),
        )
        .await;
        world.pl.race_projects.push(p);
    }
}

/// "Lena deletes each fresh project at the same moment as Sam files an issue on it".
pub async fn when_delete_races_file(world: &mut FoundryWorld, deleter: &str, filer: &str) {
    let deleter = tm::resolve_actor(world, deleter);
    let filer = tm::resolve_actor(world, filer);
    // Sign both in before racing.
    tm::cookie_for(world, &deleter).await;
    tm::cookie_for(world, &filer).await;
    world.pl.delete_race.clear();
    let projects = world.pl.race_projects.clone();
    assert!(!projects.is_empty(), "a Given seeded the race's projects");
    for p in projects {
        let delete_req =
            race_request(world, &deleter, &delete_path(&p.team_slug, &p.slug), &[]).await;
        let file_req = race_request(
            world,
            &filer,
            &format!("{}/issues", project_path(&p)),
            &[("title", "Filed during the delete"), ("description", "")],
        )
        .await;
        let (delete, create) = tokio::join!(delete_req.send(), file_req.send());
        let delete = tm::into_answer(
            &delete_path(&p.team_slug, &p.slug),
            delete.expect("the delete is answered"),
        )
        .await;
        let create = tm::into_answer(
            &format!("{}/issues", project_path(&p)),
            create.expect("the filing is answered"),
        )
        .await;
        let board_for_sam = tm::open(world, &filer, &project_path(&p)).await;
        let issues_kept = issues_in(world, p.id).await;
        world.pl.delete_race.push(DeleteRace {
            project: p.name.clone(),
            delete,
            create,
            board_for_sam,
            issues_kept,
        });
    }
}

async fn race_request(
    world: &mut FoundryWorld,
    actor: &Actor,
    path: &str,
    fields: &[(&str, &str)],
) -> reqwest::RequestBuilder {
    let cookie = tm::cookie_for(world, actor).await;
    let base = tm::harness(world).base_url();
    let mut form: Vec<(String, String)> = fields
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    form.push(("_csrf".to_string(), world.tm.csrf.clone()));
    tm::http(world)
        .post(format!("{base}{path}"))
        .header(COOKIE, cookie)
        .form(&form)
}

/// "5 fresh teams each hold one project with one issue, which Priya has deleted"
/// (preconditions: the teams and projects seeded, the issue filed and the
/// project deleted through the product).
pub async fn given_restore_race_teams(world: &mut FoundryWorld, times: usize, admin: &str) {
    let admin_actor = tm::resolve_actor(world, admin);
    let admin_email = tm::actor_key(&admin_actor);
    world.pl.race_projects.clear();
    for i in 0..times {
        let team_name = format!("Lab{}", letter(i));
        let ws = tm::workspace_id(world, tm::HOME);
        let team_id = uuid::Uuid::now_v7();
        let team_slug = team_name.to_ascii_lowercase();
        sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
            .bind(team_id)
            .bind(ws)
            .bind(&team_name)
            .bind(&team_slug)
            .execute(&tm::pool(world))
            .await
            .expect("seed a race team");
        world.tm.teams.insert(
            team_name.clone(),
            tm::TeamRef {
                id: team_id,
                workspace: tm::HOME.to_string(),
                name: team_name.clone(),
                slug: team_slug.clone(),
            },
        );
        tm::seed_team_row(world, &team_name, &admin_email, tm::TeamRole::Member).await;
        let name = format!("Graveyard {}", letter(i));
        let p = seed_raw_project(
            world,
            &team_name,
            &name,
            &format!("GRAV{}", letter(i)),
            &format!("graveyard-{}", letter(i).to_ascii_lowercase()),
        )
        .await;
        let filed = file_issue(world, &admin_actor, &p, "Kept through the race").await;
        require_written(&filed, "the race project's issue");
        let deleted = delete_through_product(world, &admin_actor, &p).await;
        assert!(
            deleted.status == StatusCode::OK && tm::says(&deleted, &format!("Deleted {name}.")),
            "precondition: {admin} deleted {name} through the product; {}",
            tm::describe(&deleted)
        );
        world.pl.race_projects.push(p);
    }
}

/// "the operator restores each of those projects at the same moment as Priya
/// deletes its team".
pub async fn when_restore_races_team_delete(world: &mut FoundryWorld, admin: &str) {
    let admin_actor = tm::resolve_actor(world, admin);
    world.pl.restore_race.clear();
    let projects = world.pl.race_projects.clone();
    assert!(!projects.is_empty(), "a Given seeded the race's teams");
    for p in projects {
        let base = crate::support::harness::ensure_postgres().await;
        let schema = tm::harness(world).schema.clone();
        let database_url = format!("{base}?options=-csearch_path%3D{schema}");
        let id = p.id.to_string();
        let restore = tokio::task::spawn_blocking(move || {
            assert_cmd::Command::cargo_bin("foundry")
                .expect("the prebuilt foundry binary")
                .env("DATABASE_URL", database_url)
                .args(["doctor", "restore-project", &id])
                .output()
                .expect("run foundry doctor restore-project")
        });
        let team_path = format!("/team/{}/delete", p.team_slug);
        let team_delete = race_request(world, &admin_actor, &team_path, &[]).await;
        let (restore, team_delete) = tokio::join!(restore, team_delete.send());
        let restore = restore.expect("join the operator's command");
        let restore = CliRun {
            args: vec!["doctor".into(), "restore-project".into(), p.id.to_string()],
            code: restore.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&restore.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&restore.stderr).into_owned(),
        };
        let team_delete = tm::into_answer(
            &team_path,
            team_delete.expect("the team delete is answered"),
        )
        .await;
        let (kept,): (bool,) =
            sqlx::query_as("SELECT EXISTS (SELECT 1 FROM projects WHERE id = $1)")
                .bind(p.id)
                .fetch_one(&tm::pool(world))
                .await
                .expect("look the project row up");
        let issues_kept = issues_in(world, p.id).await;
        world.pl.restore_race.push(RestoreRace {
            project: p.name.clone(),
            key: p.key.clone(),
            restore,
            team_delete,
            project_row_kept: kept,
            issues_kept,
        });
    }
}

// ===========================================================================
// Thens
// ===========================================================================

pub async fn then_renamed(
    world: &FoundryWorld,
    handle: &str,
    raw_new: &str,
    address: &str,
    actor: &str,
) {
    let p = project(world, handle);
    let new_name = expand(raw_new);
    assert_eq!(
        p.slug, address,
        "the scenario names the project's stored address"
    );
    let before = world
        .pl
        .before
        .clone()
        .expect("the When captured the projects");
    let old = before.facts[&p.id].clone();
    let actor = tm::resolve_person(world, actor).email;
    assert_state_delta(
        world,
        &[
            Change::Renamed {
                project: handle.to_string(),
                new_name: new_name.clone(),
            },
            Change::Recorded(ProjectRecord {
                workspace: old.workspace,
                kind: RecordKind::Renamed,
                project_slug: old.slug,
                key: old.key,
                name: old.name,
                new_name: Some(new_name),
                actor: Some(actor),
            }),
        ],
    )
    .await;
}

pub async fn then_deleted(world: &FoundryWorld, handle: &str, by: &str, recorded_by: &str) {
    let p = project(world, handle);
    let before = world
        .pl
        .before
        .clone()
        .expect("the When captured the projects");
    let old = before.facts[&p.id].clone();
    let by = tm::resolve_person(world, by).email;
    let recorded_by = tm::resolve_person(world, recorded_by).email;
    assert_state_delta(
        world,
        &[
            Change::Deleted {
                project: handle.to_string(),
                by,
            },
            Change::Recorded(ProjectRecord {
                workspace: old.workspace,
                kind: RecordKind::Deleted,
                project_slug: old.slug,
                key: old.key,
                name: old.name,
                new_name: None,
                actor: Some(recorded_by),
            }),
        ],
    )
    .await;
}

pub async fn then_restored(world: &FoundryWorld, handle: &str) {
    let p = project(world, handle);
    let before = world
        .pl
        .before
        .clone()
        .expect("the When captured the projects");
    let old = before.facts[&p.id].clone();
    assert_state_delta(
        world,
        &[
            Change::Restored {
                project: handle.to_string(),
            },
            Change::Recorded(ProjectRecord {
                workspace: old.workspace,
                kind: RecordKind::Restored,
                project_slug: old.slug,
                key: old.key,
                name: old.name,
                new_name: None,
                actor: None,
            }),
        ],
    )
    .await;
}

pub async fn then_team_deleted_project_kept(
    world: &FoundryWorld,
    team: &str,
    actor: &str,
    handle: &str,
    issues: i64,
) {
    let tr = tm::team_ref(world, team);
    let email = tm::resolve_person(world, actor).email;
    let p = project(world, handle);
    let before = world
        .pl
        .before
        .clone()
        .expect("the When captured the projects");
    assert_eq!(
        before.holdings.get(&p.id).map(|h| h.issues.0),
        Some(issues),
        "precondition: {handle} held {issues} issues"
    );
    assert_state_delta(
        world,
        &[
            Change::Team(tl::Change::Deleted {
                team: team.to_string(),
            }),
            Change::Team(tl::Change::Recorded(tl::Record {
                workspace: tr.workspace.clone(),
                kind: tl::RecordKind::Deleted,
                team_slug: tr.slug.clone(),
                name: tr.name.clone(),
                new_name: None,
                actor: email,
            })),
            Change::Detached {
                project: handle.to_string(),
            },
        ],
    )
    .await;
}

pub async fn then_nothing_changed(world: &FoundryWorld) {
    assert_state_delta(world, &[]).await;
}

/// "the store still holds X's 3 issues, 3 comments of which 1 awaits cleanup,
/// 1 attachment, 4 lanes, their history and their queued notifications, unchanged".
pub async fn then_holdings_unchanged(
    world: &FoundryWorld,
    handle: &str,
    issues: i64,
    comments: i64,
    awaiting: i64,
    attachments: i64,
    lanes: i64,
) {
    let p = project(world, handle);
    let before = world
        .pl
        .before
        .clone()
        .expect("the When captured the projects");
    let (_, now) = holdings_of(world, p.id).await;
    assert_eq!(
        now,
        before.holdings.get(&p.id).cloned().unwrap_or_default(),
        "INV-PL-1: nothing under {handle} may change"
    );
    assert_eq!(now.issues.0, issues, "{handle}'s issues");
    assert_eq!(
        now.comments_live + now.comments_awaiting_cleanup,
        comments,
        "{handle}'s comments"
    );
    assert_eq!(
        now.comments_awaiting_cleanup, awaiting,
        "{handle}'s comments awaiting cleanup"
    );
    assert_eq!(now.attachments.0, attachments, "{handle}'s attachments");
    assert_eq!(now.lanes.0, lanes, "{handle}'s lanes");
}

pub async fn then_next_issue(world: &FoundryWorld, handle: &str, key: &str) {
    let p = project(world, handle);
    let (facts, _) = holdings_of(world, p.id).await;
    let n: i64 = key
        .rsplit_once('-')
        .expect("a key")
        .1
        .parse()
        .expect("a number");
    assert_eq!(
        facts.expect("the project row").next_number,
        n,
        "{handle}'s next issue is {key}"
    );
}

pub async fn then_holds_as_before_delete(world: &FoundryWorld, handle: &str) {
    let p = project(world, handle);
    let (facts, holdings) = world
        .pl
        .before_delete
        .get(&p.id)
        .cloned()
        .expect("the holdings were noted just before the delete");
    let (now_facts, now_holdings) = holdings_of(world, p.id).await;
    assert_eq!(
        now_facts.as_ref(),
        Some(&facts),
        "INV-PL-4: {handle}'s row equals its state just before the delete"
    );
    assert_eq!(
        now_holdings, holdings,
        "INV-PL-4: everything under {handle} equals its state just before the delete"
    );
}

pub fn then_projects_section_lists(world: &FoundryWorld, team: &str, rows: &[String]) {
    let answer = tm::last(world);
    assert_eq!(
        tl::heading(answer).as_deref(),
        Some(team),
        "the answer is {team}'s members page; {}",
        tm::describe(answer)
    );
    let labels: Vec<String> = project_rows(answer)
        .into_iter()
        .map(|(text, _)| {
            rows.iter()
                .find(|r| text.starts_with(r.as_str()))
                .cloned()
                .unwrap_or(text)
        })
        .collect();
    assert_eq!(
        labels,
        rows,
        "{team}'s Projects section, by name then key (D8, DDD-25); {}",
        tm::describe(answer)
    );
}

pub async fn then_section_as_seen_by(
    world: &mut FoundryWorld,
    team: &str,
    who: &str,
    row: &str,
    only: bool,
) {
    let actor = tm::resolve_actor(world, who);
    let page = members_page(world, &actor, team).await;
    tl::require_status(&page, StatusCode::OK, "the members page renders");
    let rows: Vec<String> = project_rows(&page).into_iter().map(|(t, _)| t).collect();
    if only {
        assert!(
            rows.len() == 1 && rows[0].starts_with(row),
            "{team}'s Projects section lists only {row:?}; rows {rows:?}"
        );
    } else {
        assert!(
            rows.iter().any(|r| r.starts_with(row)),
            "{team}'s Projects section lists {row:?}; rows {rows:?}"
        );
    }
}

/// Every row of the last members page, as "{name} · {KEY}" handles.
fn rows_with_projects(world: &FoundryWorld, answer: &Answer) -> Vec<(PlProject, String)> {
    project_rows(answer)
        .into_iter()
        .map(|(text, html)| {
            let p = world
                .pl
                .projects
                .values()
                .find(|p| text.contains(&format!(" · {}", p.key)))
                .cloned()
                .unwrap_or_else(|| panic!("the row {text:?} names a seeded project"));
            (p, html)
        })
        .collect()
}

pub fn then_rows_link(world: &FoundryWorld, links: bool) {
    let answer = tm::last(world).clone();
    let rows = rows_with_projects(world, &answer);
    assert!(
        !rows.is_empty(),
        "the Projects section lists projects; {}",
        tm::describe(&answer)
    );
    for (p, html) in rows {
        let row = row_answer(&answer, html);
        let doc = parse(&row.body);
        let linked = select_all(&doc, "a[href]")
            .into_iter()
            .any(|a| a.value().attr("href") == Some(project_path(&p).as_str()));
        assert_eq!(
            linked,
            links,
            "{}'s row {} link to its board (OQ-U1 = (b): linked only when the viewer is on the team); {}",
            p.name,
            if links { "must" } else { "must not" },
            tm::describe(&row)
        );
    }
}

pub fn then_rows_offer_controls(world: &FoundryWorld, offers: bool) {
    let answer = tm::last(world).clone();
    let rows = rows_with_projects(world, &answer);
    assert!(
        !rows.is_empty(),
        "the Projects section lists projects; {}",
        tm::describe(&answer)
    );
    for (p, html) in rows {
        let row = row_answer(&answer, html);
        let rename = tl::has_form(&row, &rename_path(&p.team_slug, &p.slug))
            && tl::has_control(&row, "Rename");
        let delete = tl::has_link(&row, "Delete project");
        assert_eq!(
            (rename, delete),
            (offers, offers),
            "{}'s row must {} Project name, Rename and Delete project (D-B, D8); {}",
            p.name,
            if offers { "offer" } else { "offer no" },
            tm::describe(&row)
        );
        if offers {
            let values = tl::field_values(&row, "name");
            assert!(
                values.iter().any(|v| !v.trim().is_empty()),
                "the Project name field is prefilled with the project's name; {values:?}"
            );
        }
    }
}

/// NFR-PL-1: each field's accessible name carries "Project name" and the project.
pub fn then_fields_labelled(world: &FoundryWorld) {
    let answer = tm::last(world).clone();
    for (p, html) in rows_with_projects(world, &answer) {
        let doc = parse(&format!("<html><body>{html}</body></html>"));
        let input = select_all(&doc, "input[name=name]")
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("{}'s row has a Project name field", p.name));
        let aria = input.value().attr("aria-label").unwrap_or("").to_string();
        let label = select_all(&doc, "label")
            .into_iter()
            .map(|l| squash(&text_of(&l)))
            .collect::<Vec<_>>()
            .join(" ");
        let accessible = format!("{aria} {label}");
        assert!(
            accessible.contains("Project name") && accessible.contains(&p.name),
            "{}'s field must be labelled \"Project name\" with the project's name; got {accessible:?}",
            p.name
        );
    }
}

/// NFR-PL-1: per row, the field, then Rename, then Delete project, with no
/// positive tabindex reordering them.
pub fn then_row_tab_order(world: &FoundryWorld) {
    let answer = tm::last(world).clone();
    for (p, html) in rows_with_projects(world, &answer) {
        let field = html.find("name=\"name\"");
        let rename = html.find(">Rename<");
        let delete = html.find(">Delete project<");
        assert!(
            matches!((field, rename, delete), (Some(f), Some(r), Some(d)) if f < r && r < d),
            "{}'s row: the name field, then Rename, then Delete project, in document order; {html}",
            p.name
        );
        assert!(
            !html.contains("tabindex=\"1") && !html.contains("tabindex=\"2"),
            "{}'s row sets no positive tabindex; {html}",
            p.name
        );
    }
}

pub fn then_row_refusal(world: &FoundryWorld, copy: &str, handle: &str, raw: &str) {
    let answer = tm::last(world);
    tl::require_status(
        answer,
        StatusCode::UNPROCESSABLE_ENTITY,
        &format!("a name refusal is a 422 saying {copy:?} (DDD-24)"),
    );
    let p = project(world, handle);
    let rows = rows_with_projects(world, answer);
    let (_, html) = rows
        .into_iter()
        .find(|(q, _)| q.id == p.id)
        .unwrap_or_else(|| panic!("the refusal re-renders {handle}'s row"));
    let row = row_answer(answer, html);
    assert!(
        tm::page_text(&row.body).contains(copy),
        "the refusal {copy:?} stands in {handle}'s row; {}",
        tm::describe(&row)
    );
    let typed = expand(raw);
    assert!(
        tl::field_values(&row, "name").contains(&typed),
        "{handle}'s row keeps the typed text {typed:?}; {:?}",
        tl::field_values(&row, "name")
    );
}

pub fn then_sees_renamed(world: &FoundryWorld, old: &str, new: &str) {
    tm::then_sees(
        world,
        &format!("Renamed {} to {}.", expand(old), expand(new)),
    );
}

pub async fn then_still_open(world: &mut FoundryWorld, name: &str, issue_key: &str, who: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let board = tm::open(world, &actor, &project_path(&p)).await;
    tl::require_status(&board, StatusCode::OK, "the board opens at its old address");
    assert!(
        tm::says(&board, name),
        "the board shows the new name {name:?}; {}",
        tm::describe(&board)
    );
    let n: i64 = issue_key
        .rsplit_once('-')
        .expect("a key")
        .1
        .parse()
        .expect("n");
    let issue = tm::open(world, &actor, &format!("{}/issues/{n}", project_path(&p))).await;
    tl::require_status(&issue, StatusCode::OK, "the issue opens at its old address");
    assert!(
        tm::says(&issue, issue_key),
        "the issue is still {issue_key}; {}",
        tm::describe(&issue)
    );
}

pub async fn then_still_finds_rename_form(world: &mut FoundryWorld, who: &str, name: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let page = members_page(world, &actor, &p.team).await;
    let shown = current_name(world, &p).await;
    let row = row_answer(&page, row_of(&page, &shown, &p.key));
    assert!(
        tl::has_form(&row, &rename_path(&p.team_slug, &p.slug)),
        "control: {who} still finds {name}'s rename form, so the refusal hid a real project; {}",
        tm::describe(&row)
    );
}

pub async fn then_still_finds_delete_page(world: &mut FoundryWorld, who: &str, name: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let shown = current_name(world, &p).await;
    let page = follow_delete_link(world, &actor, &p).await;
    tl::require_status(&page, StatusCode::OK, "control: the delete page opens");
    assert_eq!(
        tl::heading(&page),
        Some(format!("Delete {shown}?")),
        "control: {who} still finds {name}'s delete page; {}",
        tm::describe(&page)
    );
}

pub fn then_confirm_offers(world: &FoundryWorld) {
    let answer = tm::last(world);
    tl::require_status(answer, StatusCode::OK, "the delete page renders");
    let (team_slug, project_slug) = answer
        .path
        .strip_prefix("/team/")
        .and_then(|r| r.strip_suffix("/delete"))
        .and_then(|r| r.split_once("/project/"))
        .map(|(t, p)| (t.to_string(), p.to_string()))
        .unwrap_or_else(|| {
            panic!(
                "the last page is a project's delete page, not {}",
                answer.path
            )
        });
    assert!(
        tl::has_form(answer, &delete_path(&team_slug, &project_slug))
            && tl::has_control(answer, "Delete project"),
        "the delete page offers Delete project (D7); {}",
        tm::describe(answer)
    );
    assert_eq!(
        tm::link_to(answer, "Cancel"),
        members_path(&team_slug),
        "Cancel is a link back to the members page (D14)"
    );
    let doc = parse(&answer.body);
    let typed: Vec<_> = select_all(&doc, "input")
        .into_iter()
        .filter(|i| !matches!(i.value().attr("type"), Some("hidden" | "submit")))
        .collect();
    assert!(
        typed.is_empty(),
        "the delete page asks for no typed name (OQ-5); {}",
        tm::describe(answer)
    );
}

pub async fn then_same_rename_with_token(world: &mut FoundryWorld, name: &str, raw: &str) {
    let who = world.tm.last_actor.clone().expect("a previous actor");
    let p = project(world, name);
    let answer = tm::post(
        world,
        &who,
        &rename_path(&p.team_slug, &p.slug),
        &[("name".to_string(), expand(raw))],
        true,
    )
    .await;
    assert!(
        answer.status == StatusCode::OK
            && tm::says(&answer, &format!("Renamed {} to {}.", p.name, expand(raw))),
        "control: the same rename with the page's token goes through; {}",
        tm::describe(&answer)
    );
}

pub async fn then_same_delete_with_token(world: &mut FoundryWorld, name: &str) {
    let who = world.tm.last_actor.clone().expect("a previous actor");
    let p = project(world, name);
    let answer = tm::post(world, &who, &delete_path(&p.team_slug, &p.slug), &[], true).await;
    assert!(
        answer.status == StatusCode::OK && tm::says(&answer, &format!("Deleted {}.", p.name)),
        "control: the same delete with the page's token goes through; {}",
        tm::describe(&answer)
    );
}

// ------------------------------------------------------------------ differential thens

pub async fn then_byte_identical(world: &mut FoundryWorld, answered: &str) {
    let kind = Answered::parse(answered);
    let diff = world.pl.diff.clone().expect("a differential When ran");
    let (a, b) = (&diff.deleted, &diff.never_existed);
    assert_eq!(
        (a.status, &a.content_type, &a.body),
        (b.status, &b.content_type, &b.body),
        "INV-PL-2 ({}): the deleted project must answer byte-identically to a project that never existed; deleted: {}; never existed: {}",
        diff.door,
        tm::describe(a),
        tm::describe(b)
    );
    match kind {
        Answered::UniformNotFound => {
            let canon = tm::never_existed(world).await;
            assert!(
                b.status == StatusCode::NOT_FOUND && b.body == canon.body,
                "OQ-D1 ({}): the never-existed answer is the uniform 404; {}",
                diff.door,
                tm::describe(b)
            );
        }
        Answered::ApiNotFound => assert!(
            b.status == StatusCode::NOT_FOUND
                && b.content_type
                    .as_deref()
                    .is_some_and(|c| c.starts_with("application/json")),
            "the API's not-found answer is a JSON 404; {}",
            tm::describe(b)
        ),
    }
}

/// The machine-token routes never read the project (re-verified at DISTILL):
/// under the deleted and the never-existed address they answer alike, a 2xx
/// naming the same tokens (`last_used_at` moves with every call, so the list
/// is compared by token id).
pub fn then_token_routes_alike(world: &FoundryWorld) {
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    let (a, b) = (&diff.deleted, &diff.never_existed);
    assert!(
        a.status.is_success() && a.status == b.status && a.content_type == b.content_type,
        "{}: both answers are the token route's shipped success; deleted {}; never existed {}",
        diff.door,
        tm::describe(a),
        tm::describe(b)
    );
    assert_eq!(
        token_ids(a),
        token_ids(b),
        "{}: both answers name the same tokens",
        diff.door
    );
}

/// The live project's answer on the same door has the same status and type.
pub fn then_live_answers_alike(world: &FoundryWorld, live_project: &str) {
    assert_eq!(
        live_project, "Identity Platform",
        "the differential's live control is Identity Platform"
    );
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    let (live, never) = (&diff.live, &diff.never_existed);
    assert!(
        live.status == never.status && live.content_type == never.content_type,
        "{}: the token route answers a live project's address the same way; live {}; never existed {}",
        diff.door,
        tm::describe(live),
        tm::describe(never)
    );
}

fn token_ids(answer: &Answer) -> Vec<String> {
    let mut ids: Vec<String> = serde_json::from_str::<serde_json::Value>(&answer.body)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t.get("jti").and_then(|j| j.as_str()).map(str::to_string))
        .collect();
    ids.sort();
    ids
}

/// The universe a differential row's two compared requests are judged against.
async fn everything(world: &FoundryWorld) -> (tl::Universe, Projects) {
    (
        blank_projects(tl::universe(world).await),
        read_projects(world).await,
    )
}

/// Mandate 8 for the differential: the deleted-side and never-existed-side
/// requests together leave the whole universe unchanged (fail-closed). The
/// live control runs after this image is taken; its writes are a control's.
pub fn then_neither_request_changed_anything(world: &FoundryWorld) {
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    assert_eq!(
        diff.everything_before, diff.everything_after,
        "{}: neither compared request may change anything (INV-PL-2: nothing can be written into a deleted project)",
        diff.door
    );
}

pub fn then_nothing_under_changed(world: &FoundryWorld, name: &str) {
    assert_eq!(
        name, "Auth v2 Spike",
        "the differential rows name Auth v2 Spike"
    );
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    assert_eq!(
        diff.held_before, diff.held_after,
        "{}: neither request may write anything under the deleted project",
        diff.door
    );
}

pub fn then_live_answers_otherwise(world: &FoundryWorld, live_project: &str) {
    assert_eq!(
        live_project, "Identity Platform",
        "the differential's live control is Identity Platform"
    );
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    let (live, never) = (&diff.live, &diff.never_existed);
    assert!(
        live.status.as_u16() < 500,
        "control ({}): the live project's answer is not a server error; {}",
        diff.door,
        tm::describe(live)
    );
    assert!(
        live.status != never.status || live.body != never.body,
        "control ({}): the live project must answer otherwise than a project that never existed, or the row proves nothing; {}",
        diff.door,
        tm::describe(live)
    );
}

pub async fn then_each_never_existed(world: &mut FoundryWorld) {
    let canon = tm::never_existed(world).await;
    let answers = world.pl.answers.clone();
    assert!(!answers.is_empty(), "the When produced answers");
    for a in answers {
        assert!(
            a.status == StatusCode::NOT_FOUND && a.body == canon.body,
            "every answer is byte-identical to a page that never existed; {}",
            tm::describe(&a)
        );
    }
}

// ------------------------------------------------------------------ outsiders

/// Someone who cannot reach Backend: the deleted and the never-existed
/// address answer exactly alike (status, type and body).
pub fn then_same_as_out_of_reach(world: &FoundryWorld) {
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    let (a, b) = (&diff.deleted, &diff.never_existed);
    assert_eq!(
        (a.status, &a.content_type, &a.body),
        (b.status, &b.content_type, &b.body),
        "INV-PL-2 ({}): for someone out of reach the deleted project answers exactly as a never-existed one; deleted {}; never existed {}",
        diff.door,
        tm::describe(a),
        tm::describe(b)
    );
}

/// ... and a live project they cannot reach answers them the same way, so no
/// door tells them anything about which projects exist.
pub fn then_live_answers_them_alike(world: &FoundryWorld, live_project: &str) {
    assert_eq!(
        live_project, "Identity Platform",
        "the differential's live control is Identity Platform"
    );
    let diff = world.pl.diff.as_ref().expect("a differential When ran");
    let (live, never) = (&diff.live, &diff.never_existed);
    assert_eq!(
        (live.status, &live.content_type, &live.body),
        (never.status, &never.content_type, &never.body),
        "{}: a live project out of reach answers exactly as a never-existed one; live {}; never existed {}",
        diff.door,
        tm::describe(live),
        tm::describe(never)
    );
}

// ------------------------------------------------------------------ the token revoke pair (U1)

async fn spare_revoked(world: &mut FoundryWorld) -> bool {
    let spare = world.pl.script.as_ref().expect("a script token").1;
    let p = project(world, "Identity Platform");
    let list = api(
        world,
        reqwest::Method::GET,
        &format!("/api/v1/teams/{}/projects/{}/tokens", p.team_slug, p.slug),
        Body::None,
    )
    .await;
    serde_json::from_str::<serde_json::Value>(&list.body)
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .find(|t| t.get("jti").and_then(|j| j.as_str()) == Some(spare.to_string().as_str()))
        .and_then(|t| t.get("revoked").and_then(serde_json::Value::as_bool))
        .unwrap_or_else(|| {
            panic!(
                "the token list names the spare token; {}",
                tm::describe(&list)
            )
        })
}

/// "the script revokes Priya's spare machine token under X's address, and again
/// under a project that never existed".
pub async fn when_script_revokes_twice(world: &mut FoundryWorld, name: &str) {
    let p = project(world, name);
    let spare = world.pl.script.as_ref().expect("a script token").1;
    let revoked_before = spare_revoked(world).await;
    let before = everything(world).await;
    let first = api(
        world,
        reqwest::Method::DELETE,
        &format!(
            "/api/v1/teams/{}/projects/{}/tokens/{spare}",
            p.team_slug, p.slug
        ),
        Body::None,
    )
    .await;
    let after_first = everything(world).await;
    let second = api(
        world,
        reqwest::Method::DELETE,
        &format!(
            "/api/v1/teams/{}/projects/{UNKNOWN_PROJECT}/tokens/{spare}",
            p.team_slug
        ),
        Body::None,
    )
    .await;
    let after_second = everything(world).await;
    let revoked_after = spare_revoked(world).await;
    world.pl.revoke = Some(RevokePair {
        first,
        second,
        before,
        after_first,
        after_second,
        revoked_before,
        revoked_after,
    });
}

pub fn then_first_revoke(world: &FoundryWorld) {
    let r = world.pl.revoke.as_ref().expect("the revoke pair ran");
    assert!(
        r.first.status.is_success(),
        "the first revoke goes through (the token route names no project, U1); {}",
        tm::describe(&r.first)
    );
    assert!(
        !r.revoked_before && r.revoked_after,
        "the spare machine token was live before and is revoked after"
    );
    assert_eq!(
        r.before, r.after_first,
        "the revoke changes nothing else (teams, people, tokens' scope, every project and record)"
    );
}

pub fn then_second_revoke(world: &FoundryWorld) {
    let r = world.pl.revoke.as_ref().expect("the revoke pair ran");
    assert_eq!(
        (r.second.status, &r.second.content_type, &r.second.body),
        (r.first.status, &r.first.content_type, &r.first.body),
        "the second revoke (already revoked, under a never-existed address) answers as the first; {}",
        tm::describe(&r.second)
    );
    assert_eq!(
        r.after_first, r.after_second,
        "the second revoke changes nothing"
    );
}

// ------------------------------------------------------------------ scripting disabled (NFR-PL-3, U3a)

/// Every form in `html` is a plain HTML POST to a `/team/…` address carrying
/// the hidden CSRF field and a submit button; no control needs a script
/// (no `javascript:` link, no inline handler).
fn assert_scriptless(html: &str, what: &str) {
    let doc = parse(html);
    let forms = select_all(&doc, "form");
    assert!(!forms.is_empty(), "{what}: has at least one form");
    let hidden = scraper::Selector::parse("input[type=hidden][name=_csrf]").expect("selector");
    let submit = scraper::Selector::parse("button, input[type=submit]").expect("selector");
    for f in forms {
        let method = f
            .value()
            .attr("method")
            .unwrap_or("get")
            .to_ascii_lowercase();
        let action = f.value().attr("action").unwrap_or("");
        assert!(
            method == "post" && action.starts_with("/team/"),
            "{what}: a plain POST form to a /team/ address, not {method} {action:?}"
        );
        assert!(
            f.select(&hidden).next().is_some(),
            "{what}: the form carries the hidden _csrf field"
        );
        assert!(
            f.select(&submit).next().is_some(),
            "{what}: the form has a submit button"
        );
    }
    for a in select_all(&doc, "a[href]") {
        let href = a.value().attr("href").unwrap_or("");
        assert!(
            !href.starts_with("javascript:") && !href.is_empty() && href != "#",
            "{what}: every link is a real address, not {href:?}"
        );
    }
    let lower = html.to_ascii_lowercase();
    for handler in ["onclick=", "onsubmit="] {
        assert!(
            !lower.contains(handler),
            "{what}: no inline {handler} handler"
        );
    }
}

pub fn then_rows_work_without_scripts(world: &FoundryWorld) {
    let answer = tm::last(world).clone();
    let rows = rows_with_projects(world, &answer);
    assert!(!rows.is_empty(), "the Projects section lists projects");
    for (p, html) in rows {
        assert_scriptless(&html, &format!("{}'s row", p.name));
        let row = row_answer(&answer, html);
        assert_eq!(
            tm::link_to(&row, "Delete project"),
            delete_path(&p.team_slug, &p.slug),
            "Delete project is a plain link to the delete page"
        );
    }
}

/// D14 (review TG-8): the team delete page's "{team} also has {k} deleted
/// project(s): …" line is shown only when k > 0; a team with no deleted
/// project gets no such sentence, not even "0 deleted project(s)".
pub fn then_delete_page_silent_on_deleted_projects(world: &FoundryWorld) {
    let answer = tm::last(world);
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "the delete page renders; {}",
        tm::describe(answer)
    );
    let text = tm::page_text(&answer.body);
    for copy in ["also has", "deleted project(s)"] {
        assert!(
            !text.contains(copy),
            "the page must not mention {copy:?} for a team with no deleted project; {}",
            tm::describe(answer)
        );
    }
}

pub fn then_page_works_without_scripts(world: &FoundryWorld) {
    let answer = tm::last(world);
    tl::require_status(answer, StatusCode::OK, "the delete page renders");
    let doc = parse(&answer.body);
    let main = select_all(&doc, "main")
        .first()
        .map(|m| m.html())
        .unwrap_or_else(|| answer.body.clone());
    assert_scriptless(&main, "the delete page");
}

// ------------------------------------------------------------------ controls

/// Control: a live project's real attachment still downloads.
pub async fn then_attachment_still_downloads(world: &mut FoundryWorld, who: &str, name: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let id = attachment_of(world, &p, 1).await;
    let a = tm::open(
        world,
        &actor,
        &format!("{}/issues/1/attachments/{id}", project_path(&p)),
    )
    .await;
    tl::require_status(
        &a,
        StatusCode::OK,
        "control: a live project's attachment still downloads",
    );
}

/// Control: the instance dashboard's rename still works on a live project.
pub async fn then_instance_rename_still_works(
    world: &mut FoundryWorld,
    who: &str,
    name: &str,
    raw: &str,
) {
    when_instance_renames(world, who, name, raw, false).await;
    let a = tm::last(world);
    tl::require_status(
        a,
        StatusCode::OK,
        "control: the instance dashboard still renames a live project",
    );
}

/// Control: a fresh edit of a live project's issue still saves.
pub async fn then_title_still_saves(world: &mut FoundryWorld, who: &str, title: &str, key: &str) {
    given_opened_edit_form_earlier(world, who, key).await;
    when_saves_title_from_earlier(world, who, title).await;
    let a = tm::last(world);
    assert!(
        a.status.is_success() || a.status.is_redirection(),
        "control: a fresh edit of {key} still saves; {}",
        tm::describe(a)
    );
}

// ------------------------------------------------------------------ list thens

pub async fn then_dashboard_lists(world: &mut FoundryWorld, who: &str, listed: &str, not: &str) {
    let actor = tm::resolve_actor(world, who);
    let section = tm::page_text(&tm::dashboard_projects(world, &actor).await);
    assert!(
        section.contains(listed) && !section.contains(not),
        "{who}'s dashboard lists {listed:?} and not {not:?}; got {section:?}"
    );
}

pub async fn then_instance_dashboard_lists(
    world: &mut FoundryWorld,
    who: &str,
    listed: &[&str],
    not: &str,
) {
    let actor = tm::resolve_actor(world, who);
    let page = tm::open(world, &actor, "/admin/instance/workspaces").await;
    tl::require_status(&page, StatusCode::OK, "the instance dashboard renders");
    let text = tm::page_text(&page.body);
    for l in listed {
        assert!(
            text.contains(l),
            "{who}'s instance dashboard lists {l:?}; {text:?}"
        );
    }
    assert!(
        !text.contains(not),
        "{who}'s instance dashboard does not list {not:?}; {text:?}"
    );
}

pub async fn then_issue_page_opens(
    world: &mut FoundryWorld,
    key: &str,
    who: &str,
    comments: usize,
) {
    let actor = tm::resolve_actor(world, who);
    let (p, n) = issue_of(world, key);
    let page = tm::open(world, &actor, &format!("{}/issues/{n}", project_path(&p))).await;
    tl::require_status(&page, StatusCode::OK, "the issue opens at its old address");
    for c in 1..=comments {
        let body = format!("Comment {c} on {key}");
        assert!(
            tm::says(&page, &body),
            "{key} shows {body:?}; {}",
            tm::describe(&page)
        );
    }
    assert!(
        tm::says(&page, ATTACHMENT_NAME),
        "{key} shows its attachment; {}",
        tm::describe(&page)
    );
}

pub async fn then_next_filed_is(world: &mut FoundryWorld, who: &str, name: &str, key: &str) {
    let actor = tm::resolve_actor(world, who);
    let p = project(world, name);
    let answer = file_issue(world, &actor, &p, "Filed after the restore").await;
    require_written(&answer, "the issue filed after the restore");
    let n: i64 = key.rsplit_once('-').expect("a key").1.parse().expect("n");
    // The issue page names an issue by its key alone (`<h1>{key}</h1>`); the
    // title is shown on the board's card. So: the key's own page answers with
    // that key, and the board's card for that key carries the title just filed.
    let page = tm::open(world, &actor, &format!("{}/issues/{n}", project_path(&p))).await;
    assert!(
        page.status == StatusCode::OK && tm::says(&page, key),
        "{key} opens at its own address; {}",
        tm::describe(&page)
    );
    let board = tm::open(world, &actor, &project_path(&p)).await;
    tl::require_status(&board, StatusCode::OK, "the board opens");
    let doc = parse(&board.body);
    let titles: Vec<String> = select_all(
        &doc,
        &format!("article.issue-card[data-issue-key=\"{key}\"] .title"),
    )
    .iter()
    .map(|t| squash(&text_of(t)))
    .collect();
    assert_eq!(
        titles,
        vec!["Filed after the restore".to_string()],
        "the next issue filed on {name} is {key}; {}",
        tm::describe(&board)
    );
}

// ------------------------------------------------------------------ live view

pub async fn then_hears_nothing(world: &mut FoundryWorld) {
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let sub = world.pl.watch.as_ref().expect("someone is watching");
    let events = sub.drain();
    assert_eq!(
        events.len(),
        world.pl.heard,
        "the open live view hears of no change made after the delete (Changed Assumption 1); new: {:?}",
        &events[world.pl.heard.min(events.len())..]
    );
}

// ------------------------------------------------------------------ operator thens

fn cli_run(world: &FoundryWorld) -> CliRun {
    world.pl.cli.clone().expect("the operator ran a command")
}

pub fn then_succeeded(world: &FoundryWorld) {
    let run = cli_run(world);
    assert_eq!(run.code, 0, "the command succeeded; {run:?}");
}

pub fn then_reads_line(world: &mut FoundryWorld, line: &str) {
    let expected = resolve(world, line);
    let run = cli_run(world);
    assert_eq!(run.code, 0, "the command succeeded; {run:?}");
    assert_eq!(
        run.stdout.trim_end(),
        expected,
        "the operator reads exactly {expected:?}"
    );
}

pub fn then_reads_two_lines(world: &mut FoundryWorld, first: &str, second: &str) {
    let expected = format!("{}\n{}", resolve(world, first), resolve(world, second));
    let run = cli_run(world);
    assert_eq!(run.code, 0, "the command succeeded; {run:?}");
    assert_eq!(
        run.stdout.trim_end(),
        expected,
        "the operator reads exactly {expected:?}"
    );
}

/// The blocks of `list-deleted-projects`; `{when it was deleted}` is the
/// block's project's deleted marker, in RFC 3339 UTC.
pub async fn then_reads_blocks(world: &mut FoundryWorld, step: &cucumber::gherkin::Step) {
    let rows: Vec<(String, String)> = step
        .table
        .as_ref()
        .expect("a table of lines")
        .rows
        .iter()
        .map(|r| (r[0].trim().to_string(), r[1].trim().to_string()))
        .collect();
    let mut expected_blocks: Vec<Vec<(String, String)>> = vec![Vec::new()];
    for (k, v) in rows {
        if k.is_empty() && v.is_empty() {
            expected_blocks.push(Vec::new());
        } else {
            let v = if v == "{when it was deleted}" {
                v
            } else {
                resolve(world, &v)
            };
            expected_blocks.last_mut().expect("a block").push((k, v));
        }
    }
    let run = cli_run(world);
    assert_eq!(run.code, 0, "the command succeeded; {run:?}");
    let actual_blocks: Vec<Vec<String>> = run
        .stdout
        .trim_end()
        .split("\n\n")
        .map(|b| b.lines().map(str::to_string).collect())
        .collect();
    assert_eq!(
        actual_blocks.len(),
        expected_blocks.len(),
        "one block per deleted project (D14); stdout {:?}",
        run.stdout
    );
    for (actual, expected) in actual_blocks.iter().zip(&expected_blocks) {
        let id = expected
            .iter()
            .find(|(k, _)| k == "project-id")
            .map(|(_, v)| v.clone())
            .expect("a project-id line");
        assert_eq!(
            actual.len(),
            expected.len(),
            "the block's lines; {actual:?}"
        );
        for (line, (k, v)) in actual.iter().zip(expected) {
            if v == "{when it was deleted}" {
                let stamp = line
                    .strip_prefix(&format!("{k}: "))
                    .unwrap_or_else(|| panic!("a {k} line, got {line:?}"));
                let printed = time::OffsetDateTime::parse(
                    stamp,
                    &time::format_description::well_known::Rfc3339,
                )
                .unwrap_or_else(|e| panic!("deleted-at is RFC 3339 ({stamp:?}): {e}"));
                assert_eq!(printed.offset(), time::UtcOffset::UTC, "deleted-at is UTC");
                let (stored,): (time::OffsetDateTime,) =
                    sqlx::query_as("SELECT deleted_at FROM projects WHERE id = $1::uuid")
                        .bind(&id)
                        .fetch_one(&tm::pool(world))
                        .await
                        .expect("read the deleted marker");
                assert!(
                    (printed - stored).abs() < time::Duration::seconds(1),
                    "deleted-at is the project's deleted marker ({stored}); printed {printed}"
                );
            } else {
                assert_eq!(line, &format!("{k}: {v}"), "the block's {k} line (D14)");
            }
        }
    }
}

pub fn then_exits_saying(world: &mut FoundryWorld, code: i32, message: &str) {
    let expected = resolve(world, message);
    let run = cli_run(world);
    assert_eq!(run.code, code, "the command exits {code}; {run:?}");
    assert!(
        run.stderr.lines().any(|l| l.starts_with(&expected)),
        "stderr carries {expected:?} (D14); {run:?}"
    );
}

// ------------------------------------------------------------------ race thens

pub async fn then_delete_race_outcomes(world: &mut FoundryWorld) {
    let canon = tm::never_existed(world).await;
    let runs = world.pl.delete_race.clone();
    assert!(!runs.is_empty(), "the race ran");
    for r in runs {
        assert!(
            r.delete.status == StatusCode::OK
                && tm::says(&r.delete, &format!("Deleted {}.", r.project)),
            "{}: the delete went through; {}",
            r.project,
            tm::describe(&r.delete)
        );
        let kept_and_hidden = r.create.status.as_u16() < 400
            && r.issues_kept == 1
            && r.board_for_sam.status == StatusCode::NOT_FOUND
            && r.board_for_sam.body == canon.body;
        let refused = r.create.status == StatusCode::NOT_FOUND
            && r.create.body == canon.body
            && r.issues_kept == 0;
        assert!(
            kept_and_hidden || refused,
            "{}: either the issue was kept and is hidden with the project, or the filing was answered as for a project that never existed (INV-PL-3); create {}; issues kept {}; board {}",
            r.project,
            tm::describe(&r.create),
            r.issues_kept,
            tm::describe(&r.board_for_sam)
        );
    }
}

pub fn then_no_server_error_in_race(world: &FoundryWorld) {
    for r in &world.pl.delete_race {
        for a in [&r.delete, &r.create, &r.board_for_sam] {
            assert!(
                a.status.as_u16() < 500,
                "{}: no server error; {}",
                r.project,
                tm::describe(a)
            );
        }
    }
    for r in &world.pl.restore_race {
        assert!(
            r.team_delete.status.as_u16() < 500,
            "{}: no server error; {}",
            r.project,
            tm::describe(&r.team_delete)
        );
        assert!(
            matches!(r.restore.code, 0 | 4),
            "{}: the restore exits 0 or 4; {:?}",
            r.project,
            r.restore
        );
    }
}

pub fn then_no_filed_issue_lost(world: &FoundryWorld) {
    for r in &world.pl.delete_race {
        if r.create.status.as_u16() < 400 {
            assert_eq!(
                r.issues_kept, 1,
                "{}: a filing that went through is never lost",
                r.project
            );
        }
    }
}

pub fn then_restore_race_outcomes(world: &FoundryWorld) {
    assert!(!world.pl.restore_race.is_empty(), "the race ran");
    for r in &world.pl.restore_race {
        let named = format!("{} ({})", r.project, r.key);
        let restored_first = r.restore.code == 0
            && r.team_delete.status == StatusCode::CONFLICT
            && tm::says(&r.team_delete, &named);
        let deleted_first = r.team_delete.status == StatusCode::OK && r.restore.code == 4;
        assert!(
            restored_first || deleted_first,
            "{}: either the project came back and the team delete was refused naming it, or the team was deleted and the restore exited 4 (INV-PL-6); restore {:?}; team delete {}",
            r.project,
            r.restore,
            tm::describe(&r.team_delete)
        );
    }
}

pub fn then_nothing_removed_in_race(world: &FoundryWorld) {
    for r in &world.pl.restore_race {
        assert!(
            r.project_row_kept && r.issues_kept == 1,
            "{}: no project row and no issue is ever removed (INV-PL-6); kept {}, issues {}",
            r.project,
            r.project_row_kept,
            r.issues_kept
        );
    }
}
