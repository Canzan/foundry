//! release-version-footer US-RVF-02 — unit examples for the build stamp
//! (AC-5 precedence, AC-6 degraded and escaped rendering, DDD-5 placeholder
//! parity).
//!
//! What is tested is exactly what cargo executes: cargo never runs tests
//! written inside a build script, so this module compiles THE SAME FILE
//! (`include!` of `build.rs`, DDD-12) and drives its pure `stamp` directly.
//! The rendering examples target `views::SiteFooter::of`, the pure seam
//! `base.html` renders through `views::site_footer()`.

use proptest::prelude::*;

use crate::views::{SiteFooter, BUILD_FIELD_UNKNOWN};

/// `build.rs`, compiled into the test target. Its `main` and the shell helpers
/// (`given`, `git`, `emit_rerun_directives`) are unused here, hence the allow.
#[allow(dead_code)]
mod build_script {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
}

use build_script::stamp;

/// The footer for explicit field values, rendered to the HTML base.html embeds.
fn footer_html(version: &str, sha: &str, date: &str) -> String {
    SiteFooter::of(version, sha, date).to_string()
}

// --- AC-5: precedence, per field -------------------------------------------

proptest! {
    /// AC-5 / D9. An explicit stamp input (the container build's only source of
    /// truth: `.dockerignore` keeps `.git` out of its context) wins over
    /// whatever git says, trimmed.
    #[test]
    fn an_explicit_stamp_input_beats_git(
        raw in "[ \t]{0,3}[!-~]{1,12}[ \t]{0,3}",
        from_git in proptest::option::of("[0-9a-f]{7}"),
    ) {
        let stamped = stamp(Some(&raw), || from_git.clone());
        prop_assert_eq!(stamped, raw.trim());
    }
}

/// AC-5 / D9. A Dockerfile `ARG X=` with no `--build-arg` arrives as an EMPTY
/// variable, not an absent one: blank or whitespace-only means "not given".
/// With neither an input nor git, the field is the honest `unknown`.
#[test]
fn a_blank_or_absent_stamp_input_falls_through_to_git_then_unknown() {
    for (explicit, from_git, expected) in [
        (Some("817c16d"), Some("ae5b675"), "817c16d"),
        (Some("  817c16d\n"), None, "817c16d"),
        (None, Some("ae5b675"), "ae5b675"),
        (Some(""), Some("ae5b675"), "ae5b675"),
        (Some(" \t\n"), Some("ae5b675"), "ae5b675"),
        (Some(""), None, "unknown"),
        (Some("   "), None, "unknown"),
        (None, None, "unknown"),
    ] {
        assert_eq!(
            stamp(explicit, || from_git.map(str::to_string)),
            expected,
            "explicit {explicit:?}, git {from_git:?}"
        );
    }
}

/// DDD-2. An explicit stamp means git's answer would be thrown away, so git is
/// not asked at all.
#[test]
fn git_is_not_asked_when_an_explicit_stamp_is_given() {
    let asked = std::cell::Cell::new(false);
    let stamped = stamp(Some("817c16d"), || {
        asked.set(true);
        Some("ae5b675".to_string())
    });
    assert_eq!(stamped, "817c16d");
    assert!(
        !asked.get(),
        "git must not be asked when an explicit stamp is given"
    );
}

// --- DDD-5: one placeholder on both sides ----------------------------------

/// DDD-5. build.rs and the renderer must say the SAME word for "nobody could
/// answer"; canzan-lift kept them in step by comment, here a test does.
#[test]
fn the_build_script_and_the_footer_agree_on_the_unknown_placeholder() {
    let (build_script_unknown, footer_unknown) = (build_script::UNKNOWN, BUILD_FIELD_UNKNOWN);
    assert_eq!(build_script_unknown, footer_unknown);
    assert_eq!(footer_unknown, "unknown");
}

// --- AC-6 / DDD-9 / DDD-10: what the footer renders ------------------------

/// DDD-9. A release build reads as its release and its commit date; the SHA is
/// on the element, not in the painted text.
#[test]
fn a_release_build_renders_the_exact_footer() {
    assert_eq!(
        footer_html("0.8.0", "817c16d", "2026-10-04"),
        "<footer class=\"site-footer\" data-commit=\"817c16d\">Foundry v0.8.0 \u{00B7} 2026-10-04</footer>"
    );
}

/// AC-6. A build that could not ask git (a `docker build` without build-args,
/// a source tarball) SAYS so: never an empty attribute, never a bare trailing
/// ` · `, never `vunknown` — and a blank field reads as `unknown` too.
#[test]
fn an_unanswerable_build_renders_the_honest_placeholder() {
    let degraded = "<footer class=\"site-footer\" data-commit=\"unknown\">Foundry v0.8.0 \u{00B7} unknown</footer>";
    for (sha, date) in [("unknown", "unknown"), ("", ""), ("  ", "\t\n")] {
        assert_eq!(
            footer_html("0.8.0", sha, date),
            degraded,
            "sha {sha:?}, date {date:?}"
        );
    }
}

/// DDD-10. Every field is HTML-escaped, in the attribute and in the text, so
/// the single `|safe` in base.html never lets machine-written input become
/// markup.
#[test]
fn hostile_build_fields_come_out_escaped() {
    for (version, sha, date, expected) in [
        (
            "0.8.0",
            "a\"b<c&d",
            "2026-10-04",
            "<footer class=\"site-footer\" data-commit=\"a&quot;b&lt;c&amp;d\">Foundry v0.8.0 \u{00B7} 2026-10-04</footer>",
        ),
        (
            "0.8.0",
            "817c16d",
            "<b>2026</b>",
            "<footer class=\"site-footer\" data-commit=\"817c16d\">Foundry v0.8.0 \u{00B7} &lt;b&gt;2026&lt;/b&gt;</footer>",
        ),
        (
            "0.8.0&<x>",
            "817c16d",
            "2026-10-04",
            "<footer class=\"site-footer\" data-commit=\"817c16d\">Foundry v0.8.0&amp;&lt;x&gt; \u{00B7} 2026-10-04</footer>",
        ),
    ] {
        assert_eq!(
            footer_html(version, sha, date),
            expected,
            "version {version:?}, sha {sha:?}, date {date:?}"
        );
    }
}

proptest! {
    /// AC-6, quantified. Whatever the build machine hands over for the SHA and
    /// the date, the footer renders one element whose commit attribute is never
    /// empty, whose text never ends in a bare separator, and whose interpolated
    /// text never contains markup.
    #[test]
    fn the_footer_renders_sanely_for_any_build_fields(
        sha in "[ -~]{0,40}",
        date in "[ -~]{0,40}",
    ) {
        let html = footer_html("0.8.0", &sha, &date);

        prop_assert!(
            html.starts_with("<footer class=\"site-footer\" data-commit=\""),
            "the footer element must always render; got {html}"
        );
        prop_assert!(
            !html.contains("data-commit=\"\""),
            "the commit attribute must never be empty; got {html}"
        );
        prop_assert!(
            !html.contains("\u{00B7} </footer>") && !html.contains("vunknown"),
            "no hole and no `vunknown`; got {html}"
        );
        let text = html
            .split_once('>')
            .and_then(|(_, rest)| rest.rsplit_once("</footer>"))
            .map(|(text, _)| text.to_string())
            .unwrap_or_default();
        prop_assert!(
            text.starts_with("Foundry v0.8.0 \u{00B7} ") && !text.contains('<') && !text.contains('>'),
            "the painted text must be the release, the separator and an escaped date; got {html}"
        );
    }
}
