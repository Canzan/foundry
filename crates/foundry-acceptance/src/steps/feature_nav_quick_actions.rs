//! nav-quick-actions — step definitions for the three destinations the operator
//! now opens from the left nav (`tests/features/nav-quick-actions.feature`).
//!
//! Every stimulus is a real HTTP GET on the running application through the
//! in-process harness (`support::harness::InProcHarness`) with a real session
//! cookie and a real per-scenario Postgres (`@real-io`), and every observation is
//! read off the served page. Nothing below reaches a template, a handler or a
//! store read of its own.
//!
//! Reused unchanged, per the globally-unique-phrase rule (cucumber-rs) — DO NOT
//! redefine here:
//!   - `a workspace "…" exists with admin "…" and display name "…"`   (feature_dashboard_enhancements)
//!   - `a project "…" with key prefix "…" exists in "…"`              (feature_dashboard_enhancements)
//!   - `(\w+) is signed in`                                           (us_07_project_create)
//!   - `(\w+) visits "/"`                                             (feature_dashboard_enhancements)
//!   - `Ada is an instance super-admin`                               (feature_dashboard_enhancements)
//!   - `a member "…" who is not an instance admin is signed in`       (feature_dashboard_enhancements)
//!   - `the "…" workspace has no projects`                            (feature_navigation_bar)
//!   - `(\w+) opens the authenticated page "…"`                       (feature_navigation_bar)
//!   - `the user menu contains / does not contain a link to "…"`      (feature_navigation_bar)
//!   - `the "…" navigation item is marked as the current page`        (feature_navigation_bar)
//!   - `exactly one primary navigation item is marked as the current page` (feature_navigation_bar)
//!   - `the response body contains a project card "…" for "…"`        (feature_dashboard_enhancements)
//!
//! NEW here: the ordered list of destinations the primary nav offers, the
//! glyph-and-word shape of each one, and what the landing page keeps once its
//! quick actions are gone. Two Givens construct the two rights rows the shipped
//! personas cannot reach on their own (a member on the board's team, and an
//! instance super-admin who administers no workspace); both seed through the same
//! real Postgres and the same shipped grant the other persona Givens use.

use crate::support::html_assertions as html;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then};
use scraper::Selector;

fn body_of(world: &FoundryWorld) -> String {
    world.last_body.clone().unwrap_or_default()
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).unwrap_or_else(|_| panic!("{css:?} is a valid selector"))
}

/// One text run, whitespace collapsed, so a word authored across lines still
/// reads as the operator reads it.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every destination the primary nav offers on the served page, in the order the
/// operator meets them: its whole word and the page it opens.
fn offered_destinations(body: &str) -> Vec<(String, String)> {
    let doc = html::parse(body);
    html::select_all(&doc, "nav#sidebar-nav a.sidebar__item")
        .into_iter()
        .map(|el| {
            (
                one_line(&el.text().collect::<String>()),
                el.value().attr("href").unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// The email the reused persona Givens recorded for the signed-in operator.
fn signed_in_email(world: &FoundryWorld, who: &str) -> String {
    world
        .us_07_signed_in_email
        .clone()
        .unwrap_or_else(|| panic!("{who} must be signed in before this Given can place her"))
}

// ----- Given: the two rights rows the shipped personas cannot reach alone -----

/// Put the signed-in member on the team that owns the seeded board, so she can
/// open a board page at all (the board is gated on TEAM membership). Additive and
/// idempotent, exactly as the shipped project Given grants the workspace admin.
#[given(regex = r"^(\w+) is on the team that owns the board$")]
async fn is_on_the_board_team(world: &mut FoundryWorld, who: String) {
    let email = signed_in_email(world, &who);
    let harness = world
        .harness
        .as_ref()
        .expect("the harness is already running");
    let pool = harness.app.state.store.pool();
    let member: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&email)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("{who} has a user row to place on the team"));
    let team: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM teams WHERE slug = 'general' LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("the seeded board's team exists");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')
              ON CONFLICT DO NOTHING",
    )
    .bind(team.0)
    .bind(member.0)
    .execute(pool)
    .await
    .unwrap_or_else(|_| panic!("{who} joins the board's team"));
}

/// Promote the signed-in member to instance super-admin through the shipped
/// grant — the same upgrade path a real operator takes. She administers no
/// workspace, which is the one rights row the shipped personas do not cover.
#[given(regex = r"^(\w+) has since been made an instance super-admin$")]
async fn has_been_made_instance_super_admin(world: &mut FoundryWorld, who: String) {
    let email = signed_in_email(world, &who);
    let harness = world
        .harness
        .as_ref()
        .expect("the harness is already running");
    let pool = harness.app.state.store.pool();
    let member: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&email)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|_| panic!("{who} has a user row to promote"));
    harness
        .app
        .state
        .store
        .grant_instance_admin(member.0)
        .await
        .unwrap_or_else(|_| panic!("{who} is granted instance super-admin"));
}

// ----- Then: what the left nav offers ----------------------------------------

