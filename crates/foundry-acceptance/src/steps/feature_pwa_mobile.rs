//! pwa-mobile-rendering — the `@needs-browser` MOBILE lane's step definitions.
//!
//! SLICE 01 / STEP 01-01 lands BOTH the mechanism (the `<meta name=viewport>` tag
//! in base.html + the mobile `@media` rules that fit the authed shell to a phone)
//! AND the instrument that proves it: a real headless Chrome under chromedriver
//! MOBILE EMULATION (`open_mobile_session()`), driving each primary surface at a
//! 390×844 phone viewport and asserting LAYOUT FACTS (a declared mobile viewport
//! meta; `documentElement.scrollWidth <= window.innerWidth`).
//!
//! WHY MOBILE EMULATION AND NOT A NARROW WINDOW (DESIGN ADR-003 — the load-bearing
//! test decision): `--headless=new` is DESKTOP Chrome. At a narrow OS window it
//! still lays out at the window width regardless of the viewport meta, so a
//! resize-only test would be GREEN whether or not the meta exists — green over
//! nothing. chromedriver `goog:chromeOptions.mobileEmulation` makes Chrome apply
//! REAL mobile viewport semantics: the ~980px fallback layout when NO viewport
//! meta is declared (the defect reproduces → RED), and the device-width layout
//! once the meta is present (the fix is measurable → GREEN). `open_mobile_session`
//! lives in `support::browser_harness` beside the desktop `open_session`.
//!
//! WHY ITS OWN STEP MODULE: the acceptance crate carries one `feature_*.rs` step
//! module per feature (feature_board_new_issue, feature_form_error_display, …). A
//! parallel structural convention, not a parallel implementation — cucumber-rs
//! requires globally-unique step text, so each feature owns its phrases. The
//! Background lines (`a workspace … exists`, `a project … exists`, `Mei is signed
//! in`) are the shipped HTTP-lane steps (feature_board_new_issue.rs): they seed
//! Acme / Mei / Backend / Sandbox and spawn the ONE shared `InProcHarness`. The
//! mobile Givens below open a fantoccini session against THAT SAME in-process
//! origin and sign Mei in through the real form — both lanes exercise one app.

use crate::support::browser_harness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use std::time::{Duration, Instant};

const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";
const PROJECT_KEY_PREFIX: &str = "GEN";

/// The board's shipped "New issue" trigger (board.html:6) — clicking it fires the
/// same `hx-get` → swap into `#modal-root` a pointer click does.
const NEW_ISSUE_TRIGGER: &str = "[data-action='new-issue']";
/// The mounted new-issue dialog (new_issue_modal.html:1).
const DIALOG_SELECTOR: &str = "#modal-root [data-modal='new-issue']";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Sign Mei in through the REAL form on a fresh MOBILE session and return the
/// signed-in client pointed at the shared origin. The HTTP Background already
/// seeded Acme / Mei / Backend / Sandbox and spawned the harness, so this only
/// adds the emulated-phone view onto the same app.
async fn mobile_session_signed_in(world: &FoundryWorld) -> fantoccini::Client {
    let browser = browser_harness::open_mobile_session().await;
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned harness");
    browser_harness::sign_in_through_browser(&browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
    // The sign-in form submit navigates to "/". WAIT for that redirect to SETTLE
    // before returning: a follow-up `goto(surface)` fired while the sign-in
    // navigation is still in flight gets clobbered by it and lands back on "/",
    // not the surface we asked for (observed: the board request resolved to "/").
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let cur = browser
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();
        if !cur.is_empty() && !cur.contains("/sign-in") {
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    browser
}

/// The Sandbox board URL on the shared origin.
fn board_url(world: &FoundryWorld) -> String {
    let harness = world.harness.as_ref().expect("harness");
    format!(
        "{}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}",
        harness.base_url()
    )
}

/// Seed issue GEN-1 into the EXISTING Sandbox project (created by the HTTP
/// Background) so an issue page renders. Mirrors `sandbox_has_issue` in
/// feature_form_error_display.rs — the store pool directly, no HTTP round-trip.
async fn seed_sandbox_issue(world: &FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = $1")
            .bind(PROJECT_KEY_PREFIX)
            .fetch_one(pool)
            .await
            .expect("fetch the Sandbox project seeded by the Background");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MEI_EMAIL)
        .fetch_one(pool)
        .await
        .expect("fetch Mei");
    // board-lane-management sweep: 0015 dropped the state DEFAULT — INSERT it.
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, 1, 'Fits a phone', '', 'backlog', $4)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.0)
    .bind(project.1)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("insert the Sandbox issue");
}

