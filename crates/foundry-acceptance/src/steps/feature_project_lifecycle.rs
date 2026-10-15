//! project-lifecycle step definitions, slices 01-04
//! (`tests/features/project-lifecycle-0{1,2,3,4}-*.feature`, DISTILL
//! 2026-10-13, every scenario `@pending`).
//!
//! Every step body is one call into `support::project_lifecycle` (the driver:
//! this feature's vocabulary, doors, universe and state-delta oracle), which
//! builds on `support::team_lifecycle` and `support::team_membership`. Reused
//! unchanged from those step files: the Background (four steps), "she sees",
//! "she is told", "the answer is the page that never existed", "the answer is
//! turned away as a forged form", "… opens X's members page", "… opened X's
//! members page earlier", "… has made X a lead/member of Y", "she follows
//! Cancel", "she is back on X's members page", "she gets X's members page back
//! with no notice", the team delete page's "asks", "says" and "offers", "…
//! follows Delete team on X's members page", "… has deleted X" (a team), "…
//! submits a delete of X" (a team), "… follows New project … and creates …",
//! "she is refused with …, and her text … is kept" and "no team, project or
//! membership changed". No step holds business logic, control flow or SQL of
//! its own (Mandate 12, criterion 3).
//!
//! Driving ports (DESIGN Driving Ports 1-8): the members page's Projects
//! section, `POST /team/{t}/project/{p}/rename`, `GET`/`POST
//! /team/{t}/project/{p}/delete`, every shipped project route of the web and
//! the API, the dashboard, the nav, the instance dashboard and its rename,
//! project create, the team delete pages, and `foundry doctor
//! list-deleted-projects` / `restore-project` / `restore-comment` through the
//! real binary. Layer 3 (real HTTP, real Postgres, real binary): example-only,
//! sad paths enumerated (Mandates 9 and 11).
//!
//! Phrases are global across the suite (cucumber-rs has one registry), so every
//! regex is anchored and says "the project" where a team phrase would
//! otherwise match.

use crate::support::project_lifecycle as pl;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};

fn table(step: &Step) -> Vec<String> {
    step.table
        .as_ref()
        .expect("this step carries a table")
        .rows
        .iter()
        .map(|r| r[0].trim().to_string())
        .collect()
}

// ------------------------------------------------------------------ Givens

#[given(regex = r"^these projects are live:$")]
async fn projects_live(world: &mut FoundryWorld, step: &Step) {
    pl::given_projects_live(world, step).await;
}

#[given(
    regex = r"^([A-Z][\w&]*(?: [\w&]+)*) has (\d+) issues?(?:, (\d+) comments? and (\d+) attachments?)?$"
)]
async fn work(
    world: &mut FoundryWorld,
    project: String,
    issues: usize,
    comments: String,
    attachments: String,
) {
    pl::given_work(
        world,
        &project,
        issues,
        comments.parse().unwrap_or(0),
        attachments.parse().unwrap_or(0),
    )
    .await;
}

#[given(regex = r"^one more comment on ([A-Z]+-\d+) was deleted and awaits cleanup$")]
async fn awaiting_cleanup(world: &mut FoundryWorld, key: String) {
    pl::given_comment_awaiting_cleanup(world, &key).await;
}

#[given(regex = r"^([A-Z]\w*) has commented on ([A-Z]+-\d+) and on ([A-Z]+-\d+)$")]
async fn has_commented(world: &mut FoundryWorld, who: String, a: String, b: String) {
    pl::given_has_commented(world, &who, &[&a, &b]).await;
}

#[given(regex = r"^([A-Z]\w*) has deleted the project (.+)$")]
async fn has_deleted(world: &mut FoundryWorld, who: String, project: String) {
    pl::given_has_deleted(world, &who, &project).await;
}

#[given(regex = r"^([A-Z]\w*) opened the delete page of the project (.+) earlier$")]
async fn opened_delete_page(world: &mut FoundryWorld, who: String, project: String) {
    pl::given_opened_delete_page_earlier(world, &who, &project).await;
}

