//! nav-collapse-icon-rail — the narrow left nav keeps every destination, as icons.
//!
//! WHAT THE STEPS BELOW ARE ALLOWED TO KNOW. Four public facts, and nothing else:
//!
//!   * what the live rail OFFERS — which members are in the layout, which can be
//!     seen, which can take the keyboard focus, and the name the BROWSER computes
//!     for each of them;
//!   * which member is marked as the page the operator is on, and whether its
//!     tint sets it apart from the others;
//!   * where each member's box sits inside the bar, and whether the page slides
//!     sideways;
//!   * the rights the operator holds, which is what decides how many destinations
//!     she is owed in the first place.
//!
//! THE EXPECTED DESTINATION SET IS DERIVED FROM HER RIGHTS, NOT FROM THE RAIL.
//! Every assertion about "every destination she had" is measured against a set
//! built from the rights the scenario granted her — Home, Board, Teams, Settings
//! and Keyboard shortcuts for anyone, Invite a member, Machine tokens and
//! Integrations for a workspace admin, Instance admin for an instance admin, and
//! Sign out always. Reading the expectation off
//! the rail would make the oracle agree with whatever the rail happens to render,
//! including a rail that silently dropped a destination.
//!
//! THE NAME IS READ FROM THE BROWSER'S ACCESSIBILITY TREE, never from the markup.
//! A word taken out of the layout is still in `textContent` and is NOT in what an
//! assistive technology is given, so a name read from the markup would be green
//! over a strip of silent glyphs — which is the whole risk of replacing words
//! with icons.
//!
//! WHY ITS OWN STEP MODULE: the crate carries one `feature_*.rs` module per
//! feature and cucumber-rs requires globally-unique step text. The Background
//! lines are the SHIPPED HTTP-lane seed steps (`feature_board_new_issue.rs`), so
//! this lane drives the SAME in-process origin the fold lane drives. It imports
//! from `support` only — the rail reading, the session machinery — and from no
//! other step module: the fold lane and this one share ONE reading of the rail
//! (`support::rail_reading`) so the two can never read the same strip
//! differently, and neither depends on the other.

use crate::support::browser_harness;
use crate::support::rail_reading::{
    members, px, read_nav, read_rail_with_names, walk_the_tab_order, NAV_CONTROL, OPEN_RAIL_PX,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- the contract

/// The destinations every signed-in operator is owed, whatever her rights, plus
/// the two each admin right adds and the one that is always last. This is the
/// rail's promise, written out: it is what "every destination she had" MEANS, and
/// it is derived from her rights rather than from the page.
///
/// Teams joined the list when the owner moved it off the landing page into the
/// nav: every signed-in operator is owed it, so folded it is hers exactly as
/// Home and Board are. Invite a member and Machine tokens moved with it but are
/// NOT hers — those two pages let a workspace admin in and answer every other
/// operator nowhere, so the nav offers them to a workspace admin only and the
/// owed set splits with the rights it is built from. They are named HERE rather
/// than read back from the nav on purpose — a rail compared only against the
/// page it is drawn from agrees with itself even when both lose a destination.
const EVERY_OPERATOR: [&str; 5] = ["Home", "Board", "Teams", "Settings", "Keyboard shortcuts"];
const FOR_A_WORKSPACE_ADMIN: [&str; 3] = ["Invite a member", "Machine tokens", "Integrations"];
const FOR_AN_INSTANCE_ADMIN: &str = "Instance admin";
const ALWAYS_LAST: &str = "Sign out";

/// The Sign out submit in the rail, located by the form it posts: the rail
/// renders the SAME post form the open nav renders, and one press of its button
/// is the whole of signing out.
const RAIL_SIGN_OUT: &str = ".sidebar form[action='/sign-out'] button[type='submit']";

/// The stored sentinel the collapse choice travels as, and the key it is kept
/// under — the one channel, written and read by the shipped asset alone.
const NAV_CHOICE_KEY: &str = "foundry.nav";
const NARROWED_CHOICE: &str = "collapsed";

/// The smallest box this lane will call a drawn icon, and the smallest step up
/// this lane will call a label REVEALED.
///
/// Neither is a design value and neither bounds the product: the icons ship far
/// larger than this and the revealed word far wider. They are the floor between
/// "the operator has something to look at and press" and "a sub-pixel sliver that
/// satisfies a greater-than-zero test" — the sliver being exactly what the
/// collapsed geometry leaves behind when the glyph is absent or the word is never
/// revealed.
const PRESSABLE_PX: f64 = 8.0;
const MATERIALLY_WIDER_PX: f64 = 8.0;

/// The phone boundary the promise names, so every phone assertion can prove it
/// was measured on the side of it that it claims to be about.
const PHONE_BREAKPOINT_PX: f64 = 480.0;
const PHONE_HEIGHT_PX: u32 = 844;

const DASHBOARD_WITNESS: &str = ".dash .card-grid .card a";
const SIGN_IN_WITNESS: &str = "form[action='/sign-in'] input[type='password']";

const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Where a step leaves a reading for the step after it. `sessionStorage`, so it
/// cannot collide with the `localStorage` key the production asset owns and so it
/// survives the navigation these scenarios drive.
const WALK_KEY: &str = "foundry.acceptance.icon-rail-walk";

// ==========================================================================
// GIVEN — her rights, and the rail she is looking at
// ==========================================================================

/// The open rail at desk width, with the rights the row names, asserted BEFORE
/// anything is pressed: every destination her rights give her is there, named,
/// visible and reachable, nothing invisible is reachable, and no icon is drawn
/// beside the words — the open nav stays the text nav it is today.
///
/// Without this half the scenario could report a narrow rail that lost a
/// destination the open rail never had.
#[given(
    regex = r"^the operator is (.+), with the dashboard open at desk width and the left nav open$"
)]
async fn given_rights_with_the_nav_open(world: &mut FoundryWorld, rights: String) {
    grant(world, &rights).await;
    let browser = browser_harness::new_session().await;
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    let reading = read_rail_with_names(&browser).await;
    assert!(
        !reading["marked"].as_bool().unwrap_or(false),
        "the document already carries the narrowed mark {:?} before the operator pressed \
         anything, so this scenario would report a press as the thing that narrowed the rail",
        reading["markValue"]
    );
    let open_rail = px(&reading, "railWidth");
    assert!(
        (open_rail - OPEN_RAIL_PX).abs() <= 2.0,
        "the open rail is {open_rail:.1}px wide instead of the {OPEN_RAIL_PX}px it ships with, so \
         `narrower than the open rail` would be measured against some other geometry"
    );
    let owed = owed_destinations(world).await;
    assert_rail_offers(
        &reading,
        &owed,
        RailShape::Open,
        "with the left nav open at desk width",
    );
    world.browser = Some(browser);
}

