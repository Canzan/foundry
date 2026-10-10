//! team-membership step definitions, slices 01-04 and the shared Background
//! (`tests/features/team-membership-0{1,2,3,4}-*.feature`, DISTILL 2026-10-09,
//! every scenario `@pending`).
//!
//! Every step body is one call into `support::team_membership` (the driver: the
//! typed vocabulary, the HTTP doors, the universe and its state-delta oracle). No
//! step holds business logic, control flow or SQL of its own.
//!
//! Driving ports (DESIGN DDD-2, Driving Ports 1-10): `GET /teams`,
//! `GET|POST /team/{slug}/members`, `POST /team/{slug}/members/{user_id}/role`,
//! `POST /team/{slug}/members/{user_id}/remove`, the dashboard `GET /`, the shipped
//! `POST /workspace/members/remove`, and the unchanged board
//! `GET /team/{t}/project/{p}`. Layer 3 (real HTTP, real Postgres): example-only,
//! sad paths enumerated (Mandates 9 and 11).
//!
//! Phrases are global across the suite (cucumber-rs has one registry), so every
//! regex is anchored and quoted copy is `[^"]+`: `she sees "Auth v2" and …` (iapr)
//! must not match this file's `she sees "…"`.

use crate::support::team_membership as tm;
use crate::support::team_membership::{Race, Submission};
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};

// ---------------------------------------------------------------- Background

#[given(regex = r"^Canzan Labs is the instance's original workspace, with these teams:$")]
async fn home_with_teams(world: &mut FoundryWorld, step: &Step) {
    tm::given_home_teams(world, step).await;
}

#[given(regex = r"^these people belong to Canzan Labs:$")]
async fn roster(world: &mut FoundryWorld, step: &Step) {
    tm::given_roster(world, step).await;
}

#[given(
    regex = r"^Globex is another workspace, whose team ([A-Z]\w*) is led by ([A-Z]\w* [A-Z]\w*) \((\S+@\S+)\)$"
)]
async fn globex(world: &mut FoundryWorld, team: String, name: String, email: String) {
    tm::seed_foreign_workspace(world, &team, &name, &email).await;
}

// ------------------------------------------------------------------ Givens

#[given(regex = r#"^Jeff's Keycloak account "([^"]+)" has signed in for the first time$"#)]
async fn keycloak_account(world: &mut FoundryWorld, email: String) {
    tm::given_keycloak_account(world, &email).await;
}

#[given(regex = r"^(.+?) is on no team and cannot open the (.+) board$")]
async fn on_no_team_no_board(world: &mut FoundryWorld, who: String, project: String) {
    tm::given_on_no_team_and_no_board(world, &who, &project).await;
}

#[given(regex = r"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) opened ([A-Z]\w*)'s members page earlier$")]
async fn opened_earlier(world: &mut FoundryWorld, who: String, team: String) {
    tm::given_opened_earlier(world, &who, &team).await;
}

#[given(regex = r"^([A-Z]\w*(?: [A-Z]\w*)?) has added ([A-Z]\w*(?: [A-Z]\w*)?) to ([A-Z]\w*)$")]
async fn has_added(world: &mut FoundryWorld, who: String, target: String, team: String) {
    tm::given_has_added(world, &who, &target, &team).await;
}

#[given(
    regex = r"^([A-Z]\w*(?: [A-Z]\w*)?) has made ([A-Z]\w*(?: [A-Z]\w*)?) a (lead|member) of ([A-Z]\w*)$"
)]
async fn has_made(
    world: &mut FoundryWorld,
    who: String,
    target: String,
    role: String,
    team: String,
) {
    tm::given_has_made(world, &who, &target, &role, &team).await;
}

#[given(regex = r"^([A-Z]\w*(?: [A-Z]\w*)?) has removed ([A-Z]\w*(?: [A-Z]\w*)?) from ([A-Z]\w*)$")]
async fn has_removed(world: &mut FoundryWorld, who: String, target: String, team: String) {
    tm::given_has_removed(world, &who, &target, &team).await;
}

#[given(regex = r"^([A-Z]\w*(?: [A-Z]\w*)?) has since left Canzan Labs$")]
async fn has_left(world: &mut FoundryWorld, who: String) {
    tm::given_left_workspace(world, &who).await;
}

