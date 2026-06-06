//! `keyboard-fragments-templating` step definitions — the move-only tail of
//! the web-tier templating arc. The two remaining inline-`format!()` BARE htmx
//! fragments in `keyboard.rs` (`render_search_fragment`, `show_keyboard_help`)
//! move to Askama partials, selector-and-substring-identical.
//!
//! LEAN-DISTILL contract (ADR-025 / Mandate 7):
//!
//! These are BARE fragments with byte-identical markup — no base.html, no new
//! `/static` link, no NEW observable user delta. So the move is proven by the
//! EXISTING us-12-keyboard-nav suite staying green (the regression net), NOT by
//! new "renders styled" scenarios. This module therefore adds only:
//!
//!   - TWO regression-net-tightening Then steps that pin the render-contract
//!     markers us-12 leaves thin. Search: the `ul.search-results` wrapper, the
//!     `.key` span, and the empty `ul.search-results[data-empty="true"]` state
//!     (us-12 only pins `li.search-result[data-issue-key]` + `.title`). Help:
//!     the `section.keyboard-help[role="dialog"][aria-label]` container and the
//!     `header>h2` heading (us-12 only pins the `dt[data-shortcut]`+`dd` pairs).
//!     These PASS today and STAY green after the move — they tighten the net,
//!     they are NOT RED.
//!
//!   - The COMPLETION GUARD (the one genuine new RED): a source-tree contract
//!     that scans `keyboard.rs` for the two inline-`format!()` BARE-FRAGMENT
//!     HTML literals and asserts ZERO remain. It mirrors remaining-surfaces
//!     US-R07 (`inline_full_page_sites`) and Feature B's `vendored_htmx_files`
//!     on-disk count check. Sibling of `inline_full_page_sites`: that guard
//!     matches `<!doctype` (full pages only — fragments have no `<head>`), so
//!     it CANNOT see these bare fragments. `inline_html_fragment_sites` below
//!     matches the bare-fragment tells (`<ul class="search-results"` and
//!     `<section class="keyboard-help"`) instead. It is RED NOW (2 sites) and
//!     flips GREEN only when DELIVER has moved both fragments into partials.
//!
//! REUSED step phrases (cucumber-rs requires globally-unique step text — these
//! are declared elsewhere and MUST NOT be re-declared here):
//!   - `a workspace "..." exists with admin "..."`                (us_06_signin)
//!   - `a member "..." belongs to the team "..."`                 (us_07_project_create)
//!   - `a project "..." with key prefix "..." exists in the "..." team` (us_08_file_issue)
//!   - `the "..." project already has issues AUTH-1 through AUTH-3` (us_12 background)
//!   - `(\w+) is signed in`                                        (us_12 background)
//!   - `the "..." project already has an issue titled "..."`       (us_12_keyboard_nav)
//!   - `(\w+) searches "..." for the query "..."`                  (us_12_keyboard_nav)
//!   - `(\w+) requests the keyboard-help overlay`                  (us_12_keyboard_nav)
//!     The reused When steps cache the response in `world.us_12_last_get_body`;
//!     this module's Then steps read that SAME slot.
//!
//! What DELIVER must wire to flip the guard GREEN is enumerated in
//! `docs/feature/keyboard-fragments-templating/distill/step-skeletons.md`.

use crate::support::html_assertions;
use crate::world::FoundryWorld;
use cucumber::{then, when};
use scraper::{Html, Selector};

/// Read the body the reused us-12 When step cached. All Then steps in this
/// module operate on a fragment fetched by a us-12-owned When.
fn last_fragment(world: &FoundryWorld) -> String {
    world
        .us_12_last_get_body
        .clone()
        .expect("a keyboard fragment was fetched by a (reused us-12) When step")
}

// ==========================================================================
// Then — US-K01 search-results wrapper + key + title (regression-net tighten)
// ==========================================================================

