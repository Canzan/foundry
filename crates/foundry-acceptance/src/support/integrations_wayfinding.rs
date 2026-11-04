//! integrations-wayfinding driver (`tests/features/integrations-wayfinding.feature`,
//! DISTILL 2026-11-03).
//!
//! Extends, not duplicates, the external-integration-plugins driver: the
//! Background, people, sessions, instance settings (`rewire`), the Slack double
//! and the page readers all come from `support::external_integration_plugins`.
//! This module adds only the doors and oracles this feature names: the sidebar
//! entry (US-IW-01), the new-route form opened, previewed and saved per kind
//! (US-IW-02), and the "fix it here" links (KPI-IW-G1: every link offered is
//! followed and must answer 200).
//!
//! Every oracle reads what a person reads: page text, the page's own links
//! (sidebar excluded) and the sidebar's links. No table is read.

use crate::support::external_integration_plugins::{
    self as eip, in_sidebar, link_hrefs, Kind, NEW_SECRETS_KEY, WORKSPACE_INTEGRATIONS,
};
use crate::support::html_assertions::{parse, select_all};
use crate::support::team_membership::{self as tm, Answer};
use crate::world::FoundryWorld;

/// The sidebar entry's words (W-1).
pub const SIDEBAR_ENTRY: &str = "Integrations";

/// The door a new route is posted through (US-IW-02 AC 7; Send test added
/// at the DISTILL review, DESIGN D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDoor {
    Preview,
    Save,
    Test,
}

impl RouteDoor {
    pub fn parse(word: &str) -> Self {
        match word.trim() {
            "previews" => Self::Preview,
            "saves" => Self::Save,
            "sends a test from" => Self::Test,
            other => panic!("unknown route door {other:?}"),
        }
    }

    fn path(self) -> &'static str {
        match self {
            Self::Preview => "/routes/preview",
            Self::Save => "/routes",
            Self::Test => "/routes/test",
        }
    }
}

/// "offered" | "not offered".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Offer(pub bool);

impl Offer {
    pub fn parse(word: &str) -> Self {
        match word.trim() {
            "offered" => Self(true),
            "not offered" => Self(false),
            other => panic!("unknown offer {other:?}"),
        }
    }
}

/// "A, B and C" as a list.
pub fn items(words: &str) -> Vec<String> {
    words
        .replace(" and ", ", ")
        .split(", ")
        .map(|s| s.trim().to_string())
        .collect()
}

/// "the link <name>" (captured name) | "no way to fix it here" (empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    Link(String),
    None,
}

impl Fix {
    pub fn parse(captured: &str) -> Self {
        match captured.trim() {
            "" => Self::None,
            link => Self::Link(link.to_string()),
        }
    }
}

fn answer(world: &FoundryWorld) -> Answer {
    world
        .eip
        .when_answer
        .clone()
        .unwrap_or_else(|| tm::last(world).clone())
}

fn answered(answer: &Answer, who: &str) {
    assert!(
        answer.status.is_success(),
        "precondition: {who}'s page answered; {}",
        tm::describe(answer)
    );
}

// =========================================================================== instance

/// "Foundry has since been given a new secrets key without the old one": the
/// saved connection was sealed with the old key, so this Foundry cannot read
/// it (K-19).
pub async fn given_key_replaced(world: &mut FoundryWorld) {
    world.eip.spec.secrets_key = Some(NEW_SECRETS_KEY.to_string());
    world.eip.spec.previous_secrets_key = None;
    eip::rewire(world).await;
}

/// "Foundry has since lost its secrets key": nothing it saved can be read,
/// and nothing new can be sealed (K-19 ahead of W-3, US-IW-02 AC 6).
pub async fn given_key_removed(world: &mut FoundryWorld) {
    world.eip.spec.secrets_key = None;
    world.eip.spec.previous_secrets_key = None;
    eip::rewire(world).await;
}