/// Bring up the requested authed SURFACE in an emulated-phone session. One Given
/// serves S1 and every S2 outline row — a single regex, so S1's `the "Sandbox"
/// board` line and the S2 outline's identical surface never register two
/// definitions for the same text (a cucumber-rs ambiguity error).
#[given(regex = r#"^Mei opens (.+) in a mobile browser at 390x844$"#)]
async fn opens_surface_in_mobile(world: &mut FoundryWorld, surface: String) {
    // The issue-page surface needs a card to exist BEFORE the page loads.
    if surface.contains("issue page") {
        seed_sandbox_issue(world).await;
    }
    let browser = mobile_session_signed_in(world).await;

    let url = if surface.contains("dashboard") {
        format!("{}/", world.harness.as_ref().expect("harness").base_url())
    } else if surface.contains("issue page") {
        format!("{}/issues/1", board_url(world))
    } else {
        // Both `the "Sandbox" board` and `… with the new-issue dialog open` start
        // on the board; the dialog variant opens it after navigation.
        board_url(world)
    };
    browser.goto(&url).await.expect("navigate to the surface");

    // Wait for the authed shell to render (layout settled), THEN assert we landed
    // on the surface we asked for. `.app-shell` alone is too weak an anchor — it
    // is present on `/` too — so a board→"/" bounce would pass it vacuously; the
    // exact-URL assertion below is the real anti-vacuity guard: it fails loudly if
    // the requested surface redirected elsewhere, so the overflow check can never
    // measure the wrong page.
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(".app-shell"))
        .await
        .unwrap_or_else(|_| {
            panic!("the surface {surface:?} never rendered the authed .app-shell within {WAIT_TIMEOUT:?}")
        });
    let landed = browser
        .current_url()
        .await
        .expect("read the browser's current URL")
        .to_string();
    assert_eq!(
        landed, url,
        "opening {surface:?} landed on {landed} instead of {url} — the surface redirected away \
         (a non-member bounce, or a sign-in redirect race). Measuring overflow here would test \
         the wrong page."
    );

    if surface.contains("new-issue dialog open") {
        browser
            .wait()
            .at_most(WAIT_TIMEOUT)
            .for_element(Locator::Css(NEW_ISSUE_TRIGGER))
            .await
            .expect("the board must render the New issue trigger")
            .click()
            .await
            .expect("click the New issue trigger");
        browser
            .wait()
            .at_most(WAIT_TIMEOUT)
            .for_element(Locator::Css(DIALOG_SELECTOR))
            .await
            .expect("clicking New issue must open the dialog");
    }

    world.browser = Some(browser);
}

/// S1 — the page must declare a mobile viewport meta. Without
/// `<meta name="viewport" content="width=device-width …">` mobile Chrome falls
/// back to the ~980px layout and the phone gets a shrunk-to-fit desktop page;
/// this asserts the tag that opts the document into device-width layout.
#[then(regex = r"^the page declares a mobile viewport meta$")]
async fn declares_viewport_meta(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let content = browser
        .execute(
            "var m = document.querySelector('meta[name=\"viewport\"]');\
             return m ? m.getAttribute('content') : null;",
            vec![],
        )
        .await
        .expect("read the viewport meta");
    let content = content.as_str().unwrap_or_else(|| {
        panic!(
            "the page declares NO <meta name=\"viewport\"> — mobile Chrome falls back to its \
             ~980px layout and the phone renders a shrunk desktop page. base.html must carry \
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">."
        )
    });
    assert!(
        content.contains("width=device-width"),
        "the viewport meta is present but does not declare width=device-width (content={content:?}) \
         — without it the layout viewport is not tied to the device and the page still overflows."
    );
}

/// S1 + S2 — THE LAYOUT ORACLE (ADR-003). The document must not overflow its own
/// viewport horizontally: `documentElement.scrollWidth <= window.innerWidth`. On
/// the no-viewport tree mobile Chrome lays every surface out at ~980px, so this
/// exceeds the 390px viewport — the defect, reproduced. With the viewport meta +
/// mobile `@media` rules the shell fits and the board scrolls within its own
/// container instead of widening the page.
#[then(regex = r"^the page has no horizontal overflow$")]
async fn no_horizontal_overflow(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    // Oracle = scrollWidth vs documentElement.clientWidth, NOT window.innerWidth.
    // Under mobile emulation `window.innerWidth` EXPANDS to the overflowed content
    // width (observed: a 390px layout with an 848px-wide board reported
    // innerWidth == scrollWidth == 1120, so a scrollWidth<=innerWidth check passed
    // VACUOUSLY over a real overflow). `documentElement.clientWidth` is the fixed
    // device-width layout viewport (390) that does NOT expand — the honest ceiling
    // a horizontally-overflowing surface exceeds.
    let dims = browser
        .execute(
            "return [document.documentElement.scrollWidth, document.documentElement.clientWidth];",
            vec![],
        )
        .await
        .expect("measure the document vs layout viewport width");
    let arr = dims
        .as_array()
        .expect("the probe returns [scrollWidth, clientWidth]");
    let scroll_width = arr[0].as_f64().expect("scrollWidth is a number");
    let client_width = arr[1].as_f64().expect("clientWidth is a number");
    assert!(
        scroll_width <= client_width,
        "the page overflows its {client_width}px mobile layout viewport horizontally \
         (documentElement.scrollWidth = {scroll_width}px). Some surface is wider than the phone — \
         the viewport meta and the mobile @media rules must contain the fixed-width desktop shell \
         (the 240px sidebar rail, the board's min-width columns) so nothing widens the document."
    );
}

