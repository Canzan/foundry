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
use cucumber::{given, then};
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
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, author_id)
              VALUES ($1, $2, $3, 1, 'Fits a phone', '', $4)",
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
