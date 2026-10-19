//! api-openapi-document — step definitions (`@api-openapi-document`,
//! `@aod-01`..`@aod-03`). Authored by DISTILL 2026-10-19 under ADR-025: every
//! scenario is `@pending`; DELIVER un-pends them step by step.
//!
//! Every step body is ONE call into the driver
//! (`support::api_openapi_document`), which owns the typed vocabulary, the
//! doors and the oracles. No step holds control flow or business logic
//! (Mandate 12 criterion 3). Phrases are new and globally unique; none reuses
//! another feature's step (cucumber-rs requires unique matches).

use crate::support::api_openapi_document as aod;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

// ---- Background --------------------------------------------------------------------------------

#[given(regex = r#"^Foundry is running for the workspace "([^"]+)", administered by Priya$"#)]
async fn foundry_running(world: &mut FoundryWorld, workspace: String) {
    aod::given_foundry_running(world, &workspace).await;
}

#[given(regex = r#"^team Backend's project "auth" holds the issues AUTH-1 to AUTH-7$"#)]
async fn backend_issues(world: &mut FoundryWorld) {
    aod::given_backend_issues(world).await;
}

#[given(regex = r#"^Sven holds the machine token "ci-issue-filer" that Priya minted$"#)]
async fn sven_token(world: &mut FoundryWorld) {
    aod::given_sven_token(world).await;
}

#[given(regex = r"^Mei, a member of team Backend, holds her own machine token$")]
async fn mei_token(world: &mut FoundryWorld) {
    aod::given_mei_token(world).await;
}

#[given(regex = r"^team Ops exists, and Sven also holds a machine token that reaches only Ops$")]
async fn ops_only_token(world: &mut FoundryWorld) {
    aod::given_ops_only_token(world).await;
}

#[given(
    regex = r#"^Canzan Labs also has the machine tokens "scratch-rotation" and the revoked "old-ci-issue-filer"$"#
)]
async fn scratch_and_revoked(world: &mut FoundryWorld) {
    aod::given_scratch_and_revoked(world).await;
}

// ---- US-AOD-01: fetching the description -------------------------------------------------------

#[when(regex = r"^Sven fetches /api/v1/openapi\.json with no credential$")]
async fn fetches(world: &mut FoundryWorld) {
    aod::when_fetches(world).await;
}

#[given(regex = r"^Sven has fetched /api/v1/openapi\.json with no credential$")]
async fn has_fetched(world: &mut FoundryWorld) {
    aod::given_fetched(world).await;
}

#[when(regex = r"^Sven fetches /api/v1/openapi\.json with (an .+|a .+|Priya's .+)$")]
async fn fetches_with(world: &mut FoundryWorld, credential: String) {
    aod::when_fetches_with(world, aod::Credential::parse(&credential)).await;
}

#[then(
    regex = r#"^Sven gets the API description as an OpenAPI 3\.1 JSON document titled "([^"]+)"$"#
)]
async fn openapi_31_titled(world: &mut FoundryWorld, title: String) {
    aod::then_openapi_31_titled(world, &title);
}

#[then(
    regex = r"^it describes listing a project's issues, the machine token and the error answer$"
)]
async fn describes_list_scheme_error(world: &mut FoundryWorld) {
    aod::then_describes_list_scheme_error(world);
}

#[then(regex = r"^its version is the running release, the same one the page footer shows$")]
async fn version_is_release(world: &mut FoundryWorld) {
    aod::then_version_is_release_and_footer(world).await;
}

#[then(
    regex = r#"^it names "/" as its only server, so every address resolves on the instance it came from$"#
)]
async fn only_server_root(world: &mut FoundryWorld) {
    aod::then_only_server_is_root(world);
}

#[then(regex = r"^the bytes he receives equal the committed API description exactly$")]
async fn equals_committed(world: &mut FoundryWorld) {
    aod::then_equals_committed(world);
}

#[then(regex = r"^he gets the same 200 and the same bytes as with no credential$")]
async fn same_as_no_credential(world: &mut FoundryWorld) {
    aod::then_same_as_no_credential(world);
}