// ---------------------------------------------------------------- S11 (source)

/// S11's precondition — narrative only. The assertion below reads the two source
/// files directly; there is no browser or runtime state to arrange here.
#[given(regex = r"^a CSS change has landed for this feature$")]
async fn css_change_landed(_world: &mut FoundryWorld) {}

/// S11 — the SOURCE-level hash guard (a DELIVER crafter check, not a browser
/// assertion). The stylesheet is hand-hashed `foundry.<sha256-prefix>.css` and its
/// name is referenced in TWO places — base.html's `<link>` and lib.rs's immutable-
/// cache assertion. A CSS edit rotates the hash; if only one reference is updated
/// the browser pins a stale sheet or the unit test asserts a dead URL. This reads
/// both files and asserts they name the SAME hash.
#[then(regex = r"^base\.html and lib\.rs reference the same foundry\.<hash>\.css$")]
async fn hash_consistent_across_base_and_lib(_world: &mut FoundryWorld) {
    let base_html = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../foundry-app/templates/base.html"
    ))
    .expect("read base.html");
    let lib_rs = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../foundry-app/src/lib.rs"
    ))
    .expect("read lib.rs");

    let hash_of = |source: &str, whom: &str| -> String {
        let marker = "foundry.";
        let start = source
            .find(marker)
            .unwrap_or_else(|| panic!("{whom} references no foundry.<hash>.css at all"))
            + marker.len();
        let rest = &source[start..];
        let end = rest
            .find(".css")
            .unwrap_or_else(|| panic!("{whom}'s foundry. reference has no .css suffix"));
        rest[..end].to_string()
    };

    let base_hash = hash_of(&base_html, "base.html");
    let lib_hash = hash_of(&lib_rs, "lib.rs");
    assert_eq!(
        base_hash, lib_hash,
        "base.html links foundry.{base_hash}.css but lib.rs's immutable-cache assertion names \
         foundry.{lib_hash}.css — a CSS hash rotation updated one reference and not the other, so \
         either the browser pins a stale stylesheet or the unit test guards a dead URL."
    );
}

// ---------------------------------------------------------- S3–S7 (slice 02)
//
// Slice 02 makes the fitted-but-still-desktop-shaped surfaces from slice 01
// genuinely responsive: the new-issue modal becomes a full-width sheet that caps
// at the screen and scrolls its own body (S3), the board columns scroll WITHIN
// their container while the page never widens (S4), the sidebar rail reflows to a
// mobile top-bar affordance (S5), primary controls grow to a ~44px thumb target
// (S6), and — the blast-radius guard — the DESKTOP session is UNCHANGED (S7),
// proving every new rule stayed inside the `@media (max-width:480px)` bound.

/// The centered white card inside the mounted new-issue modal. S3 measures THIS
/// element (not the full-viewport `.modal` backdrop) for the sheet behaviour.
const MODAL_DIALOG_SELECTOR: &str = "#modal-root [data-modal='new-issue'] .modal-dialog";

