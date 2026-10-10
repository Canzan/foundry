//! team-membership step definitions, slices 05-06: the default team for new
//! members, and the two newcomer doors that apply it — invite accept and a first
//! Keycloak sign-in (`tests/features/team-membership-0{5,6}-*.feature`, DISTILL
//! 2026-10-09, every scenario `@pending`).
//!
//! The Background, the refusal oracle and the universe steps are shared with
//! `feature_team_membership` (cucumber-rs has one step registry). Every step body
//! is one call into `support::team_membership`.
//!
//! Driving ports (DESIGN DDD-2, DDD-13, DDD-14): `GET /teams` (the default-team
//! select), `POST /teams/default`, the shipped `POST /workspace/invites` and
//! `GET|POST /invites/accept`, and the shipped Keycloak start and callback
//! (`/auth/oidc/start`, `oidc::CALLBACK_PATH`) against the in-process identity
//! provider double.

use crate::support::team_membership as tm;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

// ------------------------------------------------------------------ Givens

#[given(regex = r"^([A-Z]\w*) has set (No team|[A-Z]\w*) as the default team for new members$")]
async fn has_set_default(world: &mut FoundryWorld, who: String, choice: String) {
    tm::given_default_set(world, &who, &choice).await;
}

#[given(regex = r"^Canzan Labs has no default team$")]
async fn no_default(world: &mut FoundryWorld) {
    tm::given_no_default(world).await;
}

#[given(
    regex = r"^the ([A-Z]\w*) team, once Canzan Labs's default, has since been deleted by hand$"
)]
async fn default_deleted(world: &mut FoundryWorld, team: String) {
    tm::given_default_deleted(world, &team).await;
}

// ------------------------------------------------------------------- Whens

#[when(regex = r"^([A-Z]\w*) sets (No team|[A-Z]\w*) as the default team for new members$")]
async fn sets_default(world: &mut FoundryWorld, who: String, choice: String) {
    tm::when_sets_default(world, &who, &choice).await;
}

#[when(
    regex = r"^(a signed-out visitor|[A-Z]\w*) submits (a team that does not exist|[A-Z]\w*) as Canzan Labs's default team$"
)]
async fn submits_default(world: &mut FoundryWorld, who: String, team: String) {
    tm::when_submits_default(world, &who, &team).await;
}

#[when(regex = r"^([A-Z]\w* [A-Z]\w*) accepts an invite to Canzan Labs at (\S+@\S+)$")]
async fn accepts_invite(world: &mut FoundryWorld, name: String, email: String) {
    tm::when_accepts_invite(world, &name, &email).await;
}

#[when(regex = r"^([A-Z]\w* [A-Z]\w*) signs in with Keycloak for the first time as (\S+@\S+)$")]
async fn first_keycloak_sign_in(world: &mut FoundryWorld, name: String, email: String) {
    tm::when_first_keycloak_sign_in(world, &name, &email).await;
}

#[when(regex = r"^([A-Z]\w*) signs in with Keycloak using (?:her|his) own address$")]
async fn keycloak_own_address(world: &mut FoundryWorld, who: String) {
    tm::when_keycloak_own_address(world, &who).await;
}

// ------------------------------------------------------------------- Thens

#[then(regex = r"^Canzan Labs's default team is now (No team|[A-Z]\w*), and nothing else changed$")]
async fn default_now(world: &mut FoundryWorld, choice: String) {
    tm::then_default_now(world, &choice).await;
}

#[then(regex = r"^the teams page shows (No team|[A-Z]\w*) as the default team for new members$")]
async fn teams_page_shows_default(world: &mut FoundryWorld, choice: String) {
    tm::then_teams_page_shows_default(world, &choice).await;
}

#[then(regex = r"^(.+?) is still on no team$")]
async fn still_no_team(world: &mut FoundryWorld, who: String) {
    tm::then_still_on_no_team(world, &who).await;
}

#[then(
    regex = r"^([A-Z]\w* [A-Z]\w*) has a new account in Canzan Labs and is a Member of ([A-Z]\w*), and nothing else changed$"
)]
async fn new_account_on_team(world: &mut FoundryWorld, who: String, team: String) {
    tm::then_new_account(world, &who, Some(&team)).await;
}

#[then(
    regex = r"^([A-Z]\w* [A-Z]\w*) has a new account in Canzan Labs and is on no team, and nothing else changed$"
)]
async fn new_account_no_team(world: &mut FoundryWorld, who: String) {
    tm::then_new_account(world, &who, None).await;
}

#[then(regex = r"^([A-Z]\w*(?: [A-Z]\w*)?) is signed in$")]
async fn is_signed_in(world: &mut FoundryWorld, who: String) {
    tm::then_signed_in(world, &who).await;
}

#[then(regex = r"^the teams page has no default team setting$")]
async fn no_default_setting(world: &mut FoundryWorld) {
    tm::then_no_default_setting(world);
}

#[then(regex = r"^([A-Z]\w*)'s teams page offers the default team setting$")]
async fn offers_default_setting(world: &mut FoundryWorld, who: String) {
    tm::then_offers_default_setting(world, &who).await;
}
