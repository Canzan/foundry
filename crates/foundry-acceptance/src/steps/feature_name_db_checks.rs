//! name-db-checks step definitions
//! (`tests/features/name-db-checks.feature`, DISTILL 2026-10-08, every scenario
//! `@pending`).
//!
//! The seams these steps drive are the DESIGN-pinned driving ports (feature delta,
//! DESIGN [REF] Driving ports):
//!
//! - **The operator's SQL session** (DDD-4/5, OQ-D2): an `INSERT` or
//!   `UPDATE … SET name` on `workspaces` / `projects`, sent as a parameterised
//!   statement on the scenario's own schema (the scenario pool), exactly what
//!   Priya's psql session sends. A refusal is the database error itself: SQLSTATE
//!   23514, `constraint()` = `<table>_name_<arm>`, `table()`, `column()` = `name`,
//!   and the native message `new row for relation "<table>" violates check
//!   constraint "<arm>"`.
//! - **The app doors** (unchanged, D7): the instance-admin renames (iawr, pnr
//!   steps) and the board's new-issue POST `/team/{team}/project/{slug}/issues`.
//! - **Backup, restore and backup-verify** (unchanged, D9): the us-03 steps and
//!   their restored instance (`world.us_03_restored_harness`).
//!
//! LEGACY ROWS go through ONE seam (DESIGN DDD-10): a `foundry-store` test-support
//! function that writes a row "that predates the rule" with the table's two name
//! triggers switched off for that one transaction. It does not exist yet, so
//! [`seed_through_legacy_seam`] is a SCAFFOLD placeholder that panics naming the
//! seam — every scenario seeding a legacy row is RED at that call, for the right
//! reason. DELIVER replaces its body with the seam call; nothing else in this
//! module moves.
//!
//! __SCAFFOLD__
//! SCAFFOLD: true (only [`seed_through_legacy_seam`]; every other step is real)
//!
//! STATE DELTA (Mandate 8): every write captures the [`NameUniverse`] — every
//! workspace `(id, name)`, every project `(name, address, key prefix, issue
//! counter)`, the issue count — just before it, after every Given. A refusal moves
//! none of it (fail-closed); an accepted rename changes exactly one name; an
//! accepted addition adds exactly one row under exactly the typed name; a new issue
//! adds one issue and moves only that project's counter.
//!
//! LAYER 3 (real Postgres, real HTTP, real CLI, `@real-io`): example-based only
//! (Mandates 9 and 11). The generative parity proof (≥10,000 names per table, arm
//! for arm against `WorkspaceName` / `ProjectName`) lives at the store layer
//! (`crates/foundry-store/tests/name_rule_parity.rs`); here a few representative
//! names per arm prove the rule is wired to every write and that it names its arm.
//!
//! NAMES: a quoted name goes through the pnr [`expand_name`] (the `[N×c]` repeat
//! mark, then the iwnr invisible marks) before it is sent or compared.

use crate::steps::feature_instance_admin_project_rename::{
    harness, http, pool, project_id_of, stored_slugs_of, PRIYA_EMAIL, PRIYA_PASSWORD,
};
use crate::steps::feature_instance_admin_workspace_rename::resolve;
use crate::steps::feature_project_name_rule::expand_name;
use crate::support::harness::{seed_lanes_for_project, signed_in_get, signed_in_post};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use sqlx::postgres::PgDatabaseError;
use sqlx::PgPool;
use std::collections::BTreeMap;

/// The workspace and team the Background seeds (iapr step).
const WORKSPACE: &str = "Canzan Labs";
const TEAM: &str = "Backend";
const TEAM_SLUG: &str = "backend";

// ===========================================================================
// Domain types — the vocabulary the steps share
// ===========================================================================

/// The two tables the name rule guards (the seam's closed enum, DDD-10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRuleTable {
    Workspaces,
    Projects,
}

