//! team-lifecycle step definitions, slices 01-04
//! (`tests/features/team-lifecycle-0{1,2,3,4}-*.feature`, DISTILL 2026-10-11,
//! every scenario `@pending`).
//!
//! Every step body is one call into `support::team_lifecycle` (the driver: this
//! feature's vocabulary, doors, universe and state-delta oracle), which builds
//! on `support::team_membership`. The Background, sessions, the members and
//! teams pages, the uniform-404 oracle, "she sees", "she is told", the forged
//! form and the default-team steps are team-membership's, reused unchanged from
//! `feature_team_membership*.rs`. No step holds business logic, control flow or
//! SQL of its own.
//!
//! Driving ports (DESIGN Driving Ports 1-7, DDD-17, DDD-21): `GET`/`POST
//! /teams`, `GET /team/{slug}/members` (New project, Rename, Delete team),
//! `POST /team/{slug}/rename`, `GET`/`POST /team/{slug}/delete`, the dashboard
//! `GET /`, the shipped project create and board, `POST /workspace/switch`, and
//! the operator's database prompt (OQ-7). Layer 3 (real HTTP, real Postgres):
//! example-only, sad paths enumerated (Mandates 9 and 11).
//!
//! Phrases are global across the suite (cucumber-rs has one registry), so every
//! regex is anchored and none overlaps a shipped one.

use crate::support::team_lifecycle as tl;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};

// ------------------------------------------------------------------ Givens

#[given(
    regex = r"^Homelab is a newly provisioned workspace with no teams, whose workspace admin is ([A-Z]\w* [A-Z]\w*)$"
)]
async fn homelab(world: &mut FoundryWorld, admin: String) {
    tl::given_homelab(world, &admin).await;
}

#[given(regex = r"^(she|he|[A-Z]\w*) is working in (Homelab|Canzan Labs)$")]
async fn working_in(world: &mut FoundryWorld, who: String, workspace: String) {
    tl::given_working_in(world, &who, &workspace).await;
}

#[given(
    regex = r"^([A-Z]\w* [A-Z]\w*) is an instance admin of this foundry, but not a workspace admin$"
)]
async fn instance_admin(world: &mut FoundryWorld, who: String) {
    tl::given_instance_admin(world, &who).await;
}

#[given(regex = r#"^([A-Z]\w*) has created the team "([^"]+)"$"#)]
async fn has_created(world: &mut FoundryWorld, who: String, name: String) {
    tl::given_has_created(world, &who, &name).await;
}

#[given(regex = r"^Canzan Labs also has these teams, made before team create existed:$")]
async fn legacy_teams(world: &mut FoundryWorld, step: &Step) {
    tl::given_legacy_teams(world, step).await;
}

#[given(regex = r"^([A-Z]\w*) has these projects:$")]
async fn has_projects(world: &mut FoundryWorld, team: String, step: &Step) {
    tl::given_projects(world, &team, step).await;
}

#[given(regex = r"^([A-Z]\w*) has (\d+) machine tokens? scoped to it$")]
async fn scoped_tokens(world: &mut FoundryWorld, team: String, count: usize) {
    tl::given_scoped_tokens(world, &team, count).await;
}

#[given(regex = r"^(she|[A-Z]\w*) opened ([A-Z]\w*)'s delete page earlier$")]
async fn opened_delete_earlier(world: &mut FoundryWorld, who: String, team: String) {
    tl::given_opened_delete_earlier(world, &who, &team).await;
}

#[given(regex = r"^([A-Z]\w*) has deleted ([A-Z]\w*)$")]
async fn has_deleted(world: &mut FoundryWorld, who: String, team: String) {
    tl::given_has_deleted(world, &who, &team).await;
}

#[given(regex = r#"^([A-Z]\w*) has renamed ([A-Z]\w*) to "([^"]+)"$"#)]
async fn has_renamed(world: &mut FoundryWorld, who: String, team: String, name: String) {
    tl::given_has_renamed(world, &who, &team, &name).await;
}

#[given(
    regex = r#"^([A-Z]\w*) has since created the project "([^"]+)" with key ([A-Z]+) on ([A-Z]\w*)$"#
)]
async fn has_created_project(
    world: &mut FoundryWorld,
    who: String,
    project: String,
    key: String,
    team: String,
) {
    tl::given_has_created_project(world, &who, &project, &key, &team).await;
}

// ------------------------------------------------------------------- Whens

