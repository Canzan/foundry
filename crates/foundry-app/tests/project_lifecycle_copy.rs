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
#[ignore = "pending: project-lifecycle 03-01 — CLI constant bytes: list, restore, exit-4 reasons, restore-comment"]
fn the_operator_cli_copy_is_byte_exact() {
    panic!(
        "Not yet implemented -- RED scaffold (project-lifecycle 03-01: list-deleted-projects, \
         restore-project and restore-comment constants)"
    );
}

/// 04-01: the team delete page's extra line and the detached-project refusal.
/// - `TeamNotice::TeamAlsoHasDeletedProjects` → "{team} also has {k} deleted project(s):
///   {list}. They are kept, but can't be restored after {team} is deleted, and their keys
///   stay taken in {workspace}." with `{list}` reading "Prototype Graveyard (PROTO)".
/// - DIST-2: `foundry doctor restore-project: project {uuid}: its team {team_slug} was
///   deleted — status: not restorable`.
#[test]
#[ignore = "pending: project-lifecycle 04-01 — TeamNotice and CLI bytes: also-has line, team-gone refusal"]
fn the_team_delete_line_and_team_gone_refusal_are_byte_exact() {
    panic!(
        "Not yet implemented -- RED scaffold (project-lifecycle 04-01: TeamNotice::\
         TeamAlsoHasDeletedProjects, the restore-project team-gone constant)"
    );
}
