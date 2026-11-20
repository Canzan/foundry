//! ui-nav-polish — the left nav says where the operator is, offers only what
//! opens, and stays out of her way (`tests/features/ui-nav-polish.feature`).
//!
//! WHAT THE STEPS BELOW ARE ALLOWED TO KNOW. Public facts of one running
//! application, and nothing else:
//!
//!   * what the served document OFFERS her — each primary member's own word and
//!     the page it opens;
//!   * which member the document marks as the page she is on;
//!   * the HTTP answer each offered href gives THE SAME session cookie the rail
//!     was rendered for;
//!   * where the rail box sits inside the viewport, how tall it is against the
//!     screen, and whether each member can be seen and pressed;
//!   * the fill, border, colour and weight the browser RESOLVES for the two
//!     controls at the foot of the nav, each beside the same reading of a
//!     neighbour link.
//!
//! THE EXPECTED SET IS WRITTEN OUT, never read back off the rail. A rail
//! compared against the rail agrees with itself even when both lose a
//! destination, which is why the places each rights row is owed are named in the
//! feature file as a table and why every offered href is then OPENED.
//!
//! Reused unchanged, per the globally-unique-phrase rule (cucumber-rs) — DO NOT
//! redefine here:
//!   - `a workspace "…" exists with a member "…" on team "…"`  (feature_board_new_issue)
//!   - `a project "…" with key prefix "…" exists under "…"`    (feature_board_new_issue)
//!   - `(\w+) is signed in`                                    (us_07_project_create)
//!
//! WHY ITS OWN STEP MODULE: the crate carries one `feature_*.rs` module per
//! feature and cucumber-rs requires globally-unique step text. It imports from
//! `support` only — the HTTP session machinery, the browser session machinery
//! and the ONE live reading of the rail (`support::rail_reading`), which the two
//! shipped rail lanes read the same strip through — and from no other step
//! module.

use crate::support::harness::{establish_session, get_with_cookie, InProcHarness};
use crate::support::html_assertions as html;
use crate::support::rail_reading::{members, px, read_nav, NAV_CONTROL};
use crate::support::{browser_harness, rail_reading};
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::redirect::Policy;
use std::time::Duration;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";

/// The operator the Background signs in, and the board she owns a team on.
const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const BOARD_PATH: &str = "/team/backend/project/sandbox";

/// The ONE phone device this lane measures the bar against, and the fraction of
/// its screen the bar is promised to stay inside. 844 / 5 = 168.8 px.
const PHONE_WIDTH_PX: u32 = 390;
const PHONE_HEIGHT_PX: u32 = 844;
const SCREEN_FRACTION: f64 = 5.0;

/// The smallest a primary control may be at phone width — a thumb's width, the
/// floor the shipped mobile promise already holds the rail to.
const THUMB_PX: f64 = 44.0;

/// Sub-pixel slack on a box read from a live layout. Geometry is read in CSS
/// pixels off a fractional box, so an exact comparison would fail on a rail
/// pinned exactly at the top of the viewport.
const SLACK_PX: f64 = 1.0;

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const DASHBOARD_WITNESS: &str = ".dash";
const SIGN_IN_WITNESS: &str = "form[action='/sign-in'] input[type='password']";
const APP_SHELL_WITNESS: &str = ".app-shell";
const THEME_CONTROL: &str = ".theme-toggle";
const SIGN_OUT_SUBMIT: &str = ".sidebar form[action='/sign-out'] button[type='submit']";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(http_client());
    }
}

fn harness_of(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("the Background must have spawned the application")
}

fn body_of(world: &FoundryWorld) -> String {
    world.last_body.clone().unwrap_or_default()
}