#[when(regex = r#"^(she|he|[A-Z]\w*) creates the team "([^"]*)" from the teams page$"#)]
async fn creates_from_page(world: &mut FoundryWorld, who: String, name: String) {
    tl::when_creates_from_page(world, &who, &name).await;
}

#[when(regex = r#"^(she|he|[A-Z]\w*) sends the create form for "[^"]+" a second time$"#)]
async fn sends_create_again(world: &mut FoundryWorld, who: String) {
    tl::when_sends_create_again(world, &who).await;
}

#[when(
    regex = r#"^(a signed-out visitor|[A-Z]\w*) submits a team create named "([^"]*)"( without the page's token)?$"#
)]
async fn submits_create(world: &mut FoundryWorld, who: String, name: String, tokenless: String) {
    tl::when_submits_create(world, &who, &name, tokenless.is_empty()).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*) follows "create your first project" from (?:her|his) dashboard$"#
)]
async fn follows_first_project(world: &mut FoundryWorld, who: String) {
    tl::when_follows_first_project_link(world, &who).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*) follows New project on ([A-Z]\w*)'s members page and creates "([^"]+)" with key ([A-Z]+)$"#
)]
async fn new_project(
    world: &mut FoundryWorld,
    who: String,
    team: String,
    project: String,
    key: String,
) {
    tl::when_new_project_from_members_page(world, &who, &team, &project, &key).await;
}

#[when(regex = r"^(she|he|[A-Z]\w*) follows Delete team on ([A-Z]\w*)'s members page$")]
async fn follows_delete(world: &mut FoundryWorld, who: String, team: String) {
    tl::when_follows_delete(world, &who, &team).await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*) confirms the delete( from the page (?:she|he) opened earlier)?$"
)]
async fn confirms(world: &mut FoundryWorld, who: String, earlier: String) {
    tl::when_confirms(world, &who, !earlier.is_empty()).await;
}

#[when(regex = r"^(she|he|[A-Z]\w*) follows Cancel$")]
async fn follows_cancel(world: &mut FoundryWorld, who: String) {
    tl::when_follows_cancel(world, &who).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*) asks for the delete page of (a team that does not exist|[A-Z]\w*)$"
)]
async fn asks_delete_page(world: &mut FoundryWorld, who: String, team: String) {
    tl::when_asks_delete_page(world, &who, &team).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*) submits a delete of (a team that does not exist|[A-Z]\w*)( without the page's token)?$"
)]
async fn submits_delete(world: &mut FoundryWorld, who: String, team: String, tokenless: String) {
    tl::when_submits_delete(world, &who, &team, tokenless.is_empty()).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*) renames ([A-Z]\w*) to "([^"]*)"( from the page (?:she|he) opened earlier)?$"#
)]
async fn renames(
    world: &mut FoundryWorld,
    who: String,
    team: String,
    name: String,
    earlier: String,
) {
    tl::when_renames(world, &who, &team, &name, !earlier.is_empty()).await;
}

#[when(regex = r"^(she|he|[A-Z]\w*) submits ([A-Z]\w*)'s rename form unchanged$")]
async fn rename_unchanged(world: &mut FoundryWorld, who: String, team: String) {
    tl::when_submits_rename_unchanged(world, &who, &team).await;
}

#[when(
    regex = r#"^(a signed-out visitor|[A-Z]\w*) submits a rename of (a team that does not exist|[A-Z]\w*) to "([^"]*)"( without the page's token)?$"#
)]
async fn submits_rename(
    world: &mut FoundryWorld,
    who: String,
    team: String,
    name: String,
    tokenless: String,
) {
    tl::when_submits_rename(world, &who, &team, &name, tokenless.is_empty()).await;
}

#[when(
    regex = r#"^([A-Z]\w*) deletes ([A-Z]\w*) at the same moment as ([A-Z]\w*) creates "([^"]+)" with key ([A-Z]+) on it, (\d+) times over$"#
)]
async fn delete_race(
    world: &mut FoundryWorld,
    admin: String,
    team: String,
    creator: String,
    project: String,
    key: String,
    times: usize,
) {
    tl::when_delete_races_create(world, &admin, &team, &creator, &project, &key, times).await;
}

#[when(
    regex = r#"^([A-Z]\w*) and ([A-Z]\w*) create the team "([^"]+)" at the same moment, (\d+) times over$"#
)]
async fn same_name_race(
    world: &mut FoundryWorld,
    a: String,
    b: String,
    name: String,
    times: usize,
) {
    tl::when_create_race(world, (&a, &name), (&b, &name), times).await;
}