#[given(regex = r"^([A-Z]\w*) opened the edit form of ([A-Z]+-\d+) earlier$")]
async fn opened_edit_form(world: &mut FoundryWorld, who: String, key: String) {
    pl::given_opened_edit_form_earlier(world, &who, &key).await;
}

#[given(regex = r"^([A-Z]\w*) opened the instance dashboard earlier$")]
async fn opened_instance_dashboard(world: &mut FoundryWorld, who: String) {
    pl::given_opened_instance_dashboard_earlier(world, &who).await;
}

#[given(
    regex = r"^a script holds a machine token of ([A-Z]\w*)'s for ([A-Z]\w*), and [A-Z]\w* has a spare machine token$"
)]
async fn script_token(world: &mut FoundryWorld, who: String, team: String) {
    pl::given_script_token(world, &who, &team).await;
}

#[given(
    regex = r"^([A-Z]\w*) is watching (.+)'s live updates, and has heard of an issue ([A-Z]\w*) filed$"
)]
async fn watching(world: &mut FoundryWorld, who: String, project: String, filer: String) {
    pl::given_watching(world, &who, &project, &filer).await;
}

#[given(regex = r"^the operator has restored (\{[^}]+\})$")]
async fn has_restored(world: &mut FoundryWorld, target: String) {
    pl::given_operator_restored(world, &target).await;
}

// ------------------------------------------------------------------- Whens

#[when(
    regex = r#"^(she|he|[A-Z]\w*) renames the project (.+?) to "([^"]*)" on ([A-Z]\w*)'s members page$"#
)]
async fn renames(
    world: &mut FoundryWorld,
    who: String,
    project: String,
    name: String,
    team: String,
) {
    pl::when_renames(world, &who, &project, &name, false, Some(&team)).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*) renames the project (.+?) to "([^"]*)" from the page (?:she|he) opened earlier$"#
)]
async fn renames_earlier(world: &mut FoundryWorld, who: String, project: String, name: String) {
    pl::when_renames(world, &who, &project, &name, true, None).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*) renames the project (.+?) to "([^"]*)" from the instance dashboard$"#
)]
async fn instance_renames(world: &mut FoundryWorld, who: String, project: String, name: String) {
    pl::when_instance_renames(world, &who, &project, &name, false).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*) renames the project (.+?) to "([^"]*)" from the dashboard (?:she|he) opened earlier$"#
)]
async fn instance_renames_earlier(
    world: &mut FoundryWorld,
    who: String,
    project: String,
    name: String,
) {
    pl::when_instance_renames(world, &who, &project, &name, true).await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*) submits (.+?)'s project rename unchanged on ([A-Z]\w*)'s members page$"
)]
async fn rename_unchanged(world: &mut FoundryWorld, who: String, project: String, team: String) {
    pl::when_submits_rename_unchanged(world, &who, &project, &team).await;
}

#[when(
    regex = r#"^(a signed-out visitor|[A-Z]\w*) submits a rename of (?:the project (.+?)|(a project that does not exist)) to "([^"]*)"( without the page's token)?$"#
)]
async fn submits_rename(
    world: &mut FoundryWorld,
    who: String,
    project: String,
    unknown: String,
    name: String,
    tokenless: String,
) {
    pl::when_submits_rename(
        world,
        &who,
        &format!("{project}{unknown}"),
        &name,
        tokenless.is_empty(),
    )
    .await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*) follows Delete project on (.+)'s row of ([A-Z]\w*)'s members page$"
)]
async fn follows_delete(world: &mut FoundryWorld, who: String, project: String, team: String) {
    pl::when_follows_delete(world, &who, &project, &team).await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*) confirms the project delete( from the page (?:she|he) opened earlier)?$"
)]
async fn confirms(world: &mut FoundryWorld, who: String, earlier: String) {
    pl::when_confirms(world, &who, !earlier.is_empty()).await;
}