#[given(
    regex = r#"^the instance also holds the team "Platform Ops" and its project "Identity Platform"$"#
)]
async fn distinct_names(world: &mut FoundryWorld) {
    aod::given_distinct_names(world).await;
}

#[then(
    regex = r"^the description names no workspace, team, project, person, address or token of the instance$"
)]
async fn names_nothing_of_instance(world: &mut FoundryWorld) {
    aod::then_names_nothing_of_instance(world);
}

#[given(
    regex = r#"^a second Foundry of the same release runs for the workspace "([^"]+)" with its own teams$"#
)]
async fn second_instance(world: &mut FoundryWorld, workspace: String) {
    aod::given_second_instance(world, &workspace).await;
}

#[when(regex = r"^Sven fetches /api/v1/openapi\.json from both instances$")]
async fn fetches_both(world: &mut FoundryWorld) {
    aod::when_fetches_both(world).await;
}

#[then(regex = r"^the two descriptions are the same bytes$")]
async fn same_bytes(world: &mut FoundryWorld) {
    aod::then_same_bytes(world);
}

#[when(regex = r"^Sven requests (/\S+) with no credential$")]
async fn requests_address(world: &mut FoundryWorld, address: String) {
    aod::when_requests_address(world, &address).await;
}

#[when(regex = r"^Sven sends (a POST|a PUT|a PATCH|a DELETE) to /api/v1/openapi\.json$")]
async fn sends_write(world: &mut FoundryWorld, write: String) {
    aod::when_sends_write(world, &write).await;
}

#[then(
    regex = r"^he gets the API's not-found answer, the same as for an address that was never part of the API$"
)]
async fn api_not_found(world: &mut FoundryWorld) {
    aod::then_api_not_found(world, false).await;
}

#[then(
    regex = r"^he gets the API's not-found answer, the same as for an address that was never part of the API, and no list of allowed methods$"
)]
async fn api_not_found_no_allow(world: &mut FoundryWorld) {
    aod::then_api_not_found(world, true).await;
}

#[then(regex = r"^the description is still served at /api/v1/openapi\.json$")]
async fn still_served(world: &mut FoundryWorld) {
    aod::then_still_served(world).await;
}

#[then(regex = r"^the description says reading it needs no credential$")]
async fn reading_needs_no_credential(world: &mut FoundryWorld) {
    aod::then_reading_needs_no_credential(world);
}

#[then(regex = r"^every other operation it lists asks for the machine token$")]
async fn others_need_token(world: &mut FoundryWorld) {
    aod::then_others_need_machine_token(world);
}

// ---- The real-request matrix --------------------------------------------------------------------

#[given(
    regex = r#"^(Canzan Labs is as the Background left it|AUTH-7 has moved to todo|Backend's project auth has its own lane "review"|Sven has commented on AUTH-7|Mei has commented on AUTH-7|Sven has commented on AUTH-6|Sven's comment on AUTH-7 was deleted|Sven has used his revoke budget)$"#
)]
async fn precondition(world: &mut FoundryWorld, words: String) {
    aod::given_precondition(world, aod::Precondition::parse(&words)).await;
}

#[when(
    regex = r"^Sven tries to (read the API description|list a project's issues|create an issue|change an issue's state|read an issue's history|comment on an issue|edit a comment|list the workspace's tokens|revoke a token|reach an address outside the API's operations) (.+)$"
)]
async fn tries(world: &mut FoundryWorld, operation: String, situation: String) {
    aod::when_tries(
        world,
        aod::Operation::parse(&operation),
        aod::Situation::parse(&situation),
    )
    .await;
}

#[then(regex = r"^he gets (\d{3}), exactly as the description documents it for that operation$")]
async fn gets_as_documented(world: &mut FoundryWorld, status: u16) {
    aod::then_gets_as_documented(world, status).await;
}

#[then(regex = r"^nothing in Canzan Labs changed$")]
async fn nothing_changed(world: &mut FoundryWorld) {
    aod::then_nothing_changed(world).await;
}

#[then(regex = r"^Canzan Labs changed only as that answer reports$")]
async fn changed_only_as_reported(world: &mut FoundryWorld) {
    aod::then_changed_only_as_reported(world).await;
}