#[then(regex = r"^the search fragment is a search-results list$")]
async fn search_is_results_list(world: &mut FoundryWorld) {
    let body = last_fragment(world);
    // The byte-stable wrapper the DESIGN render contract pins; us-12 only
    // asserts the inner `.search-result` items, never the `ul` wrapper.
    html_assertions::assert_has(&body, "ul.search-results");
    // A populated list is NOT the empty-state list.
    html_assertions::assert_not_has(&body, r#"ul.search-results[data-empty="true"]"#);
}

#[then(regex = r#"^the matched result carries the issue key "(\w+)-(\d+)" and a title$"#)]
async fn matched_result_has_key_and_title(world: &mut FoundryWorld, prefix: String, number: i32) {
    let body = last_fragment(world);
    let doc = Html::parse_fragment(&body);
    let li_sel = Selector::parse("li.search-result").expect("li.search-result");
    let li = doc
        .select(&li_sel)
        .next()
        .unwrap_or_else(|| panic!("no li.search-result in body:\n{body}"));

    let issue_key = format!("{prefix}-{number}");
    assert_eq!(
        li.value().attr("data-issue-key"),
        Some(issue_key.as_str()),
        "expected li.search-result[data-issue-key=\"{issue_key}\"]; body:\n{body}"
    );

    // The `.key` span and the `.title` span are byte-stable markers the
    // render contract pins; us-12 asserts `.title` but never `.key`.
    let key_sel = Selector::parse("span.key").expect("span.key");
    let key_text: String = li
        .select(&key_sel)
        .next()
        .unwrap_or_else(|| panic!("search result missing span.key in:\n{body}"))
        .text()
        .collect();
    assert_eq!(
        key_text.trim(),
        issue_key,
        "expected span.key to read {issue_key:?}; got {key_text:?}"
    );

    let title_sel = Selector::parse("span.title").expect("span.title");
    let title_text: String = li
        .select(&title_sel)
        .next()
        .unwrap_or_else(|| panic!("search result missing span.title in:\n{body}"))
        .text()
        .collect();
    assert!(
        !title_text.trim().is_empty(),
        "expected a non-empty span.title; got {title_text:?}"
    );
}

#[then(regex = r"^the search fragment is an empty search-results list$")]
async fn search_is_empty_results_list(world: &mut FoundryWorld) {
    let body = last_fragment(world);
    // The empty-state marker the render contract pins; us-12 never exercises
    // the no-match path, so this marker is otherwise unguarded.
    html_assertions::assert_has(&body, r#"ul.search-results[data-empty="true"]"#);
    html_assertions::assert_not_has(&body, "li.search-result");
}

// ==========================================================================
// Then — US-K02 help dialog container + heading (regression-net tighten)
// ==========================================================================

#[then(regex = r#"^the help overlay is a dialog labelled "([^"]+)"$"#)]
async fn help_is_labelled_dialog(world: &mut FoundryWorld, label: String) {
    let body = last_fragment(world);
    // The byte-stable dialog container the DESIGN render contract pins; us-12
    // asserts the dt/dd pairs but never the section[role][aria-label].
    let css = format!(r#"section.keyboard-help[role="dialog"][aria-label="{label}"]"#);
    html_assertions::assert_has(&body, &css);
}

#[then(regex = r#"^the help overlay shows the heading "([^"]+)"$"#)]
async fn help_shows_heading(world: &mut FoundryWorld, heading: String) {
    let body = last_fragment(world);
    let doc = Html::parse_fragment(&body);
    let sel = Selector::parse("section.keyboard-help header h2").expect("header h2 selector");
    let h2 = doc
        .select(&sel)
        .next()
        .unwrap_or_else(|| panic!("no header>h2 in keyboard-help body:\n{body}"));
    let text: String = h2.text().collect();
    assert_eq!(
        text.trim(),
        heading,
        "expected header>h2 to read {heading:?}; got {text:?}"
    );
}

// ==========================================================================
// When/Then — US-K01/K02 completion guard (source-tree contract; RED now)
// ==========================================================================

#[when(regex = r"^the keyboard handler source is scanned for inline fragment HTML$")]
async fn scan_keyboard_source(world: &mut FoundryWorld) {
    // No HTTP — a source-tree contract (mirrors feature_b's on-disk
    // `vendored_htmx_files` count and remaining-surfaces' `inline_full_page_sites`).
    // The result is stashed as a pseudo-body so the Then step reads it through
    // the same world slot the rest of this module uses.
    let sites = inline_html_fragment_sites();
    world.us_12_last_get_body = Some(sites.join("\n"));
}

#[then(regex = r"^no keyboard handler emits an inline fragment HTML literal$")]
async fn no_inline_fragment(world: &mut FoundryWorld) {
    let captured = world.us_12_last_get_body.clone().unwrap_or_default();
    let sites: Vec<&str> = captured.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        sites.is_empty(),
        "{n} keyboard.rs site(s) still emit an inline-format!() BARE-FRAGMENT HTML literal \
         — the move is not complete (goal: 0). Offending sites:\n{list}",
        n = sites.len(),
        list = sites.join("\n")
    );
}

// ==========================================================================
// Internals — source-tree scan (sibling of `inline_full_page_sites`)
// ==========================================================================

/// Scan `keyboard.rs` for the tell of an inline-`format!()` BARE-FRAGMENT HTML
/// literal. Returns one `"file:line"` per offending site. Empty ⇒ the move is
/// complete (both fragments now render from Askama partials).
///
/// Sibling of `feature_remaining_surfaces::inline_full_page_sites`, which
/// matches `<!doctype` (FULL pages only — bare fragments have no `<head>`, so
/// that guard is blind to them). This guard matches the two bare-fragment
/// opening tells the DESIGN render contract names as byte-stable:
///   - `<ul class="search-results"`  (render_search_fragment)
///   - `<section class="keyboard-help"`  (show_keyboard_help)
///
/// RED contract: today BOTH literals exist in `keyboard.rs`, so this returns 2
/// sites and the completion scenario fails RED for MISSING_FUNCTIONALITY. It
/// flips GREEN only when DELIVER has moved both fragments into
/// `partials/search_results.html` + `partials/keyboard_help.html` (the Rust
/// then renders the template, so the literal no longer appears in the source).
fn inline_html_fragment_sites() -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../foundry-app/src/keyboard.rs");
    let text = std::fs::read_to_string(&path).expect("read foundry-app/src/keyboard.rs");

    // The byte-stable bare-fragment opening tells. We match the literal class
    // selector the render contract pins; an Askama-rendered template emits the
    // same markup from a `.html` file, so the SOURCE no longer carries the tell.
    const TELLS: &[&str] = &[
        r#"<ul class="search-results"#,
        r#"<section class="keyboard-help"#,
    ];

    let mut hits = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if TELLS.iter().any(|tell| line.contains(tell)) {
            hits.push(format!("keyboard.rs:{}", idx + 1));
        }
    }
    hits
}