impl NameRuleTable {
    pub fn table(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Projects => "projects",
        }
    }

    /// The table's two name triggers (DDD-5).
    pub fn triggers(self) -> [String; 2] {
        let t = self.table();
        [
            format!("{t}_name_rule_on_insert"),
            format!("{t}_name_rule_on_rename"),
        ]
    }

    /// The four arm names the table's refusal may carry (DDD-6). A rule named in
    /// the feature that is not one of these is a typo in the scenario.
    pub fn arms(self) -> [String; 4] {
        let t = self.table();
        let cap = match self {
            Self::Workspaces => "max_24_chars",
            Self::Projects => "max_256_chars",
        };
        [
            format!("{t}_name_not_empty"),
            format!("{t}_name_trimmed"),
            format!("{t}_name_no_control_chars"),
            format!("{t}_name_{cap}"),
        ]
    }

    fn of_arm(arm: &str) -> Self {
        if arm.starts_with("workspaces_") {
            Self::Workspaces
        } else if arm.starts_with("projects_") {
            Self::Projects
        } else {
            panic!("the scenario names rule {arm:?}, which belongs to neither table")
        }
    }
}

/// One project as an operator or member can observe it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    pub name: String,
    pub slug: String,
    pub key_prefix: String,
    pub next_issue_number: i32,
}

/// The observable universe of a name write (port-exposed: rows read back by SQL).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameUniverse {
    pub workspaces: BTreeMap<uuid::Uuid, String>,
    pub projects: BTreeMap<uuid::Uuid, ProjectRow>,
    pub issues: i64,
}

/// What the operator's statement did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteKind {
    /// `UPDATE <table> SET name = … WHERE id = …`.
    Rename(NameRuleTable, uuid::Uuid),
    /// `INSERT INTO <table> …` of a row with this id.
    Add(NameRuleTable, uuid::Uuid),
}

/// The database's answer to the operator's statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbAnswer {
    /// The statement succeeded, touching this many rows.
    Accepted(u64),
    /// The database refused it with these error fields.
    Refused {
        code: String,
        constraint: Option<String>,
        table: Option<String>,
        column: Option<String>,
        message: String,
    },
    /// Any other failure (connection, a non-Postgres error).
    Other(String),
}

/// The last statement the operator sent, and its answer.
#[derive(Debug, Clone)]
pub struct DbWrite {
    pub kind: WriteKind,
    pub typed: String,
    pub answer: DbAnswer,
}

/// A row to store as it was before the upgrade.
#[derive(Debug, Clone)]
pub enum LegacyRow {
    Workspace {
        id: uuid::Uuid,
        name: String,
    },
    Project {
        id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        team_id: uuid::Uuid,
        name: String,
        slug: String,
        key_prefix: String,
    },
}

impl LegacyRow {
    fn table(&self) -> NameRuleTable {
        match self {
            Self::Workspace { .. } => NameRuleTable::Workspaces,
            Self::Project { .. } => NameRuleTable::Projects,
        }
    }
}

// ===========================================================================
// The legacy seam (SCAFFOLD, DDD-10)
// ===========================================================================

/// SCAFFOLD (DELIVER, DDD-10): store `row` the way history wrote it — with the
/// table's two name triggers switched off for this one transaction only.
///
/// DELIVER replaces this body with the call to the `foundry-store` test-support
/// seam (working name `seed_row_predating_name_rule`, with the closed
/// `NameRuleTable` enum; rename it here, in the store test scaffolds and in the
/// check-arch rule together if DELIVER picks another name), passing
/// [`LegacyRow::table`] and the row's INSERT. It must NOT be a plain INSERT: once
/// 0019/0020 land, a plain INSERT of these names is refused (that is the feature).
async fn seed_through_legacy_seam(pool: &PgPool, row: &LegacyRow) {
    let _ = pool;
    panic!(
        "SCAFFOLD: the legacy seam (name-db-checks DDD-10) does not exist yet — \
         DELIVER adds foundry-store's test-support seed_row_predating_name_rule \
         (NameRuleTable::{:?}) and calls it here to store {row:?}",
        row.table()
    );
}

// ===========================================================================
// The universe (Mandate 8)
// ===========================================================================

pub(crate) async fn capture_name_universe(pool: &PgPool) -> NameUniverse {
    let workspaces: Vec<(uuid::Uuid, String)> = sqlx::query_as("SELECT id, name FROM workspaces")
        .fetch_all(pool)
        .await
        .expect("read every workspace");
    type Row = (uuid::Uuid, String, String, String, i32);
    let projects: Vec<Row> =
        sqlx::query_as("SELECT id, name, slug, key_prefix, next_issue_number FROM projects")
            .fetch_all(pool)
            .await
            .expect("read every project");
    let (issues,): (i64,) = sqlx::query_as("SELECT count(*) FROM issues")
        .fetch_one(pool)
        .await
        .expect("count issues");
    NameUniverse {
        workspaces: workspaces.into_iter().collect(),
        projects: projects
            .into_iter()
            .map(|(id, name, slug, key_prefix, next_issue_number)| {
                (
                    id,
                    ProjectRow {
                        name,
                        slug,
                        key_prefix,
                        next_issue_number,
                    },
                )
            })
            .collect(),
        issues,
    }
}