/// A browser that runs no page scripts at all — a real content setting, not a
/// simulation. Nothing stamps the narrowed mark there, so the operator gets the
/// open nav; what this lane asks is whether ALL of her destinations are in it.
#[given(regex = r"^the operator is (.+) and her browser runs no page scripts$")]
async fn given_rights_with_no_scripting(world: &mut FoundryWorld, rights: String) {
    grant(world, &rights).await;
    world.browser = Some(browser_harness::new_session_without_scripting().await);
}

/// The narrowed rail itself, reached by the operator's own press of the real
/// control, with the rights the whole journey is driven with. The press is
/// asserted to have narrowed something: without that, every scenario below would
/// pass on a browser that narrowed nothing.
#[given(
    regex = r"^the operator is both a workspace and an instance admin, with the left nav narrowed to its icon rail on the dashboard$"
)]
async fn given_narrowed_on_the_dashboard(world: &mut FoundryWorld) {
    grant(world, "both a workspace and an instance admin").await;
    let browser = browser_harness::new_session().await;
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    press_the_control(&browser, "to narrow the left nav to its icon rail").await;
    let reading = read_nav(&browser).await;
    assert_narrowed(&reading, "after the operator narrowed the left nav");
    world.browser = Some(browser);
}

/// She leaves the nav as the row names it and signs out FROM IT, in the same
/// browser she will come back in. Signing out is not opening the nav again, so
/// what she left is what the next sign-in must give her.
#[given(
    regex = r"^the operator left the left nav (narrow|open) and signed out from it in the same browser$"
)]
async fn given_left_the_nav_and_signed_out(world: &mut FoundryWorld, geometry: String) {
    grant(world, "both a workspace and an instance admin").await;
    let browser = browser_harness::new_session().await;
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    if geometry == "narrow" {
        press_the_control(&browser, "to narrow the left nav before signing out").await;
        let reading = read_nav(&browser).await;
        assert_narrowed(
            &reading,
            "before the operator signed out of the narrow rail",
        );
    }
    press_sign_out(&browser, &format!("from the {geometry} left nav")).await;
    wait_for(&browser, SIGN_IN_WITNESS, "the sign-in screen").await;
    assert_eq!(
        remembered_choice(&browser).await.as_deref(),
        if geometry == "narrow" {
            Some(NARROWED_CHOICE)
        } else {
            None
        },
        "after signing out of the {geometry} nav the browser remembers {:?}. Signing out must \
         leave the remembered choice exactly as the operator left it: it is kept per browser, as \
         the theme choice is, and only the control changes it.",
        remembered_choice(&browser).await
    );
    world.browser = Some(browser);
}

/// The no-script arm of the surviving choice. The choice she made in an earlier
/// visit is left at the origin and READ BACK, she signs out of the open nav she
/// gets without scripting, and what the browser still holds is the subject of the
/// Then. Nothing narrows the rail here: no script runs to stamp the mark, which
/// is the bound on the promise rather than a deviation from it.
#[given(
    regex = r"^the operator is both a workspace and an instance admin, her browser runs no page scripts, and she signed out of the left nav after it remembered the narrow nav she chose earlier$"
)]
async fn given_signed_out_with_no_scripting(world: &mut FoundryWorld) {
    grant(world, "both a workspace and an instance admin").await;
    let browser = browser_harness::new_session_without_scripting().await;
    goto_sign_in(&browser, world).await;
    browser
        .execute(
            "try { window.localStorage.setItem(arguments[0], arguments[1]); } catch (err) {} \
             return true;",
            vec![
                serde_json::Value::String(NAV_CHOICE_KEY.to_string()),
                serde_json::Value::String(NARROWED_CHOICE.to_string()),
            ],
        )
        .await
        .expect("leave the choice an earlier visit made at the origin");
    assert_eq!(
        remembered_choice(&browser).await.as_deref(),
        Some(NARROWED_CHOICE),
        "the browser does not hold the choice an earlier visit made, so this scenario would \
         observe a browser that never chose anything and call its emptiness preservation"
    );
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    press_sign_out(&browser, "from the left nav with no page scripts running").await;
    wait_for(&browser, SIGN_IN_WITNESS, "the sign-in screen").await;
    world.browser = Some(browser);
}

/// Her rights alone: the phone scenario opens its own emulated session in the
/// When, because a phone is a real session of its own and a resized desk window
/// would measure the desk rules.
#[given(regex = r"^the operator is both a workspace and an instance admin$")]
async fn given_both_admin_rights(world: &mut FoundryWorld) {
    grant(world, "both a workspace and an instance admin").await;
}

// ==========================================================================
// WHEN — her own press, keystroke, navigation or screen
// ==========================================================================