/// One text run, whitespace collapsed, so a word authored across lines reads as
/// the operator reads it.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every place the primary nav offers on the served page, in the order the
/// operator meets them: its whole word and the page it opens.
fn offered_places(body: &str) -> Vec<(String, String)> {
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

/// Every member of the served rail that the nav marks as the page she is on.
fn marked_places(body: &str) -> Vec<String> {
    let doc = html::parse(body);
    html::select_all(&doc, ".sidebar a.sidebar__item[aria-current='page']")
        .into_iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect()
}

// ==========================================================================
// GIVEN — the rights she holds
// ==========================================================================

/// Grant the rights the row names at the store the application reads them from,
/// and READ THEM BACK: a grant that silently failed would turn the row into a
/// second copy of the no-rights one while still passing.
async fn grant(world: &mut FoundryWorld, rights: &str) {
    ensure_harness(world).await;
    let wants_workspace = matches!(
        rights,
        "a workspace admin" | "both a workspace and an instance admin"
    );
    let wants_instance = matches!(
        rights,
        "an instance admin" | "both a workspace and an instance admin"
    );
    assert!(
        wants_workspace || wants_instance || rights == "a member with no admin rights",
        "the row names the rights {rights:?}, which no step knows how to grant — write it the way \
         the others are written rather than letting the scenario run with some other operator"
    );
    let pool = harness_of(world).app.state.store.pool();
    if wants_workspace {
        let updated = sqlx::query(
            "UPDATE workspace_memberships SET role = 'admin'
             WHERE user_id = (SELECT id FROM users WHERE email_lower = $1)",
        )
        .bind(MEI_EMAIL)
        .execute(pool)
        .await
        .expect("make the operator an admin of her workspace");
        assert_eq!(
            updated.rows_affected(),
            1,
            "the operator was not made an admin of her workspace, so the nav would be measured \
             against rights she does not hold"
        );
    }
    if wants_instance {
        let inserted = sqlx::query(
            "INSERT INTO instance_admins (user_id)
             SELECT id FROM users WHERE email_lower = $1 ON CONFLICT DO NOTHING",
        )
        .bind(MEI_EMAIL)
        .execute(pool)
        .await
        .expect("make the operator an admin of the instance");
        assert_eq!(
            inserted.rows_affected(),
            1,
            "the operator was not made an admin of the instance, so the nav would be measured \
             against rights she does not hold"
        );
    }
}

#[given(
    regex = r"^she holds the rights of (a member with no admin rights|a workspace admin|an instance admin|both a workspace and an instance admin)$"
)]
async fn given_her_rights(world: &mut FoundryWorld, rights: String) {
    grant(world, &rights).await;
}

// ==========================================================================
// WHEN — one authenticated GET through the public port
// ==========================================================================