#[given(regex = r"^everyone in Canzan Labs is on ([A-Z]\w*)$")]
async fn everyone_on(world: &mut FoundryWorld, team: String) {
    tm::given_everyone_on(world, &team).await;
}

#[given(regex = r"^([A-Z]\w* [A-Z]\w*) is ([A-Z]\w*)'s only lead$")]
async fn only_lead(world: &mut FoundryWorld, who: String, team: String) {
    tm::given_only_lead(world, &who, &team).await;
}

#[given(regex = r"^([A-Z]\w* [A-Z]\w*) and ([A-Z]\w* [A-Z]\w*) are ([A-Z]\w*)'s only leads$")]
async fn only_leads(world: &mut FoundryWorld, a: String, b: String, team: String) {
    tm::given_only_leads(world, &[a, b], &team).await;
}

#[given(
    regex = r"^Canzan Labs also has a team ([A-Z]\w*), on which ([A-Z]\w* [A-Z]\w*) and ([A-Z]\w* [A-Z]\w*) are both Members$"
)]
async fn leaderless_team(world: &mut FoundryWorld, team: String, a: String, b: String) {
    tm::given_leaderless_team(world, &team, &[a, b]).await;
}

#[given(
    regex = r"^([A-Z]\w* [A-Z]\w*) can see (.+) on (?:her|his|its) dashboard and open its board$"
)]
async fn can_see_project(world: &mut FoundryWorld, who: String, project: String) {
    tm::then_sees_project_and_opens_board(world, &who, &project).await;
}

// ------------------------------------------------------------------- Whens

#[when(regex = r"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) follows Teams from (?:her|his) dashboard$")]
async fn follows_teams(world: &mut FoundryWorld, who: String) {
    tm::when_follows_teams(world, &who).await;
}

#[when(regex = r"^(a signed-out visitor|she|he|[A-Z]\w*(?: [A-Z]\w*)?) opens the teams page$")]
async fn opens_teams_page(world: &mut FoundryWorld, who: String) {
    tm::when_opens_teams_page(world, &who).await;
}

#[when(regex = r"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) opens ([A-Z]\w*) from the teams page$")]
async fn opens_team_from_index(world: &mut FoundryWorld, who: String, team: String) {
    tm::when_opens_team_from_teams_page(world, &who, &team).await;
}

#[when(regex = r"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) opens ([A-Z]\w*)'s members page$")]
async fn opens_members_page(world: &mut FoundryWorld, who: String, team: String) {
    tm::when_opens_members_page(world, &who, &team).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*(?: [A-Z]\w*)?) asks for the members page of (a team that does not exist|[A-Z]\w*)$"
)]
async fn asks_members_page(world: &mut FoundryWorld, who: String, team: String) {
    tm::when_asks_members_page(world, &who, &team).await;
}

#[when(
    regex = r#"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) adds ("[^"]+"|[A-Z]\w*(?: [A-Z]\w*)?) to ([A-Z]\w*)( from the page (?:she|he) opened earlier)?$"#
)]
async fn adds(
    world: &mut FoundryWorld,
    who: String,
    target: String,
    team: String,
    earlier: String,
) {
    tm::when_adds(world, &who, &target, &team, Some(earlier)).await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) makes (herself|himself|[A-Z]\w*(?: [A-Z]\w*)?) a (lead|member) of ([A-Z]\w*)( from the page (?:she|he) opened earlier)?$"
)]
async fn makes(
    world: &mut FoundryWorld,
    who: String,
    target: String,
    role: String,
    team: String,
    earlier: String,
) {
    tm::when_makes(world, &who, &target, &role, &team, Some(earlier)).await;
}

#[when(
    regex = r"^(she|he|[A-Z]\w*(?: [A-Z]\w*)?) removes (herself|himself|[A-Z]\w*(?: [A-Z]\w*)?) from ([A-Z]\w*)( from the page (?:she|he) opened earlier)?$"
)]
async fn removes(
    world: &mut FoundryWorld,
    who: String,
    target: String,
    team: String,
    earlier: String,
) {
    tm::when_removes(world, &who, &target, &team, Some(earlier)).await;
}