#[when(regex = r"^she narrows the left nav to its icon rail$")]
async fn when_she_narrows_the_nav(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    press_the_control(&browser, "to narrow the left nav to its icon rail").await;
    world.browser = Some(browser);
}

#[when(regex = r"^she opens the dashboard with no scripting$")]
async fn when_she_opens_the_dashboard_with_no_scripting(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    world.browser = Some(browser);
}

/// One real press on the Board icon, through the real pointer. The icon is found
/// by the board it opens, so an icon that draws a glyph and opens nothing fails
/// here rather than passing under a locator of its own.
#[when(regex = r"^she presses the Board icon in the narrow rail$")]
async fn when_she_presses_the_board_icon(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let selector = format!(".sidebar a.sidebar__item[href$='/project/{PROJECT_SLUG}']");
    let icon = browser
        .find(Locator::Css(&selector))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no Board icon in the narrow rail to press: nothing in the rail matches \
                 `{selector}` ({err}). While the left nav is narrow the rail is the operator's \
                 ONLY route, so every destination the open nav gives her — Board included — has \
                 to be in it as an icon that opens the same page the word opened."
            )
        });
    icon.click().await.unwrap_or_else(|err| {
        panic!(
            "the Board icon in the narrow rail could not be pressed: {err}. An icon the operator \
             cannot press is not a destination; a destination taken out of the layout, or covered \
             by something else, is what makes the narrow rail a dead end."
        )
    });
    wait_for(
        &browser,
        browser_harness::BOARD_READY_SELECTOR,
        "the project board she pressed the icon for",
    )
    .await;
    world.browser = Some(browser);
}

/// The whole reading order, walked once, with every stop recorded — and, taken
/// first, the rail as it sits with NOTHING focused.
///
/// That first reading is the other half of the observation. The collapsed
/// geometry leaves each word in the document as a clipped sliver, so a label box
/// of its own says nothing about whether the operator can read it; what says it
/// is the step up from the sliver to the revealed word. Both readings are of the
/// same page, the unfocused one taken while the focus is cleared, so the pair is
/// a differential and not two unrelated measurements.
#[when(regex = r"^she walks the whole page with the Tab key, stopping on each icon in the rail$")]
async fn when_she_walks_the_rail_with_tab(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let unfocused = read_nav(&browser).await;
    let walk = walk_the_tab_order(&browser).await;
    remember(
        &browser,
        WALK_KEY,
        &serde_json::json!({ "unfocused": unfocused, "walk": walk }),
    )
    .await;
    world.browser = Some(browser);
}

#[when(regex = r"^she presses Sign out in the narrow rail, once$")]
async fn when_she_presses_sign_out_once(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    press_sign_out(&browser, "from the narrow rail").await;
    world.browser = Some(browser);
}

#[when(regex = r"^she signs in again with no page scripts$")]
async fn when_she_signs_in_again_with_no_scripting(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    world.browser = Some(browser);
}

#[when(
    regex = r"^she signs in again (with the same rights|with her instance-wide rights taken away)$"
)]
async fn when_she_signs_in_again(world: &mut FoundryWorld, change: String) {
    if change == "with her instance-wide rights taken away" {
        revoke_instance_admin(world).await;
    }
    let browser = world.browser.take().expect("a browser session");
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    world.browser = Some(browser);
}

/// A REAL emulated phone of the stated width, carrying the narrowed choice at the
/// origin before the page loads — which is how an operator arrives on a phone
/// with the rail she narrowed earlier.
#[when(regex = r"^she opens the dashboard with the left nav narrowed on a phone (\d+) px wide$")]
async fn when_she_opens_the_dashboard_on_a_phone(world: &mut FoundryWorld, width: u32) {
    let phone = browser_harness::open_mobile_session_sized(width, PHONE_HEIGHT_PX).await;
    goto_sign_in(&phone, world).await;
    phone.execute(
        "try { window.localStorage.setItem(arguments[0], arguments[1]); } catch (err) {} return true;",
        vec![
            serde_json::Value::String(NAV_CHOICE_KEY.to_string()),
            serde_json::Value::String(NARROWED_CHOICE.to_string()),
        ],
    )
    .await
    .expect("leave the narrowed choice at the origin before the page loads");
    assert_eq!(
        remembered_choice(&phone).await.as_deref(),
        Some(NARROWED_CHOICE),
        "the phone does not hold the narrowed choice, so this row would measure the OPEN bar and \
         report it as the narrow one"
    );
    sign_in_and_settle(&phone, world).await;
    goto_dashboard(&phone, world).await;
    world.browser = Some(phone);
}

// ==========================================================================
// THEN — what the operator can see of it
// ==========================================================================

/// The slice's own observation: narrowing the rail costs her no destination and
/// no name. Measured on the NARROW rail — the mark present and the strip strictly
/// narrower than the width the open rail ships with — so it can never be taken on
/// the open nav twice.
#[then(
    regex = r"^every destination she had, and Sign out, is still in the rail as an icon she can see, named and ready to press$"
)]
async fn then_every_destination_is_still_in_the_rail(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_rail_with_names(browser).await;
    assert_narrowed(&reading, "while the left nav is narrow");
    let owed = owed_destinations(world).await;
    assert_rail_offers(
        &reading,
        &owed,
        RailShape::Narrow,
        "while the left nav is narrow",
    );
}

#[then(regex = r"^the workspace name and her own name are out of the narrow rail$")]
async fn then_the_names_are_out_of_the_rail(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    for (member, what) in [
        ("workspaceDisplay", "the workspace name"),
        ("identityDisplay", "the operator's own name"),
    ] {
        assert_eq!(
            reading[member].as_str(),
            Some("none"),
            "{what} is still laid out in the narrow rail ({:?}). The strip carries destinations \
             and controls only — a name in it is width spent on something she cannot press, and \
             the workspace name is the one member long enough to widen the strip and the page \
             with it.",
            reading[member]
        );
    }
}

