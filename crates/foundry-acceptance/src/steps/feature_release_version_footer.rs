//! release-version-footer — step definitions for US-RVF-01 (`@rvf`).
//!
//! Every full page renders `Foundry v<version>` once, in a `<footer>` that
//! follows the main content; htmx fragments render no footer.
//!
//! THE ORACLE'S VERSION is foundry-app's `[package] version`, read from its
//! Cargo.toml at test compile time. It is deliberately NOT
//! `foundry_app::…` (the production constant would make the check circular) and
//! NOT a literal (a release bump must not need a test edit).
//!
//! REUSED phrases (globally unique, declared in feature_board_new_issue.rs):
//! the Background, `Mei fetches the "…" board` and
//! `Mei fetches the new-issue modal for "…"` — all write `world.last_body`, the
//! slot the Then steps below read.

use crate::world::FoundryWorld;
use cucumber::{then, when};
use scraper::{ElementRef, Html, Selector};

const FOUNDRY_APP_MANIFEST: &str = include_str!("../../../foundry-app/Cargo.toml");
const FOOTER_PREFIX: &str = "Foundry v";

/// foundry-app's crate version — the `version = "…"` line of its `[package]`
/// table (the first `version` key in the manifest).
fn running_release_version() -> &'static str {
    FOUNDRY_APP_MANIFEST
        .lines()
        .find_map(|line| {
            line.strip_prefix("version = \"")
                .and_then(|rest| rest.strip_suffix('"'))
        })
        .expect("foundry-app/Cargo.toml declares a [package] version")
}

fn last_response(world: &FoundryWorld) -> String {
    let status = world
        .last_status
        .expect("a page was fetched by a When step");
    assert!(
        status.is_success(),
        "expected a 2xx page, got {status}; body:\n{}",
        world.last_body.as_deref().unwrap_or("")
    );
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        !body.trim().is_empty(),
        "the fetched response has an empty body"
    );
    body
}

// --- When -------------------------------------------------------------------

/// GET /sign-in carrying no session cookie — the page an operator without an
/// account can open to check the running release.
#[when(regex = r"^a visitor with no session opens the sign-in page$")]
async fn visitor_opens_signin(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{}/sign-in", harness.base_url()))
        .send()
        .await
        .expect("get /sign-in");
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

// --- Then -------------------------------------------------------------------

#[then(regex = r"^the page shows the running release version once, below the main content$")]
async fn page_shows_release_once_below_content(world: &mut FoundryWorld) {
    let body = last_response(world);
    let expected = format!("{FOOTER_PREFIX}{}", running_release_version());

    let occurrences = body.matches(FOOTER_PREFIX).count();
    assert_eq!(
        occurrences, 1,
        "expected {FOOTER_PREFIX:?} exactly once on the page, found {occurrences}; body:\n{body}"
    );

    let doc = Html::parse_document(&body);
    let body_sel = Selector::parse("body").expect("body selector");
    let page_body = doc.select(&body_sel).next().expect("a <body> element");
    let children: Vec<ElementRef> = page_body.children().filter_map(ElementRef::wrap).collect();
    let footer_idx = children
        .iter()
        .position(|el| el.text().collect::<String>().contains(FOOTER_PREFIX))
        .unwrap_or_else(|| {
            panic!("no top-level <body> child carries {FOOTER_PREFIX:?}; body:\n{body}")
        });
    let footer = children[footer_idx];

    assert_eq!(
        footer.value().name(),
        "footer",
        "the release version must sit in a <footer>, not <{}>",
        footer.value().name()
    );
    let footer_text = footer.text().collect::<String>();
    assert_eq!(
        footer_text.trim(),
        expected,
        "the footer must read exactly the running release"
    );
    assert!(
        footer_idx > 0,
        "the footer must follow the page's main content, but it is the first <body> child"
    );
    let content_after: Vec<String> = children[footer_idx + 1..]
        .iter()
        .filter(|el| el.value().id() != Some("kb-overlay-root") && el.value().name() != "script")
        .map(|el| el.html())
        .collect();
    assert!(
        content_after.is_empty(),
        "the footer must be below the main content, but content follows it: {content_after:?}"
    );
}

#[then(regex = r"^the response carries no release version footer$")]
async fn response_has_no_footer(world: &mut FoundryWorld) {
    let body = last_response(world);
    assert!(
        !body.contains(FOOTER_PREFIX),
        "an htmx fragment must not render the page footer, but {FOOTER_PREFIX:?} is present:\n{body}"
    );
    let doc = Html::parse_fragment(&body);
    let footer_sel = Selector::parse("footer").expect("footer selector");
    assert!(
        doc.select(&footer_sel).next().is_none(),
        "an htmx fragment must not carry a <footer>:\n{body}"
    );
}
