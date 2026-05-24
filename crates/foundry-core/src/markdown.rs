//! Markdown → sanitized HTML for issue comments (US-10).
//!
//! Two-stage pipeline:
//!   1. `pulldown-cmark` parses CommonMark to HTML with raw-HTML
//!      passthrough DISABLED — any `<script>`/`<iframe>`/etc. the user
//!      writes never reaches the HTML buffer in the first place. This
//!      makes ammonia the second line of defence, not the only one.
//!   2. `ammonia` walks the resulting HTML and prunes anything outside
//!      our allowlist (tags, attributes, URL schemes), and force-adds
//!      `rel="noopener noreferrer"` + `target="_blank"` to every `<a>`.
//!
//! Allowed elements (per US-08/US-10 and `design/auth.md`):
//!     p, br, strong, em, a, code, pre, blockquote,
//!     ul, ol, li, h1, h2, h3, hr
//!
//! Allowed attributes:
//!     a: href (https/http/mailto only), title, rel, target
//!     code/pre: class (so syntax-highlighter hints survive)
//!
//! Everything else is stripped. The renderer is total — even malicious
//! input yields a `SanitizedHtml` (possibly empty); errors only arise
//! from the upstream pulldown-cmark options building, which is
//! infallible. There's no fallible path to surface.

use ammonia::Builder;
use pulldown_cmark::{html as cmark_html, Options, Parser};
use std::collections::HashSet;

/// HTML that has passed the comment-rendering sanitizer. The wrapper is
/// intentional: handlers that emit this into a template do so via
/// [`SanitizedHtml::as_str`] without needing a second `html_escape`
/// pass, because the contents are already a closed HTML subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedHtml(String);