#[when(regex = r"^(she|he|[A-Z]\w*) deletes the project (.+) from ([A-Z]\w*)'s members page$")]
async fn deletes(world: &mut FoundryWorld, who: String, project: String, team: String) {
    pl::when_deletes(world, &who, &project, &team).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*) asks for the delete page of (?:the project (.+)|(a project that does not exist))$"
)]
async fn asks_delete_page(world: &mut FoundryWorld, who: String, project: String, unknown: String) {
    pl::when_asks_delete_page(world, &who, &format!("{project}{unknown}")).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*) submits a delete of (?:the project (.+?)|(a project that does not exist))( without the page's token)?$"
)]
async fn submits_delete(
    world: &mut FoundryWorld,
    who: String,
    project: String,
    unknown: String,
    tokenless: String,
) {
    pl::when_submits_delete(
        world,
        &who,
        &format!("{project}{unknown}"),
        tokenless.is_empty(),
    )
    .await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*) confirms the team delete( from the page (?:she|he) opened earlier)?$"
)]
async fn confirms_team_delete(world: &mut FoundryWorld, who: String, earlier: String) {
    pl::when_confirms_team_delete(world, &who, !earlier.is_empty()).await;
}

#[when(regex = r"^([A-Z]\w*) cancels the delete page (?:she|he) opened earlier$")]
async fn cancels(world: &mut FoundryWorld, who: String) {
    pl::when_cancels(world, &who).await;
}

#[when(regex = r"^([A-Z]\w*) looks at (?:her|his|their) dashboard$")]
async fn opens_dashboard(world: &mut FoundryWorld, who: String) {
    pl::when_opens_dashboard(world, &who).await;
}

#[when(regex = r"^([A-Z]\w*) tries to (file an issue on .+|comment on [A-Z]+-\d+)$")]
async fn tries_one_write(world: &mut FoundryWorld, who: String, write: String) {
    pl::when_tries_one_write(world, &who, &write).await;
}

#[when(
    regex = r"^the script revokes [A-Z]\w*'s spare machine token under (.+)'s address, and again under a project that never existed$"
)]
async fn script_revokes(world: &mut FoundryWorld, project: String) {
    pl::when_script_revokes_twice(world, &project).await;
}

#[when(
    regex = r#"^([A-Z]\w*) saves the title "([^"]+)" from the edit form (?:she|he) opened earlier$"#
)]
async fn saves_title(world: &mut FoundryWorld, who: String, title: String) {
    pl::when_saves_title_from_earlier(world, &who, &title).await;
}

#[when(regex = r"^([A-Z]\w*), ([A-Z]\w*) and ([A-Z]\w*) each download (.+)'s attachment$")]
async fn each_downloads(
    world: &mut FoundryWorld,
    a: String,
    b: String,
    c: String,
    project: String,
) {
    pl::when_each_downloads(world, &[&a, &b, &c], &project).await;
}

#[when(
    regex = r"^(she|he|a signed-out visitor|[A-Z]\w*) tries (.+) in (.+), and the same in a project that never existed$"
)]
async fn tries_door(world: &mut FoundryWorld, who: String, door: String, project: String) {
    pl::when_tries_door(world, &who, &door, &project).await;
}

#[when(regex = r"^the script tries (.+) in (.+), and the same in a project that never existed$")]
async fn script_tries_door(world: &mut FoundryWorld, door: String, project: String) {
    pl::when_script_tries_door(world, &door, &project).await;
}

#[when(regex = r"^the operator lists the deleted projects$")]
async fn operator_lists(world: &mut FoundryWorld) {
    pl::when_operator_lists(world).await;
}

#[when(
    regex = r"^the operator restores (\{[^}]+\}|\S+)( while foundry's records cannot be reached)?$"
)]
async fn operator_restores(world: &mut FoundryWorld, target: String, unreachable: String) {
    pl::when_operator_restores(world, &target, unreachable.is_empty()).await;
}

