//! release-version-footer — step definitions for US-RVF-01 and US-RVF-02 (`@rvf`).
//!
//! Every full page renders `Foundry v<version> · <commit date>` once, in a
//! `<footer>` that follows the main content and carries the build's short commit
//! id in `data-commit`; htmx fragments render no footer.
//!
//! THE ORACLE (DDD-15) is independent of the production constants — reading
//! `foundry_app::views::*` would make the check circular:
//!   * the version is foundry-app's `[package] version`, read from its
//!     Cargo.toml at test compile time (NOT a literal: a release bump must not
//!     need a test edit);
//!   * the short id and the commit date are asked of git at test time with the
//!     exact DDD-3 argument vectors the build script uses, from this crate's
//!     manifest dir (same repository as foundry-app); `unknown` when git cannot
//!     answer — the same honest placeholder the build falls back to;
//!   * precondition guard: if `FOUNDRY_STAMP_SHA` / `FOUNDRY_STAMP_DATE` is set
//!     and non-blank in the test process, the step fails naming the variable.
//!     An explicit input outranks git in build.rs (D9), so the oracle would be
//!     reading the wrong source. Explicit-input precedence is AC-5's unit
//!     examples, never re-implemented here.
//!
//! Accepted race (OQ-D5): a commit landing between compiling the test binary
//! and running it reds this lane with a value diff naming both values; rerun.
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
/// D5: U+00B7 MIDDLE DOT with one ASCII space on each side.
const STAMP_SEPARATOR: &str = " \u{00B7} ";
/// D8: the footer attribute carrying the short commit id.
const COMMIT_ATTRIBUTE: &str = "data-commit";
/// D9/D10: what a stamp field says when nothing could answer.
const UNANSWERED: &str = "unknown";
/// D9: the explicit build inputs. Set in this process, they would outrank git
/// in build.rs and make the git-based oracle compare against the wrong source.
const STAMP_INPUTS: [&str; 2] = ["FOUNDRY_STAMP_SHA", "FOUNDRY_STAMP_DATE"];

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

/// The build stamp the server should be showing: what git says about HEAD in
/// this checkout, asked exactly as the build asks it (DDD-3).
struct ExpectedBuildStamp {
    sha: String,
    date: String,
}

fn expected_build_stamp() -> ExpectedBuildStamp {
    for input in STAMP_INPUTS {
        let value = std::env::var(input).unwrap_or_default();
        assert!(
            value.trim().is_empty(),
            "{input}={value:?} is set in the test process. It outranks git when foundry-app \
             compiles (D9), so this git-based oracle would check the wrong source. Unset it and \
             rebuild; explicit-input precedence is covered by foundry-app's unit examples (AC-5)."
        );
    }
    ExpectedBuildStamp {
        sha: git_answer(&["rev-parse", "--short=7", "HEAD"]),
        date: git_answer(&["log", "-1", "--format=%cd", "--date=short"]),
    }
}

/// git's trimmed answer from this crate's checkout, or `unknown` for every
/// failure (no git, not a repository, non-zero exit, empty or non-UTF-8 output)
/// — the same degradation build.rs applies (DDD-4).
fn git_answer(args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|answer| answer.trim().to_string())
        .filter(|answer| !answer.is_empty())
        .unwrap_or_else(|| UNANSWERED.to_string())
}

/// The page's single release footer: `Foundry v` occurs exactly once, inside
/// a top-level `<footer>` child of `<body>` that follows the main content, with
/// nothing after it but the keyboard overlay root and scripts (D3/D14).
fn footer_below_content<'doc>(doc: &'doc Html, body: &str) -> ElementRef<'doc> {
    let occurrences = body.matches(FOOTER_PREFIX).count();
    assert_eq!(
        occurrences, 1,
        "expected {FOOTER_PREFIX:?} exactly once on the page, found {occurrences}; body:\n{body}"
    );

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
        "the release must sit in a <footer>, not <{}>",
        footer.value().name()
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
    footer
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

#[then(
    regex = r"^the page names the running release and the date of the commit it was built from, once, below the main content$"
)]
async fn page_names_release_and_commit_date_once_below_content(world: &mut FoundryWorld) {
    let body = last_response(world);
    let stamp = expected_build_stamp();
    let expected = format!(
        "{FOOTER_PREFIX}{}{STAMP_SEPARATOR}{}",
        running_release_version(),
        stamp.date
    );

    let doc = Html::parse_document(&body);
    let footer = footer_below_content(&doc, &body);
    let footer_text = footer.text().collect::<String>();
    assert_eq!(
        footer_text.trim(),
        expected,
        "the footer must read exactly the running release and the date of the commit it was \
         built from (D5/D7; the date is the COMMIT date from `git log -1 --format=%cd \
         --date=short`, never the build time)"
    );
}

#[then(regex = r"^that footer carries the short id of the commit the server was built from$")]
async fn footer_carries_commit_short_id(world: &mut FoundryWorld) {
    let body = last_response(world);
    let stamp = expected_build_stamp();

    let doc = Html::parse_document(&body);
    let footer = footer_below_content(&doc, &body);
    let carried = footer.value().attr(COMMIT_ATTRIBUTE);
    assert_eq!(
        carried,
        Some(stamp.sha.as_str()),
        "the footer must carry the short id of the commit the server was built from in \
         `{COMMIT_ATTRIBUTE}` (D8; expected from `git rev-parse --short=7 HEAD`), on the same \
         element as the release text; footer: {}",
        footer.html()
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
