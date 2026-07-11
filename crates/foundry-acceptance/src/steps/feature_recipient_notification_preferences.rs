//! recipient-notification-preferences (v1 = recipient unsubscribe) — step definitions
//! for the signed per-(email, workspace) opt-out over the two SUPPRESSIBLE notification
//! events, with the three MANDATORY security events held structurally exempt.
//!
//! HARNESS BOUNDARY (distill/acceptance-review.md): the app + Postgres are REAL (the
//! shipped in-process axum harness + testcontainers, `@real-io`), mirroring the predecessor
//! `notification-delivery-providers`. DELIVER wires the NEW production seams this feature
//! introduces — `UnsubscribeToken` (foundry-auth), the `0014_notification_unsubscribes`
//! table + `Store` methods, the `SuppressionPolicy` port + `StoreSuppression` +
//! `AllowAllSuppression`, the suppression gate inside the infallible `Notifier::notify`, the
//! public `GET`/`POST /unsubscribe` confirm+mutate routes, the signed-in
//! `/account/notifications` status + resubscribe surface, and the sibling
//! `foundry_notification_suppressions_total{event}` counter — through the composition root.
//! The DELIVERY TRANSPORTS stay in-process recording doubles (the shipped
//! `support::notify_recorder` providers) so a `Then` can observe delivered-vs-suppressed
//! without a real SMTP/webhook call. The register-at-0 + bounded-label metric scenarios
//! drive a REAL `foundry` subprocess + scrape its `/metrics` sidecar (the in-process harness
//! installs no recorder — the same split the predecessor used). Every scenario enters through
//! a DRIVING PORT (the email link, the `/unsubscribe` GET/POST, the signed-in page, a real
//! emit flow, the recorder, `/metrics`) — never an internal function (Mandate 1).
//!
//! SCAFFOLD STATUS (Mandate 7 — RED-ready, not BROKEN): every scenario in the feature file
//! is `@pending` (excluded from all lanes), and none of the production seams above exist yet.
//! Each step body below is therefore a compiling scaffold that `panic!`s (an assertion-class
//! failure = RED, never an ImportError-class BROKEN). DELIVER removes `@pending` slice-by-
//! slice and replaces each stub with a body that wires the real harness seam it builds
//! (a `spawn_with_unsubscribe`-style composition root + the shipped recording doubles),
//! turning the scenario GREEN. Run one slice with
//! `FOUNDRY_ACCEPTANCE_TAGS=recipient-unsubscribe`.
//!
//! __SCAFFOLD__
//! SCAFFOLD: true
//!
//! Every phrase below is globally unique unsubscribe-domain wording — verified against every
//! other step module (cucumber-rs panics on duplicate step registration; the invite-domain
//! `Sam ...` phrases in `feature_member_invites.rs` are all invite/account wording, never the
//! `unsubscribe`/`muted`/`resubscribe`/`suppressed` vocabulary used here). Reuse of the shipped
//! harness happens at the HELPER level inside DELIVER's future bodies (`InProcHarness`,
//! `signed_in_post`, the seed helpers), NOT at the Gherkin phrase level — so this module
//! declares only new, non-colliding phrases and registers no duplicate.

use crate::support::harness::{signed_in_post, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_app::ProviderKind;
use secrecy::SecretString;
use std::collections::HashMap;

/// Fixed scenario clock anchor (mirrors the other in-process step modules).
const RNP_NOW: &str = "2026-01-15T12:00:00Z";
/// The account-less recipient the walking skeleton targets (already lower-cased so
/// the token payload, the store row, and the emit recipient all normalize alike).
const SAM_EMAIL: &str = "sam@northwind.example";
/// A seeded workspace admin used to drive the REAL `POST /invites` emit whose
/// `workspace_invite` the suppression gate then intercepts.
const ADMIN_EMAIL: &str = "admin@northwind.example";
const ADMIN_PASSWORD: &str = "rnp-correct-horse-battery-staple";

fn rnp_now() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(RNP_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn rnp_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

/// Seed a workspace named `workspace` plus an `admin` member with a known password,
/// so the scenario can sign the admin in and drive the shipped `POST /invites`
/// emit. Returns the new workspace id (also the id the unsubscribe token binds).
async fn seed_workspace_and_admin(harness: &InProcHarness, workspace: &str) -> uuid::Uuid {
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let lower = ADMIN_EMAIL.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&SecretString::new(ADMIN_PASSWORD.to_string().into()))
        .await
        .expect("hash admin pw");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(workspace)
        .execute(pool)
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&lower)
    .bind(ADMIN_EMAIL)
    .bind("Ada Admin")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert admin user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert admin membership");
    workspace_id
}