// ---- Reading the description --------------------------------------------------------------------

#[when(
    regex = r"^Sven reads how the description says he can (read the API description|list a project's issues|create an issue|change an issue's state|read an issue's history|comment on an issue|edit a comment|list the workspace's tokens|revoke a token)$"
)]
async fn reads_operation(world: &mut FoundryWorld, operation: String) {
    aod::when_reads_operation(world, aod::Operation::parse(&operation)).await;
}

#[then(regex = r"^it lists exactly the answers (.+)$")]
async fn lists_exactly(world: &mut FoundryWorld, answers: String) {
    aod::then_lists_exactly(world, &answers);
}

#[then(regex = r"^its (\d{3}) answer carries only the error codes (.+)$")]
async fn carries_codes(world: &mut FoundryWorld, status: u16, codes: String) {
    aod::then_codes(world, status, &codes);
}

#[when(regex = r"^Sven reads the API description$")]
async fn reads_description(world: &mut FoundryWorld) {
    aod::when_reads_description(world).await;
}

#[then(
    regex = r#"^no operation lists a 409 answer, and no answer carries the code "invalid_json"$"#
)]
async fn no_409(world: &mut FoundryWorld) {
    aod::then_no_409_no_invalid_json(world);
}

#[when(
    regex = r"^Sven reads the shape of (an issue|a comment|an issue history entry|a token|the error answer|a new issue request|a state change request|a comment request) in the API description$"
)]
async fn reads_shape(world: &mut FoundryWorld, thing: String) {
    aod::when_reads_shape(world, aod::Shape::parse(&thing)).await;
}

#[then(regex = r"^it has exactly the fields (.+)$")]
async fn has_fields(world: &mut FoundryWorld, fields: String) {
    aod::then_fields(world, &fields);
}

#[then(regex = r"^the fields it requires are (.+)$")]
async fn requires_fields(world: &mut FoundryWorld, required: String) {
    aod::then_required(world, &required);
}

#[then(regex = r"^the fields that may be null are (.+)$")]
async fn nullable_fields(world: &mut FoundryWorld, nullable: String) {
    aod::then_nullable(world, &nullable);
}

#[then(regex = r"^nothing in it could hold a token's value$")]
async fn no_token_value(world: &mut FoundryWorld) {
    aod::then_no_token_value(world);
}

#[when(regex = r"^Sven reads how the description describes an issue's state$")]
async fn reads_state(world: &mut FoundryWorld) {
    aod::when_reads_state(world).await;
}

#[then(regex = r"^it is a text with no fixed list of values$")]
async fn state_open_text(world: &mut FoundryWorld) {
    aod::then_state_open_text(world);
}

#[then(regex = r#"^its description reads "(.+)"$"#)]
async fn state_reads(world: &mut FoundryWorld, copy: String) {
    aod::then_state_reads(world, &copy);
}

#[then(regex = r"^its 400, 413 and 415 answers and one of its 422 answers have plain-text bodies$")]
async fn create_plain_text(world: &mut FoundryWorld) {
    aod::then_create_plain_text(world);
}

#[then(
    regex = r"^its other 422 answer is the error answer with the codes title_required and description_too_long$"
)]
async fn create_envelope_422(world: &mut FoundryWorld) {
    aod::then_create_envelope_422(world);
}

#[then(
    regex = r#"^every operation's 404 answer is the error answer "not_found" with the message "not found"$"#
)]
async fn every_404_uniform(world: &mut FoundryWorld) {
    aod::then_every_404_uniform(world, None);
}

#[then(regex = r#"^that 404 answer is described as "(.+)"$"#)]
async fn every_404_described(world: &mut FoundryWorld, copy: String) {
    aod::then_every_404_uniform(world, Some(&copy));
}

#[then(regex = r#"^the 401 answer is described once, for the machine token, and says "(.+)"$"#)]
async fn unauthorized_once(world: &mut FoundryWorld, clause: String) {
    aod::then_401_once(world, &clause);
}

#[when(regex = r"^Sven reads how the description describes the address of one issue$")]
async fn reads_one_issue_address(world: &mut FoundryWorld) {
    aod::when_reads_one_issue_address(world).await;
}