async fn snapshot_before(world: &mut FoundryWorld) {
    world.ndc_before = Some(capture_name_universe(&pool(world)).await);
}

fn universe_before(world: &FoundryWorld) -> NameUniverse {
    world
        .ndc_before
        .clone()
        .expect("the name universe was captured before the write")
}

fn last_write(world: &FoundryWorld) -> DbWrite {
    world
        .ndc_write
        .clone()
        .expect("the operator sent a statement")
}

// ===========================================================================
// The operator's SQL session
// ===========================================================================

fn answer_of(result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>) -> DbAnswer {
    match result {
        Ok(done) => DbAnswer::Accepted(done.rows_affected()),
        Err(sqlx::Error::Database(db)) => match db.try_downcast_ref::<PgDatabaseError>() {
            Some(pg) => DbAnswer::Refused {
                code: pg.code().to_string(),
                constraint: pg.constraint().map(str::to_string),
                table: pg.table().map(str::to_string),
                column: pg.column().map(str::to_string),
                message: pg.message().to_string(),
            },
            None => DbAnswer::Other(db.to_string()),
        },
        Err(other) => DbAnswer::Other(other.to_string()),
    }
}

/// `UPDATE <table> SET name = $1 WHERE id = $2` — the operator's rename.
async fn rename_by_hand(
    pool: &PgPool,
    table: NameRuleTable,
    id: uuid::Uuid,
    name: &str,
) -> DbAnswer {
    let sql = format!("UPDATE {} SET name = $1 WHERE id = $2", table.table());
    answer_of(sqlx::query(&sql).bind(name).bind(id).execute(pool).await)
}

async fn operator_renames(
    world: &mut FoundryWorld,
    table: NameRuleTable,
    id: uuid::Uuid,
    raw: &str,
) {
    let typed = expand_name(raw);
    snapshot_before(world).await;
    let answer = rename_by_hand(&pool(world), table, id, &typed).await;
    world.ndc_write = Some(DbWrite {
        kind: WriteKind::Rename(table, id),
        typed,
        answer,
    });
}

fn workspace_id(world: &FoundryWorld, name: &str) -> uuid::Uuid {
    *world
        .iapr_workspace_ids
        .get(name)
        .unwrap_or_else(|| panic!("workspace {name:?} is seeded by the Background"))
}

fn team_id(world: &FoundryWorld, name: &str) -> uuid::Uuid {
    *world
        .iapr_team_ids
        .get(name)
        .unwrap_or_else(|| panic!("team {name:?} is seeded by the Background"))
}

// ===========================================================================
// Oracles
// ===========================================================================

fn assert_refused_under(answer: &DbAnswer, arm: &str) {
    let table = NameRuleTable::of_arm(arm);
    assert!(
        table.arms().iter().any(|a| a == arm),
        "the scenario names rule {arm:?}, which is not one of {:?} (DDD-6)",
        table.arms()
    );
    match answer {
        DbAnswer::Refused {
            code,
            constraint,
            table: refused_table,
            column,
            message,
        } => {
            assert_eq!(
                code, "23514",
                "the refusal must be a check violation (SQLSTATE 23514, D5); got {answer:?}"
            );
            assert_eq!(
                constraint.as_deref(),
                Some(arm),
                "the refusal must name the rule it broke (D5, DDD-4); got {answer:?}"
            );
            assert_eq!(
                refused_table.as_deref(),
                Some(table.table()),
                "the refusal must name the table (DDD-4); got {answer:?}"
            );
            assert_eq!(
                column.as_deref(),
                Some("name"),
                "the refusal must name the column (DDD-4); got {answer:?}"
            );
            assert_eq!(
                message,
                &format!(
                    "new row for relation \"{}\" violates check constraint \"{arm}\"",
                    table.table()
                ),
                "the operator reads the native check-violation line (DDD-4)"
            );
        }
        other => panic!(
            "the database must refuse the write under {arm:?} (D1-D5); it answered {other:?}"
        ),
    }
}

