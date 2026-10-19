//! api-openapi-document — the API page below the browser lane (DESIGN DDD-15,
//! DDD-16, DDD-19, review H3; [REF] Test Seams "foundry-app integration,
//! slice 03").
//!
//! Authored RED in DISTILL 2026-10-19 as pending panic stubs (the
//! project-lifecycle `project_lifecycle_copy.rs` idiom): `/docs/api`,
//! `api_docs::show`, `API_DOCS_CSP` and the vendored files do not exist yet.
//! DELIVER step 03-02 replaces each stub with requests through
//! `foundry_app::build_router` (the `api_comment_scoping.rs` harness; Docker
//! required, as for every testcontainers test) and removes the `#[ignore]`.
//! These run in `smoke` and `ci` with no browser; the browser lane keeps only
//! what needs one (`api-openapi-document-03-api-page.feature`, `@needs-browser`).

/// `GET /docs/api` with no cookie and with a signed-in session: 200, no
/// `Set-Cookie`, the exact `API_DOCS_CSP` string, `X-Frame-Options: DENY`,
/// `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
/// `Cache-Control: no-cache`.
#[test]
#[ignore = "pending: api-openapi-document 03-02 — page headers, no Set-Cookie, the exact CSP"]
fn the_api_page_carries_its_policy_and_sets_no_cookie() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: /docs/api headers)");
}

/// D14 copy, byte-exact: the title "Foundry API", the heading line with
/// `RELEASE_VERSION`, the `<noscript>` copy and its link to `/api/v1/openapi.json`.
#[test]
#[ignore = "pending: api-openapi-document 03-02 — the page's D14 copy, byte-exact"]
fn the_api_page_copy_is_byte_exact() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: title, heading, noscript)");
}

/// Exactly three subresource references, all same-origin `/static/…`: the
/// bundle, the stylesheet and `api-docs.js`; no inline script, handler or style.
#[test]
#[ignore = "pending: api-openapi-document 03-02 — exactly three same-origin subresources"]
fn the_api_page_references_exactly_its_three_files() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: subresources)");
}

/// The policy is the page's only: `GET /`, `GET /static/…` and
/// `GET /api/v1/openapi.json` carry no `Content-Security-Policy` (D12, SC-3).
#[test]
#[ignore = "pending: api-openapi-document 03-02 — no other answer carries the page's policy"]
fn no_other_answer_carries_the_page_policy() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: page-only CSP)");
}

/// Every vendored swagger-ui file and `api-docs.js` answers 200 through the
/// real `ServeDir`, `/vendor/` immutable and `/js/` no-cache (ADR-B03).
#[test]
#[ignore = "pending: api-openapi-document 03-01 — every vendored file and the initializer are served"]
fn every_page_file_is_served_with_its_cache_policy() {
    panic!(
        "Not yet implemented -- RED scaffold (api-openapi-document 03-01: vendored files served)"
    );
}