async fn open_as_her(world: &mut FoundryWorld, path: &str) {
    ensure_harness(world).await;
    let harness = harness_of(world);
    let http = world.http.as_ref().expect("http");
    let session = establish_session(harness, http, MEI_EMAIL, MEI_PASSWORD).await;
    let outcome = get_with_cookie(harness, http, path, &session).await;
    world.dash_session_cookie = Some(session);
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

#[when(regex = r#"^she opens "([^"]+)"$"#)]
async fn when_she_opens(world: &mut FoundryWorld, path: String) {
    open_as_her(world, &path).await;
    let status = world.last_status.expect("a status");
    assert!(
        status.is_success(),
        "the operator asked for {path:?} and the application answered {status}. Every page in \
         this table is one her rights open, so a refusal here means the scenario would read a nav \
         off a page she never reached."
    );
}

/// She types an address the nav no longer offers her. The GET is the same public
/// GET; only the expectation differs, which is the whole point of the row.
#[when(regex = r#"^she types the address "([^"]+)" herself$"#)]
async fn when_she_types_the_address(world: &mut FoundryWorld, path: String) {
    open_as_her(world, &path).await;
}

// ==========================================================================
// THEN — where she is, and what she is offered
// ==========================================================================

#[then(regex = r#"^the left nav marks "([^"]+)" as the page she is on, and nothing else$"#)]
async fn then_the_nav_marks(world: &mut FoundryWorld, word: String) {
    let body = body_of(world);
    let marked = marked_places(&body);
    assert_eq!(
        marked,
        vec![word.clone()],
        "WHAT: the left nav marks {marked:?} as the page the operator is on; it must mark exactly \
         {word:?}. WHY: the mark is how she reads her own place off the nav — nothing marked and \
         she cannot tell where she is, the wrong one marked and she believes she is somewhere she \
         is not, two marked and the nav contradicts itself. FIX: let the page she is on name the \
         one item that owns it, and let the nav mark that item and no other."
    );
}

#[then(regex = r"^the item the nav marks is one of the places it offers her$")]
async fn then_the_marked_item_is_offered(world: &mut FoundryWorld) {
    let body = body_of(world);
    let marked = marked_places(&body);
    let offered: Vec<String> = offered_places(&body)
        .into_iter()
        .map(|(word, _)| word)
        .collect();
    let adrift: Vec<&String> = marked.iter().filter(|m| !offered.contains(m)).collect();
    assert!(
        adrift.is_empty(),
        "WHAT: the left nav marks {adrift:?} as the page the operator is on while it does not \
         offer her {adrift:?} at all; it offers {offered:?}. WHY: a mark on a place she is not \
         offered tells her she is somewhere the nav will not take her back to. FIX: never mark an \
         item the nav leaves out for her rights."
    );
}

#[then(regex = r"^the left nav offers her exactly these places$")]
async fn then_the_nav_offers_exactly(world: &mut FoundryWorld, step: &Step) {
    let body = body_of(world);
    let table = step
        .table
        .as_ref()
        .expect("this step is written with its table of places");
    let mut wanted: Vec<(String, String)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| (one_line(&row[0]), one_line(&row[1])))
        .collect();
    let mut found = offered_places(&body);
    // THE SET, NOT THE SEQUENCE. The promise is which places the nav offers her,
    // not the order the document happens to write them in; the order the rail
    // draws its members in is a separate reading that no row here claims. Both
    // sides are sorted, so an extra or a missing place still fails and a moved
    // one does not.
    wanted.sort();
    found.sort();
    assert_eq!(
        found, wanted,
        "WHAT: the left nav must offer this operator exactly {wanted:?}; it offers \
         {found:?}. WHY: an extra place is one the application will refuse her when she presses \
         it, and a missing place is one she could reach before and cannot now. FIX: offer each of \
         the two admin destinations only when the identity the nav was rendered for is an admin \
         of this workspace, and offer every other destination to everyone, exactly as today."
    );
}

#[then(regex = r"^every place the left nav offers her opens for her$")]
async fn then_every_offered_place_opens(world: &mut FoundryWorld) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let offered: Vec<String> = html::select_all(&doc, ".sidebar a[href]")
        .into_iter()
        .filter_map(|el| el.value().attr("href").map(str::to_string))
        .filter(|href| href.starts_with('/'))
        .collect();
    assert!(
        !offered.is_empty(),
        "WHAT: the left nav offers no place at all on this page. WHY: with nothing offered there \
         is nothing for this observation to hold of, so a green here would mean nothing. \
         FIX: render the nav before asserting what it offers."
    );
    let session = world
        .dash_session_cookie
        .clone()
        .expect("the GET that rendered this nav established her session");
    let harness = harness_of(world);
    let http = world.http.as_ref().expect("http");
    let mut refused = Vec::new();
    for href in &offered {
        let answer = get_with_cookie(harness, http, href, &session).await;
        if !answer.status.is_success() {
            refused.push(format!("{href} answered {}", answer.status));
        }
    }
    assert!(
        refused.is_empty(),
        "WHAT: {} of the places the left nav offers this operator do not open for her: {refused:?}. \
         She was offered {offered:?} and each one was then asked for with the SAME session cookie \
         the nav was rendered for. WHY: a link that answers her own session with a refusal teaches \
         her the application is broken, and it advertises a surface she may not use. FIX: offer a \
         destination only when the identity the nav was rendered for is one that destination lets \
         in.",
        refused.len()
    );
}

#[then(regex = r"^the application answers her nowhere$")]
async fn then_the_application_answers_nowhere(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a status was recorded");
    assert_eq!(
        status.as_u16(),
        404,
        "WHAT: the operator typed the address of a page her rights do not open and the \
         application answered {status}. WHY: taking the link out of the nav is not an admission \
         decision — the page decides who may enter, and it answers everyone else the same \
         not-found it answers for an address that does not exist, so nobody can map the surface by \
         reading refusals. FIX: leave the page's own gate exactly as it is."
    );
}