#[when(regex = r"^the operator brings back that deleted comment$")]
async fn operator_restores_comment(world: &mut FoundryWorld) {
    pl::when_operator_restores_comment(world).await;
}

#[given(regex = r"^([A-Z]\w*) has (\d+) fresh projects, Race 1 to Race \d+$")]
async fn race_projects(world: &mut FoundryWorld, team: String, times: usize) {
    pl::given_race_projects(world, &team, times).await;
}

#[when(
    regex = r"^([A-Z]\w*) deletes each fresh project at the same moment as ([A-Z]\w*) files an issue on it$"
)]
async fn delete_race(world: &mut FoundryWorld, who: String, filer: String) {
    pl::when_delete_races_file(world, &who, &filer).await;
}

#[given(
    regex = r"^(\d+) fresh teams each hold one project with one issue, which ([A-Z]\w*) has deleted$"
)]
async fn restore_race_teams(world: &mut FoundryWorld, times: usize, admin: String) {
    pl::given_restore_race_teams(world, times, &admin).await;
}

#[when(
    regex = r"^the operator restores each of those projects at the same moment as ([A-Z]\w*) deletes its team$"
)]
async fn restore_race(world: &mut FoundryWorld, admin: String) {
    pl::when_restore_races_team_delete(world, &admin).await;
}

#[given(regex = r"^([A-Z]\w*) has noted how every page of (.+) answers (?:her|him|them)$")]
async fn noted_pages(world: &mut FoundryWorld, who: String, project: String) {
    pl::given_noted_pages(world, &who, &project).await;
}

// ------------------------------------------------------------------- Thens

#[then(regex = r#"^(?:she|he) sees the project renamed from "([^"]*)" to "([^"]*)"$"#)]
async fn sees_renamed(world: &mut FoundryWorld, old: String, new: String) {
    pl::then_sees_renamed(world, &old, &new);
}

#[then(regex = r"^([A-Z]\w*)'s Projects section lists, in this order:$")]
async fn section_lists(world: &mut FoundryWorld, team: String, step: &Step) {
    pl::then_projects_section_lists(world, &team, &table(step));
}

#[then(
    regex = r#"^(.+?) is now called "([^"]*)" at the address "([^"]+)", one rename record names ([A-Z]\w* [A-Z]\w*), and nothing else changed$"#
)]
async fn now_called(
    world: &mut FoundryWorld,
    project: String,
    name: String,
    address: String,
    actor: String,
) {
    pl::then_renamed(world, &project, &name, &address, &actor).await;
}

#[then(
    regex = r"^(.+?)'s board and its issue ([A-Z]+-\d+) still open for ([A-Z]\w*) at their old addresses$"
)]
async fn still_open(world: &mut FoundryWorld, project: String, key: String, who: String) {
    pl::then_still_open(world, &project, &key, &who).await;
}

#[then(regex = r"^no project, issue or record changed$")]
async fn nothing_changed(world: &mut FoundryWorld) {
    pl::then_nothing_changed(world).await;
}

#[then(
    regex = r#"^(?:she|he) is refused with "([^"]+)" in (.+?)'s row, and (?:her|his) text "([^"]*)" is kept$"#
)]
async fn row_refusal(world: &mut FoundryWorld, copy: String, project: String, typed: String) {
    pl::then_row_refusal(world, &copy, &project, &typed);
}

#[then(regex = r#"^each project's name field is labelled "Project name" with the project's name$"#)]
async fn fields_labelled(world: &mut FoundryWorld) {
    pl::then_fields_labelled(world);
}

#[then(
    regex = r"^within each project row the keyboard reaches the name field, then Rename, then Delete project$"
)]
async fn tab_order(world: &mut FoundryWorld) {
    pl::then_row_tab_order(world);
}

#[then(regex = r"^each project row (links to its board|is not a link)$")]
async fn rows_link(world: &mut FoundryWorld, link: String) {
    pl::then_rows_link(world, link == "links to its board");
}

