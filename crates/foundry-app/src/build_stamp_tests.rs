//! release-version-footer US-RVF-02 — unit examples for the build stamp
//! (AC-5 precedence, AC-6 degraded and escaped rendering, DDD-5 placeholder
//! parity).
//!
//! What is tested is exactly what cargo executes: cargo never runs tests
//! written inside a build script, so this module compiles THE SAME FILE
//! (`include!` of `build.rs`, DDD-12) and drives its pure `stamp` and its pure
//! rerun-path choice (`ref_watch_path`, `git_watch_paths`, D11) directly.
//! The rendering examples target `views::SiteFooter::of`, the pure seam
//! `base.html` renders through `views::site_footer()`.

use proptest::prelude::*;

use crate::views::{SiteFooter, BUILD_FIELD_UNKNOWN};

/// `build.rs`, compiled into the test target. Its `main` and the shell helpers
/// (`given`, `git`, `emit_rerun_directives`, `rerun_if_present`) are unused
/// here, hence the allow.
#[allow(dead_code)]
mod build_script {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
}

use std::path::{Path, PathBuf};

use build_script::{git_watch_paths, ref_watch_path, stamp};

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

// --- D11: what cargo watches so the stamp never goes stale ------------------

/// D11. A commit on a branch rewrites the branch's loose ref, not `HEAD`. After
/// `git pack-refs` (or in a fresh clone) that loose file is ABSENT, and the
/// next commit CREATES it, so the watch must sit on the nearest directory that
/// exists or the footer keeps naming the previous commit. The climb stops at
/// `refs`: watching the git dir itself would rerun the build script on every
/// index write.
#[test]
fn the_branch_tip_watch_falls_back_to_the_nearest_existing_ref_directory() {
    let git = Path::new("/repo/.git");
    let main = git.join("refs/heads/main");
    let nested = git.join("refs/heads/feat/x/y");
    for (case, ref_path, existing, expected) in [
        (
            "a loose ref is watched itself",
            &main,
            vec![git.join("refs/heads/main"), git.join("refs/heads")],
            Some(git.join("refs/heads/main")),
        ),
        (
            "a packed ref is watched through its directory",
            &main,
            vec![git.join("refs/heads"), git.join("refs")],
            Some(git.join("refs/heads")),
        ),
        (
            "a namespaced branch climbs past its missing directories",
            &nested,
            vec![git.join("refs/heads"), git.join("refs")],
            Some(git.join("refs/heads")),
        ),
        (
            "a partly present namespace stops at the deepest existing directory",
            &nested,
            vec![git.join("refs/heads/feat"), git.join("refs/heads")],
            Some(git.join("refs/heads/feat")),
        ),
        (
            "only refs itself exists",
            &main,
            vec![git.join("refs"), git.to_path_buf()],
            Some(git.join("refs")),
        ),
        (
            "nothing below refs exists: never watch the git dir or above",
            &main,
            vec![
                git.to_path_buf(),
                PathBuf::from("/repo"),
                PathBuf::from("/"),
            ],
            None,
        ),
        ("nothing exists at all", &main, vec![], None),
    ] {
        let exists = |path: &Path| existing.iter().any(|known| known == path);
        assert_eq!(ref_watch_path(ref_path, exists), expected, "{case}");
    }
}

/// D11, quantified over every combination of existing ancestors of a
/// namespaced ref: the watch is the ref itself or a directory between it and
/// `refs`, never anything above `refs`, and it exists.
#[test]
fn the_branch_tip_watch_never_climbs_above_refs() {
    let refs = Path::new("/repo/.git/refs");
    let ref_path = refs.join("heads/feat/x/y");
    let candidates: Vec<&Path> = ref_path.ancestors().collect();
    for mask in 0u32..(1 << candidates.len()) {
        let existing: Vec<&Path> = candidates
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, path)| *path)
            .collect();
        let exists = |path: &Path| existing.contains(&path);
        let nearest_at_or_below_refs = candidates
            .iter()
            .take_while(|path| path.starts_with(refs))
            .find(|path| exists(path))
            .map(|path| path.to_path_buf());
        assert_eq!(
            ref_watch_path(&ref_path, exists),
            nearest_at_or_below_refs,
            "existing {existing:?}"
        );
    }
}

/// D11. The full list cargo is asked to watch: `HEAD`, the branch tip as
/// chosen above, and `packed-refs`, each only if it exists (a missing path
/// would rerun the script on every build). A detached HEAD has no branch tip;
/// a checkout git cannot answer for watches nothing.
#[test]
fn cargo_watches_head_the_branch_tip_and_packed_refs_when_they_exist() {
    let git = Path::new("/repo/.git");
    let (head, packed) = (git.join("HEAD"), git.join("packed-refs"));
    let (main, heads) = (git.join("refs/heads/main"), git.join("refs/heads"));
    for (case, branch_ref, existing, expected) in [
        (
            "a loose branch ref",
            Some(&main),
            vec![&head, &main, &heads, &packed],
            vec![&head, &main, &packed],
        ),
        (
            "a packed branch ref is watched through its directory",
            Some(&main),
            vec![&head, &heads, &packed],
            vec![&head, &heads, &packed],
        ),
        (
            "no packed-refs yet",
            Some(&main),
            vec![&head, &main, &heads],
            vec![&head, &main],
        ),
        (
            "a detached HEAD has no branch tip to watch",
            None,
            vec![&head, &heads, &packed],
            vec![&head, &packed],
        ),
    ] {
        let exists = |path: &Path| existing.iter().any(|known| *known == path);
        let watched = git_watch_paths(
            Some(&head),
            branch_ref.map(PathBuf::as_path),
            Some(&packed),
            exists,
        );
        let expected: Vec<PathBuf> = expected.into_iter().cloned().collect();
        assert_eq!(watched, expected, "{case}");
    }
    assert_eq!(
        git_watch_paths(None, None, None, |_: &Path| true),
        Vec::<PathBuf>::new(),
        "git could not answer"
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