// ==========================================================================
// THE BROWSER LANE — the three geometry promises
// ==========================================================================

fn base_url(world: &FoundryWorld) -> String {
    harness_of(world).base_url().to_string()
}

fn browser_of(world: &FoundryWorld) -> &fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first")
}

async fn wait_for(browser: &fantoccini::Client, selector: &str, what: &str) {
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(selector))
        .await
        .unwrap_or_else(|err| panic!("{what} never rendered `{selector}`: {err}"));
}

async fn sign_in(browser: &fantoccini::Client, world: &FoundryWorld) {
    browser_harness::sign_in_through_browser(browser, harness_of(world), MEI_EMAIL, MEI_PASSWORD)
        .await;
}

async fn goto(browser: &fantoccini::Client, world: &FoundryWorld, path: &str, witness: &str) {
    let url = format!("{}{path}", base_url(world));
    browser
        .goto(&url)
        .await
        .unwrap_or_else(|err| panic!("navigate to {path}: {err}"));
    wait_for(browser, witness, path).await;
}

/// One real press of the real control that narrows the nav, and the proof it
/// narrowed something: without that every geometry row below would pass on a
/// browser that narrowed nothing.
async fn narrow_the_nav(browser: &fantoccini::Client) {
    browser
        .find(Locator::Css(NAV_CONTROL))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no control to narrow the left nav with: nothing matches \
                 `{NAV_CONTROL}` ({err})"
            )
        })
        .click()
        .await
        .unwrap_or_else(|err| panic!("the control for the left nav could not be pressed: {err}"));
    let reading = read_nav(browser).await;
    assert!(
        reading["marked"].as_bool().unwrap_or(false),
        "the operator pressed the control and the nav did not narrow, so this row would measure \
         the open geometry twice and call one of them folded"
    );
}

/// Enough issues on her board that the page is taller than her window. Seeded at
/// the store the board reads from, as the shipped board lanes seed it, and the
/// overflow is then ASSERTED: a page that happened to fit would make a scroll
/// assertion pass over a page that never scrolls.
async fn seed_a_long_board(world: &mut FoundryWorld) {
    let pool = harness_of(world).app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = 'GEN'")
            .fetch_one(pool)
            .await
            .expect("her board's project exists");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("an author exists");
    for number in 1..=40 {
        sqlx::query(
            "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id)
                  VALUES ($1, $2, $3, $4, $5, 'backlog', 'medium', $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project.1)
        .bind(project.0)
        .bind(number)
        .bind(format!("A piece of work waiting to be picked up, number {number}"))
        .bind(author.0)
        .execute(pool)
        .await
        .expect("seed a piece of work onto her board");
    }
    sqlx::query(
        "UPDATE projects SET next_issue_number = GREATEST(next_issue_number, 41) WHERE id = $1",
    )
    .bind(project.0)
    .execute(pool)
    .await
    .expect("leave the board able to take the next piece of work");
}

#[given(
    regex = r"^the operator is (looking at the open nav|looking at the nav narrowed to icons) on a page (taller than her window|that fits in her window) at desk width$"
)]
async fn given_a_page_at_desk_width(world: &mut FoundryWorld, geometry: String, page: String) {
    grant(world, "both a workspace and an instance admin").await;
    let long = page == "taller than her window";
    if long {
        seed_a_long_board(world).await;
    }
    let browser = browser_harness::new_session().await;
    sign_in(&browser, world).await;
    if long {
        goto(
            &browser,
            world,
            BOARD_PATH,
            browser_harness::BOARD_READY_SELECTOR,
        )
        .await;
    } else {
        goto(&browser, world, "/account/settings", APP_SHELL_WITNESS).await;
    }
    if geometry == "looking at the nav narrowed to icons" {
        narrow_the_nav(&browser).await;
    }
    let reading = read_nav(&browser).await;
    let (document, window) = (px(&reading, "docHeight"), px(&reading, "viewportHeight"));
    if long {
        assert!(
            document > window + window / 2.0,
            "the page the operator opened is {document:.0} px tall in a {window:.0} px window, so \
             it is not the long page this row is about and scrolling it would prove nothing about \
             a nav that stays in view. Seed more work onto the board, or the fixed window grew."
        );
    } else {
        assert!(
            document <= window + window / 10.0,
            "the page the operator opened is {document:.0} px tall in a {window:.0} px window, \
             which is more than a tenth of a screen more than it holds, so it is not the \
             nothing-to-scroll page this row is about and it would assert the short-page half of \
             the law over a long one. Every page of this application carries a few pixels over the \
             window - the shell is at least a screen tall and the release footer sits in its \
             bottom band - so the reading allows that much and no more."
        );
    }
    world.browser = Some(browser);
}