#[then(
    regex = r"^every destination she had, and Sign out, is in the open left nav, named and ready to press$"
)]
async fn then_every_destination_is_in_the_open_nav(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_rail_with_names(browser).await;
    // ANTI-VACUITY: a session that quietly kept scripting ON would make this a
    // second copy of the desk-width row and say nothing about the no-script path.
    assert_eq!(
        reading["kbReady"].as_u64(),
        Some(0),
        "the keyboard layer's own marker is on the page, so this session is running page scripts \
         after all and the claim about the no-script path would prove nothing"
    );
    assert!(
        !reading["marked"].as_bool().unwrap_or(false),
        "the document carries the narrowed mark {:?} although no script ran to stamp it",
        reading["markValue"]
    );
    let rail = px(&reading, "railWidth");
    assert!(
        (rail - OPEN_RAIL_PX).abs() <= 2.0,
        "with no scripting the rail is {rail:.1}px wide instead of the {OPEN_RAIL_PX}px open nav \
         it has always been"
    );
    let owed = owed_destinations(world).await;
    assert_rail_offers(
        &reading,
        &owed,
        RailShape::Open,
        "with no page scripts running",
    );
}

/// The no-script arm's observation: the open nav with everything in it, and the
/// choice she made earlier still there, byte for byte. Signing out is not opening
/// the nav again, so nothing in that door may touch what she chose.
#[then(
    regex = r"^the left nav is open with every destination in it, and the choice she made earlier is byte-for-byte the one she left$"
)]
async fn then_the_no_script_nav_is_open_and_the_choice_is_kept(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_rail_with_names(browser).await;
    assert_eq!(
        reading["kbReady"].as_u64(),
        Some(0),
        "the keyboard layer's own marker is on the page, so this session is running page scripts \
         after all and nothing here would be about the no-script path"
    );
    assert!(
        !reading["marked"].as_bool().unwrap_or(false),
        "the document carries the narrowed mark {:?} although no script ran to stamp it from the \
         remembered choice",
        reading["markValue"]
    );
    let rail = px(&reading, "railWidth");
    assert!(
        (rail - OPEN_RAIL_PX).abs() <= 2.0,
        "with no scripting the rail is {rail:.1}px wide instead of the {OPEN_RAIL_PX}px open nav \
         it has always been"
    );
    let owed = owed_destinations(world).await;
    assert_rail_offers(
        &reading,
        &owed,
        RailShape::Open,
        "at her next sign-in in a browser that runs no page scripts",
    );
    assert_eq!(
        remembered_choice(browser).await.as_deref(),
        Some(NARROWED_CHOICE),
        "the browser now remembers {:?} where she left the narrow nav. Signing out is not opening \
         the nav again: the door that ends her session must leave what she chose exactly as it \
         found it, or one visit in a browser that runs no scripts throws her choice away.",
        remembered_choice(browser).await
    );
}