/// Sign Mei in on a fresh DESKTOP session and settle the redirect — the S7
/// counterpart of [`mobile_session_signed_in`], driving the SHIPPED desktop
/// `open_session` (fixed 1280×900 window) so the blast-radius guard measures the
/// real desktop layout, not the emulated phone.
async fn desktop_session_signed_in(world: &FoundryWorld) -> fantoccini::Client {
    let browser = browser_harness::new_session().await;
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned harness");
    browser_harness::sign_in_through_browser(&browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let cur = browser
            .current_url()
            .await
            .map(|u| u.to_string())
            .unwrap_or_default();
        if !cur.is_empty() && !cur.contains("/sign-in") {
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    browser
}

/// Open the Sandbox board in an emulated-phone session and settle on it WITHOUT
/// opening the dialog — the S3/S6 "Mei is on the board" precondition, a leaner
/// sibling of [`opens_surface_in_mobile`].
async fn open_board_mobile(world: &mut FoundryWorld) {
    let browser = mobile_session_signed_in(world).await;
    let url = board_url(world);
    browser.goto(&url).await.expect("navigate to the board");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(".app-shell"))
        .await
        .expect("the board must render the authed .app-shell");
    let landed = browser
        .current_url()
        .await
        .expect("read the browser's current URL")
        .to_string();
    assert_eq!(
        landed, url,
        "opening the board landed on {landed} instead of {url} — the surface redirected away."
    );
    world.browser = Some(browser);
}

#[given(regex = r#"^Mei is on the "Sandbox" board in a mobile browser at 390x844$"#)]
async fn on_board_mobile(world: &mut FoundryWorld) {
    open_board_mobile(world).await;
}

/// S4 precondition — the default Sandbox board already renders the four fixed
/// status columns (`Backlog/Todo/In-Progress/Done`, projects.rs `DEFAULT_COLUMNS`),
/// so this Given is narrative: nothing to arrange, the assertion reads the live
/// board. The follow-up `Mei opens the "Sandbox" board …` Given (S1's regex) opens
/// the session.
#[given(regex = r#"^the "Sandbox" board has several status columns$"#)]
async fn board_has_several_columns(_world: &mut FoundryWorld) {}

/// S7 — open the Sandbox board in the SHIPPED DESKTOP session (blast-radius guard).
#[given(regex = r#"^Mei opens the "Sandbox" board in a desktop browser$"#)]
async fn opens_board_desktop(world: &mut FoundryWorld) {
    let browser = desktop_session_signed_in(world).await;
    let url = board_url(world);
    browser.goto(&url).await.expect("navigate to the board");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(".app-shell"))
        .await
        .expect("the desktop board must render the authed .app-shell");
    world.browser = Some(browser);
}

/// S3 — open the new-issue dialog on the current mobile board session.
#[when(regex = r"^she opens the new-issue dialog$")]
async fn opens_new_issue_dialog(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(NEW_ISSUE_TRIGGER))
        .await
        .expect("the board must render the New issue trigger")
        .click()
        .await
        .expect("click the New issue trigger");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(DIALOG_SELECTOR))
        .await
        .expect("clicking New issue must open the dialog");
}

/// S3 — the mounted dialog card must be no wider than the phone's layout viewport.
#[then(regex = r"^the dialog is no wider than the viewport$")]
async fn dialog_not_wider_than_viewport(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dims = browser
        .execute(
            "var d = document.querySelector(arguments[0]);\
             if (!d) { throw new Error('no .modal-dialog mounted'); }\
             return [d.getBoundingClientRect().width, document.documentElement.clientWidth];",
            vec![serde_json::Value::String(MODAL_DIALOG_SELECTOR.to_string())],
        )
        .await
        .expect("measure the dialog vs the layout viewport width");
    let arr = dims.as_array().expect("[dialogWidth, clientWidth]");
    let dialog_w = arr[0].as_f64().expect("dialog width is a number");
    let client_w = arr[1].as_f64().expect("clientWidth is a number");
    assert!(
        dialog_w <= client_w + 0.5,
        "the new-issue dialog is {dialog_w}px wide but the phone layout viewport is only \
         {client_w}px — below the breakpoint the dialog must become a full-width sheet \
         (width:100%) so it fits the screen."
    );
}

/// S3 — with content taller than the viewport the dialog caps at screen height and
/// scrolls its own body, instead of growing past the phone. Injects a tall probe
/// node so the overflow is real, not assumed.
#[then(regex = r"^the dialog body scrolls when its content is taller than the viewport$")]
async fn dialog_body_scrolls(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dims = browser
        .execute(
            "var d = document.querySelector(arguments[0]);\
             if (!d) { throw new Error('no .modal-dialog mounted'); }\
             var tall = document.createElement('div');\
             tall.style.height = '4000px';\
             tall.setAttribute('data-probe', 'tall');\
             d.appendChild(tall);\
             var vh = document.documentElement.clientHeight;\
             var r = d.getBoundingClientRect();\
             return [r.height, d.scrollHeight, d.clientHeight, vh];",
            vec![serde_json::Value::String(MODAL_DIALOG_SELECTOR.to_string())],
        )
        .await
        .expect("inject tall content and measure the dialog");
    let arr = dims
        .as_array()
        .expect("[height, scrollHeight, clientHeight, vh]");
    let height = arr[0].as_f64().unwrap();
    let scroll_h = arr[1].as_f64().unwrap();
    let client_h = arr[2].as_f64().unwrap();
    let vh = arr[3].as_f64().unwrap();
    assert!(
        height <= vh + 1.0,
        "with content taller than the {vh}px viewport the dialog grew to {height}px instead of \
         capping at the screen height — the mobile sheet must set max-height:100vh."
    );
    assert!(
        scroll_h > client_h,
        "the dialog does not scroll its overflowing body (scrollHeight {scroll_h} <= clientHeight \
         {client_h}) — the mobile sheet must set overflow-y:auto so a tall form scrolls inside the \
         capped dialog."
    );
}

