//! project-lifecycle copy byte tests (DESIGN DDD-26, Reuse Analysis "byte tests
//! per new string"; D14 plus DISTILL DIST-1 and DIST-2).
//!
//! Authored RED in DISTILL 2026-10-13 as pending panic stubs: the strings'
//! homes (`TeamNotice` variants in `foundry-app`, the CLI constants in
//! `admin_cli.rs`) do not exist yet, so nothing can be named. DELIVER replaces
//! each stub with byte assertions on the real variant or constant, in the
//! team-lifecycle `TeamNotice` byte-test idiom, and removes the `#[ignore]`.
//! The acceptance scenarios already read every string through the page text or
//! the binary's output; these tests pin the bytes at their single home.

use foundry_app::teams::TeamNotice;

/// 01-04: the members page Projects section and the rename notice.
/// - `TeamNotice::ProjectRenamed` → "Renamed {old} to {new}."
/// - `TeamNotice::NoProjectsYet` → "{team} has no projects yet."
#[test]
fn the_projects_section_and_rename_copy_is_byte_exact() {
    let renamed = TeamNotice::ProjectRenamed {
        old: "Identity Platform".into(),
        new: "Identity & Access".into(),
    };
    assert_eq!(
        renamed.to_string(),
        "Renamed Identity Platform to Identity & Access."
    );
    let none = TeamNotice::NoProjectsYet {
        team: "Labs".into(),
    };
    assert_eq!(none.to_string(), "Labs has no projects yet.");
}

/// 02-03: the confirm page and the delete notice.
/// - `TeamNotice::ProjectDeleteHeading` → "Delete {project}?"
/// - `TeamNotice::ProjectDeleteCount` → "{project} ({KEY}) has {n} issue(s). After it is
///   deleted, nobody can open its board, its issues or its report, and links to {KEY}
///   issues stop working."
/// - `TeamNotice::ProjectDeleteNothingErased` → "Nothing is erased. The instance operator
///   can restore {project} with every issue, comment and attachment."
/// - `TeamNotice::ProjectDeleted` → "Deleted {project}."
#[test]
fn the_confirm_page_and_delete_copy_is_byte_exact() {
    let heading = TeamNotice::ProjectDeleteHeading {
        project: "Auth v2 Spike".into(),
    };
    assert_eq!(heading.to_string(), "Delete Auth v2 Spike?");
    let count = TeamNotice::ProjectDeleteCount {
        project: "Reading List".into(),
        key: "READ".into(),
        issues: 0,
    };
    assert_eq!(
        count.to_string(),
        "Reading List (READ) has 0 issue(s). After it is deleted, nobody can open its board, \
         its issues or its report, and links to READ issues stop working."
    );
    let nothing_erased = TeamNotice::ProjectDeleteNothingErased {
        project: "Auth v2 Spike".into(),
    };
    assert_eq!(
        nothing_erased.to_string(),
        "Nothing is erased. The instance operator can restore Auth v2 Spike with every issue, \
         comment and attachment."
    );
    let deleted = TeamNotice::ProjectDeleted {
        project: "Auth v2 Spike".into(),
    };
    assert_eq!(deleted.to_string(), "Deleted Auth v2 Spike.");
}