#[when(
    regex = r#"^([A-Z]\w*) creates the team "([^"]+)" at the same moment as ([A-Z]\w*) creates "([^"]+)", (\d+) times over$"#
)]
async fn two_names_race(
    world: &mut FoundryWorld,
    a: String,
    first: String,
    b: String,
    second: String,
    times: usize,
) {
    tl::when_create_race(world, (&a, &first), (&b, &second), times).await;
}

#[when(
    regex = r#"^the operator adds a team named "([^"]*)" to Canzan Labs at the database prompt$"#
)]
async fn operator_adds_team(world: &mut FoundryWorld, name: String) {
    tl::when_operator_adds_team(world, &name).await;
}

#[when(regex = r#"^the operator renames team ([A-Z]\w*) to "([^"]*)" at the database prompt$"#)]
async fn operator_renames_team(world: &mut FoundryWorld, team: String, name: String) {
    tl::when_operator_renames_team(world, &team, &name).await;
}

// ------------------------------------------------------------------- Thens

#[then(regex = r#"^(?:she|he) is refused with "([^"]+)", and (?:her|his) text "([^"]*)" is kept$"#)]
async fn refused_with(world: &mut FoundryWorld, copy: String, typed: String) {
    tl::then_refused_with(world, &copy, &typed);
}

#[then(regex = r"^no team, project or membership changed$")]
async fn nothing_changed(world: &mut FoundryWorld) {
    tl::then_nothing_changed(world).await;
}

#[then(
    regex = r#"^(Homelab|Canzan Labs) now has the team "([^"]+)" at the address "([^"]+)", led by ([A-Z]\w* [A-Z]\w*), and nothing else changed$"#
)]
async fn now_has_team(
    world: &mut FoundryWorld,
    workspace: String,
    name: String,
    slug: String,
    lead: String,
) {
    tl::then_now_has_team(world, &workspace, &name, &slug, &lead).await;
}

#[then(
    regex = r"^([A-Z]\w*) is gone with everyone's place on it(?: and its (\d+) machine tokens?)?(, Canzan Labs has no default team)?, one delete record names ([A-Z]\w* [A-Z]\w*), and nothing else changed$"
)]
async fn gone(
    world: &mut FoundryWorld,
    team: String,
    tokens: String,
    default: String,
    actor: String,
) {
    tl::then_deleted(
        world,
        &team,
        tokens.parse().ok(),
        !default.is_empty(),
        &actor,
    )
    .await;
}

#[then(
    regex = r#"^([A-Z]\w*) is now called "([^"]+)" at the same address, one rename record names ([A-Z]\w* [A-Z]\w*), and nothing else changed$"#
)]
async fn now_called(world: &mut FoundryWorld, team: String, name: String, actor: String) {
    tl::then_renamed(world, &team, &name, &actor).await;
}

#[then(regex = r#"^the (.+) board opens, and (?:her|his) dashboard lists "([^"]+)"$"#)]
async fn board_opens(world: &mut FoundryWorld, project: String, entry: String) {
    tl::then_board_opens_and_dashboard_lists(world, &project, &entry).await;
}

#[then(regex = r"^(?:she|he) gets ([A-Z]\w*)'s new project form$")]
async fn new_project_form(world: &mut FoundryWorld, team: String) {
    tl::then_new_project_form(world, &team);
}

#[then(regex = r"^the members page (offers|does not offer) (New project|Rename|Delete team)$")]
async fn members_page_offers(world: &mut FoundryWorld, offers: String, control: String) {
    tl::then_members_page_offers(world, offers == "offers", &control);
}

#[then(regex = r"^the teams page offers (creating a team|no way to create a team)$")]
async fn teams_page_offers(world: &mut FoundryWorld, what: String) {
    tl::then_teams_page_offers_create(world, what == "creating a team");
}

#[then(regex = r"^([A-Z]\w*)'s teams page still offers creating a team$")]
async fn still_offers_create(world: &mut FoundryWorld, who: String) {
    tl::then_still_offers_create(world, &who).await;
}

#[then(regex = r#"^([A-Z]\w*)'s teams page (lists|does not list) "([^"]+)"$"#)]
async fn teams_page_lists(world: &mut FoundryWorld, who: String, lists: String, entry: String) {
    tl::then_teams_page_lists(world, &who, &entry, lists == "lists").await;
}

#[then(regex = r#"^the delete page asks "([^"]+)"$"#)]
async fn delete_page_asks(world: &mut FoundryWorld, question: String) {
    tl::then_delete_page_asks(world, &question);
}