/// Fail-closed: nothing in the universe moved.
fn assert_nothing_moved(before: &NameUniverse, after: &NameUniverse) {
    assert_eq!(
        after.workspaces, before.workspaces,
        "no workspace may be added, removed or renamed"
    );
    assert_eq!(
        after.projects, before.projects,
        "no project may be added, removed, renamed, re-addressed or re-counted"
    );
    assert_eq!(after.issues, before.issues, "no issue may be added");
}

/// Exactly the one write the operator sent, under exactly the typed name.
fn assert_only_this_write(before: &NameUniverse, after: &NameUniverse, write: &DbWrite) {
    let mut expected = before.clone();
    match &write.kind {
        WriteKind::Rename(NameRuleTable::Workspaces, id) => {
            assert!(
                before.workspaces.contains_key(id),
                "the renamed workspace existed"
            );
            expected.workspaces.insert(*id, write.typed.clone());
        }
        WriteKind::Add(NameRuleTable::Workspaces, id) => {
            expected.workspaces.insert(*id, write.typed.clone());
        }
        WriteKind::Rename(NameRuleTable::Projects, id) => {
            let old = before
                .projects
                .get(id)
                .expect("the renamed project existed");
            expected.projects.insert(
                *id,
                ProjectRow {
                    name: write.typed.clone(),
                    ..old.clone()
                },
            );
        }
        WriteKind::Add(NameRuleTable::Projects, id) => {
            let added = after
                .projects
                .get(id)
                .unwrap_or_else(|| panic!("the added project {id} must be stored"));
            expected.projects.insert(
                *id,
                ProjectRow {
                    name: write.typed.clone(),
                    ..added.clone()
                },
            );
        }
    }
    assert_eq!(
        after, &expected,
        "exactly the operator's one write may land, under exactly {:?}",
        write.typed
    );
}

// ===========================================================================
// Given — rows from before the upgrade
// ===========================================================================

#[given(regex = r#"^workspace "([^"]+)" was stored before the upgrade$"#)]
async fn workspace_stored_before_the_upgrade(world: &mut FoundryWorld, raw: String) {
    let name = expand_name(&raw);
    let id = uuid::Uuid::now_v7();
    seed_through_legacy_seam(
        &pool(world),
        &LegacyRow::Workspace {
            id,
            name: name.clone(),
        },
    )
    .await;
    world.iapr_workspace_ids.insert(name, id);
}

#[given(
    regex = r#"^project "([^"]+)" \(([A-Z]+)\) was stored before the upgrade, with ([A-Z]+)-(\d+) its last issue$"#
)]
async fn project_stored_before_the_upgrade(
    world: &mut FoundryWorld,
    raw: String,
    key: String,
    issue_key: String,
    last: i32,
) {
    assert_eq!(
        issue_key, key,
        "the last issue carries the project's key prefix"
    );
    let name = expand_name(&raw);
    let id = uuid::Uuid::now_v7();
    let slug = format!("legacy-{}", key.to_ascii_lowercase());
    let workspace_id = workspace_id(world, WORKSPACE);
    seed_through_legacy_seam(
        &pool(world),
        &LegacyRow::Project {
            id,
            workspace_id,
            team_id: team_id(world, TEAM),
            name: name.clone(),
            slug: slug.clone(),
            key_prefix: key.clone(),
        },
    )
    .await;
    let pool = pool(world);
    seed_lanes_for_project(&pool, id).await;
    let author = world
        .iapr_priya_id
        .expect("Priya is seeded by the Background");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, state, author_id)
              VALUES ($1, $2, $3, $4, 'Swap the NAS fan', 'backlog', $5)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(id)
    .bind(workspace_id)
    .bind(last)
    .bind(author)
    .execute(&pool)
    .await
    .expect("seed the legacy project's last issue");
    // An UPDATE of another column: the rule must never fire on it (D6, DDD-5).
    sqlx::query("UPDATE projects SET next_issue_number = $1 WHERE id = $2")
        .bind(last + 1)
        .bind(id)
        .execute(&pool)
        .await
        .expect("set the legacy project's issue counter past its last issue");
    world.iapr_project_ids.insert(name.clone(), id);
    world
        .iapr_stored_slugs
        .insert(name, (TEAM_SLUG.to_string(), slug));
}

