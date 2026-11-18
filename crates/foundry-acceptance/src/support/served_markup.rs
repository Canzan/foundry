//! ONE reading of a served document head, for every oracle that asserts the
//! shape of the head foundry SENDS.
//!
//! Two step modules each carried their own copy of this parse, and the gate in
//! `xtask` carries a third. Two of the three live in this crate and must never
//! disagree: the moment a fourth deferring attribute, or a new quoting form,
//! has to be understood, one copy learns it and the other silently keeps the
//! old reading, and whichever oracle reads the stale copy goes green over a
//! head it no longer understands. So the reading is owned here, once, and both
//! step modules call it.
//!
//! The third copy, the one in `xtask`, stays separate ON PURPOSE: the gate must
//! be able to CONTRADICT this crate, which it cannot do if it shares this
//! crate's eyes. That the two agree today is observed rather than asserted —
//! each side is compared with the same independently written table of the
//! shipped head, so a reader that drifts fails on exactly the side that
//! drifted. A drift that changes both readers identically is not caught; the
//! two inputs differ by construction (template source against served markup),
//! which is what makes the pairing worth having rather than a tautology.
//!
//! This file knows no feature. It depends on `scraper` and on nothing else.

/// Every `<script src=…>` the served `<head>` declares, IN DOCUMENT ORDER,
/// paired with the attributes that would DEFER its execution past parsing.
///
/// `defer`, `async` and `type="module"` are collected together because all
/// three defeat the one property a head script is put in the head for: a plain
/// external `<script>` in `<head>` is render-blocking by specification, so the
/// browser blocks rather than paints and whatever that script stamps on the
/// document is there before the first paint. Each of the three moves the stamp
/// PAST that paint, and each does it silently — every assertion that looks at
/// the settled page still passes while the operator watches the wrong state
/// flash on every navigation.
///
/// The pairing is deliberately a reading, not a verdict: it says what the head
/// DECLARES, and leaves which scripts may defer to the oracle that knows the
/// promise.
pub fn head_scripts(body: &str) -> Vec<(String, Vec<String>)> {
    let doc = scraper::Html::parse_document(body);
    let selector = scraper::Selector::parse("head script[src]").expect("valid selector");
    doc.select(&selector)
        .map(|el| {
            let src = el.value().attr("src").unwrap_or_default().to_string();
            let mut deferring = Vec::new();
            if el.value().attr("defer").is_some() {
                deferring.push("defer".to_string());
            }
            if el.value().attr("async").is_some() {
                deferring.push("async".to_string());
            }
            if el
                .value()
                .attr("type")
                .is_some_and(|value| value.eq_ignore_ascii_case("module"))
            {
                deferring.push("type=\"module\"".to_string());
            }
            (src, deferring)
        })
        .collect()
}