#[then(regex = r#"^the delete page says "([^"]+)"$"#)]
async fn delete_page_says(world: &mut FoundryWorld, copy: String) {
    tl::then_delete_page_says(world, &copy);
}

#[then(regex = r"^the delete page says nothing about the default team or machine tokens$")]
async fn delete_page_silent(world: &mut FoundryWorld) {
    tl::then_delete_page_silent_on_default_and_tokens(world);
}

#[then(regex = r"^the delete page offers (Delete team and Cancel|no Delete team)$")]
async fn delete_page_offers(world: &mut FoundryWorld, what: String) {
    tl::then_delete_page_offers(world, what == "Delete team and Cancel");
}

#[then(regex = r"^(?:she|he) is back on ([A-Z]\w*)'s members page$")]
async fn back_on_members(world: &mut FoundryWorld, team: String) {
    tl::then_back_on_members_page(world, &team);
}

#[then(regex = r"^(?:she|he) gets ([A-Z]\w*)'s members page back with no notice$")]
async fn no_notice(world: &mut FoundryWorld, team: String) {
    tl::then_no_notice(world, &team);
}

#[then(regex = r"^([A-Z]\w*) still finds ([A-Z]\w*)'s delete page$")]
async fn still_finds_delete(world: &mut FoundryWorld, who: String, team: String) {
    tl::then_still_finds_delete_page(world, &who, &team).await;
}

#[then(regex = r"^([A-Z]\w*) still finds ([A-Z]\w*)'s rename form$")]
async fn still_finds_rename(world: &mut FoundryWorld, who: String, team: String) {
    tl::then_still_finds_rename_form(world, &who, &team).await;
}

#[then(regex = r#"^the members page at the address "([^"]+)" opens for ([A-Z]\w*)$"#)]
async fn members_page_at(world: &mut FoundryWorld, slug: String, who: String) {
    tl::then_members_page_at_opens(world, &slug, &who).await;
}

#[then(regex = r"^the (.+) board still opens for ([A-Z]\w*)$")]
async fn board_still_opens(world: &mut FoundryWorld, project: String, who: String) {
    tl::then_board_still_opens(world, &project, &who).await;
}

#[then(regex = r#"^the same create with the page's token creates "([^"]+)"$"#)]
async fn same_create(world: &mut FoundryWorld, name: String) {
    tl::then_same_create_with_token(world, &name).await;
}

#[then(regex = r"^the same delete with the page's token deletes ([A-Z]\w*)$")]
async fn same_delete(world: &mut FoundryWorld, team: String) {
    tl::then_same_delete_with_token(world, &team).await;
}

#[then(regex = r#"^the same rename with the page's token renames ([A-Z]\w*) to "([^"]+)"$"#)]
async fn same_rename(world: &mut FoundryWorld, team: String, name: String) {
    tl::then_same_rename_with_token(world, &team, &name).await;
}

#[then(
    regex = r"^each time, either the project was kept and the delete was refused, or the team was deleted and the create found no team$"
)]
async fn delete_race_outcomes(world: &mut FoundryWorld) {
    tl::then_delete_race_outcomes(world);
}

#[then(regex = r"^no answer was a server error$")]
async fn no_server_error(world: &mut FoundryWorld) {
    tl::then_no_server_error(world);
}

#[then(regex = r"^no project was ever lost$")]
async fn no_project_lost(world: &mut FoundryWorld) {
    tl::then_no_project_lost(world);
}

#[then(
    regex = r#"^each time, exactly one "([^"]+)" team exists and the other admin was refused with "([^"]+)"$"#
)]
async fn one_team_each_time(world: &mut FoundryWorld, name: String, copy: String) {
    tl::then_one_team_each_time(world, &name, &copy);
}

#[then(regex = r#"^each time, both teams were created, at the addresses "([^"]+)" and "([^"]+)"$"#)]
async fn both_created(world: &mut FoundryWorld, a: String, b: String) {
    tl::then_both_created(world, &a, &b);
}

#[then(regex = r#"^the database refuses the team name under the rule "([^"]+)"$"#)]
async fn db_refuses(world: &mut FoundryWorld, arm: String) {
    tl::then_db_refuses(world, &arm);
}

#[then(regex = r"^the database stores the team name exactly as typed, and nothing else changed$")]
async fn db_stores(world: &mut FoundryWorld) {
    tl::then_db_stores_exactly(world).await;
}