/// Fetch a URL and return the `foundry_csrf` token minted in its `Set-Cookie`.
async fn csrf_from_get(http: &reqwest::Client, url: &str) -> String {
    let get = http.get(url).send().await.expect("GET for csrf");
    let raw = get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("GET must mint a foundry_csrf cookie");
    raw.strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string()
}

/// Shared pending-scaffold panic (assertion-class = RED, never BROKEN). DELIVER replaces
/// each caller's body with the real harness wiring as it unskips the owning slice.
macro_rules! pending {
    ($phrase:literal) => {
        panic!(concat!(
            "pending — DELIVER wires this step when it unskips the owning slice: ",
            $phrase
        ))
    };
}

// ============================================================================
// Background — the app is serving with the recipient-unsubscribe feature wired
// (DELIVER: spawn the real in-process harness with StoreSuppression + the
// /unsubscribe + /account/notifications routes + the recording provider doubles).
// ============================================================================

#[given(regex = r#"^Foundry is serving with recipient unsubscribe enabled$"#)]
async fn foundry_serving_with_unsubscribe(world: &mut FoundryWorld) {
    // Spawn the REAL in-process app + testcontainers Postgres with the `log`
    // recording provider double. The 0014 table + StoreSuppression gate + the
    // /unsubscribe route are all wired through the composition root (harness).
    let harness = InProcHarness::spawn_with_providers(rnp_now(), &[ProviderKind::Log]).await;
    world.harness = Some(harness);
    world.http = Some(rnp_client());
}

// ============================================================================
// Given — recipient state, links, and adverse preconditions
// ============================================================================

#[given(
    regex = r#"^Sam has a workspace-invite email for "([^"]+)" carrying a signed unsubscribe link$"#
)]
async fn sam_has_workspace_invite_email_with_link(world: &mut FoundryWorld, workspace: String) {
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    // Seed the workspace + an admin who can drive the shipped invite emit.
    let workspace_id = seed_workspace_and_admin(harness, &workspace).await;
    // Mint the SAME signed link a suppressible email body would carry: a
    // domain-separated HMAC over (email_lower, workspace_id) keyed on SESSION_SECRET
    // (foundry_auth::UnsubscribeToken), plus the base64url `t` param.
    let email_lower = SAM_EMAIL.to_ascii_lowercase();
    let token = foundry_auth::UnsubscribeToken::new(
        &email_lower,
        workspace_id,
        &harness.app.state.session_secret,
    )
    .expect("mint unsubscribe token");
    let t = foundry_app::unsubscribe::encode_t(&email_lower, workspace_id);
    world.unsub_email = Some(email_lower);
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_t = Some(t);
    world.unsub_sig = Some(token.signature);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
}

#[given(
    regex = r#"^Sam has a member-invite email for "([^"]+)" carrying a signed unsubscribe link$"#
)]
async fn sam_has_member_invite_email_with_link(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam has a member-invite email carrying a signed unsubscribe link");
}

#[given(regex = r#"^Sam has confirmed unsubscribing from "([^"]+)"$"#)]
async fn sam_has_confirmed_unsubscribing_from(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam has confirmed unsubscribing from a workspace");
}

#[given(regex = r#"^Sam also has an invite for workspace "([^"]+)"$"#)]
async fn sam_also_has_invite_for_workspace(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam also has an invite for another workspace");
}

#[given(regex = r#"^Sam has unsubscribed from "([^"]+)" via a workspace-invite link$"#)]
async fn sam_unsubscribed_via_workspace_invite_link(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam has unsubscribed via a workspace-invite link");
}