/// The journey's own observation: the icon opens the place it names, the rail
/// stays narrow across the navigation, and the rail says where she now is — on
/// exactly one member, set apart from the rest by its tint.
#[then(
    regex = r"^the board opens with the rail still narrow, and Board alone is marked as the page she is on$"
)]
async fn then_the_board_opens_with_board_marked(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let here = browser
        .current_url()
        .await
        .map(|url| url.to_string())
        .unwrap_or_default();
    assert!(
        here.contains(&format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}")),
        "pressing the Board icon landed on {here} instead of her project board, so the icon in \
         the narrow rail does not open the destination it is named for"
    );
    let reading = read_rail_with_names(browser).await;
    assert_narrowed(&reading, "on the board she opened from the narrow rail");

    let marked: Vec<String> = members(&reading)
        .iter()
        .filter(|member| member["current"].as_str() == Some("page"))
        .map(destination_of)
        .collect();
    assert_eq!(
        marked,
        vec!["Board".to_string()],
        "the narrow rail marks {marked:?} as the page she is on while she is on the board. \
         Exactly one member carries the mark, and it is the one she is looking at: no mark leaves \
         her guessing where she is in a strip of glyphs, and two marks tell her two places."
    );
    let tint = members(&reading)
        .iter()
        .find(|member| member["current"].as_str() == Some("page"))
        .map(|member| {
            member["background"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .expect("the marked member was just found");
    let others: Vec<String> = members(&reading)
        .iter()
        .filter(|member| member["current"].as_str() != Some("page"))
        .map(|member| {
            member["background"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert!(
        !others.is_empty() && others.iter().all(|other| *other != tint),
        "the marked icon is painted {tint:?} and the icons she is not on are painted {others:?}. \
         The mark has to be visible as well as announced: an icon-sized target with the same fill \
         as its neighbours marks nothing for the operator who can see the strip."
    );
}

/// The name she can SEE, not only the one she is told — and the one assertion in
/// this lane that has to be a DIFFERENTIAL to mean anything.
///
/// The collapsed geometry keeps each word in the document and clips it to a
/// sliver, which is how the name survives the loss of the word on screen. So
/// "the label has a box" is true of a rail whose flyout rule was never written:
/// a layout box is unmoved by `clip`, by `clip-path` and by an ancestor's
/// `overflow`, and a one-pixel box passes any greater-than-zero test. What
/// separates revealed from clipped is the STEP UP: the label's box while its icon
/// holds the focus against the box the same label has while it does not. A hit
/// test at the centre of the revealed box travels with the failure message as
/// corroboration and is never the whole reading, because the painted sliver of a
/// clipped label answers a hit test too.
///
/// The walk must also have STOPPED on every destination she has, or the claim
/// would hold over a strip the keyboard cannot enter at all.
#[then(
    regex = r"^each icon she stops on reveals the name of the place it opens, in a box materially larger than the sliver that name is while the icon does not hold the focus$"
)]
async fn then_each_focused_icon_reveals_its_name(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let recorded = recall(browser, WALK_KEY).await;
    let unfocused = recorded["unfocused"].clone();
    let stops = recorded["walk"]
        .as_array()
        .expect("the Tab walk recorded its stops")
        .clone();
    let rail_stops: Vec<serde_json::Value> = stops
        .iter()
        .filter(|stop| stop["isRailMember"].as_bool().unwrap_or(false))
        .cloned()
        .collect();
    let reached: Vec<String> = rail_stops
        .iter()
        .map(|stop| stop["text"].as_str().unwrap_or_default().trim().to_string())
        .collect();
    let owed = owed_destinations(world).await;
    let missed: Vec<&String> = owed
        .iter()
        .filter(|wanted| !reached.iter().any(|stop| stop == *wanted))
        .collect();
    assert!(
        missed.is_empty(),
        "walking the page with the Tab key never stopped on {missed:?} in the narrow rail. The \
         walk reached {reached:?} of her {} destinations, so `each icon reveals its name` would \
         hold over a strip her keyboard cannot enter. Every destination stays in the reading order \
         while the rail is narrow.",
        owed.len()
    );

    let mut still_clipped: Vec<String> = Vec::new();
    let mut wrong_word: Vec<String> = Vec::new();
    for stop in &rail_stops {
        let destination = stop["text"].as_str().unwrap_or_default().trim().to_string();
        let focused = &stop["labelBox"];
        let clipped = members(&unfocused)
            .iter()
            .find(|member| member["text"].as_str().unwrap_or_default().trim() == destination)
            .map(|member| member["labelBox"].clone())
            .unwrap_or(serde_json::Value::Null);
        let focused_width = focused["width"].as_f64().unwrap_or_default();
        let focused_height = focused["height"].as_f64().unwrap_or_default();
        let clipped_width = clipped["width"].as_f64().unwrap_or_default();
        let revealed = stop["labelPresent"].as_bool().unwrap_or(false)
            && stop["labelInTheLayout"].as_bool().unwrap_or(false)
            && focused_height >= 1.0
            && focused_width >= clipped_width + MATERIALLY_WIDER_PX;
        if !revealed {
            still_clipped.push(format!(
                "{destination:?} shows a label {focused_width:.1}px wide while it holds the focus \
                 and {clipped_width:.1}px wide while it does not (in the layout: {}, its centre \
                 hit-tests to the label: {})",
                stop["labelInTheLayout"], stop["labelCentreHitsTheLabel"]
            ));
        }
        if stop["labelText"].as_str().unwrap_or_default().trim() != destination {
            wrong_word.push(format!(
                "{destination:?} shows {:?}",
                stop["labelText"].as_str().unwrap_or("nothing")
            ));
        }
    }
    assert!(
        still_clipped.is_empty(),
        "the word stays clipped where the operator cannot read it on {} of the icons she stopped \
         on: {still_clipped:?}. The narrow rail clips each word out of the strip and REVEALS it \
         when the icon takes the focus or the pointer, so the box has to grow by at least \
         {MATERIALLY_WIDER_PX}px over the sliver it is while unfocused. A flyout rule that was \
         never written, or that the focus never triggers, leaves every name in the document and \
         none of them legible - and an operator who cannot read a glyph has to press it to find \
         out where it goes.",
        still_clipped.len()
    );
    assert!(
        wrong_word.is_empty(),
        "the word a focused icon reveals is not the name of the place it opens: {wrong_word:?}"
    );
}

/// One press, and she is out. No second press, no step in between: the session is
/// over and the app gives her no page of hers until she signs in again.
#[then(
    regex = r"^her session is over with nothing else asked of her, and the app opens no page of hers until she signs in again$"
)]
async fn then_one_press_signed_her_out(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    wait_for(&browser, SIGN_IN_WITNESS, "the sign-in screen").await;
    let asked = browser
        .execute(
            "return document.querySelectorAll('dialog[open], [role=dialog], [role=alertdialog]').length;",
            Vec::new(),
        )
        .await
        .expect("look for anything asking her to confirm");
    assert_eq!(
        asked.as_u64(),
        Some(0),
        "one press of Sign out in the narrow rail put {:?} in front of the operator. In the rail \
         Sign out is the one press it is in the open nav: a confirmation step in the strip is a \
         second press she never had to make.",
        asked
    );
    goto(&browser, &format!("{}/", base_url(world)), "the dashboard").await;
    let shells = browser
        .execute(
            "return [document.querySelectorAll('.app-shell').length, window.location.pathname];",
            Vec::new(),
        )
        .await
        .expect("read whether the app still opens her pages");
    let row = shells.as_array().expect("the probe returns a pair");
    assert_eq!(
        row[0].as_u64(),
        Some(0),
        "after one press of Sign out the app still renders her signed-in shell at {:?}, so the \
         press did not end the session and the next person on this browser gets her workspace",
        row[1]
    );
    world.browser = Some(browser);
}

/// The choice survives the sign-out, and the rail she comes back to is drawn from
/// the rights she holds NOW — not from the ones the ended session had.
#[then(
    regex = r"^the left nav is (narrow|open) again, with the destinations her rights give her now$"
)]
async fn then_the_nav_is_as_she_left_it(world: &mut FoundryWorld, geometry: String) {
    let browser = browser_of(world);
    let reading = read_rail_with_names(browser).await;
    if geometry == "narrow" {
        assert_narrowed(
            &reading,
            "at her next sign-in in the same browser, where she left the nav narrow",
        );
    } else {
        assert!(
            !reading["marked"].as_bool().unwrap_or(false),
            "she left the left nav open and her next sign-in carries the narrowed mark {:?}",
            reading["markValue"]
        );
        let rail = px(&reading, "railWidth");
        assert!(
            (rail - OPEN_RAIL_PX).abs() <= 2.0,
            "she left the left nav open and her next sign-in renders it {rail:.1}px wide instead \
             of the {OPEN_RAIL_PX}px it ships with"
        );
    }
    let owed = owed_destinations(world).await;
    assert_rail_offers(
        &reading,
        &owed,
        if geometry == "narrow" {
            RailShape::Narrow
        } else {
            RailShape::Open
        },
        "at her next sign-in in the same browser, where the rail must show the rights she holds \
         now",
    );
}