/// S4 — the columns overflow their container, and the container (not the page)
/// carries the horizontal scroll.
#[then(regex = r"^the board columns container is horizontally scrollable$")]
async fn board_columns_scrollable(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dims = browser
        .execute(
            "var b = document.querySelector('.board');\
             if (!b) { throw new Error('no .board on the page'); }\
             return [b.scrollWidth, b.clientWidth, getComputedStyle(b).overflowX];",
            vec![],
        )
        .await
        .expect("measure the board's scroll vs client width");
    let arr = dims.as_array().unwrap();
    let scroll_w = arr[0].as_f64().unwrap();
    let client_w = arr[1].as_f64().unwrap();
    let overflow_x = arr[2].as_str().unwrap_or("");
    assert!(
        scroll_w > client_w,
        "the board columns do not overflow their container (scrollWidth {scroll_w} <= clientWidth \
         {client_w}) — the columns must keep a min-width so the strip scrolls within .board instead \
         of collapsing to fit."
    );
    assert!(
        overflow_x == "auto" || overflow_x == "scroll",
        "the .board container is not horizontally scrollable (overflow-x: {overflow_x}) — it must \
         be auto/scroll so the columns scroll within it, not the page."
    );
}

/// S5 — the fixed 240px desktop rail is gone; the sidebar spans the phone as a bar.
#[then(regex = r"^the full desktop sidebar rail is not shown at full width$")]
async fn desktop_rail_not_shown(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dims = browser
        .execute(
            "var s = document.querySelector('.sidebar');\
             if (!s) { throw new Error('no .sidebar on the page'); }\
             return [s.getBoundingClientRect().width, document.documentElement.clientWidth];",
            vec![],
        )
        .await
        .expect("measure the sidebar width");
    let arr = dims.as_array().unwrap();
    let width = arr[0].as_f64().unwrap();
    let client_w = arr[1].as_f64().unwrap();
    assert!(
        (width - 240.0).abs() > 2.0 && width >= client_w * 0.9,
        "the sidebar still renders as the fixed 240px desktop rail (width {width}px, viewport \
         {client_w}px) — below the breakpoint it must reflow to a full-width top bar, not the \
         vertical rail."
    );
}

/// S5 — a mobile nav affordance is present: the primary links lay out on ONE
/// horizontal row (a top bar), not the desktop rail's stacked list.
#[then(regex = r"^a mobile navigation affordance is present$")]
async fn mobile_nav_affordance_present(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dims = browser
        .execute(
            "var links = document.querySelectorAll('.sidebar .sidebar__nav .sidebar__item');\
             if (links.length < 2) { throw new Error('the sidebar nav has fewer than two links'); }\
             var a = links[0].getBoundingClientRect();\
             var b = links[1].getBoundingClientRect();\
             return [a.top, b.top, a.height];",
            vec![],
        )
        .await
        .expect("measure the sidebar nav links");
    let arr = dims.as_array().unwrap();
    let a_top = arr[0].as_f64().unwrap();
    let b_top = arr[1].as_f64().unwrap();
    let a_height = arr[2].as_f64().unwrap();
    assert!(
        (a_top - b_top).abs() < a_height.max(1.0),
        "the primary nav links are stacked vertically (tops {a_top} vs {b_top}, item height \
         {a_height}) — the mobile affordance must lay them out as a horizontal top bar, not the \
         desktop rail's stacked list."
    );
}

/// S6 — the New issue control's smaller dimension is at least a ~44px thumb target.
#[then(regex = r#"^the "New issue" control is at least 44px in its smaller dimension$"#)]
async fn new_issue_control_tappable(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dims = browser
        .execute(
            "var el = document.querySelector(arguments[0]);\
             if (!el) { throw new Error('no New issue control on the board'); }\
             var r = el.getBoundingClientRect();\
             return [r.width, r.height];",
            vec![serde_json::Value::String(NEW_ISSUE_TRIGGER.to_string())],
        )
        .await
        .expect("measure the New issue control");
    let arr = dims.as_array().unwrap();
    let w = arr[0].as_f64().unwrap();
    let h = arr[1].as_f64().unwrap();
    let smaller = w.min(h);
    assert!(
        smaller >= 44.0,
        "the New issue control's smaller dimension is {smaller}px (w {w} × h {h}), below the ~44px \
         thumb target — the mobile @media must grow primary controls to at least 44px."
    );
}