#[when(regex = r"^she scrolls that page to its bottom$")]
async fn when_she_scrolls_to_the_bottom(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    browser
        .execute(
            "window.scrollTo(0, document.documentElement.scrollHeight); return window.scrollY;",
            Vec::new(),
        )
        .await
        .expect("scroll the page to its bottom");
    let reading = read_nav(browser).await;
    let scrolled = px(&reading, "scrolledTo");
    assert!(
        scrolled > 0.0,
        "the page did not move when the operator scrolled it, so nothing below would be read at \
         the bottom of a long page at all"
    );
}

#[then(regex = r"^the whole left nav is in view, with every place she can press$")]
async fn then_the_whole_nav_is_in_view(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    let window = px(&reading, "viewportHeight");
    let scrolled = px(&reading, "scrolledTo");
    let top = reading["railBox"]["top"].as_f64().expect("the rail's top");
    let bottom = reading["railBox"]["bottom"]
        .as_f64()
        .expect("the rail's bottom");
    assert!(
        top >= -SLACK_PX && bottom <= window + SLACK_PX,
        "WHAT: with the page scrolled {scrolled:.0} px down, the left nav runs from {top:.0} to \
         {bottom:.0} in a window {window:.0} px tall, so part of it is outside what the operator \
         can see. WHY: the nav is how she goes anywhere; a nav that scrolls away with the page \
         sends her back to the top of a long board before she can leave it. FIX: hold the nav at \
         the top of the window, at the height of the window, and let it scroll inside itself if \
         it is ever taller than that — never cap it without a way to reach the rest."
    );
    let out_of_reach: Vec<String> = members(&reading)
        .iter()
        .filter(|member| {
            !member["visible"].as_bool().unwrap_or(false)
                || !member["focusable"].as_bool().unwrap_or(false)
        })
        .map(describe_member)
        .collect();
    assert!(
        out_of_reach.is_empty(),
        "WHAT: {} place(s) in the left nav cannot be seen or pressed where the operator is now: \
         {out_of_reach:?}. WHY: holding the nav in view must not cost her a destination — a place \
         clipped out of a fixed box, or reachable from the keyboard while invisible, is worse than \
         one that scrolled away. FIX: let the nav scroll its own content instead of hiding any of \
         it.",
        out_of_reach.len()
    );
}

fn describe_member(member: &serde_json::Value) -> String {
    format!(
        "{} (in the layout: {}, visible: {}, pressable: {}, {} px high)",
        member["text"].as_str().unwrap_or("an unnamed place"),
        member["inLayout"],
        member["visible"],
        member["focusable"],
        member["box"]["height"]
    )
}

// ----- her phone -------------------------------------------------------------

#[when(
    regex = r"^she opens her phone on the dashboard (as it comes|having narrowed the nav earlier|with no page scripts running)$"
)]
async fn when_she_opens_her_phone(world: &mut FoundryWorld, how: String) {
    let browser = if how == "with no page scripts running" {
        let browser = browser_harness::new_session_without_scripting().await;
        browser
            .set_window_size(PHONE_WIDTH_PX, PHONE_HEIGHT_PX)
            .await
            .expect("hold the window at the phone's own size");
        browser
    } else {
        browser_harness::open_mobile_session().await
    };
    sign_in(&browser, world).await;
    goto(&browser, world, "/", DASHBOARD_WITNESS).await;
    if how == "having narrowed the nav earlier" {
        narrow_the_nav(&browser).await;
    }
    let reading = read_nav(&browser).await;
    let across = px(&reading, "viewport");
    assert!(
        (across - f64::from(PHONE_WIDTH_PX)).abs() <= 1.0,
        "the page is laid out {across:.0} px across instead of the {PHONE_WIDTH_PX} px phone this \
         row is about, so whatever it measures is not the phone bar"
    );
    world.browser = Some(browser);
}