// ===========================================================================
// When — the operator at the database prompt
// ===========================================================================

#[when(regex = r#"^the operator renames workspace "([^"]+)" to "([^"]*)" at the database prompt$"#)]
async fn operator_renames_workspace(world: &mut FoundryWorld, label: String, raw: String) {
    let id = resolve(world, &expand_name(&label)).await;
    operator_renames(world, NameRuleTable::Workspaces, id, &raw).await;
}

#[when(regex = r#"^the operator renames project "([^"]+)" to "([^"]*)" at the database prompt$"#)]
async fn operator_renames_project(world: &mut FoundryWorld, label: String, raw: String) {
    let id = project_id_of(world, &expand_name(&label));
    operator_renames(world, NameRuleTable::Projects, id, &raw).await;
}

#[given(
    regex = r#"^the operator's rename of workspace "([^"]+)" to "([^"]*)" at the database prompt was refused$"#
)]
async fn operator_rename_was_refused(world: &mut FoundryWorld, label: String, raw: String) {
    operator_renames_workspace(world, label, raw).await;
    let write = last_write(world);
    assert!(
        matches!(&write.answer, DbAnswer::Refused { code, .. } if code == "23514"),
        "the rename to {:?} must have been refused by the rule; it answered {:?}",
        write.typed,
        write.answer
    );
    let after = capture_name_universe(&pool(world)).await;
    assert_nothing_moved(&universe_before(world), &after);
}

#[when(
    regex = r#"^the operator renames project "([^"]+)" to "([^"]*)" and moves its issue counter on by (\d+) in the same statement at the database prompt$"#
)]
async fn operator_renames_project_and_counter(
    world: &mut FoundryWorld,
    label: String,
    raw: String,
    by: i32,
) {
    let id = project_id_of(world, &expand_name(&label));
    let typed = expand_name(&raw);
    snapshot_before(world).await;
    let answer = answer_of(
        sqlx::query(
            "UPDATE projects SET name = $1, next_issue_number = next_issue_number + $2 WHERE id = $3",
        )
        .bind(&typed)
        .bind(by)
        .bind(id)
        .execute(&pool(world))
        .await,
    );
    world.ndc_write = Some(DbWrite {
        kind: WriteKind::Rename(NameRuleTable::Projects, id),
        typed,
        answer,
    });
}

#[when(regex = r#"^the operator adds a workspace named "([^"]*)" at the database prompt$"#)]
async fn operator_adds_workspace(world: &mut FoundryWorld, raw: String) {
    let typed = expand_name(&raw);
    snapshot_before(world).await;
    let id = uuid::Uuid::now_v7();
    let answer = answer_of(
        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
            .bind(id)
            .bind(&typed)
            .execute(&pool(world))
            .await,
    );
    world.ndc_write = Some(DbWrite {
        kind: WriteKind::Add(NameRuleTable::Workspaces, id),
        typed,
        answer,
    });
}