#[given(regex = r#"^Sam has unsubscribed from "([^"]+)" via a member-invite link$"#)]
async fn sam_unsubscribed_via_member_invite_link(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam has unsubscribed via a member-invite link");
}

#[given(regex = r#"^Sam is unsubscribed from every workspace he belongs to$"#)]
async fn sam_unsubscribed_from_every_workspace(_world: &mut FoundryWorld) {
    pending!("Sam is unsubscribed from every workspace he belongs to");
}

#[given(regex = r#"^Sam's unsubscribe link for "([^"]+)" has a tampered token$"#)]
async fn sam_link_has_tampered_token(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam's unsubscribe link has a tampered token");
}

#[given(regex = r#"^an unsubscribe request for a real recipient carries an invalid token$"#)]
async fn unsubscribe_request_real_recipient_invalid_token(_world: &mut FoundryWorld) {
    pending!("an unsubscribe request for a real recipient carries an invalid token");
}

#[given(regex = r#"^an unsubscribe request for a non-existent address carries an invalid token$"#)]
async fn unsubscribe_request_nonexistent_invalid_token(_world: &mut FoundryWorld) {
    pending!("an unsubscribe request for a non-existent address carries an invalid token");
}

#[given(regex = r#"^Sam has a valid unsubscribe link for "([^"]+)" he has not confirmed$"#)]
async fn sam_has_valid_unconfirmed_link(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam has a valid unsubscribe link he has not confirmed");
}

#[given(regex = r#"^Maria is signed in and belongs to "([^"]+)", "([^"]+)", and "([^"]+)"$"#)]
async fn maria_signed_in_belongs_to(
    _world: &mut FoundryWorld,
    _ws_a: String,
    _ws_b: String,
    _ws_c: String,
) {
    pending!("Maria is signed in and belongs to three workspaces");
}

#[given(regex = r#"^Maria has confirmed unsubscribing from "([^"]+)"$"#)]
async fn maria_has_confirmed_unsubscribing_from(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Maria has confirmed unsubscribing from a workspace");
}

#[given(
    regex = r#"^several suppressible deliveries to unsubscribed recipients have been suppressed$"#
)]
async fn several_suppressible_deliveries_suppressed(_world: &mut FoundryWorld) {
    pending!("several suppressible deliveries to unsubscribed recipients have been suppressed");
}

#[given(regex = r#"^Olivia boots Foundry with recipient unsubscribe enabled$"#)]
async fn olivia_boots_foundry_with_unsubscribe(_world: &mut FoundryWorld) {
    pending!("Olivia boots Foundry with recipient unsubscribe enabled");
}

#[given(
    regex = r#"^a workspace "([^"]+)" with an unsubscribed recipient is scheduled for deletion$"#
)]
async fn workspace_with_unsubscribed_recipient_scheduled_for_deletion(
    _world: &mut FoundryWorld,
    _workspace: String,
) {
    pending!("a workspace with an unsubscribed recipient is scheduled for deletion");
}

#[given(regex = r#"^the suppression lookup is failing$"#)]
async fn the_suppression_lookup_is_failing(_world: &mut FoundryWorld) {
    pending!("the suppression lookup is failing");
}

#[given(regex = r#"^the suppression lookup is slow$"#)]
async fn the_suppression_lookup_is_slow(_world: &mut FoundryWorld) {
    pending!("the suppression lookup is slow");
}

// ============================================================================
// When — driving-port actions (the link, the GET/POST routes, real emit flows,
// the signed-in page, the /metrics scrape)
// ============================================================================