#[then(regex = r"^the bar above the page takes at most a fifth of her screen$")]
async fn then_the_phone_bar_is_short(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    let screen = px(&reading, "viewportHeight");
    let bar = reading["railBox"]["height"]
        .as_f64()
        .expect("the bar's height");
    let allowed = screen / SCREEN_FRACTION;
    assert!(
        bar <= allowed + SLACK_PX,
        "WHAT: the bar above the page is {bar:.0} px tall on a {screen:.0} px screen, which is \
         more than the {allowed:.1} px a fifth of it allows. WHY: on a phone the nav sits above \
         everything she came for, so a tall bar leaves her reading her work through a letterbox. \
         FIX: draw each place in the bar as its glyph, as the narrowed nav already does, so the \
         places wrap into two rows instead of four — never cap the bar's height and let it hide \
         what does not fit."
    );
}

#[then(regex = r"^every place in that bar can be seen and pressed, each a thumb's width high$")]
async fn then_every_place_in_the_bar_is_pressable(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    let out_of_reach: Vec<String> = members(&reading)
        .iter()
        .filter(|member| {
            !member["visible"].as_bool().unwrap_or(false)
                || !member["focusable"].as_bool().unwrap_or(false)
        })
        .map(describe_member)
        .collect();
    assert!(
        out_of_reach.is_empty(),
        "WHAT: {} place(s) in the phone bar cannot be seen or pressed: {out_of_reach:?}. \
         WHY: a short bar must not be bought by hiding a destination — a place left in the \
         keyboard's path while the operator cannot see it is the worst of both. FIX: keep every \
         place drawn inside the bar and let the bar grow a row rather than clip one.",
        out_of_reach.len()
    );
    let too_small: Vec<String> = members(&reading)
        .iter()
        .filter(|member| member["box"]["height"].as_f64().unwrap_or(0.0) < THUMB_PX - SLACK_PX)
        .map(describe_member)
        .collect();
    assert!(
        too_small.is_empty(),
        "WHAT: {} place(s) in the phone bar are under {THUMB_PX:.0} px high: {too_small:?}. \
         WHY: she presses these with a thumb; a target smaller than that is one she misses. \
         FIX: keep every place in the bar a {THUMB_PX:.0} px square at least.",
        too_small.len()
    );
    let across = px(&reading, "viewport");
    let document = px(&reading, "docWidth");
    assert!(
        document <= across + SLACK_PX,
        "WHAT: the page is {document:.0} px wide on a {across:.0} px screen, so it slides \
         sideways. WHY: a page that slides sideways on a phone hides half of itself behind the \
         edge of the screen. FIX: let the bar wrap its places instead of laying them in one row."
    );
}

// ----- the foot of the nav ---------------------------------------------------

#[given(regex = r"^the operator is looking at the foot of the nav in the (light|dark) palette$")]
async fn given_the_foot_of_the_nav(world: &mut FoundryWorld, palette: String) {
    grant(world, "both a workspace and an instance admin").await;
    let browser = if palette == "dark" {
        browser_harness::new_dark_session().await
    } else {
        browser_harness::new_session().await
    };
    sign_in(&browser, world).await;
    goto(&browser, world, "/", DASHBOARD_WITNESS).await;
    wait_for(
        &browser,
        THEME_CONTROL,
        "the theme control the nav's foot carries",
    )
    .await;
    world.browser = Some(browser);
}

#[given(regex = r"^the operator is looking at the foot of the nav with no page scripts running$")]
async fn given_the_foot_of_the_nav_without_scripting(world: &mut FoundryWorld) {
    grant(world, "both a workspace and an instance admin").await;
    let browser = browser_harness::new_session_without_scripting().await;
    sign_in(&browser, world).await;
    goto(&browser, world, "/", DASHBOARD_WITNESS).await;
    world.browser = Some(browser);
}