/// "the Foundry Slack app has since been taken off this instance".
pub async fn given_slack_app_removed(world: &mut FoundryWorld) {
    world.eip.spec.slack_app = eip::SlackAppSetup::None;
    eip::rewire(world).await;
}

// =========================================================================== route form

fn new_route_path(world: &FoundryWorld, project: &str, kind: Kind) -> String {
    format!(
        "{}/routes/new?kind={}",
        eip::integrations_path(world, project),
        kind.wire()
    )
}

/// "<who> opens a new <kind> route for <project>".
pub async fn when_open_new_route(world: &mut FoundryWorld, who: &str, kind: Kind, project: &str) {
    let path = new_route_path(world, project, kind);
    eip::when_open(world, who, &path).await;
}

/// "<who> previews | saves a new <kind> route for <project>": the form's own
/// fields with a destination no connection could have listed, so the answer
/// depends only on the connection state, never on a channel.
pub async fn when_post_new_route(
    world: &mut FoundryWorld,
    who: &str,
    door: RouteDoor,
    kind: Kind,
    project: &str,
) {
    eip::capture_before(world).await;
    let destination = match kind {
        Kind::Slack => "C0IWNOCONNECTION",
        Kind::Matrix => "!iw-no-connection:canzan.dev",
    };
    let fields: Vec<(String, String)> = [
        ("kind", kind.wire()),
        ("destination", destination),
        ("content_level", "summary"),
        ("events", "issue.created"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let path = format!("{}{}", eip::integrations_path(world, project), door.path());
    let posted = eip::post(world, who, &path, &fields, true).await;
    let answer = eip::follow(world, who, posted).await;
    world.eip.when_answer = Some(answer);
}

// =========================================================================== reading

/// "<who> does not read <words>".
pub fn then_does_not_read(world: &FoundryWorld, who: &str, words: &str) {
    let answer = answer(world);
    answered(&answer, who);
    let seen = world.eip.names.as_scenario(&eip::text(&answer));
    assert!(
        !seen.contains(words),
        "{who} does not read {words:?}; {}",
        tm::describe(&answer)
    );
}

/// "<who> is offered | not offered a <link> link": the eip oracle (one page
/// link when offered; none anywhere, the sidebar included, when not).
pub fn then_link(world: &FoundryWorld, who: &str, link: &str, offer: Offer) {
    eip::then_link_offered(world, who, link, offer.0);
}

/// The page's own links (sidebar excluded) into the workspace integrations
/// pages. The sidebar's W-1 is navigation offered on every page to an admin,
/// not a fix for this refusal, so it does not count here (US-IW-01 tests it).
fn workspace_links(body: &str) -> Vec<String> {
    let doc = parse(body);
    select_all(&doc, "a[href]")
        .into_iter()
        .filter(|a| !in_sidebar(a))
        .filter_map(|a| a.value().attr("href").map(str::to_string))
        .filter(|href| href.starts_with(WORKSPACE_INTEGRATIONS))
        .collect()
}

/// "<who> is offered the link <link> | no way to fix it here". A fix link is
/// followed (KPI-IW-G1): it must answer `who` with a page, never not-found.
pub async fn then_fix_offered(world: &mut FoundryWorld, who: &str, fix: Fix) {
    let answer = answer(world);
    answered(&answer, who);
    match fix {
        Fix::None => {
            let links = workspace_links(&answer.body);
            assert!(
                links.is_empty(),
                "{who} is offered no way to fix it here; the page links {links:?}; {}",
                tm::describe(&answer)
            );
        }
        Fix::Link(link) => {
            let hrefs = link_hrefs(&answer.body, &link);
            assert_eq!(
                hrefs.len(),
                1,
                "{who} is offered one {link:?} link; {}",
                tm::describe(&answer)
            );
            let landed = eip::get(world, who, &hrefs[0]).await;
            assert!(
                landed.status.is_success(),
                "{who} follows {link:?} to a page that answers; {}",
                tm::describe(&landed)
            );
        }
    }
}

/// "following <link> lands <who> on <page>".
pub async fn then_following_lands(world: &mut FoundryWorld, who: &str, link: &str, page: &str) {
    let answer = answer(world);
    answered(&answer, who);
    let hrefs = link_hrefs(&answer.body, link);
    assert_eq!(
        hrefs.len(),
        1,
        "{who} finds one {link:?} link; {}",
        tm::describe(&answer)
    );
    let landed = eip::get(world, who, &hrefs[0]).await;
    let want = eip::page_path(world, page);
    assert!(
        landed.status.is_success() && landed.path == want,
        "following {link:?} lands {who} on {page} ({want}); {}",
        tm::describe(&landed)
    );
}

// =========================================================================== sidebar

/// The sidebar's links (text, href, current) on `body`.
fn sidebar_links(body: &str) -> Vec<(String, String, bool)> {
    let doc = parse(body);
    select_all(&doc, "aside a")
        .into_iter()
        .map(|a| {
            (
                a.text().collect::<String>().trim().to_string(),
                a.value().attr("href").unwrap_or_default().to_string(),
                a.value().attr("aria-current") == Some("page"),
            )
        })
        .collect()
}

/// "Priya follows Integrations in the sidebar on <page>".
pub async fn when_follow_sidebar(world: &mut FoundryWorld, who: &str, page: &str) {
    let path = eip::page_path(world, page);
    let opened = eip::get(world, who, &path).await;
    answered(&opened, who);
    let hrefs: Vec<String> = sidebar_links(&opened.body)
        .into_iter()
        .filter(|(text, _, _)| text == SIDEBAR_ENTRY)
        .map(|(_, href, _)| href)
        .collect();
    assert_eq!(
        hrefs.len(),
        1,
        "{who} finds one {SIDEBAR_ENTRY:?} in the sidebar on {page}; {}",
        tm::describe(&opened)
    );
    eip::when_open(world, who, &hrefs[0]).await;
}

/// "<who> is offered | not offered Integrations in the sidebar".
pub fn then_sidebar_offered(world: &FoundryWorld, who: &str, offer: Offer) {
    let answer = answer(world);
    answered(&answer, who);
    let links = sidebar_links(&answer.body);
    assert!(
        !links.is_empty(),
        "precondition: {who}'s page has the sidebar; {}",
        tm::describe(&answer)
    );
    let entries: Vec<&(String, String, bool)> = links
        .iter()
        .filter(|(t, _, _)| t == SIDEBAR_ENTRY)
        .collect();
    match offer.0 {
        true => assert!(
            entries.len() == 1 && entries[0].1 == WORKSPACE_INTEGRATIONS,
            "{who} is offered {SIDEBAR_ENTRY:?} to {WORKSPACE_INTEGRATIONS} in the sidebar; it lists {links:?}"
        ),
        false => assert!(
            entries.is_empty(),
            "{who} is not offered {SIDEBAR_ENTRY:?} in the sidebar; it lists {links:?}"
        ),
    }
}

/// "Priya's sidebar lists A, B and C in that order": the user section's links,
/// exactly.
pub fn then_sidebar_lists(world: &FoundryWorld, who: &str, words: &str) {
    let items = items(words);
    let answer = answer(world);
    answered(&answer, who);
    let doc = parse(&answer.body);
    let user: Vec<String> = select_all(&doc, ".sidebar__user a")
        .into_iter()
        .map(|a| a.text().collect::<String>().trim().to_string())
        .collect();
    assert_eq!(user, items, "{who}'s sidebar user section");
}

/// "Home is the sidebar's current page, not Integrations" (FR-4: exactly one
/// primary item is current; user-section items never are).
pub fn then_home_current(world: &FoundryWorld, who: &str) {
    let answer = answer(world);
    answered(&answer, who);
    let current: Vec<String> = sidebar_links(&answer.body)
        .into_iter()
        .filter(|(_, _, current)| *current)
        .map(|(text, _, _)| text)
        .collect();
    assert_eq!(
        current,
        vec!["Home".to_string()],
        "{who}'s sidebar marks only Home as current"
    );
}