#[then(regex = r"^it offers only changing the issue's state there$")]
async fn only_change_state(world: &mut FoundryWorld) {
    aod::then_only_change_state(world);
}

#[then(regex = r"^it says any other way of asking answers 405, naming the ways that are offered$")]
async fn method_not_allowed_documented(world: &mut FoundryWorld) {
    aod::then_405_with_allow(world);
}

#[when(regex = r"^Sven looks for a way to get a machine token in the API description$")]
async fn looks_for_mint(world: &mut FoundryWorld) {
    aod::when_looks_for_mint(world).await;
}

#[then(
    regex = r"^no operation creates a machine token, and asking to create one at the tokens address is described as answering 405$"
)]
async fn no_mint_operation(world: &mut FoundryWorld) {
    aod::then_no_mint_operation(world);
}

#[then(regex = r#"^the machine token is described as "(.+)"$"#)]
async fn scheme_reads(world: &mut FoundryWorld, copy: String) {
    aod::then_scheme_reads(world, &copy);
}

#[given(regex = r"^Priya is signed in to Canzan Labs as its workspace admin$")]
async fn priya_signed_in(world: &mut FoundryWorld) {
    aod::given_priya_signed_in(world).await;
}

#[when(regex = r"^Priya opens the address where the description says machine tokens are minted$")]
async fn opens_mint_address(world: &mut FoundryWorld) {
    aod::when_opens_mint_address(world).await;
}

#[then(regex = r"^she reaches the page where she can mint a machine token$")]
async fn mint_page(world: &mut FoundryWorld) {
    aod::then_mint_page(world);
}

// ---- US-AOD-02: the drift check -----------------------------------------------------------------

#[given(regex = r"^the maintainer works in a copy of the current repository$")]
async fn tree_copy(world: &mut FoundryWorld) {
    aod::given_tree_copy(world).await;
}

#[when(regex = r"^the maintainer runs the drift check$")]
async fn runs_drift_check(world: &mut FoundryWorld) {
    aod::when_runs_drift_check(world).await;
}

#[then(regex = r"^the drift check passes$")]
async fn drift_passes(world: &mut FoundryWorld) {
    aod::then_drift_passes(world);
}

#[then(regex = r"^the drift check fails$")]
async fn drift_fails(world: &mut FoundryWorld) {
    aod::then_drift_fails(world);
}

#[then(
    regex = r"^it reports that it compared the API description with the code's routes, statuses and answer shapes$"
)]
async fn reports_compared(world: &mut FoundryWorld) {
    aod::then_reports_compared(world);
}

#[when(
    regex = r#"^the maintainer adds a field "([^"]+)" to (the issue answer|the error answer) and runs the drift check$"#
)]
async fn adds_field(world: &mut FoundryWorld, field: String, answer: String) {
    aod::when_plants_and_checks(world, aod::DriftPlant::add_field(&field, &answer)).await;
}

#[then(regex = r#"^it names (.+) and the field "([^"]+)"$"#)]
async fn names_field(world: &mut FoundryWorld, wherever: String, field: String) {
    aod::then_names_field(world, &wherever, &field);
}

#[when(
    regex = r"^the maintainer makes listing a project's issues able to answer 409 and runs the drift check$"
)]
async fn plants_409(world: &mut FoundryWorld) {
    aod::when_plants_and_checks(world, aod::DriftPlant::Conflict409).await;
}

#[then(regex = r"^it names listing a project's issues and the undocumented 409$")]
async fn names_409(world: &mut FoundryWorld) {
    aod::then_names_409(world);
}

#[when(
    regex = r"^the maintainer adds an operation that lists a team's projects and runs the drift check$"
)]
async fn plants_operation(world: &mut FoundryWorld) {
    aod::when_plants_and_checks(world, aod::DriftPlant::NewOperation).await;
}

#[then(regex = r#"^it names "([^"]+)" as (?:undocumented|described but not offered by the code)$"#)]
async fn names_operation(world: &mut FoundryWorld, operation: String) {
    aod::then_names_operation(world, &operation);
}