#[then(
    regex = r"^each project row (offers Project name, Rename and Delete project|offers no Project name, Rename or Delete project)$"
)]
async fn rows_controls(world: &mut FoundryWorld, controls: String) {
    pl::then_rows_offer_controls(world, !controls.starts_with("offers no"));
}

#[then(regex = r"^([A-Z]\w*) still finds the rename form of (.+)$")]
async fn still_finds_rename(world: &mut FoundryWorld, who: String, project: String) {
    pl::then_still_finds_rename_form(world, &who, &project).await;
}

#[then(regex = r"^([A-Z]\w*) still finds the delete page of (.+)$")]
async fn still_finds_delete(world: &mut FoundryWorld, who: String, project: String) {
    pl::then_still_finds_delete_page(world, &who, &project).await;
}

#[then(regex = r#"^the same project rename with the page's token renames (.+?) to "([^"]+)"$"#)]
async fn same_rename(world: &mut FoundryWorld, project: String, name: String) {
    pl::then_same_rename_with_token(world, &project, &name).await;
}

#[then(regex = r"^the same project delete with the page's token deletes (.+)$")]
async fn same_delete(world: &mut FoundryWorld, project: String) {
    pl::then_same_delete_with_token(world, &project).await;
}

#[then(regex = r"^the delete page offers Delete project and Cancel, and asks for no typed name$")]
async fn confirm_offers(world: &mut FoundryWorld) {
    pl::then_confirm_offers(world);
}

#[then(
    regex = r"^(.+?) is deleted by ([A-Z]\w* [A-Z]\w*), one delete record names ([A-Z]\w* [A-Z]\w*), and nothing else changed$"
)]
async fn deleted(world: &mut FoundryWorld, project: String, by: String, recorded_by: String) {
    pl::then_deleted(world, &project, &by, &recorded_by).await;
}

#[then(
    regex = r"^(.+?) still holds its (\d+) issues, (\d+) comments of which (\d+) awaits? cleanup, (\d+) attachments?, (\d+) lanes, their history and their queued notifications, unchanged$"
)]
async fn holdings_unchanged(
    world: &mut FoundryWorld,
    project: String,
    issues: i64,
    comments: i64,
    awaiting: i64,
    attachments: i64,
    lanes: i64,
) {
    pl::then_holdings_unchanged(
        world,
        &project,
        issues,
        comments,
        awaiting,
        attachments,
        lanes,
    )
    .await;
}

#[then(regex = r"^(.+?)'s next issue is still ([A-Z]+-\d+)$")]
async fn next_issue(world: &mut FoundryWorld, project: String, key: String) {
    pl::then_next_issue(world, &project, &key).await;
}

#[then(regex = r"^both answers are exactly the same, and they are (.+)$")]
async fn byte_identical(world: &mut FoundryWorld, answered: String) {
    pl::then_byte_identical(world, &answered).await;
}

#[then(regex = r"^both answers are the machine-token answer, alike for both$")]
async fn token_routes_alike(world: &mut FoundryWorld) {
    pl::then_token_routes_alike(world);
}

#[then(regex = r"^the same door in (.+), which is live, answers the same way$")]
async fn live_alike(world: &mut FoundryWorld, project: String) {
    pl::then_live_answers_alike(world, &project);
}

#[then(regex = r"^neither request changed anything$")]
async fn neither_changed(world: &mut FoundryWorld) {
    pl::then_neither_request_changed_anything(world);
}

#[then(regex = r"^([A-Z]\w*) can still download (.+)'s attachment$")]
async fn still_downloads(world: &mut FoundryWorld, who: String, project: String) {
    pl::then_attachment_still_downloads(world, &who, &project).await;
}

#[then(
    regex = r#"^([A-Z]\w*) can still rename the project (.+?) to "([^"]+)" from the instance dashboard$"#
)]
async fn still_instance_renames(
    world: &mut FoundryWorld,
    who: String,
    project: String,
    name: String,
) {
    pl::then_instance_rename_still_works(world, &who, &project, &name).await;
}