#[then(regex = r"^the left nav offers these destinations in this order$")]
async fn nav_offers_destinations_in_order(world: &mut FoundryWorld, step: &Step) {
    let body = body_of(world);
    let table = step
        .table
        .as_ref()
        .expect("this step is written with its destination table");
    let wanted: Vec<(String, String)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| (one_line(&row[0]), one_line(&row[1])))
        .collect();
    let found = offered_destinations(&body);
    assert_eq!(
        found, wanted,
        "WHAT: the left nav must offer {wanted:?}, in that order; it offers {found:?}. \
         WHY: a destination the nav leaves out, puts before the board, or points elsewhere is a \
         destination the operator cannot open from this page — she is sent back to the landing \
         page for it, which is the detour this feature removes. \
         FIX: author the missing destinations as anchors inside nav#sidebar-nav, after Board, each \
         linking exactly the page named here, with no condition on who is signed in."
    );
}

#[then(regex = r"^each of those destinations shows one icon and its own word$")]
async fn each_destination_shows_one_icon_and_its_word(world: &mut FoundryWorld) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let members = html::select_all(&doc, "nav#sidebar-nav a.sidebar__item");
    assert!(
        !members.is_empty(),
        "WHAT: the left nav offers no destination at all on this page. \
         WHY: with no member there is nothing for this observation to hold of, so a green here \
         would mean nothing. FIX: render the nav destinations before asserting their shape."
    );
    let icon = selector("svg.sidebar__icon");
    let word = selector("span.sidebar__label");
    let titled = selector("title");
    let labelled = selector("[aria-label]");
    for member in members {
        let name = one_line(&member.text().collect::<String>());
        let icons = member.select(&icon).count();
        assert_eq!(
            icons, 1,
            "WHAT: the {name:?} destination carries {icons} inline icons, not one. \
             WHY: folded into its icon rail the nav shows the operator the icon alone, so a \
             destination with none is an invisible gap in the rail and one with two is a double \
             glyph. FIX: give it exactly one inline svg.sidebar__icon."
        );
        let words: Vec<String> = member
            .select(&word)
            .map(|el| one_line(&el.text().collect::<String>()))
            .collect();
        assert_eq!(
            words.len(),
            1,
            "WHAT: the {name:?} destination carries {} words in span.sidebar__label, not one; \
             they are {words:?}. WHY: the folded rail keeps the word in the document and computes \
             the destination's spoken name from it, so a destination with no word is a silent icon \
             for a screen-reader operator. FIX: give it exactly one span.sidebar__label holding \
             its whole word.",
            words.len()
        );
        assert_eq!(
            words[0], name,
            "WHAT: the {name:?} destination reads {name:?} in total but only {:?} inside its \
             word. WHY: a second text node beside the word makes the destination announce itself \
             twice (\"Home Home\"). FIX: keep the word the destination's only text, and give the \
             icon no <title>.",
            words[0]
        );
        assert_eq!(
            member.select(&titled).count(),
            0,
            "WHAT: the {name:?} destination carries a <title>. \
             WHY: it adds a second text node, so the destination announces itself twice. \
             FIX: remove the <title>; the word is the one owner of the name."
        );
        assert!(
            member.value().attr("aria-label").is_none() && member.select(&labelled).count() == 0,
            "WHAT: the {name:?} destination carries an aria-label beside its word. \
             WHY: a second name channel drifts from the word the operator reads and the oracle \
             reads, so the two disagree about what the destination is called. \
             FIX: remove the aria-label; the word in span.sidebar__label is the name."
        );
    }
}

// ----- Then: what the landing page keeps -------------------------------------

#[then(regex = r"^the landing page shows no quick actions$")]
async fn landing_page_shows_no_quick_actions(world: &mut FoundryWorld) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let headings: Vec<String> = html::select_all(&doc, "h1, h2, h3")
        .into_iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect();
    assert!(
        !headings.iter().any(|h| h == "Quick actions"),
        "WHAT: the landing page still heads a \"Quick actions\" section; its headings are \
         {headings:?}. WHY: the operator now reaches those destinations from the left nav on \
         every page, so a second copy on the landing page is one more place to keep in step and \
         one more place to disagree with the nav. FIX: delete the quick-actions section from the \
         landing page."
    );
    html::assert_not_has(&body, "ul.actions");
}

#[then(regex = r#"^the landing page still greets "([^"]+)" and names the workspace "([^"]+)"$"#)]
async fn landing_page_still_greets(world: &mut FoundryWorld, name: String, workspace: String) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let greeting: String = html::select_all(&doc, ".dash__welcome")
        .into_iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        greeting.contains(&name),
        "WHAT: the landing page no longer greets {name:?}; it reads {greeting:?}. \
         WHY: removing the quick actions must take nothing else with it — the operator still \
         arrives on a page that knows who she is. FIX: leave the greeting line untouched."
    );
    let where_she_is: String = html::select_all(&doc, ".dash__workspace")
        .into_iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        where_she_is.contains(&workspace),
        "WHAT: the landing page no longer names the workspace {workspace:?}; it reads \
         {where_she_is:?}. WHY: the operator reads which workspace she is in from this line. \
         FIX: leave the workspace line untouched."
    );
}

#[then(regex = r"^the landing page still shows the projects section$")]
async fn landing_page_still_shows_projects(world: &mut FoundryWorld) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let sections: Vec<String> = html::select_all(&doc, "section.dash__section h2")
        .into_iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect();
    assert!(
        sections.iter().any(|s| s == "Your projects"),
        "WHAT: the landing page no longer shows its projects section; its sections are \
         {sections:?}. WHY: that section is how the operator finds the board she wants, whether \
         she has projects, none yet, or no team at all. FIX: delete only the quick-actions \
         section and leave this one as it is."
    );
}