#[when(regex = r#"^Sam opens the unsubscribe link and confirms unsubscribing from "([^"]+)"$"#)]
async fn sam_opens_link_and_confirms_unsubscribe(world: &mut FoundryWorld, _workspace: String) {
    let base = {
        let harness = world.harness.as_ref().expect("harness");
        harness.base_url()
    };
    let http = world.http.as_ref().expect("http").clone();
    let t = world.unsub_t.clone().expect("link minted in Given");
    let sig = world.unsub_sig.clone().expect("link minted in Given");

    // (1) GET the confirm page (NON-DESTRUCTIVE) — mints the double-submit CSRF
    // cookie the POST re-submits.
    let get_url = format!(
        "{base}/unsubscribe?t={}&sig={}",
        urlencoding::encode(&t),
        urlencoding::encode(&sig),
    );
    let csrf = csrf_from_get(&http, &get_url).await;

    // (2) POST the confirm — CSRF-screened by the shipped middleware — which writes
    // the opt-out row.
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("t", t);
    form.insert("sig", sig);
    form.insert("action", "unsubscribe".to_string());
    form.insert("_csrf", csrf.clone());
    let resp = http
        .post(format!("{base}/unsubscribe"))
        .header(reqwest::header::COOKIE, format!("foundry_csrf={csrf}"))
        .form(&form)
        .send()
        .await
        .expect("POST /unsubscribe confirm");
    world.last_status = Some(resp.status());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^a workspace-invite for "([^"]+)" is issued to Sam$"#)]
async fn workspace_invite_issued_to_sam(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a workspace-invite is issued to Sam");
}

#[when(regex = r#"^a member-invite for "([^"]+)" is issued to Sam$"#)]
async fn member_invite_issued_to_sam(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a member-invite is issued to Sam");
}

#[when(regex = r#"^Sam confirms unsubscribing from "([^"]+)" a second time$"#)]
async fn sam_confirms_unsubscribing_second_time(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam confirms unsubscribing a second time");
}

#[when(regex = r#"^Sam requests a password reset$"#)]
async fn sam_requests_password_reset(_world: &mut FoundryWorld) {
    pending!("Sam requests a password reset");
}

#[when(regex = r#"^an admin removes Sam from "([^"]+)"$"#)]
async fn admin_removes_sam_from(_world: &mut FoundryWorld, _workspace: String) {
    pending!("an admin removes Sam from a workspace");
}

#[when(regex = r#"^a password reset, a password change, and a removal each fire for Sam$"#)]
async fn all_three_mandatory_events_fire_for_sam(_world: &mut FoundryWorld) {
    pending!("a password reset, a password change, and a removal each fire for Sam");
}

#[when(regex = r#"^the tampered unsubscribe link is opened$"#)]
async fn the_tampered_link_is_opened(_world: &mut FoundryWorld) {
    pending!("the tampered unsubscribe link is opened");
}

#[when(regex = r#"^both unsubscribe links are opened$"#)]
async fn both_unsubscribe_links_are_opened(_world: &mut FoundryWorld) {
    pending!("both unsubscribe links are opened");
}

#[when(regex = r#"^an automated client fetches the unsubscribe link without confirming$"#)]
async fn automated_client_fetches_link_without_confirming(_world: &mut FoundryWorld) {
    pending!("an automated client fetches the unsubscribe link without confirming");
}

#[when(regex = r#"^the unsubscribe confirm is posted without a valid CSRF token$"#)]
async fn unsubscribe_confirm_posted_without_csrf(_world: &mut FoundryWorld) {
    pending!("the unsubscribe confirm is posted without a valid CSRF token");
}

#[when(regex = r#"^Maria opens the notification settings page$"#)]
async fn maria_opens_notification_settings_page(_world: &mut FoundryWorld) {
    pending!("Maria opens the notification settings page");
}

#[when(regex = r#"^a request attempts to view notification status for another recipient's email$"#)]
async fn request_attempts_other_recipient_status(_world: &mut FoundryWorld) {
    pending!("a request attempts to view notification status for another recipient's email");
}

#[when(regex = r#"^Maria resubscribes to "([^"]+)"$"#)]
async fn maria_resubscribes_to(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Maria resubscribes to a workspace");
}

#[when(regex = r#"^Maria submits a resubscribe for "([^"]+)" twice from a stale page$"#)]
async fn maria_submits_resubscribe_twice_from_stale_page(
    _world: &mut FoundryWorld,
    _workspace: String,
) {
    pending!("Maria submits a resubscribe twice from a stale page");
}

#[when(
    regex = r#"^a cross-site request attempts to resubscribe Maria to "([^"]+)" without a valid CSRF token$"#
)]
async fn cross_site_resubscribe_without_csrf(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a cross-site request attempts to resubscribe Maria without a valid CSRF token");
}