#[then(regex = r#"^([A-Z]\w*) can still save the title "([^"]+)" on ([A-Z]+-\d+)$"#)]
async fn still_saves(world: &mut FoundryWorld, who: String, title: String, key: String) {
    pl::then_title_still_saves(world, &who, &title, &key).await;
}

#[then(regex = r"^nothing under (.+) changed$")]
async fn nothing_under(world: &mut FoundryWorld, project: String) {
    pl::then_nothing_under_changed(world, &project);
}

#[then(regex = r"^every page of (.+?) answers ([A-Z]\w*) as it did before the (?:rename|delete)$")]
async fn pages_as_before(world: &mut FoundryWorld, project: String, who: String) {
    pl::then_pages_as_before(world, &who, &project).await;
}

#[then(regex = r"^both answers are exactly the same, as for any project out of reach$")]
async fn same_out_of_reach(world: &mut FoundryWorld) {
    pl::then_same_as_out_of_reach(world);
}

#[then(regex = r"^the same door in (.+), which is live, answers them the same way$")]
async fn live_alike_for_them(world: &mut FoundryWorld, project: String) {
    pl::then_live_answers_them_alike(world, &project);
}

#[then(regex = r"^the first answer revoked the spare machine token, and nothing else changed$")]
async fn first_revoke(world: &mut FoundryWorld) {
    pl::then_first_revoke(world);
}

#[then(regex = r"^the second answer is the same as the first, and changed nothing$")]
async fn second_revoke(world: &mut FoundryWorld) {
    pl::then_second_revoke(world);
}

#[then(regex = r"^every project row's rename and delete controls work as plain page forms$")]
async fn rows_scriptless(world: &mut FoundryWorld) {
    pl::then_rows_work_without_scripts(world);
}

#[then(regex = r"^the delete page works as a plain page form$")]
async fn page_scriptless(world: &mut FoundryWorld) {
    pl::then_page_works_without_scripts(world);
}

#[then(regex = r"^the delete page says nothing about deleted projects$")]
async fn page_silent_on_deleted_projects(world: &mut FoundryWorld) {
    pl::then_delete_page_silent_on_deleted_projects(world);
}

#[then(regex = r"^the same door in (.+), which is live, answers otherwise$")]
async fn live_otherwise(world: &mut FoundryWorld, project: String) {
    pl::then_live_answers_otherwise(world, &project);
}

#[then(regex = r"^(?:each answer is|both answers are) the page that never existed$")]
async fn each_never_existed(world: &mut FoundryWorld) {
    pl::then_each_never_existed(world).await;
}

#[then(regex = r#"^([A-Z]\w*)'s dashboard lists "([^"]+)" and not "([^"]+)"$"#)]
async fn dashboard_lists(world: &mut FoundryWorld, who: String, listed: String, not: String) {
    pl::then_dashboard_lists(world, &who, &listed, &not).await;
}

#[then(regex = r"^([A-Z]\w*)'s Board link opens (.+)$")]
async fn board_link_after(world: &mut FoundryWorld, who: String, project: String) {
    pl::board_link_opens(world, &who, &project).await;
}

#[then(regex = r#"^([A-Z]\w*)'s Projects section, as ([A-Z]\w*) sees it, lists only "([^"]+)"$"#)]
async fn section_only(world: &mut FoundryWorld, team: String, who: String, row: String) {
    pl::then_section_as_seen_by(world, &team, &who, &row, true).await;
}

#[then(regex = r#"^([A-Z]\w*)'s Projects section, as ([A-Z]\w*) sees it, lists "([^"]+)" again$"#)]
async fn section_again(world: &mut FoundryWorld, team: String, who: String, row: String) {
    pl::then_section_as_seen_by(world, &team, &who, &row, false).await;
}

#[then(
    regex = r#"^([A-Z]\w*)'s instance dashboard lists "([^"]+)" and "([^"]+)" and not "([^"]+)"$"#
)]
async fn instance_dashboard_lists(
    world: &mut FoundryWorld,
    who: String,
    a: String,
    b: String,
    not: String,
) {
    pl::then_instance_dashboard_lists(world, &who, &[&a, &b], &not).await;
}

