//! api-openapi-document — router-level conformance of the served description
//! (DESIGN DDD-9 behavioural layer, [REF] Test Seams "foundry-app integration").
//!
//! Authored RED in DISTILL 2026-10-19 as pending panic stubs (the
//! project-lifecycle `project_lifecycle_copy.rs` idiom): the thing every test
//! compares against, the committed `crates/foundry-api/openapi.json` and the
//! `/api/v1/openapi.json` route, does not exist yet. DELIVER step 01-04 replaces
//! each stub with real requests through `foundry_app::build_router` on a
//! testcontainers Postgres (the `api_comment_scoping.rs` harness), judges every
//! answer against the document's entry for that operation and status, and
//! removes the `#[ignore]`. The acceptance matrix
//! (`api-openapi-document-02-every-answer.feature`) produces the same answers
//! through the HTTP lane; these tests hold them below it, inside
//! `cargo test --workspace` (so `smoke` and `ci` run them unconditionally).

/// D1, D8, OQ-6: GET with no header, a garbage bearer, an expired token, a
/// revoked token and a session cookie each answer 200 `application/json`,
/// `Cache-Control: no-cache`, and bytes equal to the committed file.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — served bytes equal the committed file for every credential"]
fn the_description_is_the_same_bytes_for_every_credential() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: D8 bytes across credentials)");
}

/// Changed Assumption 1 (user, 2026-10-18): POST, PUT, PATCH and DELETE on
/// `/api/v1/openapi.json` answer the 404 envelope byte-identical to
/// `/api/v1/openapi.yaml` and to the catch-all, with no `Allow` header.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — non-GET on the description answers the catch-all 404"]
fn writing_to_the_description_answers_the_catch_all() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: method fallback to the catch-all)");
}

/// DESIGN Test Seams: `HEAD /api/v1/openapi.json` answers 200 with no body.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — HEAD on the description answers 200 with no body"]
fn asking_only_for_the_headers_answers_200_with_no_body() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: HEAD)");
}

/// For each documented operation, no token answers the documented 401 envelope.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — every documented operation answers the documented 401"]
fn every_documented_operation_refuses_no_token_as_documented() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 401 per operation)");
}

/// For each documented path except the description's own, each undocumented
/// method answers 405 with an `Allow` header equal to the documented methods
/// (plus HEAD where GET).
#[test]
#[ignore = "pending: api-openapi-document 01-04 — 405 with an Allow header equal to the documented methods"]
fn every_documented_path_answers_405_with_the_documented_methods() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 405 + Allow)");
}

/// With a real token on a seeded project: a bad `{number}` / `{jti}` /
/// `{comment_id}` is 400 text/plain; no Content-Type 415; bad JSON 400; a
/// missing field 422 text/plain; a 2 MB + 1 byte body 413 (C4, C12); each
/// status and media type equals the document's entry for that operation.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — the text/plain extractor refusals match the document"]
fn the_extractor_refusals_answer_as_documented() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 400/413/415/422 text/plain)");
}

/// A valid create answers 201 with a `Location` of the documented form.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — a created issue carries the documented Location"]
fn a_created_issue_carries_the_documented_location() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 201 Location)");
}

/// Review L1: the document's `info.version` equals
/// `foundry_app::views::RELEASE_VERSION`, the footer's source.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — info.version equals the footer's release version"]
fn the_description_version_is_the_footer_version() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: info.version == RELEASE_VERSION)");
}