/// Every member of the nav's lower group, as the browser RESOLVES it: the fill
/// the operator sees painted, whether it carries a border its neighbours do not,
/// and the text tier. The neighbour links are the reference, because the promise
/// is written against them and not against a colour.
fn lower_group(reading: &serde_json::Value) -> Vec<&serde_json::Value> {
    reading["lowerGroup"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "the reading carries no lower group at all, so nothing can be said about how the \
                 nav's foot is drawn. Reading: {reading}"
            )
        })
        .iter()
        .collect()
}

fn of_kind<'a>(group: &[&'a serde_json::Value], kind: &str) -> Vec<&'a serde_json::Value> {
    group
        .iter()
        .filter(|member| member["kind"].as_str() == Some(kind))
        .copied()
        .collect()
}

fn as_text(member: &serde_json::Value, key: &str) -> String {
    member[key].as_str().unwrap_or_default().to_string()
}

/// The one comparison both quiet-group assertions make, against the neighbour
/// links of the same lower group in the same reading. Nothing here names a
/// colour: two palettes answer different hues for the same promise.
fn assert_drawn_as_quietly_as_its_neighbours(
    reading: &serde_json::Value,
    control: &serde_json::Value,
) {
    let group = lower_group(reading);
    let neighbours = of_kind(&group, "a neighbour link");
    let neighbour = neighbours.first().unwrap_or_else(|| {
        panic!(
            "the nav's foot carries no neighbour link to compare the controls with, so \"as \
             quietly as its neighbours\" has no reference and a green here would mean nothing. \
             Reading: {reading}"
        )
    });
    let surface = as_text(reading, "railSurface");
    let name = as_text(control, "name");
    let what = format!(
        "{name:?} is drawn fill {:?}, border {} px, text {:?} at weight {:?}; the links beside it \
         are drawn fill {:?}, border {} px, text {:?} at weight {:?}, over a nav surface of {:?}",
        as_text(control, "fill"),
        control["borderWidth"],
        as_text(control, "colour"),
        as_text(control, "fontWeight"),
        as_text(neighbour, "fill"),
        neighbour["borderWidth"],
        as_text(neighbour, "colour"),
        as_text(neighbour, "fontWeight"),
        surface,
    );
    let why = "WHY: these two are the things she uses least and they are drawn as the heaviest \
               things in the nav, so the operator's eye is pulled to the way out instead of to \
               where she is going. FIX: let both take the presentation of the links beside them — \
               the nav's own surface showing through, the same muted text at the same weight, and \
               no border those links do not have.";
    assert_eq!(as_text(control, "fill"), surface, "WHAT: {what}. {why}");
    assert_eq!(
        as_text(control, "fill"),
        as_text(neighbour, "fill"),
        "WHAT: {what}. {why}"
    );
    let border = control["borderWidth"].as_f64().unwrap_or(0.0);
    let neighbour_border = neighbour["borderWidth"].as_f64().unwrap_or(0.0);
    assert!(border <= neighbour_border, "WHAT: {what}. {why}");
    assert_eq!(
        as_text(control, "colour"),
        as_text(neighbour, "colour"),
        "WHAT: {what}. {why}"
    );
    assert_eq!(
        as_text(control, "fontWeight"),
        as_text(neighbour, "fontWeight"),
        "WHAT: {what}. {why}"
    );
}

#[then(regex = r"^Sign out and the theme control are drawn as quietly as the links beside them$")]
async fn then_both_controls_are_quiet(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    let group = lower_group(&reading);
    for kind in ["Sign out", "the theme control"] {
        let found = of_kind(&group, kind);
        let control = found.first().unwrap_or_else(|| {
            panic!(
                "there is no {kind} at the foot of the nav to read at all, so this scenario would \
                 call its absence quiet. Reading: {reading}"
            )
        });
        assert_drawn_as_quietly_as_its_neighbours(&reading, control);
    }
}