#[when(
    regex = r"^the maintainer removes listing a project's issues from the code and runs the drift check$"
)]
async fn plants_removal(world: &mut FoundryWorld) {
    aod::when_plants_and_checks(world, aod::DriftPlant::RemoveListIssues).await;
}

#[when(
    regex = r"^the maintainer bumps the release version without refreshing the committed description and runs the drift check$"
)]
async fn plants_bump(world: &mut FoundryWorld) {
    aod::when_plants_and_checks(world, aod::DriftPlant::BumpRelease).await;
}

#[then(
    regex = r#"^it names the description's version and the running release, and says to run "cargo xtask openapi"$"#
)]
async fn names_version(world: &mut FoundryWorld) {
    aod::then_names_version(world);
}

#[when(
    regex = r"^the maintainer rewords the description of listing a project's issues and runs the drift check$"
)]
async fn plants_reword(world: &mut FoundryWorld) {
    aod::when_plants_and_checks(world, aod::DriftPlant::Reword).await;
}

#[given(
    regex = r"^the maintainer has bumped the release version without refreshing the committed description$"
)]
async fn has_bumped(world: &mut FoundryWorld) {
    aod::given_bumped(world).await;
}

#[when(regex = r"^the maintainer runs the description refresh$")]
async fn runs_refresh(world: &mut FoundryWorld) {
    aod::when_refreshes(world).await;
}

#[when(regex = r"^the image build checks the committed description without changing it$")]
async fn checks_only(world: &mut FoundryWorld) {
    aod::when_checks_only(world).await;
}

#[then(regex = r"^the description check refuses$")]
async fn check_refuses(world: &mut FoundryWorld) {
    aod::then_check_refuses(world);
}

#[then(regex = r"^the committed description's version is the new release$")]
async fn version_is_new(world: &mut FoundryWorld) {
    aod::then_version_is_new(world);
}

#[then(regex = r"^every other byte of the committed description is unchanged$")]
async fn other_bytes_unchanged(world: &mut FoundryWorld) {
    aod::then_other_bytes_unchanged(world);
}

#[then(regex = r"^the drift check now passes on that tree$")]
async fn drift_now_passes(world: &mut FoundryWorld) {
    aod::then_drift_now_passes(world).await;
}

#[then(regex = r"^the committed description is byte for byte what it was$")]
async fn description_unchanged(world: &mut FoundryWorld) {
    aod::then_description_unchanged(world);
}

#[when(
    regex = r"^the maintainer removes revoking a token from the description and runs the drift check$"
)]
async fn plants_undocumented_revoke(world: &mut FoundryWorld) {
    aod::when_plants_and_checks(world, aod::DriftPlant::UndocumentRevoke).await;
}

// ---- US-AOD-05: the API page ---------------------------------------------------------------------

#[when(regex = r"^Sven opens the API page$")]
async fn opens_page(world: &mut FoundryWorld) {
    aod::when_opens_page(world).await;
}

#[then(regex = r#"^he sees the title "([^"]+)" and the line "(.+)"$"#)]
async fn title_and_line(world: &mut FoundryWorld, title: String, line: String) {
    aod::then_title_and_line(world, &title, &line).await;
}

#[then(regex = r"^every operation of the API description is listed on the page$")]
async fn every_operation_listed(world: &mut FoundryWorld) {
    aod::then_every_operation_listed(world).await;
}

#[when(regex = r"^Sven opens the API page and it finishes loading$")]
async fn opens_page_loaded(world: &mut FoundryWorld) {
    aod::when_opens_page_loaded(world).await;
}

#[then(regex = r"^every request the browser made went to the instance's own address$")]
async fn all_requests_own_origin(world: &mut FoundryWorld) {
    aod::then_all_requests_own_origin(world).await;
}

#[then(regex = r"^the browser refused nothing the page asked for$")]
async fn nothing_refused(world: &mut FoundryWorld) {
    aod::then_nothing_refused(world).await;
}

#[given(regex = r"^a page on another site embeds the API page in a frame$")]
async fn framing_site(world: &mut FoundryWorld) {
    aod::given_framing_site(world).await;
}

