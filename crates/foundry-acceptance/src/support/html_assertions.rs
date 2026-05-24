//! Thin wrappers around `scraper` for the US-10 markdown-rendering
//! scenarios. Keeps `scraper` import noise out of the step modules
//! and centralises the diagnostic-on-failure formatting so a missing
//! selector dumps the body once, not every time.

use scraper::{ElementRef, Html, Selector};

/// Parse a full HTML document or fragment. `scraper::Html::parse_fragment`
/// accepts both shapes.
pub fn parse(body: &str) -> Html {
    Html::parse_fragment(body)
}

/// Collect every element matching `css` under the root of `doc`. The
/// returned references borrow `doc`; callers should not retain them
/// across the document's lifetime.
pub fn select_all<'a>(doc: &'a Html, css: &str) -> Vec<ElementRef<'a>> {
    let sel = Selector::parse(css).unwrap_or_else(|err| panic!("bad selector {css:?}: {err:?}"));
    doc.select(&sel).collect()
}

/// Concatenated text content of an element (children too).
pub fn text_of(el: &ElementRef<'_>) -> String {
    el.text().collect::<String>()
}

/// Assert at least one element matches `css` in the body. Panics with a
/// body dump on miss.
pub fn assert_has(body: &str, css: &str) {
    let doc = parse(body);
    let matches = select_all(&doc, css);
    assert!(
        !matches.is_empty(),
        "expected selector {css:?} to match at least once; body was:\n{body}"
    );
}

/// Collect a named attribute's value from every element matching `css`,
/// in document order. Returns an empty vec when nothing matches; entries
/// where the attribute is missing are skipped. Used by the US-12
/// data-issue-key ordering assertion.
pub fn collect_attributes(body: &str, css: &str, attribute: &str) -> Vec<String> {
    let doc = parse(body);
    select_all(&doc, css)
        .into_iter()
        .filter_map(|el| el.value().attr(attribute).map(|s| s.to_string()))
        .collect()
}

/// Assert no element matches `css` in the body. Used by the XSS sanitization
/// scenario to assert `script` tags are stripped.
pub fn assert_not_has(body: &str, css: &str) {
    let doc = parse(body);
    let matches = select_all(&doc, css);
    assert!(
        matches.is_empty(),
        "expected selector {css:?} to match ZERO times; got {n} match(es). Body was:\n{body}",
        n = matches.len()
    );
}

/// Locate the first `.comment[data-author="<email>"]` block — the
/// container the US-10 step assertions scope into. Returns an owned
/// `Html` so the caller can hand it to `scraper::Selector` again
/// (re-parsing is cheap relative to the test overhead).
pub fn comment_section_by_author(body: &str, author_email: &str) -> Option<Html> {
    let doc = parse(body);
    let selector = format!(r#".comment[data-author="{author_email}"]"#);
    let sel = Selector::parse(&selector).expect("comment selector");
    doc.select(&sel)
        .next()
        .map(|el| Html::parse_fragment(&el.html()))
}

/// Assert the comment block authored by `author_email` contains an
/// element matching `inner_css` whose visible text equals `expected_text`.
/// Returns the matching element's collected text on success; panics
/// with a body dump on miss.
pub fn assert_comment_has_element_with_text(
    body: &str,
    author_email: &str,
    inner_css: &str,
    expected_text: &str,
) -> String {
    let Some(section) = comment_section_by_author(body, author_email) else {
        panic!("no comment by {author_email:?} found in body:\n{body}");
    };
    let sel = Selector::parse(inner_css)
        .unwrap_or_else(|err| panic!("bad selector {inner_css:?}: {err:?}"));
    for el in section.select(&sel) {
        let text = el.text().collect::<String>();
        if text.trim() == expected_text.trim() {
            return text;
        }
    }
    panic!(
        "no {inner_css:?} with text {expected_text:?} in comment by {author_email:?};\
         comment block HTML was:\n{html}",
        html = section.root_element().html()
    );
}

/// Assert the comment by `author_email` does NOT contain any element
/// matching `inner_css`. Used by the script-tag-strip scenario.
pub fn assert_comment_has_no_element(body: &str, author_email: &str, inner_css: &str) {
    let Some(section) = comment_section_by_author(body, author_email) else {
        panic!("no comment by {author_email:?} found in body:\n{body}");
    };
    let sel = Selector::parse(inner_css)
        .unwrap_or_else(|err| panic!("bad selector {inner_css:?}: {err:?}"));
    let mut found = section.select(&sel);
    assert!(
        found.next().is_none(),
        "expected NO {inner_css:?} in comment by {author_email:?}; got at least one in:\n{html}",
        html = section.root_element().html()
    );
}

/// Assert the comment by `author_email` contains an `<a>` element with
/// the given `href` and that the `rel` attribute contains `rel_fragment`
/// (substring match, since `rel` is multi-valued).
pub fn assert_comment_link_with_rel(
    body: &str,
    author_email: &str,
    href: &str,
    rel_fragment: &str,
) {
    let Some(section) = comment_section_by_author(body, author_email) else {
        panic!("no comment by {author_email:?} found in body:\n{body}");
    };
    let sel = Selector::parse("a").expect("a selector");
    for el in section.select(&sel) {
        let el_href = el.value().attr("href").unwrap_or("");
        if el_href != href {
            continue;
        }
        let rel = el.value().attr("rel").unwrap_or("");
        assert!(
            rel.contains(rel_fragment),
            "<a href={href:?}> rel={rel:?} does not contain {rel_fragment:?}"
        );
        return;
    }
    panic!(
        "no <a href={href:?}> in comment by {author_email:?}; comment HTML was:\n{html}",
        html = section.root_element().html()
    );
}