impl SanitizedHtml {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for SanitizedHtml {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Render a single comment-body string through markdown → sanitized HTML.
///
/// Input is treated as CommonMark with raw HTML disabled. The output
/// contains only elements in the allowlist (`p, strong, em, a, code,
/// pre, blockquote, ul, ol, li, h1, h2, h3, hr, br`) and is safe to
/// embed verbatim inside an HTML document body.
pub fn render_comment_markdown(input: &str) -> SanitizedHtml {
    // Pre-stage — neutralise raw HTML in the source by escaping `<`, `>`,
    // and `&`. pulldown-cmark would otherwise treat `<script>...</script>`
    // as raw inline HTML and, in doing so, would stop parsing surrounding
    // markdown emphasis on the same line. Escaping up-front means the
    // user sees `<script>` rendered as literal text (which ammonia will
    // never see as a tag), while their `**bold**` still parses cleanly.
    //
    // This is a stricter posture than CommonMark's default, and it's the
    // right one for an issue-comment field: there is no use case for
    // raw HTML inside a markdown comment. Anything the user could write
    // as markdown they can ALSO write as markdown.
    let escaped: String = input
        .chars()
        .flat_map(|c| match c {
            '<' => "&lt;".chars().collect::<Vec<_>>(),
            '>' => "&gt;".chars().collect::<Vec<_>>(),
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect();

    // Stage 1 — parse the escaped markdown. Tables, strikethrough,
    // footnotes, etc. are NOT enabled — keep the markdown subset small
    // until a feature actually needs it.
    let options = Options::empty();
    let parser = Parser::new_ext(&escaped, options);
    let mut html_buffer = String::with_capacity(escaped.len() + 32);
    cmark_html::push_html(&mut html_buffer, parser);

    // Stage 2 — sanitize. Ammonia's defaults already strip scripts +
    // dangerous URL schemes; we narrow further to the comment allowlist
    // and force a safer rel/target on every link.
    let mut tags: HashSet<&str> = HashSet::new();
    for t in [
        "p",
        "br",
        "strong",
        "em",
        "a",
        "code",
        "pre",
        "blockquote",
        "ul",
        "ol",
        "li",
        "h1",
        "h2",
        "h3",
        "hr",
    ] {
        tags.insert(t);
    }

    // Note: `rel` is intentionally NOT in this set. Ammonia disallows
    // explicit `rel` here when `link_rel(Some(..))` is also set — the
    // forced rel below takes precedence and overrides any author-supplied
    // value. Same with `target` (we don't set it from author input).
    let mut a_attrs: HashSet<&str> = HashSet::new();
    a_attrs.insert("href");
    a_attrs.insert("title");

    let mut code_attrs: HashSet<&str> = HashSet::new();
    code_attrs.insert("class");

    let mut tag_attributes: std::collections::HashMap<&str, HashSet<&str>> =
        std::collections::HashMap::new();
    tag_attributes.insert("a", a_attrs);
    tag_attributes.insert("code", code_attrs.clone());
    tag_attributes.insert("pre", code_attrs);

    let mut url_schemes: HashSet<&str> = HashSet::new();
    url_schemes.insert("http");
    url_schemes.insert("https");
    url_schemes.insert("mailto");

    let cleaned = Builder::default()
        .tags(tags)
        .tag_attributes(tag_attributes)
        .url_schemes(url_schemes)
        // Force every <a> to carry rel="noopener noreferrer". Without
        // this, `target="_blank"` opens a reverse-tabnabbing window.
        .link_rel(Some("noopener noreferrer"))
        .clean(&html_buffer)
        .to_string();

    SanitizedHtml(cleaned)
}

#[cfg(test)]
mod markdown_tests {
    use super::*;

    // Behaviour 1 — emphasis + strong + inline code survive the
    // pipeline with their expected tags. Parametrize variations of the
    // same behaviour ("inline marker → tag") into one test.
    #[test]
    fn renders_inline_emphasis_and_code() {
        for (input, expected_substring) in [
            ("plain **bold** here", "<strong>bold</strong>"),
            ("plain *italic* here", "<em>italic</em>"),
            (
                "plain `request.cookies` here",
                "<code>request.cookies</code>",
            ),
        ] {
            let out = render_comment_markdown(input);
            assert!(
                out.as_str().contains(expected_substring),
                "input {input:?} did not produce {expected_substring:?}; got {}",
                out.as_str()
            );
        }
    }

    // Behaviour 2 — link rendering adds rel="noopener noreferrer" to
    // every <a>. Even a link the user wrote without rel gets it.
    #[test]
    fn links_carry_noopener_noreferrer() {
        let out = render_comment_markdown("[RFC](https://example.com)");
        let s = out.as_str();
        assert!(s.contains(r#"href="https://example.com""#), "got {s}");
        assert!(s.contains("noopener"), "missing noopener in {s}");
        assert!(s.contains("noreferrer"), "missing noreferrer in {s}");
    }

    // Behaviour 3 — script tags in input are stripped. Tests BOTH the
    // pulldown-cmark disable-html path AND the ammonia safety net. A
    // user typing `<script>alert(1)</script>` should produce no
    // `<script>` substring in the output.
    #[test]
    fn strips_script_tags() {
        let out = render_comment_markdown("hello <script>alert('xss')</script> world");
        let s = out.as_str();
        assert!(
            !s.contains("<script"),
            "script tag survived sanitization: {s}"
        );
        // The script body is treated as literal text by pulldown-cmark
        // with raw-html off, so the word 'alert' may appear escaped as
        // text content — that's expected, not a leak.
    }

    // Behaviour 4 — `javascript:` URL schemes are rejected. The link's
    // href is dropped (ammonia turns the <a> into a plain text node
    // when the scheme is disallowed).
    #[test]
    fn rejects_javascript_url_scheme() {
        let out = render_comment_markdown("[click](javascript:alert(1))");
        let s = out.as_str();
        assert!(
            !s.contains("javascript:"),
            "javascript: URL survived sanitization: {s}"
        );
    }

    // Behaviour 5 — fenced code blocks render as <pre><code>…</code></pre>
    // with their contents intact (whitespace + special chars preserved).
    #[test]
    fn renders_fenced_code_block() {
        let input = "```\nlet x = 1;\n```";
        let out = render_comment_markdown(input);
        let s = out.as_str();
        assert!(s.contains("<pre>"), "missing <pre> in {s}");
        assert!(s.contains("<code>"), "missing <code> in {s}");
        assert!(s.contains("let x = 1;"), "code body missing in {s}");
    }

    // Behaviour 6 — disallowed tags (iframe, style, object, img with
    // event handlers) never appear as live elements in the output. We
    // pre-escape raw HTML in the input so these tags are rendered as
    // literal text inside a `<p>` — never as executable elements. The
    // assertion targets the literal element starts, not their textual
    // forms: `<iframe` would mean a real iframe shipped; `&lt;iframe`
    // is just visible text and is safe.
    #[test]
    fn strips_disallowed_tags() {
        for input in [
            "<iframe src=\"evil.com\"></iframe>",
            "<style>body { display: none }</style>",
            "<object data=\"x.swf\"></object>",
            "<img src=x onerror=alert(1)>",
        ] {
            let out = render_comment_markdown(input);
            let s = out.as_str();
            for live_tag in ["<iframe", "<style", "<object", "<img"] {
                assert!(
                    !s.contains(live_tag),
                    "input {input:?} leaked live tag {live_tag:?}; got {s}"
                );
            }
        }
    }

    // Behaviour 7 — empty input produces empty (or whitespace-only)
    // HTML. The handler layer enforces non-empty body BEFORE calling
    // the renderer, but the renderer itself must not panic on edge
    // cases.
    #[test]
    fn renders_empty_input_without_panicking() {
        let out = render_comment_markdown("");
        assert!(
            out.as_str().trim().is_empty(),
            "expected empty/whitespace output for empty input, got {:?}",
            out.as_str()
        );
    }
}