/// S7 — the shipped 240px desktop rail is still shown (mobile rules stayed bounded).
#[then(regex = r"^the desktop sidebar rail is shown$")]
async fn desktop_rail_shown(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("desktop browser session");
    let dims = browser
        .execute(
            "var s = document.querySelector('.sidebar');\
             if (!s) { throw new Error('no .sidebar on the desktop page'); }\
             return [s.getBoundingClientRect().width];",
            vec![],
        )
        .await
        .expect("measure the desktop sidebar width");
    let width = dims.as_array().unwrap()[0].as_f64().unwrap();
    assert!(
        (width - 240.0).abs() < 2.0,
        "the desktop sidebar is {width}px wide, not the shipped 240px rail — a mobile @media rule \
         leaked past its max-width bound and collapsed the desktop rail."
    );
}

/// S7 — the desktop board still fits without horizontal scroll, and the desktop
/// document does not overflow: the mobile min-width/overflow rules stayed bounded.
#[then(regex = r"^the board layout matches the shipped desktop behaviour$")]
async fn desktop_board_layout_unchanged(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("desktop browser session");
    let dims = browser
        .execute(
            "var b = document.querySelector('.board');\
             if (!b) { throw new Error('no .board on the desktop page'); }\
             return [b.scrollWidth, b.clientWidth, document.documentElement.scrollWidth, \
              document.documentElement.clientWidth];",
            vec![],
        )
        .await
        .expect("measure the desktop board layout");
    let arr = dims.as_array().unwrap();
    let board_scroll = arr[0].as_f64().unwrap();
    let board_client = arr[1].as_f64().unwrap();
    let doc_scroll = arr[2].as_f64().unwrap();
    let doc_client = arr[3].as_f64().unwrap();
    assert!(
        board_scroll <= board_client + 1.0,
        "the desktop board scrolls horizontally (scrollWidth {board_scroll} > clientWidth \
         {board_client}) — the columns must flex to fit the desktop width; a mobile \
         min-width/overflow rule leaked past the breakpoint."
    );
    assert!(
        doc_scroll <= doc_client + 1.0,
        "the desktop document overflows horizontally (scrollWidth {doc_scroll} > clientWidth \
         {doc_client}) — desktop layout changed."
    );
}

// ------------------------------------------------------------ S8–S10 (slice 03)
//
// Slice 03 makes Foundry INSTALLABLE (ADR-002 — a STATIC manifest + icons + head
// meta, NO service worker). The fitted, responsive phone shell from slices 01–02
// now advertises itself to the browser's install machinery: base.html links a web
// app manifest, the manifest is served as valid JSON declaring the install
// identity (name/short_name/start_url/scope/display/theme+background colours) and
// a full icon set (192, 512, maskable), every declared icon is a real image, and
// the apple-specific meta lets iOS treat it as a standalone home-screen app.
//
// These assertions are ASSET FACTS, driven port-to-port: S8/S10 read the LINKED
// manifest + head tags from the live mobile DOM (the page as the browser sees it),
// and every manifest/icon fetch goes through the SAME in-process origin over real
// HTTP (`reqwest`) — proving the ServeDir static route actually serves the bytes,
// not that a file merely exists on disk. `/static` is public (no auth), so the
// icon-serving scenario (S9) needs no browser session, only the shared harness.

/// The manifest's stable, content-typed URL on the shared origin. ServeDir +
/// mime_guess 2.0.5 serve `.webmanifest` as `application/manifest+json` (a JSON
/// media type the fetch+parse below accepts), so no `manifest.json` fallback was
/// needed.
const MANIFEST_PATH: &str = "/static/manifest.webmanifest";

/// GET an absolute URL on the shared origin over real HTTP. Returns
/// (status, content-type, body-bytes). `/static` is public, so no auth is set.
async fn http_get(url: &str) -> (reqwest::StatusCode, String, Vec<u8>) {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .unwrap_or_else(|e| panic!("GET {url} failed to send: {e}"));
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp
        .bytes()
        .await
        .unwrap_or_else(|e| panic!("GET {url} failed to read body: {e}"))
        .to_vec();
    (status, content_type, bytes)
}