#[when(
    regex = r#"^the operator adds project "([^"]*)" \(([A-Z]+)\) to team "([^"]+)" at the database prompt$"#
)]
async fn operator_adds_project(world: &mut FoundryWorld, raw: String, key: String, team: String) {
    let typed = expand_name(&raw);
    snapshot_before(world).await;
    let id = uuid::Uuid::now_v7();
    let answer = answer_of(
        sqlx::query(
            "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(team_id(world, &team))
        .bind(workspace_id(world, WORKSPACE))
        .bind(&typed)
        .bind(format!("hand-{}", key.to_ascii_lowercase()))
        .bind(&key)
        .execute(&pool(world))
        .await,
    );
    world.ndc_write = Some(DbWrite {
        kind: WriteKind::Add(NameRuleTable::Projects, id),
        typed,
        answer,
    });
}

// ===========================================================================
// When — a member at the board (the app door, D6)
// ===========================================================================

#[when(regex = r#"^Priya files "([^"]+)" on the "([^"]+)" board$"#)]
async fn priya_files_on_board(world: &mut FoundryWorld, title: String, raw_project: String) {
    let project = expand_name(&raw_project);
    let (team, slug) = stored_slugs_of(world, &project);
    snapshot_before(world).await;
    let outcome = signed_in_post(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &format!("/team/{team}/project/{slug}/issues"),
        &[("title", &title)],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
    world.ndc_filed = Some((project, title));
}

// ===========================================================================
// Then — the database's answer
// ===========================================================================

#[then(regex = r#"^the database refuses it under the rule "([^"]+)"$"#)]
async fn database_refuses_under(world: &mut FoundryWorld, arm: String) {
    assert_refused_under(&last_write(world).answer, &arm);
}

#[then(regex = r"^no workspace or project changed$")]
async fn no_workspace_or_project_changed(world: &mut FoundryWorld) {
    let after = capture_name_universe(&pool(world)).await;
    assert_nothing_moved(&universe_before(world), &after);
}

#[then(regex = r#"^the database stores exactly "([^"]*)" and nothing else changed$"#)]
async fn database_stores_exactly(world: &mut FoundryWorld, raw: String) {
    let write = last_write(world);
    assert_eq!(
        write.typed,
        expand_name(&raw),
        "the scenario checks the name it sent"
    );
    assert_eq!(
        write.answer,
        DbAnswer::Accepted(1),
        "a name the app would accept must be accepted by the database (D3: never stricter)"
    );
    let after = capture_name_universe(&pool(world)).await;
    assert_only_this_write(&universe_before(world), &after, &write);
}

#[then(regex = r"^the database accepts it and nothing changed$")]
async fn database_accepts_and_nothing_changed(world: &mut FoundryWorld) {
    let write = last_write(world);
    assert_eq!(
        write.answer,
        DbAnswer::Accepted(1),
        "rewriting a stored name unchanged must never meet the rule (D6, DDD-5)"
    );
    let after = capture_name_universe(&pool(world)).await;
    assert_nothing_moved(&universe_before(world), &after);
}

// ===========================================================================
// Then — the legacy project at the board
// ===========================================================================

#[then(regex = r"^the issue is created as ([A-Z]+)-(\d+) and shows on that board$")]
async fn issue_created_as(world: &mut FoundryWorld, key: String, number: i32) {
    let (project, title) = world.ndc_filed.clone().expect("an issue was filed");
    let status = world.last_status.expect("the file-issue answer");
    assert!(
        matches!(status.as_u16(), 200 | 303),
        "filing on a legacy project must succeed (D6), not {status}: {:?}",
        world
            .last_body
            .as_deref()
            .map(|b| b.chars().take(300).collect::<String>())
    );
    let id = project_id_of(world, &project);
    let before = universe_before(world);
    let after = capture_name_universe(&pool(world)).await;
    let mut expected = before.clone();
    let row = before
        .projects
        .get(&id)
        .expect("the legacy project existed");
    assert_eq!(
        row.key_prefix, key,
        "the issue key carries the project's prefix"
    );
    expected.projects.insert(
        id,
        ProjectRow {
            next_issue_number: row.next_issue_number + 1,
            ..row.clone()
        },
    );
    expected.issues += 1;
    assert_eq!(
        after, expected,
        "filing an issue adds one issue and moves only that project's counter"
    );
    let filed: Vec<(i32, String)> =
        sqlx::query_as("SELECT number, title FROM issues WHERE project_id = $1 AND number = $2")
            .bind(id)
            .bind(number)
            .fetch_all(&pool(world))
            .await
            .expect("read the filed issue");
    assert_eq!(
        filed,
        vec![(number, title.clone())],
        "the new issue must be {key}-{number}"
    );
    let (team, slug) = stored_slugs_of(world, &project);
    let board = signed_in_get(
        harness(world),
        &http(world),
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &format!("/team/{team}/project/{slug}"),
    )
    .await;
    assert_eq!(board.status.as_u16(), 200, "the legacy board must open");
    let issue_key = format!("{key}-{number}");
    assert!(
        board.body.contains(&issue_key) && board.body.contains(&title),
        "the board must show {issue_key} {title:?}"
    );
}

#[then(regex = r#"^project "([^"]+)" kept its name, address and key prefix$"#)]
async fn project_kept_identity(world: &mut FoundryWorld, raw: String) {
    let name = expand_name(&raw);
    let id = project_id_of(world, &name);
    let (_, slug) = stored_slugs_of(world, &name);
    let after = capture_name_universe(&pool(world)).await;
    let row = after
        .projects
        .get(&id)
        .expect("the legacy project still exists");
    assert_eq!(row.name, name, "the legacy name is never rewritten (D4)");
    assert_eq!(row.slug, slug, "the address never moves");
    let before = universe_before(world);
    assert_eq!(
        row.key_prefix, before.projects[&id].key_prefix,
        "the key prefix never moves"
    );
}

// ===========================================================================
// Then — the restored instance (us-03 harness)
// ===========================================================================

fn restored_pool(world: &FoundryWorld) -> PgPool {
    world
        .us_03_restored_harness
        .as_ref()
        .expect("the restored instance is up (the us-03 restore step)")
        .app
        .state
        .store
        .pool()
        .clone()
}

#[then(
    regex = r#"^the restored instance holds workspace "([^"]+)" and project "([^"]+)" byte for byte$"#
)]
async fn restored_holds_legacy(world: &mut FoundryWorld, raw_ws: String, raw_project: String) {
    let workspace = expand_name(&raw_ws);
    let project = expand_name(&raw_project);
    let restored = capture_name_universe(&restored_pool(world)).await;
    let source = capture_name_universe(&pool(world)).await;
    assert_eq!(
        restored, source,
        "the restored instance must hold every workspace, project and issue as the source did"
    );
    let ws_id = workspace_id(world, &workspace);
    let project_id = project_id_of(world, &project);
    assert_eq!(restored.workspaces.get(&ws_id), Some(&workspace));
    assert_eq!(
        restored.projects.get(&project_id).map(|p| p.name.clone()),
        Some(project)
    );
}