#[then(regex = r"^Sign out is drawn as quietly as the links beside it$")]
async fn then_sign_out_is_quiet(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    let group = lower_group(&reading);
    let found = of_kind(&group, "Sign out");
    let control = found.first().unwrap_or_else(|| {
        panic!(
            "there is no Sign out at the foot of the nav to read at all, so this scenario would \
             call its absence quiet. Reading: {reading}"
        )
    });
    assert_drawn_as_quietly_as_its_neighbours(&reading, control);
}

#[then(regex = r"^there is no theme control at all$")]
async fn then_there_is_no_theme_control(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    assert!(
        reading["themeControl"].is_null(),
        "WHAT: a theme control is present in a browser that runs no page scripts: {:?}. \
         WHY: the control is built by a script, so with scripting off there is none — a scenario \
         that found one here would be reading a page whose scripts ran after all, and the law it \
         is about would be measured on the wrong document. FIX: this row is about the quiet \
         sign-out submit alone.",
        reading["themeControl"]
    );
}

#[when(regex = r"^she presses Sign out, once$")]
async fn when_she_presses_sign_out(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    browser
        .find(Locator::Css(SIGN_OUT_SUBMIT))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no Sign out at the foot of the nav to press: nothing matches \
                 `{SIGN_OUT_SUBMIT}` ({err}). Quieter presentation must not cost the operator the \
                 way out."
            )
        })
        .click()
        .await
        .unwrap_or_else(|err| panic!("Sign out could not be pressed: {err}"));
}

#[then(regex = r"^her session is over and the application asks her to sign in again$")]
async fn then_her_session_is_over(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    wait_for(browser, SIGN_IN_WITNESS, "the sign-in screen").await;
    goto(browser, world, "/", SIGN_IN_WITNESS).await;
    let shells = browser
        .find_all(Locator::Css(APP_SHELL_WITNESS))
        .await
        .expect("look for the application shell");
    assert!(
        shells.is_empty(),
        "WHAT: after one press of Sign out the operator still gets her own pages. \
         WHY: the quiet presentation is a presentation change only; the press still has to end her \
         session. FIX: leave the submit and its form exactly as they are."
    );
}

#[when(regex = r"^she presses the theme control, once$")]
async fn when_she_presses_the_theme_control(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let before = palette_state(browser).await;
    browser
        .find(Locator::Css(THEME_CONTROL))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no theme control at the foot of the nav to press: nothing matches \
                 `{THEME_CONTROL}` ({err})"
            )
        })
        .click()
        .await
        .unwrap_or_else(|err| panic!("the theme control could not be pressed: {err}"));
    world.last_body = Some(before);
}

/// WHICH PALETTE THE APPLICATION IS IN right now: the choice the document
/// carries, the choice the browser remembers for her, and the colours the page
/// is painted in. All three, because the control cycles through three choices
/// and two of them can paint the same palette on a device that already prefers
/// one of them — read off the colours alone, a working control would look dead.
/// How the palette travels from the choice to the colours is the theme lane's
/// own law and is not re-read here.
async fn palette_state(browser: &fantoccini::Client) -> String {
    browser
        .execute(
            "var s = getComputedStyle(document.body);
             var chosen = document.documentElement.getAttribute('data-theme');
             var remembered = null;
             try { remembered = window.localStorage.getItem('foundry.theme'); } catch (err) {}
             return 'chosen ' + chosen + ', remembered ' + remembered + ', painted ' +
                    s.backgroundColor + ' on ' + s.color;",
            Vec::new(),
        )
        .await
        .expect("read which palette the application is in")
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[then(regex = r"^the application changes its palette$")]
async fn then_the_palette_changes(world: &mut FoundryWorld) {
    let before = world
        .last_body
        .clone()
        .expect("the palette before the press was recorded");
    let browser = browser_of(world);
    let after = palette_state(browser).await;
    assert_ne!(
        before, after,
        "WHAT: one press of the theme control left the application in exactly the palette it was \
         in ({after}). \
         WHY: quietening the control must not take its behaviour with it — the press is how she \
         changes the palette. FIX: change only how the control is drawn, never what it does."
    );
}

/// The one live reading this module shares with the two shipped rail lanes, so a
/// geometry fact and a presentation fact can never come from two moments.
#[allow(dead_code)]
fn reading_owner() -> &'static str {
    rail_reading::RAIL_MEMBERS
}