/// Absolute URL for a root-relative asset path on the shared origin.
fn origin_url(world: &FoundryWorld, path: &str) -> String {
    let base = world.harness.as_ref().expect("harness").base_url();
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!("{base}{path}")
    }
}

/// Read the `<link rel="manifest">` href the page ACTUALLY declares (the browser's
/// discovery surface), or panic loudly if the page links no manifest at all.
async fn linked_manifest_href(world: &FoundryWorld) -> String {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let href = browser
        .execute(
            "var l = document.querySelector('link[rel=\"manifest\"]');\
             return l ? l.getAttribute('href') : null;",
            vec![],
        )
        .await
        .expect("read the manifest link href");
    href.as_str()
        .unwrap_or_else(|| {
            panic!(
                "the page declares NO <link rel=\"manifest\"> — the browser cannot discover a web \
                 app manifest, so Foundry offers no install prompt. base.html must carry \
                 <link rel=\"manifest\" href=\"/static/manifest.webmanifest\">."
            )
        })
        .to_string()
}

/// Fetch + parse the manifest the PAGE links (S8/S10 path). Asserts 200 and JSON.
async fn fetch_linked_manifest(world: &FoundryWorld) -> serde_json::Value {
    let href = linked_manifest_href(world).await;
    let url = origin_url(world, &href);
    let (status, content_type, bytes) = http_get(&url).await;
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "the linked manifest {url} did not serve 200 (got {status}) — the ServeDir static route \
         is not serving the manifest bytes."
    );
    assert!(
        content_type.contains("json"),
        "the manifest at {url} served content-type {content_type:?}, which is not a JSON media \
         type — browsers still honour the linked manifest, but the observed type should be JSON-ish."
    );
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("the manifest at {url} is not valid JSON: {e}"))
}

/// S8 — the page must LINK a web app manifest so the browser can discover it.
#[then(regex = r"^the page links a web app manifest$")]
async fn page_links_manifest(world: &mut FoundryWorld) {
    let href = linked_manifest_href(world).await;
    assert!(
        href.contains("manifest"),
        "the <link rel=\"manifest\"> points at {href:?}, which is not a manifest URL."
    );
}

/// S8 — fetching that linked manifest over real HTTP returns 200 with valid JSON.
#[then(regex = r"^fetching the manifest returns 200 with valid JSON$")]
async fn fetching_manifest_returns_json(world: &mut FoundryWorld) {
    // fetch_linked_manifest asserts 200 + JSON-parseable, which IS the outcome.
    let _ = fetch_linked_manifest(world).await;
}

/// S8 — the manifest declares the full install identity.
#[then(
    regex = r#"^the manifest declares name, short_name, start_url, scope, display "standalone", theme_color and background_color$"#
)]
async fn manifest_declares_identity(world: &mut FoundryWorld) {
    let m = fetch_linked_manifest(world).await;
    for key in [
        "name",
        "short_name",
        "start_url",
        "scope",
        "theme_color",
        "background_color",
    ] {
        let v = m.get(key).and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            !v.is_empty(),
            "the manifest is missing a non-empty {key:?} — the install identity is incomplete \
             (manifest = {m})."
        );
    }
    assert_eq!(
        m.get("display").and_then(|v| v.as_str()),
        Some("standalone"),
        "the manifest display mode is not \"standalone\" — Foundry would open in a browser tab, \
         not as an installed standalone app (manifest = {m})."
    );
}

/// S8 — the manifest lists a usable icon set: 192x192, 512x512, and a maskable icon.
#[then(regex = r"^the manifest lists icons including 192x192, 512x512 and a maskable icon$")]
async fn manifest_lists_icons(world: &mut FoundryWorld) {
    let m = fetch_linked_manifest(world).await;
    let icons = m
        .get("icons")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("the manifest has no icons array (manifest = {m})"));
    let has_size = |size: &str| {
        icons.iter().any(|i| {
            i.get("sizes")
                .and_then(|s| s.as_str())
                .is_some_and(|s| s.split_whitespace().any(|token| token == size))
        })
    };
    assert!(
        has_size("192x192"),
        "the manifest lists no 192x192 icon — the home-screen icon set is incomplete."
    );
    assert!(
        has_size("512x512"),
        "the manifest lists no 512x512 icon — the splash/hi-dpi icon is missing."
    );
    let has_maskable = icons.iter().any(|i| {
        i.get("purpose")
            .and_then(|p| p.as_str())
            .is_some_and(|p| p.split_whitespace().any(|token| token == "maskable"))
    });
    assert!(
        has_maskable,
        "the manifest lists no purpose=\"maskable\" icon — adaptive launchers would crop the \
         square icon instead of using a safe-zone mark."
    );
}