#[then(regex = r"^([A-Z]\w*)'s live view hears of no further change$")]
async fn hears_nothing(world: &mut FoundryWorld, _who: String) {
    pl::then_hears_nothing(world).await;
}

#[then(
    regex = r"^each time, either [A-Z]\w*'s issue was kept and is hidden with the project, or [A-Z]\w* was answered as for a project that never existed$"
)]
async fn delete_race_outcomes(world: &mut FoundryWorld) {
    pl::then_delete_race_outcomes(world).await;
}

#[then(regex = r"^no answer in the race was a failure$")]
async fn no_server_error(world: &mut FoundryWorld) {
    pl::then_no_server_error_in_race(world);
}

#[then(regex = r"^no filed issue was lost$")]
async fn no_issue_lost(world: &mut FoundryWorld) {
    pl::then_no_filed_issue_lost(world);
}

#[then(regex = r"^the operator reads exactly these blocks, and the command succeeded:$")]
async fn reads_blocks(world: &mut FoundryWorld, step: &Step) {
    pl::then_reads_blocks(world, step).await;
}

#[then(regex = r#"^the operator reads "([^"]+)" then "([^"]+)", and the command succeeded$"#)]
async fn reads_two_lines(world: &mut FoundryWorld, first: String, second: String) {
    pl::then_reads_two_lines(world, &first, &second);
}

#[then(regex = r#"^the operator reads "([^"]+)", and the command succeeded$"#)]
async fn reads_line(world: &mut FoundryWorld, line: String) {
    pl::then_reads_line(world, &line);
}

#[then(
    regex = r"^(.+?) is restored, one restore record names the operator, and nothing else changed$"
)]
async fn restored(world: &mut FoundryWorld, project: String) {
    pl::then_restored(world, &project).await;
}

#[then(
    regex = r"^([A-Z]+-\d+)'s page opens for ([A-Z]\w*) at its old address, with its (\d+) comments and its attachment$"
)]
async fn issue_page_opens(world: &mut FoundryWorld, key: String, who: String, comments: usize) {
    pl::then_issue_page_opens(world, &key, &who, comments).await;
}

#[then(regex = r"^the next issue ([A-Z]\w*) files on (.+?) is ([A-Z]+-\d+)$")]
async fn next_filed(world: &mut FoundryWorld, who: String, project: String, key: String) {
    pl::then_next_filed_is(world, &who, &project, &key).await;
}

#[then(
    regex = r"^(.+?) holds exactly what it held just before it was deleted, under the same name, address, key and next issue number$"
)]
async fn holds_as_before(world: &mut FoundryWorld, project: String) {
    pl::then_holds_as_before_delete(world, &project).await;
}

#[then(regex = r#"^the command exits (\d+), saying "(.+)"$"#)]
async fn exits_saying(world: &mut FoundryWorld, code: i32, message: String) {
    pl::then_exits_saying(world, code, &message);
}

#[then(
    regex = r"^([A-Z]\w*) is gone with everyone's place on it, one delete record names ([A-Z]\w* [A-Z]\w*), (.+?) is kept apart from any team with its (\d+) issues unchanged, and nothing else changed$"
)]
async fn team_gone_project_kept(
    world: &mut FoundryWorld,
    team: String,
    actor: String,
    project: String,
    issues: i64,
) {
    pl::then_team_deleted_project_kept(world, &team, &actor, &project, issues).await;
}

#[then(
    regex = r"^each time, either the project came back and the team delete was refused naming it, or the team was deleted and the restore was refused$"
)]
async fn restore_race_outcomes(world: &mut FoundryWorld) {
    pl::then_restore_race_outcomes(world);
}

#[then(regex = r"^no project and no issue was ever removed$")]
async fn nothing_removed(world: &mut FoundryWorld) {
    pl::then_nothing_removed_in_race(world);
}