/// The phone promise, both halves together: the bar holds every icon INSIDE
/// itself — wrapping onto as many rows as it needs — and the page never slides
/// sideways. Measured below the breakpoint, proved to be below it.
#[then(
    regex = r"^every destination she has, and Sign out, is laid out inside the top bar, and that page does not slide sideways$"
)]
async fn then_the_phone_bar_holds_every_icon(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_rail_with_names(browser).await;
    let viewport = px(&reading, "viewport");
    assert!(
        viewport <= PHONE_BREAKPOINT_PX,
        "this session lays out at {viewport}px, above the {PHONE_BREAKPOINT_PX}px the promise is \
         about — the phone rules never applied, so this row would measure the desk-width layout"
    );
    assert_narrowed_mark_only(&reading, "on the phone");
    let owed = owed_destinations(world).await;
    assert_rail_offers(
        &reading,
        &owed,
        RailShape::Narrow,
        "in the narrow bar on a phone",
    );

    let bar = reading["railBox"].clone();
    let bar_left = bar["left"].as_f64().unwrap_or_default();
    let bar_right = bar["right"].as_f64().unwrap_or_default();
    let bar_bottom = bar["bottom"].as_f64().unwrap_or_default();
    let bar_top = bar["top"].as_f64().unwrap_or_default();
    assert!(
        bar_right - bar_left > 0.0,
        "the bar is not drawn at all on this phone, so nothing can be said about what it holds"
    );
    let outside: Vec<String> = members(&reading)
        .iter()
        .filter(|member| {
            let own = &member["box"];
            own["left"].as_f64().unwrap_or_default() < bar_left - 0.5
                || own["right"].as_f64().unwrap_or_default() > bar_right + 0.5
                || own["top"].as_f64().unwrap_or_default() < bar_top - 0.5
                || own["bottom"].as_f64().unwrap_or_default() > bar_bottom + 0.5
        })
        .map(destination_of)
        .collect();
    assert!(
        outside.is_empty(),
        "on a {viewport:.0}px phone the icons {outside:?} are laid out outside the top bar (the \
         bar spans {bar_left:.1}px to {bar_right:.1}px and {bar_top:.1}px to {bar_bottom:.1}px). \
         Below the breakpoint the bar is a row that WRAPS: an icon pushed past its edge is what \
         widens the document under the operator's thumb."
    );
    assert!(
        bar_right <= viewport + 0.5,
        "the top bar reaches {bar_right:.1}px in a {viewport:.1}px phone viewport, so it is wider \
         than her screen"
    );
    let document = px(&reading, "docWidth");
    assert!(
        document <= viewport + 0.5,
        "the page is {document:.1}px wide inside a {viewport:.1}px viewport, so it slides \
         sideways with the icons shown. Something in the bar is wider than the screen — a row of \
         icons told not to wrap does exactly this, at every phone width."
    );
}

// ==========================================================================
// WHAT THE RAIL OWES HER, AND WHAT IT OFFERS
// ==========================================================================

/// The destinations her rights give her, built from the rights she HOLDS and not
/// from the page: Home, Board, Teams, Settings and Keyboard shortcuts for
/// anyone, Invite a member, Machine tokens and Integrations for a workspace
/// admin, Instance admin for an instance admin, and Sign out always. Read from the store, which is where the rights the scenario
/// granted live, so the expectation cannot quietly become "whatever the rail
/// renders".
async fn owed_destinations(world: &FoundryWorld) -> Vec<String> {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let (workspace_admin, instance_admin): (bool, bool) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM workspace_memberships m JOIN users u ON u.id = m.user_id
                         WHERE u.email_lower = $1 AND m.role = 'admin'),
                EXISTS (SELECT 1 FROM instance_admins a JOIN users u ON u.id = a.user_id
                         WHERE u.email_lower = $1)",
    )
    .bind(MEI_EMAIL)
    .fetch_one(pool)
    .await
    .expect("read the rights the operator holds");
    let mut owed: Vec<String> = EVERY_OPERATOR.iter().map(|name| name.to_string()).collect();
    if workspace_admin {
        owed.extend(FOR_A_WORKSPACE_ADMIN.iter().map(|name| name.to_string()));
    }
    if instance_admin {
        owed.push(FOR_AN_INSTANCE_ADMIN.to_string());
    }
    owed.push(ALWAYS_LAST.to_string());
    owed
}

/// What the rail OFFERS her, read from one moment of the live document: the set
/// of names the browser computes for its members, every one of them in the
/// layout, visible and able to take the keyboard focus, and nothing invisible
/// able to take it.
///
/// The names are compared as a SET: the order the members are laid out in is a
/// design value and no promise fixes it.
/// Which geometry the rail is in, because what each member must DRAW is the one
/// thing the two geometries disagree about: narrow, every member draws its glyph
/// and the word is clipped out of the way; open, every member shows its word and
/// no glyph is laid out beside it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RailShape {
    Narrow,
    Open,
}