/// S9 precondition — narrative. The manifest + its icons are read live in the
/// Then below (fetched over real HTTP), so nothing to arrange here.
#[given(regex = r"^the manifest lists its icons$")]
async fn manifest_lists_its_icons(_world: &mut FoundryWorld) {}

/// S9 action — narrative. The fetch happens in the Then so it can assert per-icon.
#[when(regex = r"^each icon URL is fetched$")]
async fn each_icon_url_fetched(_world: &mut FoundryWorld) {}

/// S9 — every icon the manifest declares is actually served as an image. Fetches
/// the manifest directly (no browser — `/static` is public) and GETs each icon
/// src through the same origin, asserting 200 + an image content-type.
#[then(regex = r"^it returns 200 with an image content-type$")]
async fn icons_served_as_images(world: &mut FoundryWorld) {
    let manifest_url = origin_url(world, MANIFEST_PATH);
    let (status, _ct, bytes) = http_get(&manifest_url).await;
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "the manifest {manifest_url} did not serve 200 (got {status})."
    );
    let m: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("the manifest at {manifest_url} is not valid JSON: {e}"));
    let icons = m
        .get("icons")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("the manifest has no icons array (manifest = {m})"));
    assert!(!icons.is_empty(), "the manifest lists zero icons to fetch");
    for icon in icons {
        let src = icon
            .get("src")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| panic!("a manifest icon entry has no src (icon = {icon})"));
        let icon_url = origin_url(world, src);
        let (istatus, ict, _) = http_get(&icon_url).await;
        assert_eq!(
            istatus,
            reqwest::StatusCode::OK,
            "the declared icon {icon_url} did not serve 200 (got {istatus}) — the manifest \
             references an icon file that is not served."
        );
        assert!(
            ict.starts_with("image/"),
            "the declared icon {icon_url} served content-type {ict:?}, not an image type — it is \
             not a real served image."
        );
    }
}

/// S10 — the page carries a theme-color meta (colours the OS/browser chrome).
#[then(regex = r"^the page contains a theme-color meta$")]
async fn page_has_theme_color_meta(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let content = browser
        .execute(
            "var m = document.querySelector('meta[name=\"theme-color\"]');\
             return m ? m.getAttribute('content') : null;",
            vec![],
        )
        .await
        .expect("read the theme-color meta");
    let content = content.as_str().unwrap_or_default();
    assert!(
        !content.is_empty(),
        "the page declares no <meta name=\"theme-color\"> with a colour — the installed app's \
         status bar / browser chrome would not adopt the brand colour."
    );
}

/// S10 — the page carries the apple standalone meta + an apple-touch-icon so iOS
/// treats Foundry as a home-screen app with a real icon.
#[then(regex = r"^the page contains apple-mobile-web-app-capable and an apple-touch-icon$")]
async fn page_has_apple_meta(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("mobile browser session");
    let dom = browser
        .execute(
            "var cap = document.querySelector('meta[name=\"apple-mobile-web-app-capable\"]');\
             var ico = document.querySelector('link[rel=\"apple-touch-icon\"]');\
             return [cap ? cap.getAttribute('content') : null, ico ? ico.getAttribute('href') : null];",
            vec![],
        )
        .await
        .expect("read the apple meta + touch icon");
    let arr = dom.as_array().expect("[capable, touch-icon-href]");
    let capable = arr[0].as_str().unwrap_or_default();
    let touch_icon = arr[1].as_str().unwrap_or_default();
    assert_eq!(
        capable, "yes",
        "the page's <meta name=\"apple-mobile-web-app-capable\"> is {capable:?}, not \"yes\" — \
         iOS would open Foundry in Safari chrome, not as a standalone home-screen app."
    );
    assert!(
        !touch_icon.is_empty(),
        "the page declares no <link rel=\"apple-touch-icon\"> — iOS would fall back to a screenshot \
         thumbnail instead of the Foundry mark."
    );
}

/// S10 — the manifest's display mode is standalone (fetched directly, no browser).
#[then(regex = r#"^the manifest display mode is "standalone"$"#)]
async fn manifest_display_standalone(world: &mut FoundryWorld) {
    let manifest_url = origin_url(world, MANIFEST_PATH);
    let (status, _ct, bytes) = http_get(&manifest_url).await;
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "the manifest {manifest_url} did not serve 200 (got {status})."
    );
    let m: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("the manifest at {manifest_url} is not valid JSON: {e}"));
    assert_eq!(
        m.get("display").and_then(|v| v.as_str()),
        Some("standalone"),
        "the manifest display mode is not \"standalone\" (manifest = {m})."
    );
}