#[when(regex = r#"^Sam opens his unsubscribe link and confirms resubscribing to "([^"]+)"$"#)]
async fn sam_opens_link_and_confirms_resubscribe(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam opens his unsubscribe link and confirms resubscribing");
}

#[when(regex = r#"^Olivia scrapes the metrics endpoint$"#)]
async fn olivia_scrapes_metrics_endpoint(_world: &mut FoundryWorld) {
    pending!("Olivia scrapes the metrics endpoint");
}

#[when(regex = r#"^the "([^"]+)" workspace is deleted$"#)]
async fn the_workspace_is_deleted(_world: &mut FoundryWorld, _workspace: String) {
    pending!("the workspace is deleted");
}

// ============================================================================
// Then — observable outcomes at the driving ports (confirmation page, the
// recording double, the /metrics sidecar, the settings page)
// ============================================================================

#[then(regex = r#"^Sam sees a confirmation that "([^"]+)" invitations are stopped$"#)]
async fn sam_sees_confirmation_invitations_stopped(world: &mut FoundryWorld, workspace: String) {
    let status = world.last_status.expect("a confirm was posted");
    assert!(
        status.is_success(),
        "the unsubscribe confirm must succeed, got {status}"
    );
    let body = world.last_body.as_deref().expect("a confirm body");
    assert!(
        body.contains(&workspace),
        "the confirmation must name the workspace {workspace:?}: {body}"
    );
    assert!(
        body.contains("invitations are stopped"),
        "the confirmation must state invitations are stopped: {body}"
    );
}