#[when(regex = r"^([A-Z]\w*(?: [A-Z]\w*)?) removes ([A-Z]\w* [A-Z]\w*) from Canzan Labs$")]
async fn removes_from_workspace(world: &mut FoundryWorld, who: String, target: String) {
    tm::when_removes_from_workspace(world, &who, &target).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*(?: [A-Z]\w*)?) submits an add of (someone who never existed|a garbled name|[A-Z]\w*(?: [A-Z]\w*)?) to (a team that does not exist|[A-Z]\w*)( without the page's token)?$"
)]
async fn submits_add(
    world: &mut FoundryWorld,
    who: String,
    target: String,
    team: String,
    tokenless: String,
) {
    tm::when_submits(
        world,
        &who,
        Submission::Add,
        &target,
        &team,
        tokenless.is_empty(),
    )
    .await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*(?: [A-Z]\w*)?) submits making (someone who never existed|a garbled name|[A-Z]\w*(?: [A-Z]\w*)?) an? (\w+) of (a team that does not exist|[A-Z]\w*)$"
)]
async fn submits_role(
    world: &mut FoundryWorld,
    who: String,
    target: String,
    role: String,
    team: String,
) {
    tm::when_submits(world, &who, Submission::Role(role), &target, &team, true).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*(?: [A-Z]\w*)?) submits removing (someone who never existed|a garbled name|[A-Z]\w*(?: [A-Z]\w*)?) from (a team that does not exist|[A-Z]\w*)$"
)]
async fn submits_remove(world: &mut FoundryWorld, who: String, target: String, team: String) {
    tm::when_submits(world, &who, Submission::Remove, &target, &team, true).await;
}

#[when(
    regex = r"^([A-Z]\w*) makes ([A-Z]\w* [A-Z]\w*) and ([A-Z]\w* [A-Z]\w*) members at the same moment, (\d+) times over$"
)]
async fn race_demote_both(
    world: &mut FoundryWorld,
    who: String,
    a: String,
    b: String,
    times: usize,
) {
    tm::when_race(world, &who, Race::DemoteBoth, &a, &b, times).await;
}

#[when(
    regex = r"^([A-Z]\w*) removes ([A-Z]\w* [A-Z]\w*) and makes ([A-Z]\w* [A-Z]\w*) a member at the same moment, (\d+) times over$"
)]
async fn race_remove_demote(
    world: &mut FoundryWorld,
    who: String,
    a: String,
    b: String,
    times: usize,
) {
    tm::when_race(world, &who, Race::RemoveFirstDemoteSecond, &a, &b, times).await;
}

// ------------------------------------------------------------------- Thens

#[then(regex = r#"^(?:she|he|[A-Z]\w*(?: [A-Z]\w*)?) sees "([^"]+)"$"#)]
async fn sees(world: &mut FoundryWorld, copy: String) {
    tm::then_sees(world, &copy);
}

#[then(regex = r#"^(?:she|he|[A-Z]\w*(?: [A-Z]\w*)?) is told "([^"]+)"$"#)]
async fn is_told(world: &mut FoundryWorld, copy: String) {
    tm::then_is_told(world, &copy);
}

#[then(regex = r"^the answer is the page that never existed$")]
async fn never_existed(world: &mut FoundryWorld) {
    tm::then_never_existed(world).await;
}

#[then(regex = r"^no one's membership changed$")]
async fn nothing_changed(world: &mut FoundryWorld) {
    tm::then_nothing_changed(world).await;
}

#[then(regex = r"^(.+?) is now a (Lead|Member) of ([A-Z]\w*), and nothing else changed$")]
async fn now_role(world: &mut FoundryWorld, who: String, role: String, team: String) {
    tm::then_now_role(world, &who, &role, &team).await;
}

#[then(regex = r"^(.+?) is no longer on ([A-Z]\w*), and nothing else changed$")]
async fn no_longer_on(world: &mut FoundryWorld, who: String, team: String) {
    tm::then_no_longer_on(world, &who, &team).await;
}

#[then(
    regex = r"^([A-Z]\w* [A-Z]\w*) is no longer in Canzan Labs or on any of its teams, and nothing else changed$"
)]
async fn left_workspace(world: &mut FoundryWorld, who: String) {
    tm::then_left_workspace_and_teams(world, &who).await;
}

#[then(regex = r"^the teams page lists exactly:$")]
async fn teams_page_lists(world: &mut FoundryWorld, step: &Step) {
    tm::then_teams_page_lists(world, &tm::table_column(step));
}

#[then(regex = r"^the teams page lists no team$")]
async fn teams_page_lists_none(world: &mut FoundryWorld) {
    tm::then_teams_page_lists(world, &[]);
}