#[when(regex = r"^Sven's browser opens that page$")]
async fn opens_framing_page(world: &mut FoundryWorld) {
    aod::when_opens_framing_page(world).await;
}

#[then(regex = r"^the browser refuses to show the API page inside it$")]
async fn refuses_frame(world: &mut FoundryWorld) {
    aod::then_refuses_frame(world).await;
}

#[when(regex = r"^a visitor asks for the API page$")]
async fn visitor_asks_page(world: &mut FoundryWorld) {
    aod::when_visitor_asks_page(world).await;
}

#[then(regex = r#"^the answer carries the content policy "(.+)"$"#)]
async fn page_csp(world: &mut FoundryWorld, policy: String) {
    aod::then_page_csp(world, &policy);
}

#[then(
    regex = r"^the answer tells browsers never to frame it, never to guess its type and never to send a referrer$"
)]
async fn page_companions(world: &mut FoundryWorld) {
    aod::then_page_companions(world);
}

#[then(
    regex = r"^the description at /api/v1/openapi\.json and the sign-in page carry no content policy$"
)]
async fn others_no_csp(world: &mut FoundryWorld) {
    aod::then_others_no_csp(world).await;
}

#[given(regex = r"^Priya's browser has scripting disabled$")]
async fn scripting_disabled(world: &mut FoundryWorld) {
    aod::given_scripting_disabled(world).await;
}

#[when(regex = r"^Priya opens the API page$")]
async fn priya_opens_page(world: &mut FoundryWorld) {
    aod::when_priya_opens_page(world).await;
}

#[then(regex = r#"^she reads "(.+)" on the API page$"#)]
async fn reads_on_page(world: &mut FoundryWorld, copy: String) {
    aod::then_reads_on_page(world, &copy).await;
}

#[then(regex = r#"^"/api/v1/openapi\.json" on the API page is a link to the description$"#)]
async fn doc_link(world: &mut FoundryWorld) {
    aod::then_doc_link(world).await;
}

#[given(regex = r"^the API description cannot be loaded by Sven's browser$")]
async fn description_blocked(world: &mut FoundryWorld) {
    aod::given_description_blocked(world).await;
}

#[then(
    regex = r"^Sven sees that the description could not be loaded, and the line naming /api/v1/openapi\.json is still there$"
)]
async fn load_failure_shown(world: &mut FoundryWorld) {
    aod::then_load_failure_shown(world).await;
}

#[given(regex = r"^Sven has opened the API page, which loaded its own files$")]
async fn page_opened(world: &mut FoundryWorld) {
    aod::given_page_opened(world).await;
}

#[when(regex = r"^Priya opens Backend's board in her browser$")]
async fn priya_opens_board(world: &mut FoundryWorld) {
    aod::when_priya_opens_board(world).await;
}

#[then(regex = r"^the board loads none of the API page's files$")]
async fn board_has_no_page_files(world: &mut FoundryWorld) {
    aod::then_board_has_no_page_files(world).await;
}

#[when(regex = r"^Sven fetches every file the API page uses$")]
async fn fetches_page_files(world: &mut FoundryWorld) {
    aod::when_fetches_page_files(world).await;
}

#[then(regex = r"^each answers with the bytes the repository holds for it$")]
async fn files_match_repository(world: &mut FoundryWorld) {
    aod::then_files_match_repository(world);
}

#[when(regex = r"^a visitor and Priya, signed in, each ask for the API page$")]
async fn visitor_and_priya_ask(world: &mut FoundryWorld) {
    aod::when_visitor_and_priya_ask_page(world).await;
}

#[then(regex = r"^both get the same page, and neither answer sets a cookie$")]
async fn same_page_no_cookie(world: &mut FoundryWorld) {
    aod::then_same_page_no_cookie(world);
}

// ---- US-AOD-06: trying a call from the page ------------------------------------------------------

#[when(
    regex = r#"^Sven pastes "ci-issue-filer" into the API page and tries listing Backend's "auth" project issues$"#
)]
async fn pastes_and_lists(world: &mut FoundryWorld) {
    aod::when_pastes_and_lists(world).await;
}