fn assert_rail_offers(reading: &serde_json::Value, owed: &[String], shape: RailShape, when: &str) {
    let mut offered: Vec<String> = members(reading)
        .iter()
        .map(|member| {
            member["name"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .collect();
    let mut wanted = owed.to_vec();
    offered.sort();
    wanted.sort();
    assert_eq!(
        offered,
        wanted,
        "{when} the rail announces {offered:?} where her rights give her {wanted:?}. Every \
         destination the open nav gives her, and Sign out, is a member of the rail, and each one \
         carries the NAME of the place it opens — an icon with no name, or with the wrong one, is \
         not a destination a screen-reader operator can choose, and the rail is her only route \
         while it is narrow. Members read: {:?}",
        members(reading)
    );
    let unreachable: Vec<String> = members(reading)
        .iter()
        .filter(|member| {
            !member["inLayout"].as_bool().unwrap_or(false)
                || !member["visible"].as_bool().unwrap_or(false)
                || !member["focusable"].as_bool().unwrap_or(false)
        })
        .map(|member| {
            format!(
                "{} (in the layout: {}, visible: {}, reachable: {})",
                destination_of(member),
                member["inLayout"],
                member["visible"],
                member["focusable"]
            )
        })
        .collect();
    assert!(
        unreachable.is_empty(),
        "{when} {} of her destinations cannot be reached: {unreachable:?}. Each one is in the \
         layout, can be seen and can take the keyboard focus, or the operator is in a strip with \
         no way out of it.",
        unreachable.len()
    );
    let invisible_focusable = reading["invisibleFocusable"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        invisible_focusable.is_empty(),
        "{when} {} rail link(s) can take the keyboard focus while they are invisible: {:?}. \
         Hiding a destination by width, by opacity or by clipping leaves it reachable with \
         nothing to see, and the operator tabs into a nav she cannot see.",
        invisible_focusable.len(),
        invisible_focusable
    );
    match shape {
        // THE NARROW RAIL IS A RAIL OF ICONS, and this is the only place that
        // says so. Without it a strip of blank slivers passes everything above:
        // the name still computes from the clipped word, the member still has a
        // box and still takes the focus, and the operator is left pressing
        // nothing she can see. WHICH glyph is drawn stays unobserved — that
        // limit is recorded — but THAT one is drawn, at a size she can aim at,
        // is the promise.
        RailShape::Narrow => {
            let blank: Vec<String> = members(reading)
                .iter()
                .filter(|member| {
                    let icon = &member["iconBox"];
                    !member["iconPresent"].as_bool().unwrap_or(false)
                        || !member["iconLaidOut"].as_bool().unwrap_or(false)
                        || icon["width"].as_f64().unwrap_or_default() < PRESSABLE_PX
                        || icon["height"].as_f64().unwrap_or_default() < PRESSABLE_PX
                })
                .map(|member| {
                    format!(
                        "{} (draws a glyph: {}, laid out: {}, its box: {})",
                        destination_of(member),
                        member["iconPresent"],
                        member["iconLaidOut"],
                        member["iconBox"]
                    )
                })
                .collect();
            assert!(
                blank.is_empty(),
                "{when} {} of her destinations draw nothing she can see or aim at: {blank:?}. The \
                 narrow rail replaces each word with an ICON at least {PRESSABLE_PX}px across: a \
                 member whose glyph is absent, or clipped to a sliver, is a blank target the \
                 operator has to press to find out where it goes - which is the dead end this \
                 rail exists to remove, and it hides behind every other assertion here, because \
                 the clipped word still computes the name and the empty box still takes the focus.",
                blank.len()
            );
        }
        // The open nav keeps the look it has today. Asserted wherever the open
        // geometry is read, not only on the way into the narrow one.
        RailShape::Open => {
            let with_icons: Vec<String> = members(reading)
                .iter()
                .filter(|member| member["iconLaidOut"].as_bool().unwrap_or(false))
                .map(destination_of)
                .collect();
            assert!(
                with_icons.is_empty(),
                "{when} the OPEN left nav draws an icon beside the words of {with_icons:?}. The \
                 open nav keeps the look it has today: the glyph belongs to the narrow rail, \
                 where the word cannot fit."
            );
        }
    }
}

/// The rail is NARROW: the mark the rules select on is there and the strip is
/// strictly narrower than the width the open rail ships with, and still drawn.
/// No width of its own is named — the strip's width is a design value.
fn assert_narrowed(reading: &serde_json::Value, when: &str) {
    assert_narrowed_mark_only(reading, when);
    let rail = px(reading, "railWidth");
    assert!(
        rail > 0.0,
        "{when} the rail is not drawn at all ({rail:.1}px). A narrow nav is a strip she can work \
         from and get back from, never a rail of zero width."
    );
    assert!(
        rail < OPEN_RAIL_PX - 8.0,
        "{when} the rail is {rail:.1}px wide, not strictly narrower than the {OPEN_RAIL_PX}px it \
         ships with, so nothing was released for the page content and the state under test was \
         never reached"
    );
}

/// The narrowed state was REACHED — asserted on its own where the geometry is a
/// wrapping top bar rather than a strip, so the measurement below it cannot be
/// taken on the open nav by accident.
fn assert_narrowed_mark_only(reading: &serde_json::Value, when: &str) {
    assert!(
        reading["marked"].as_bool().unwrap_or(false),
        "{when} the document element carries no narrowed mark, so the rules that draw the narrow \
         rail have nothing to select on and every measurement below would have been taken on the \
         open nav"
    );
}

fn destination_of(member: &serde_json::Value) -> String {
    let name = member["name"].as_str().unwrap_or_default().trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let text = member["text"].as_str().unwrap_or_default().trim();
    if !text.is_empty() {
        return format!("{text} (announcing nothing)");
    }
    format!(
        "an unnamed member opening {}",
        member["opens"].as_str().unwrap_or("nothing")
    )
}

// ==========================================================================
// HER RIGHTS, THE SESSION AND THE PAGES
// ==========================================================================

/// Grant the rights the row names, at the store the app reads them from, and READ
/// THEM BACK: a grant that silently failed would turn the row into a second copy
/// of the no-rights one while still passing.
async fn grant(world: &mut FoundryWorld, rights: &str) {
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
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
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
            "the operator was not made an admin of her workspace, so the rail would be measured \
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
            "the operator was not made an admin of the instance, so the rail would be measured \
             against rights she does not hold"
        );
    }
}

/// Take the instance-wide right away between the sign-out and the next sign-in,
/// so what the next rail renders can be compared with the rights she holds NOW.
async fn revoke_instance_admin(world: &mut FoundryWorld) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let removed = sqlx::query(
        "DELETE FROM instance_admins
         WHERE user_id = (SELECT id FROM users WHERE email_lower = $1)",
    )
    .bind(MEI_EMAIL)
    .execute(pool)
    .await
    .expect("take the instance-wide right away");
    assert_eq!(
        removed.rows_affected(),
        1,
        "the operator held no instance-wide right to take away, so this row would observe an \
         unchanged rail and call it a changed one"
    );
}