#[then(regex = r"^([A-Z]\w*)'s members page lists, in this order:$")]
async fn members_in_order(world: &mut FoundryWorld, team: String, step: &Step) {
    tm::then_members_in_order(world, &team, &tm::table_column(step)).await;
}

#[then(regex = r#"^([A-Z]\w*)'s members page lists "([^"]+)"( once)?$"#)]
async fn members_lists(world: &mut FoundryWorld, team: String, row: String, once: String) {
    tm::then_members_lists(world, &team, &row, !once.is_empty()).await;
}

#[then(regex = r"^([A-Z]\w*)'s members page no longer lists ([A-Z]\w* [A-Z]\w*)$")]
async fn members_not_listing(world: &mut FoundryWorld, team: String, who: String) {
    tm::then_members_not_listing(world, &team, &who).await;
}

#[then(regex = r"^the page offers adding, changing roles and removing$")]
async fn offers_controls(world: &mut FoundryWorld) {
    tm::then_controls(world, true);
}

#[then(regex = r"^the page offers no way to add, change a role or remove anyone$")]
async fn offers_no_controls(world: &mut FoundryWorld) {
    tm::then_controls(world, false);
}

#[then(regex = r"^Add a member on ([A-Z]\w*) offers exactly:$")]
async fn offers_exactly(world: &mut FoundryWorld, team: String, step: &Step) {
    tm::then_offers_exactly(world, &team, &tm::table_column(step));
}

#[then(regex = r#"^Add a member is replaced by "([^"]+)"$"#)]
async fn add_replaced(world: &mut FoundryWorld, copy: String) {
    tm::then_add_replaced_by(world, &copy);
}

#[then(
    regex = r"^(.+?)(?:, still signed in,)? sees (.+) on (?:her|his|its) dashboard and opens its board$"
)]
async fn sees_project(world: &mut FoundryWorld, who: String, project: String) {
    tm::then_sees_project_and_opens_board(world, &who, &project).await;
}

#[then(regex = r"^(.+?)'s (?:next )?visit to the (.+) board gets the page that never existed$")]
async fn board_never_existed(world: &mut FoundryWorld, who: String, project: String) {
    tm::then_board_never_existed(world, &who, &project).await;
}

#[then(regex = r#"^([A-Z]\w* [A-Z]\w*)'s dashboard says "([^"]+)"$"#)]
async fn dashboard_says(world: &mut FoundryWorld, who: String, copy: String) {
    tm::then_dashboard_says(world, &who, &copy).await;
}

#[then(regex = r"^([A-Z]\w*) still finds ([A-Z]\w*)'s members page$")]
async fn still_finds(world: &mut FoundryWorld, who: String, team: String) {
    tm::then_still_finds(world, &who, &team).await;
}

#[then(regex = r"^([A-Z]\w*)'s teams page still lists ([A-Z]\w*)$")]
async fn teams_page_still_lists(world: &mut FoundryWorld, who: String, team: String) {
    tm::then_teams_page_still_lists(world, &who, &team).await;
}

#[then(regex = r"^the answer is turned away as a forged form$")]
async fn forged_form(world: &mut FoundryWorld) {
    tm::then_forged_form(world);
}

#[then(regex = r"^the same add with the page's token puts ([A-Z]\w* [A-Z]\w*) on ([A-Z]\w*)$")]
async fn same_add_with_token(world: &mut FoundryWorld, who: String, team: String) {
    tm::then_same_add_with_token(world, &who, &team).await;
}

#[then(regex = r"^(?:she|he) gets ([A-Z]\w*)'s members page back, with nothing to report$")]
async fn nothing_to_report(world: &mut FoundryWorld, team: String) {
    tm::then_nothing_to_report(world, &team);
}

#[then(regex = r#"^each time, exactly one went through and the other was told "([^"]+)"$"#)]
async fn race_one_each(world: &mut FoundryWorld, copy: String) {
    tm::then_race_one_each(world, &copy);
}

#[then(regex = r"^each time, ([A-Z]\w*) was left with exactly one lead$")]
async fn race_one_lead(world: &mut FoundryWorld, team: String) {
    tm::then_race_one_lead(world, &team);
}

#[then(regex = r"^([A-Z]\w* [A-Z]\w*) is sent no notice that (?:she|he) was removed$")]
async fn no_removal_notice(world: &mut FoundryWorld, who: String) {
    tm::then_sent_no_removal_notice(world, &who);
}