#[then(regex = r"^he sees a 200 answer listing AUTH-7$")]
async fn sees_auth7(world: &mut FoundryWorld) {
    aod::then_sees_200_with_auth7(world).await;
}

#[then(regex = r"^the call went to the instance's own address$")]
async fn call_own_origin(world: &mut FoundryWorld) {
    aod::then_call_own_origin(world).await;
}

#[then(regex = r"^the page shows the command line that made the call, carrying the pasted token$")]
async fn curl_with_token(world: &mut FoundryWorld) {
    aod::then_curl_with_token(world).await;
}

#[when(regex = r"^Sven opens the API page's token dialog$")]
async fn opens_token_dialog(world: &mut FoundryWorld) {
    aod::when_opens_token_dialog(world).await;
}

#[then(regex = r#"^the token field is labelled "([^"]+)" and says "(.+)"$"#)]
async fn token_field_copy(world: &mut FoundryWorld, label: String, hint: String) {
    aod::then_token_field_copy(world, &label, &hint).await;
}

#[when(
    regex = r#"^Sven pastes "ci-issue-filer" into the API page and revokes "scratch-rotation"$"#
)]
async fn pastes_and_revokes(world: &mut FoundryWorld) {
    aod::when_pastes_and_revokes(world).await;
}

#[then(regex = r#"^he sees the 204 answer, and "scratch-rotation" is revoked$"#)]
async fn sees_204_revoked(world: &mut FoundryWorld) {
    aod::then_204_and_revoked(world).await;
}

#[when(
    regex = r#"^Sven tries listing Backend's "auth" project issues from the API page without a token$"#
)]
async fn tries_without_token(world: &mut FoundryWorld) {
    aod::when_tries_without_token(world).await;
}

#[then(regex = r#"^he sees the (\d{3}) answer with the error code "([^"]+)"$"#)]
async fn sees_refusal(world: &mut FoundryWorld, status: u16, code: String) {
    aod::then_sees_refusal(world, status, &code).await;
}

#[when(
    regex = r#"^Sven tries (listing Backend's "auth" project issues|revoking "scratch-rotation") from the API page with (.+)$"#
)]
async fn tries_refused(world: &mut FoundryWorld, call: String, token: String) {
    aod::when_tries_refused(
        world,
        aod::PageCall::parse(&call),
        aod::PageToken::parse(&token),
    )
    .await;
}

#[given(regex = r#"^Sven has already made a call from the page with "ci-issue-filer"$"#)]
async fn pasted_and_called(world: &mut FoundryWorld) {
    aod::given_pasted_and_called(world).await;
}

#[when(regex = r"^Sven reloads the API page$")]
async fn reloads(world: &mut FoundryWorld) {
    aod::when_reloads(world).await;
}

#[then(regex = r"^no token is filled in$")]
async fn no_token_filled(world: &mut FoundryWorld) {
    aod::then_no_token_filled(world).await;
}

#[then(regex = r"^the browser keeps no trace of the token$")]
async fn no_trace(world: &mut FoundryWorld) {
    aod::then_no_trace(world).await;
}

#[when(
    regex = r#"^Sven pastes "ci-issue-filer" into the API page and tries every operation once$"#
)]
async fn pastes_and_tries_every(world: &mut FoundryWorld) {
    aod::when_pastes_and_tries_every(world).await;
}

#[given(regex = r"^another site serves its own API description and configuration$")]
async fn foreign_site(world: &mut FoundryWorld) {
    aod::given_foreign_site(world).await;
}

#[when(
    regex = r"^Priya opens the API page from a link that names that site's (description|configuration)$"
)]
async fn opens_foreign_link(world: &mut FoundryWorld, what: String) {
    aod::when_opens_foreign_link(world, aod::ForeignLink::parse(&what)).await;
}

#[then(regex = r"^the page shows the instance's own API description$")]
async fn own_description_shown(world: &mut FoundryWorld) {
    aod::then_own_description_shown(world).await;
}

#[then(regex = r"^the browser made no request to that site$")]
async fn no_request_to_foreign(world: &mut FoundryWorld) {
    aod::then_no_request_to_foreign(world);
}