/// 03-01: the operator's CLI constants.
/// - list: `project-id: {uuid}`, `workspace: {workspace}`, `team: {team_slug}`,
///   `name: {project}`, `key: {KEY}`, `issues: {n}`, `deleted-at: {RFC 3339 UTC}`,
///   `deleted-by: {email}`; none: `status: none`.
/// - restore: `project-id: {uuid}`, `status: restored`.
/// - exit 4: `foundry doctor restore-project: project {uuid} is not deleted — status: not restorable`,
///   `foundry doctor restore-project: project {uuid} not in database — status: not restorable`.
/// - DIST-1: `foundry doctor restore-comment: comment {uuid} is in deleted project {project uuid}
///   — status: not restorable`.
#[test]
fn the_operator_cli_copy_is_byte_exact() {
    use foundry_app::admin_cli::restore_project_copy::{
        block, comment_in_deleted_project, not_restorable, restored, NONE, NOT_DELETED,
        NOT_IN_DATABASE,
    };
    let project_id = uuid::Uuid::parse_str("0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa").unwrap();
    let comment_id = uuid::Uuid::parse_str("5f2a8c17-e4b3-4d69-8b01-6c7d9e3f2a54").unwrap();
    let row = foundry_store::DeletedProjectRow {
        project_id,
        workspace_name: "Acme".into(),
        team_slug: "backend".into(),
        name: "Auth v2 Spike".into(),
        key_prefix: "SPIKE".into(),
        issue_count: 3,
        deleted_at: time::macros::datetime!(2026-10-13 14:05:09.123456 UTC),
        deleted_by_email: "lena@acme.example".into(),
    };
    assert_eq!(
        block(&row),
        "project-id: 0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa\n\
         workspace: Acme\n\
         team: backend\n\
         name: Auth v2 Spike\n\
         key: SPIKE\n\
         issues: 3\n\
         deleted-at: 2026-10-13T14:05:09Z\n\
         deleted-by: lena@acme.example"
    );
    assert_eq!(NONE, "status: none");
    assert_eq!(
        restored(project_id),
        "project-id: 0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa\nstatus: restored"
    );
    assert_eq!(
        not_restorable(project_id, NOT_DELETED),
        "foundry doctor restore-project: project 0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa \
         is not deleted — status: not restorable"
    );
    assert_eq!(
        not_restorable(project_id, NOT_IN_DATABASE),
        "foundry doctor restore-project: project 0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa \
         not in database — status: not restorable"
    );
    assert_eq!(
        comment_in_deleted_project(comment_id, project_id),
        "foundry doctor restore-comment: comment 5f2a8c17-e4b3-4d69-8b01-6c7d9e3f2a54 \
         is in deleted project 0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa — status: not restorable"
    );
}

/// 04-03: the team delete page's extra line and the detached-project refusal.
/// - `TeamNotice::TeamAlsoHasDeletedProjects` → "{team} also has {k} deleted project(s):
///   {list}. They are kept, but can't be restored after {team} is deleted, and their keys
///   stay taken in {workspace}." with `{list}` reading "Prototype Graveyard (PROTO)".
/// - DIST-2: `foundry doctor restore-project: project {uuid}: its team {team_slug} was
///   deleted — status: not restorable`.
#[test]
fn the_team_delete_line_and_team_gone_refusal_are_byte_exact() {
    use foundry_app::admin_cli::restore_project_copy::{not_restorable, team_gone};
    let also_has = TeamNotice::TeamAlsoHasDeletedProjects {
        team: "Labs".into(),
        workspace: "Canzan Labs".into(),
        projects: vec![("Prototype Graveyard".into(), "PROTO".into())],
    };
    assert_eq!(
        also_has.to_string(),
        "Labs also has 1 deleted project(s): Prototype Graveyard (PROTO). They are kept, \
         but can't be restored after Labs is deleted, and their keys stay taken in Canzan Labs."
    );
    let project_id = uuid::Uuid::parse_str("0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa").unwrap();
    assert_eq!(
        not_restorable(project_id, &team_gone("labs")),
        "foundry doctor restore-project: project 0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa: \
         its team labs was deleted — status: not restorable"
    );
}

/// Phase 5 mutation gap A-1: the operator's two project doors, run through the
/// real `foundry` binary with `DATABASE_URL` set to the empty string, exit 3
/// with the "required" message — never a connect attempt to "".
#[test]
fn the_operator_doors_refuse_an_empty_database_url_as_missing() {
    for (args, door) in [
        (
            vec![
                "doctor",
                "restore-project",
                "0b9d6f43-7c1e-4f0a-9a52-3d8e2c41b7aa",
            ],
            "foundry doctor restore-project",
        ),
        (
            vec!["doctor", "list-deleted-projects"],
            "foundry doctor list-deleted-projects",
        ),
    ] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_foundry"))
            .args(&args)
            .env("DATABASE_URL", "")
            .output()
            .expect("run the foundry binary");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(3), "{door}: {stderr}");
        assert_eq!(
            stderr,
            format!(
                "{door}: DATABASE_URL is required to reach the live database. \
                 Set it to the same value the foundry server uses.\n"
            ),
            "{door}"
        );
        assert!(out.stdout.is_empty(), "{door}: nothing on stdout");
    }
}
