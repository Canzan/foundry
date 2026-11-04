//! integrations-wayfinding step definitions
//! (`tests/features/integrations-wayfinding.feature`, DISTILL 2026-11-03,
//! scaffolded RED with every scenario `@pending` but the `@control` ones;
//! DELIVER un-pended all of them, 2026-11-03).
//!
//! Every step body is one call into `support::integrations_wayfinding` (or the
//! eip driver it builds on); the words are parsed by the driver's types
//! (`Kind`, `RouteDoor`, `Offer`, `Fix`). Reused unchanged from the eip step
//! file: the Background, "the instance has …", "Canzan Labs has no Slack or
//! Matrix connection", "Priya has connected Canzan Labs to Slack", the
//! uninstall event, "<who> opens <page>", "<who> reads …", "<who> is on
//! <page>", "<project> still has no route", the no-access doors and "gets the
//! same not-found page". No step holds logic of its own (Mandate 12,
//! criterion 3).
//!
//! Phrases are global across the suite, so every regex is anchored and names
//! something only this feature says ("in the sidebar", "a new <kind> route
//! for", "no way to fix it here", "lands <who> on", "the Slack workspace").

use crate::support::external_integration_plugins::{self as eip, Kind};
use crate::support::integrations_wayfinding::{self as iw, Fix, Offer, RouteDoor};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

// =========================================================================== instance

#[given(regex = r#"^Priya has connected Canzan Labs to the Slack workspace "([^"]+)"$"#)]
async fn connected_to_team(world: &mut FoundryWorld, team: String) {
    eip::given_slack_connected_to(world, &team).await;
}

#[given(regex = r"^Foundry has since been given a new secrets key without the old one$")]
async fn key_replaced(world: &mut FoundryWorld) {
    iw::given_key_replaced(world).await;
}

#[given(regex = r"^Foundry has since lost its secrets key$")]
async fn key_removed(world: &mut FoundryWorld) {
    iw::given_key_removed(world).await;
}

#[given(regex = r"^the Foundry Slack app has since been taken off this instance$")]
async fn app_taken_off(world: &mut FoundryWorld) {
    iw::given_slack_app_removed(world).await;
}

// =========================================================================== sidebar (US-IW-01)

#[when(
    regex = r#"^(Priya) follows "Integrations" in the sidebar on (Identity Platform's board|Identity Platform's integrations page|a new Slack route on Identity Platform)$"#
)]
async fn follow_sidebar(world: &mut FoundryWorld, who: String, page: String) {
    iw::when_follow_sidebar(world, &who, &page).await;
}

#[then(
    regex = r#"^(Priya|Lena|Sam|Marco) is (offered|not offered) "Integrations" in the sidebar$"#
)]
async fn sidebar_offered(world: &mut FoundryWorld, who: String, offered: String) {
    iw::then_sidebar_offered(world, &who, Offer::parse(&offered));
}

#[then(regex = r"^(Priya)'s sidebar lists (.+) in that order$")]
async fn sidebar_lists(world: &mut FoundryWorld, who: String, items: String) {
    iw::then_sidebar_lists(world, &who, &items);
}

#[then(regex = r"^Home is the sidebar's current page, not Integrations$")]
async fn home_current(world: &mut FoundryWorld) {
    iw::then_home_current(world, "Priya");
}

// =========================================================================== route form (US-IW-02)

#[when(regex = r"^(Priya|Lena) opens a new (Slack|Matrix) route for (Identity Platform)$")]
async fn open_new_route(world: &mut FoundryWorld, who: String, kind: String, project: String) {
    iw::when_open_new_route(world, &who, Kind::parse(&kind), &project).await;
}

#[when(
    regex = r"^(Priya|Lena) (previews|saves|sends a test from) a new (Slack|Matrix) route for (Identity Platform)$"
)]
async fn post_new_route(
    world: &mut FoundryWorld,
    who: String,
    door: String,
    kind: String,
    project: String,
) {
    iw::when_post_new_route(
        world,
        &who,
        RouteDoor::parse(&door),
        Kind::parse(&kind),
        &project,
    )
    .await;
}

#[then(regex = r#"^(Priya|Lena) does not read "(.+)"$"#)]
async fn does_not_read(world: &mut FoundryWorld, who: String, words: String) {
    iw::then_does_not_read(world, &who, &words);
}

#[then(regex = r#"^(Priya|Lena) is offered (?:the link "([^"]+)"|no way to fix it here)$"#)]
async fn fix_offered(world: &mut FoundryWorld, who: String, link: String) {
    iw::then_fix_offered(world, &who, Fix::parse(&link)).await;
}

// =========================================================================== project page (US-IW-03)

#[then(regex = r#"^(Priya|Lena) is (offered|not offered) a "([^"]+)" link$"#)]
async fn link_offered(world: &mut FoundryWorld, who: String, offered: String, link: String) {
    iw::then_link(world, &who, &link, Offer::parse(&offered));
}

#[then(
    regex = r#"^following "([^"]+)" lands (Priya|Lena) on (the workspace integrations page|the Slack setup page)$"#
)]
async fn following_lands(world: &mut FoundryWorld, link: String, who: String, page: String) {
    iw::then_following_lands(world, &who, &link, &page).await;
}