async fn sign_in_and_settle(browser: &fantoccini::Client, world: &FoundryWorld) {
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned the harness");
    browser_harness::sign_in_through_browser(browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let here = browser
            .current_url()
            .await
            .map(|url| url.to_string())
            .unwrap_or_default();
        if (!here.is_empty() && !here.contains("/sign-in")) || Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn base_url(world: &FoundryWorld) -> String {
    world
        .harness
        .as_ref()
        .expect("harness")
        .base_url()
        .to_string()
}

async fn goto(browser: &fantoccini::Client, url: &str, what: &str) {
    browser
        .goto(url)
        .await
        .unwrap_or_else(|err| panic!("navigate to {what}: {err}"));
}

async fn goto_dashboard(browser: &fantoccini::Client, world: &FoundryWorld) {
    goto(browser, &format!("{}/", base_url(world)), "the dashboard").await;
    wait_for(browser, DASHBOARD_WITNESS, "the dashboard").await;
}

async fn goto_sign_in(browser: &fantoccini::Client, world: &FoundryWorld) {
    goto(
        browser,
        &format!("{}/sign-in", base_url(world)),
        "the sign-in screen",
    )
    .await;
    wait_for(browser, SIGN_IN_WITNESS, "the sign-in screen").await;
}

async fn wait_for(browser: &fantoccini::Client, selector: &str, what: &str) {
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(selector))
        .await
        .unwrap_or_else(|err| panic!("{what} never rendered `{selector}`: {err}"));
}

/// What the ORIGIN holds under the choice key: `None` for the absence that IS the
/// open nav.
async fn remembered_choice(client: &fantoccini::Client) -> Option<String> {
    let value = client
        .execute(
            "try { return window.localStorage.getItem(arguments[0]); } catch (err) { return null; }",
            vec![serde_json::Value::String(NAV_CHOICE_KEY.to_string())],
        )
        .await
        .expect("read the remembered nav choice");
    value.as_str().map(str::to_string)
}

/// One real press of the real control, through a real pointer.
async fn press_the_control(client: &fantoccini::Client, what_for: &str) {
    let control = client
        .find(Locator::Css(NAV_CONTROL))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no control for the left nav to press {what_for}: nothing in the rail's \
                 brand row matches `{NAV_CONTROL}` ({err})"
            )
        });
    control
        .click()
        .await
        .unwrap_or_else(|err| panic!("the nav control could not be pressed {what_for}: {err}"));
}

/// ONE press of the rail's own Sign out — the same post form the open nav
/// renders. The panic text is this lane's RED message while Sign out is not in
/// the strip, so it says WHAT is missing, WHY it matters and WHERE it belongs.
async fn press_sign_out(client: &fantoccini::Client, what_for: &str) {
    let button = client
        .find(Locator::Css(RAIL_SIGN_OUT))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no Sign out to press {what_for}: nothing in the rail matches \
                 `{RAIL_SIGN_OUT}` ({err}). Sign out stays in the strip, as the same post form \
                 the open nav renders with an icon and a clipped word on its button — an operator \
                 who has to open the nav again to leave is being told the narrow rail is a dead \
                 end."
            )
        });
    button.click().await.unwrap_or_else(|err| {
        panic!(
            "Sign out in the rail could not be pressed {what_for}: {err}. It is one press, with \
             nothing in front of it: a button out of the layout, or covered by something else, is \
             a button she cannot use."
        )
    });
}

async fn remember(client: &fantoccini::Client, key: &str, value: &serde_json::Value) {
    client
        .execute(
            "window.sessionStorage.setItem(arguments[0], arguments[1]); return true;",
            vec![
                serde_json::Value::String(key.to_string()),
                serde_json::Value::String(value.to_string()),
            ],
        )
        .await
        .expect("leave a reading for the step after this one");
}

async fn recall(client: &fantoccini::Client, key: &str) -> serde_json::Value {
    let raw = client
        .execute(
            "return window.sessionStorage.getItem(arguments[0]);",
            vec![serde_json::Value::String(key.to_string())],
        )
        .await
        .expect("read back the reading the earlier step left");
    let raw = raw.as_str().unwrap_or_else(|| {
        panic!(
            "no reading was left under {key}, so the step before this one did not run or the page \
             lost it — nothing can be compared"
        )
    });
    serde_json::from_str(raw).expect("a recorded reading is JSON")
}

fn browser_of(world: &FoundryWorld) -> &fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first")
}