#[then(regex = r#"^a subsequent workspace-invite for Sam from "([^"]+)" is not delivered$"#)]
async fn subsequent_workspace_invite_not_delivered(world: &mut FoundryWorld, _workspace: String) {
    // Drive the REAL shipped issuance (bootstrap::create_invite, POST /invites): sign
    // the seeded admin in and POST an invite for Sam. The handler emits ONE
    // `WorkspaceInvite` for Sam with `workspace_id: Some(Northwind)` through
    // `notify()`, where the suppression gate must intercept it before fan-out.
    let (admin_email, admin_pw) = world.unsub_admin.clone().expect("admin seeded");
    let outcome = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &admin_email,
            &admin_pw,
            "/invites",
            &[("email", SAM_EMAIL)],
        )
        .await
    };
    assert!(
        outcome.status.is_success() || outcome.status.is_redirection(),
        "the invite must be issued (2xx/3xx), got {}: {}",
        outcome.status,
        outcome.body
    );
    // The suppressed workspace_invite must have reached NO provider — the recording
    // double observes zero delivery for Sam. Reverting the suppression gate (delivering
    // it) re-REDs this.
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| d.event == "workspace_invite" && d.to == SAM_EMAIL)
        .collect();
    assert!(
        delivered.is_empty(),
        "an unsubscribed recipient's workspace_invite must not be delivered, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^one suppression is counted for the "([^"]+)" event$"#)]
async fn one_suppression_counted_for_event(world: &mut FoundryWorld, event: String) {
    assert_eq!(
        event, "workspace_invite",
        "the walking skeleton suppresses a workspace_invite"
    );
    // Observed at the SuppressionPolicy driven-port boundary: exactly ONE suppression
    // decision for the Northwind workspace (the only suppressible emit fired). The
    // production `foundry_notification_suppressions_total{event}` counter increments
    // in lockstep inside `notify()`; the register-at-0 /metrics scrape lands in US-07.
    let workspace_id = world.unsub_workspace_id.expect("workspace id captured");
    let harness = world.harness.as_ref().expect("harness");
    let count = harness.suppressions.count_for_workspace(workspace_id);
    assert_eq!(
        count, 1,
        "exactly one suppression must be counted for the workspace_invite, got {count}"
    );
}

#[then(regex = r#"^the "([^"]+)" invitation is delivered to Sam$"#)]
async fn the_invitation_is_delivered_to_sam(_world: &mut FoundryWorld, _workspace: String) {
    pending!("the invitation is delivered to Sam");
}

#[then(regex = r#"^Sam sees that he is already unsubscribed from "([^"]+)"$"#)]
async fn sam_sees_already_unsubscribed(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam sees that he is already unsubscribed");
}

#[then(regex = r#"^Sam sees the same confirmation both times with no error$"#)]
async fn sam_sees_same_confirmation_both_times(_world: &mut FoundryWorld) {
    pending!("Sam sees the same confirmation both times with no error");
}

#[then(regex = r#"^the workspace-invite for Sam from "([^"]+)" is delivered unchanged$"#)]
async fn workspace_invite_delivered_unchanged(_world: &mut FoundryWorld, _workspace: String) {
    pending!("the workspace-invite for Sam is delivered unchanged");
}

#[then(regex = r#"^the password-reset notification is delivered to Sam$"#)]
async fn password_reset_delivered_to_sam(_world: &mut FoundryWorld) {
    pending!("the password-reset notification is delivered to Sam");
}

#[then(regex = r#"^it is not counted as suppressed$"#)]
async fn it_is_not_counted_as_suppressed(_world: &mut FoundryWorld) {
    pending!("it is not counted as suppressed");
}

#[then(regex = r#"^the member-removed notification is delivered to Sam$"#)]
async fn member_removed_delivered_to_sam(_world: &mut FoundryWorld) {
    pending!("the member-removed notification is delivered to Sam");
}

#[then(regex = r#"^every one of those notifications is delivered$"#)]
async fn every_one_of_those_notifications_delivered(_world: &mut FoundryWorld) {
    pending!("every one of those notifications is delivered");
}

#[then(regex = r#"^none of them is counted as suppressed$"#)]
async fn none_of_them_counted_as_suppressed(_world: &mut FoundryWorld) {
    pending!("none of them is counted as suppressed");
}

#[then(regex = r#"^the uniform non-enumerable refusal page is shown$"#)]
async fn uniform_non_enumerable_refusal_shown(_world: &mut FoundryWorld) {
    pending!("the uniform non-enumerable refusal page is shown");
}

#[then(regex = r#"^no unsubscribe is recorded$"#)]
async fn no_unsubscribe_is_recorded(_world: &mut FoundryWorld) {
    pending!("no unsubscribe is recorded");
}

#[then(regex = r#"^both requests return a byte-identical refusal$"#)]
async fn both_requests_return_byte_identical_refusal(_world: &mut FoundryWorld) {
    pending!("both requests return a byte-identical refusal");
}

#[then(regex = r#"^neither response reveals whether the address, workspace, or account exists$"#)]
async fn neither_response_reveals_existence(_world: &mut FoundryWorld) {
    pending!("neither response reveals whether the address, workspace, or account exists");
}

#[then(regex = r#"^a subsequent workspace-invite to Sam in "([^"]+)" is still delivered$"#)]
async fn subsequent_workspace_invite_to_sam_still_delivered(
    _world: &mut FoundryWorld,
    _workspace: String,
) {
    pending!("a subsequent workspace-invite to Sam is still delivered");
}

#[then(regex = r#"^Sam remains subscribed to "([^"]+)" until he explicitly confirms$"#)]
async fn sam_remains_subscribed_until_confirms(_world: &mut FoundryWorld, _workspace: String) {
    pending!("Sam remains subscribed until he explicitly confirms");
}

#[then(regex = r#"^the confirm is refused and no opt-out state changes$"#)]
async fn confirm_refused_no_state_change(_world: &mut FoundryWorld) {
    pending!("the confirm is refused and no opt-out state changes");
}

#[then(regex = r#"^no unsubscribe token or recipient email appears in the logs$"#)]
async fn no_token_or_email_in_logs(_world: &mut FoundryWorld) {
    pending!("no unsubscribe token or recipient email appears in the logs");
}

#[then(regex = r#"^the member-invite for Sam from "([^"]+)" is not delivered$"#)]
async fn member_invite_for_sam_not_delivered(_world: &mut FoundryWorld, _workspace: String) {
    pending!("the member-invite for Sam is not delivered");
}

#[then(regex = r#"^both member-invite and workspace-invite emails from "([^"]+)" are suppressed$"#)]
async fn both_invite_events_suppressed(_world: &mut FoundryWorld, _workspace: String) {
    pending!("both member-invite and workspace-invite emails are suppressed");
}

#[then(regex = r#"^"([^"]+)" is shown as muted$"#)]
async fn workspace_shown_as_muted(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a workspace is shown as muted");
}

#[then(regex = r#"^"([^"]+)" is shown as subscribed$"#)]
async fn workspace_shown_as_subscribed(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a workspace is shown as subscribed");
}

#[then(regex = r#"^only Maria's own status is returned$"#)]
async fn only_marias_own_status_returned(_world: &mut FoundryWorld) {
    pending!("only Maria's own status is returned");
}

#[then(regex = r#"^only workspaces Maria belongs to are listed$"#)]
async fn only_marias_workspaces_listed(_world: &mut FoundryWorld) {
    pending!("only workspaces Maria belongs to are listed");
}

#[then(regex = r#"^"([^"]+)" is shown as subscribed again$"#)]
async fn workspace_shown_as_subscribed_again(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a workspace is shown as subscribed again");
}

#[then(regex = r#"^a subsequent invitation for "([^"]+)" is delivered to Maria$"#)]
async fn subsequent_invitation_delivered_to_maria(_world: &mut FoundryWorld, _workspace: String) {
    pending!("a subsequent invitation is delivered to Maria");
}

#[then(regex = r#"^Maria sees the same resubscribe confirmation both times with no error$"#)]
async fn maria_sees_same_resubscribe_confirmation_both_times(_world: &mut FoundryWorld) {
    pending!("Maria sees the same resubscribe confirmation both times with no error");
}

#[then(regex = r#"^the resubscribe is refused and Maria's subscription state is unchanged$"#)]
async fn resubscribe_refused_state_unchanged(_world: &mut FoundryWorld) {
    pending!("the resubscribe is refused and Maria's subscription state is unchanged");
}

#[then(regex = r#"^a subsequent workspace-invite for Sam from "([^"]+)" is delivered to Sam$"#)]
async fn subsequent_workspace_invite_delivered_to_sam(
    _world: &mut FoundryWorld,
    _workspace: String,
) {
    pending!("a subsequent workspace-invite for Sam is delivered to Sam");
}

#[then(regex = r#"^a suppression count is present split by event$"#)]
async fn suppression_count_present_split_by_event(_world: &mut FoundryWorld) {
    pending!("a suppression count is present split by event");
}

#[then(regex = r#"^the counts reflect how many suppressible deliveries were suppressed$"#)]
async fn counts_reflect_suppressed_deliveries(_world: &mut FoundryWorld) {
    pending!("the counts reflect how many suppressible deliveries were suppressed");
}

#[then(regex = r#"^no recipient email or unsubscribe token appears in any metric label or line$"#)]
async fn no_pii_in_metrics(_world: &mut FoundryWorld) {
    pending!("no recipient email or unsubscribe token appears in any metric label or line");
}

#[then(regex = r#"^the suppression metric is registered at zero for every event$"#)]
async fn suppression_metric_registered_at_zero(_world: &mut FoundryWorld) {
    pending!("the suppression metric is registered at zero for every event");
}

#[then(regex = r#"^the suppressed count for every mandatory event is zero$"#)]
async fn suppressed_count_for_mandatory_is_zero(_world: &mut FoundryWorld) {
    pending!("the suppressed count for every mandatory event is zero");
}

#[then(regex = r#"^deleting the workspace succeeds$"#)]
async fn deleting_the_workspace_succeeds(_world: &mut FoundryWorld) {
    pending!("deleting the workspace succeeds");
}

#[then(regex = r#"^a previously-unsubscribed recipient of that workspace resumes delivery$"#)]
async fn previously_unsubscribed_recipient_resumes_delivery(_world: &mut FoundryWorld) {
    pending!("a previously-unsubscribed recipient of that workspace resumes delivery");
}

#[then(regex = r#"^no orphaned suppression state remains$"#)]
async fn no_orphaned_suppression_state(_world: &mut FoundryWorld) {
    pending!("no orphaned suppression state remains");
}

#[then(regex = r#"^the emit completes without stalling$"#)]
async fn the_emit_completes_without_stalling(_world: &mut FoundryWorld) {
    pending!("the emit completes without stalling");
}