#[then(regex = r"^the name rule is switched on in the restored instance$")]
async fn rule_switched_on_in_restored(world: &mut FoundryWorld) {
    let pool = restored_pool(world);
    let triggers: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT c.relname::text, t.tgname::text, t.tgenabled::text
           FROM pg_trigger t JOIN pg_class c ON c.oid = t.tgrelid
          WHERE c.relnamespace = current_schema()::regnamespace
            AND NOT t.tgisinternal
            AND t.tgname LIKE '%name_rule%'
          ORDER BY 1, 2",
    )
    .fetch_all(&pool)
    .await
    .expect("read the name-rule triggers");
    let mut expected: Vec<(String, String, String)> =
        [NameRuleTable::Projects, NameRuleTable::Workspaces]
            .into_iter()
            .flat_map(|t| {
                t.triggers()
                    .into_iter()
                    .map(move |name| (t.table().to_string(), name, "O".to_string()))
            })
            .collect();
    expected.sort();
    assert_eq!(
        triggers, expected,
        "after a restore the four name triggers must exist and be enabled (D9, DDD-14)"
    );
    let (verdict, enforce): (bool, bool) = sqlx::query_as(
        "SELECT to_regprocedure('foundry_name_rule_violation(text, integer)') IS NOT NULL,
                to_regprocedure('foundry_enforce_name_rule()') IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("look the rule's functions up");
    assert!(
        verdict && enforce,
        "both rule functions must be restored (DDD-1/4)"
    );
}

#[then(
    regex = r#"^renaming workspace "([^"]+)" to "([^"]*)" by hand in the restored instance is refused under the rule "([^"]+)"$"#
)]
async fn restored_refuses(world: &mut FoundryWorld, label: String, raw: String, arm: String) {
    let pool = restored_pool(world);
    let id = workspace_id(world, &expand_name(&label));
    let before = capture_name_universe(&pool).await;
    let answer = rename_by_hand(&pool, NameRuleTable::Workspaces, id, &expand_name(&raw)).await;
    assert_refused_under(&answer, &arm);
    assert_nothing_moved(&before, &capture_name_universe(&pool).await);
}

#[cfg(test)]
mod tests {
    use super::NameRuleTable;

    #[test]
    fn each_table_has_its_four_arms_and_two_triggers() {
        assert_eq!(
            NameRuleTable::Workspaces.arms(),
            [
                "workspaces_name_not_empty",
                "workspaces_name_trimmed",
                "workspaces_name_no_control_chars",
                "workspaces_name_max_24_chars",
            ]
        );
        assert_eq!(
            NameRuleTable::Projects.arms()[3],
            "projects_name_max_256_chars"
        );
        assert_eq!(
            NameRuleTable::Projects.triggers(),
            [
                "projects_name_rule_on_insert",
                "projects_name_rule_on_rename"
            ]
        );
        assert_eq!(
            NameRuleTable::of_arm("projects_name_trimmed"),
            NameRuleTable::Projects
        );
    }
}
