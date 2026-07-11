//! notification-delivery-providers — step definitions for the config-selected
//! NotificationProvider registry + concurrent best-effort fan-out dispatcher.
//!
//! HARNESS BOUNDARY (distill/acceptance-review.md): the app + Postgres are REAL
//! (the shipped in-process axum harness + testcontainers, `@real-io`), mirroring
//! how `FakeEmailSender` is wired today (`support::harness::InProcHarness`). The
//! EXTERNAL transports are IN-PROCESS TEST DOUBLES — a recording log provider, a
//! local webhook receiver, and a fake SMTP / hosted-API recorder — so NO real
//! third-party SMTP/SendGrid/webhook call ever leaves the test process. The
//! notifier is driven through THREE driving ports (Mandate 1), never an internal
//! function:
//!   1. Operator config at the composition root (`NOTIFICATION_PROVIDERS` +
//!      per-provider `SMTP_*`/`WEBHOOK_*`/`EMAIL_API_*`), loaded by the
//!      `build_notifier()` seam — the fail-fast-on-unknown/misconfigured entry.
//!   2. A real shipped app flow — `POST /forgot-password` (signin.rs:235), the
//!      bootstrap + member invites, remove-member, and password-change — each
//!      emitting ONE notification through `notify()`.
//!   3. The `/metrics` sidecar + the recording-provider double (the observable side).
//!
//! SCAFFOLD STATUS (Mandate 7 — RED-ready, not BROKEN): every scenario in the
//! feature file is `@pending` (excluded from all lanes), and the production seams
//! this feature introduces — the `NotificationProvider` port, the registry loader,
//! the `Notifier` dispatcher, the four adapters, and the delivery-metric seam — do
//! NOT exist yet. Each step body below is therefore a compiling scaffold that
//! `panic!`s (an assertion-class failure = RED, never an ImportError-class BROKEN).
//! DELIVER removes `@pending` slice-by-slice and replaces each stub with a body
//! that wires the real harness seam it builds (a `spawn_with_providers`-style
//! composition root + the recording doubles), turning the scenario GREEN.
//!
//! __SCAFFOLD__
//! SCAFFOLD: true
//!
//! Every phrase below is globally unique (notification-domain wording — verified
//! against the other step modules; cucumber-rs would ambiguously match otherwise).
//! The Background workspace/member seed is a NEW notification-specific Given rather
//! than a reuse of the `FakeEmailSender`-only board/dashboard Backgrounds, because
//! this feature's whole subject is REPLACING that single hard-wired sender with the
//! config-built registry — DELIVER's harness seam must own the provider wiring.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_app::ProviderKind;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::collections::HashMap;

/// Fixed scenario clock anchor (mirrors the other in-process step modules).
const NDP_NOW: &str = "2026-01-15T12:00:00Z";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(NDP_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

/// Map the operator's comma-separated `NOTIFICATION_PROVIDERS` list to the
/// provider kinds the harness wires as recording doubles.
fn parse_provider_kinds(csv: &str) -> Vec<ProviderKind> {
    csv.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| match name {
            "log" => ProviderKind::Log,
            "smtp" => ProviderKind::Smtp,
            "webhook" => ProviderKind::Webhook,
            "email_api" => ProviderKind::EmailApi,
            other => panic!("unknown provider kind in scenario config: {other}"),
        })
        .collect()
}

/// Seed the Background workspace + member (the notification recipient) so
/// `POST /forgot-password` resolves a real user and emits a `PasswordReset`.
async fn seed_workspace_and_member(harness: &InProcHarness, workspace: &str, member_email: &str) {
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let lower = member_email.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&SecretString::new(
        "ndp-correct-horse-battery-staple".to_string().into(),
    ))
    .await
    .expect("hash member pw");
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
    .bind(member_email)
    .bind("Maria Santos")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert workspace membership");
}

/// RED-ready scaffold sentinel. A `panic!` is classified RED (implementation
/// missing, test correct), NOT BROKEN (infrastructure/import error) — so once
/// DELIVER removes `@pending`, the scenario is a genuine failing outer-loop test
/// awaiting the production seam. `slice` names the DELIVER slice that unskips it.
fn pending(slice: &str) -> ! {
    panic!(
        "@pending notification-delivery-providers ({slice}) — DELIVER implements this step \
         against the harness provider seam it builds (recording doubles + build_notifier)"
    );
}

// ============================================================================
// Background
// ============================================================================

#[given(regex = r#"^Foundry is serving workspace "([^"]+)" with member "([^"]+)"$"#)]
async fn foundry_serving_workspace_with_member(
    world: &mut FoundryWorld,
    workspace: String,
    member_email: String,
) {
    // Stash the seed; the harness is spawned once the operator's provider
    // selection is known (the following Given), so the app carries the
    // config-selected notifier before the recipient is seeded into its DB.
    world.ndp_workspace = Some(workspace);
    world.ndp_member = Some(member_email);
}

// ============================================================================
// Given — operator config (driving port 1: the composition-root registry loader)
// ============================================================================

#[given(regex = r#"^the operator has activated providers "([^"]+)"$"#)]
async fn operator_activated_providers(world: &mut FoundryWorld, providers_csv: String) {
    let kinds = parse_provider_kinds(&providers_csv);
    let harness = InProcHarness::spawn_with_providers(now_anchor(), &kinds).await;
    let workspace = world
        .ndp_workspace
        .clone()
        .expect("Background seeded a workspace");
    let member = world
        .ndp_member
        .clone()
        .expect("Background seeded a member");
    seed_workspace_and_member(&harness, &workspace, &member).await;
    world.harness = Some(harness);
    world.http = Some(client());
}

#[given(regex = r#"^the operator has activated no providers$"#)]
async fn operator_activated_no_providers(_world: &mut FoundryWorld) {
    pending("slice 01 — unset NOTIFICATION_PROVIDERS (Noop-equivalent)");
}

#[given(regex = r#"^the operator has listed an unknown provider "([^"]+)"$"#)]
async fn operator_listed_unknown_provider(_world: &mut FoundryWorld, _name: String) {
    pending("slice 01 — unknown-name fail-fast");
}

#[given(
    regex = r#"^the operator has listed provider "([^"]+)" without required setting "([^"]+)"$"#
)]
async fn operator_listed_provider_without_setting(
    _world: &mut FoundryWorld,
    _provider: String,
    _setting: String,
) {
    pending("slice 02/04/05 — misconfigured-provider fail-fast");
}

#[given(regex = r#"^the "([^"]+)" provider's endpoint is unreachable$"#)]
async fn provider_endpoint_unreachable(_world: &mut FoundryWorld, _provider: String) {
    pending("slice 02/03/06 — unreachable transport double");
}

#[given(regex = r#"^the "([^"]+)" provider's endpoint hangs on connect$"#)]
async fn provider_endpoint_hangs(_world: &mut FoundryWorld, _provider: String) {
    pending("slice 03 — slow/hanging transport double (timeout containment)");
}

#[given(regex = r#"^the "([^"]+)" endpoint rejects the delivery$"#)]
async fn provider_endpoint_rejects(_world: &mut FoundryWorld, _provider: String) {
    pending("slice 04/05 — receiver rejects (non-2xx) transport double");
}

#[given(regex = r#"^the "([^"]+)" provider is configured with a signing secret$"#)]
async fn webhook_configured_with_signing_secret(_world: &mut FoundryWorld, _provider: String) {
    pending("slice 04 — WEBHOOK_SIGNING_SECRET set");
}

// ============================================================================
// When — real shipped app flows (driving port 2) + startup (driving port 1)
// ============================================================================

#[when(regex = r#"^a member requests a password reset for "([^"]+)"$"#)]
async fn member_requests_password_reset_for(world: &mut FoundryWorld, email: String) {
    // Drive the real shipped flow: GET /forgot-password to mint the double-submit
    // CSRF cookie/token, then POST the form. The handler (signin.rs) emits ONE
    // PasswordReset notification through `notifier.notify()`.
    let base;
    let token;
    let cookie_header;
    {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        base = harness.base_url();
        let get = http
            .get(format!("{base}/forgot-password"))
            .send()
            .await
            .expect("get forgot-password form for csrf");
        let raw = get
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|s| s.starts_with("foundry_csrf="))
            .map(|s| s.to_string())
            .expect("forgot-password GET must mint a foundry_csrf cookie");
        token = raw
            .strip_prefix("foundry_csrf=")
            .and_then(|rest| rest.split(';').next())
            .unwrap_or("")
            .to_string();
        cookie_header = format!("foundry_csrf={token}");
    }
    let http = world.http.as_ref().expect("http");
    let mut form = HashMap::new();
    form.insert("email", email);
    form.insert("_csrf", token);
    let resp = http
        .post(format!("{base}/forgot-password"))
        .header(reqwest::header::COOKIE, cookie_header)
        .form(&form)
        .send()
        .await
        .expect("post forgot-password");
    world.last_status = Some(resp.status());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^Foundry starts up$"#)]
async fn foundry_starts_up(_world: &mut FoundryWorld) {
    pending("slice 01+ — build_notifier() at the composition root");
}

#[when(regex = r#"^a bootstrap workspace invite is issued for "([^"]+)"$"#)]
async fn bootstrap_invite_issued_for(_world: &mut FoundryWorld, _email: String) {
    pending("slice 03 — bootstrap.rs:258 emits workspace_invite");
}

#[when(regex = r#"^a member invite is issued for "([^"]+)"$"#)]
async fn member_invite_issued_for(_world: &mut FoundryWorld, _email: String) {
    pending("slice 03/04 — member_invites.rs:189 emits member_invite");
}

#[when(regex = r#"^an admin removes member "([^"]+)" from "([^"]+)"$"#)]
async fn admin_removes_member_from(_world: &mut FoundryWorld, _email: String, _workspace: String) {
    pending("slice 06 — remove-member handler emits member_removed");
}

#[when(regex = r#"^member "([^"]+)" changes their password$"#)]
async fn member_changes_password(_world: &mut FoundryWorld, _email: String) {
    pending("slice 06 — password-change handler emits password_changed");
}

#[when(regex = r#"^a password reset, a bootstrap invite, and a member invite each fire$"#)]
async fn all_three_existing_notifications_fire(_world: &mut FoundryWorld) {
    pending("slice 03 — all three shipped call sites routed through notify()");
}

// ============================================================================
// Then — observable outcomes (driving port 3: recorder + /metrics sidecar)
// ============================================================================

#[then(regex = r#"^the notification is delivered through the "([^"]+)" provider$"#)]
async fn notification_delivered_through(world: &mut FoundryWorld, provider: String) {
    let harness = world.harness.as_ref().expect("harness");
    let count = harness.fake_email.delivered_through(&provider);
    assert!(
        count >= 1,
        "expected at least one delivery through the {provider:?} provider, got {count}"
    );
}

#[then(regex = r#"^each notification is delivered through the "([^"]+)" provider$"#)]
async fn each_notification_delivered_through(_world: &mut FoundryWorld, _provider: String) {
    pending("slice 03 — every emitted notification reached this provider");
}

#[then(
    regex = r#"^the delivery is recorded for provider "([^"]+)", event "([^"]+)", outcome "([^"]+)"$"#
)]
async fn delivery_recorded_for(
    world: &mut FoundryWorld,
    provider: String,
    event: String,
    outcome: String,
) {
    let harness = world.harness.as_ref().expect("harness");
    let count = harness.fake_email.recorded(&provider, &event, &outcome);
    assert_eq!(
        count, 1,
        "expected exactly one delivery recorded for provider={provider} event={event} \
         outcome={outcome}, got {count}"
    );
}

#[then(regex = r#"^each delivery is recorded per provider and event$"#)]
async fn each_delivery_recorded_per_provider_and_event(_world: &mut FoundryWorld) {
    pending("slice 03 — N providers × M notifications counted split by outcome");
}

#[then(regex = r#"^the request returns its normal response$"#)]
async fn request_returns_normal_response(world: &mut FoundryWorld) {
    let status = world.last_status.expect("a request was made");
    assert_eq!(
        status,
        StatusCode::OK,
        "forgot-password must return its normal 200 response (best-effort delivery is \
         non-fatal), got {status}"
    );
}

#[then(regex = r#"^the request returns its normal response without waiting on the slow provider$"#)]
async fn request_returns_without_waiting(_world: &mut FoundryWorld) {
    pending("slice 03 — await-bounded fan-out: no stall on a hanging provider");
}

#[then(regex = r#"^no notification is delivered$"#)]
async fn no_notification_delivered(_world: &mut FoundryWorld) {
    pending("slice 01 — unset providers ⇒ zero deliveries");
}

#[then(regex = r#"^no error is raised$"#)]
async fn no_error_is_raised(_world: &mut FoundryWorld) {
    pending("slice 01 — no-op delivery raises nothing");
}

#[then(regex = r#"^no delivery is attempted through the "([^"]+)" provider$"#)]
async fn no_delivery_attempted_through(_world: &mut FoundryWorld, _provider: String) {
    pending("slice 02 — inactive provider is never constructed nor called");
}

#[then(regex = r#"^the existing notification behavior is unchanged$"#)]
async fn existing_behavior_unchanged(_world: &mut FoundryWorld) {
    pending("slice 02 — backwards-compat regression (NFR-5)");
}

#[then(regex = r#"^startup is refused and the process exits non-zero$"#)]
async fn startup_refused_nonzero(_world: &mut FoundryWorld) {
    pending("slice 01/02/04/05 — build_notifier() aborts, non-zero exit");
}

#[then(
    regex = r#"^the startup error names the unknown provider "([^"]+)" and the known providers$"#
)]
async fn startup_error_names_unknown_and_known(_world: &mut FoundryWorld, _name: String) {
    pending("slice 01 — error names the typo + the known set");
}

#[then(regex = r#"^the startup error names provider "([^"]+)" and the missing setting "([^"]+)"$"#)]
async fn startup_error_names_provider_and_missing_setting(
    _world: &mut FoundryWorld,
    _provider: String,
    _setting: String,
) {
    pending("slice 02/04/05 — error names the provider + the missing config key");
}

#[then(regex = r#"^the startup error contains no secret value$"#)]
async fn startup_error_no_secret(_world: &mut FoundryWorld) {
    pending("slice 01/02 — secret-free operator error (NFR-2)");
}

#[then(
    regex = r#"^the "([^"]+)" value never appears in any log, error, metric label, or debug output$"#
)]
async fn secret_value_never_appears(_world: &mut FoundryWorld, _secret_key: String) {
    pending("slice 02/04/05 — five-layer no-leak litmus (SecretString + no-Debug port)");
}

#[then(regex = r#"^no reset token appears in the delivery log line$"#)]
async fn no_reset_token_in_log_line(_world: &mut FoundryWorld) {
    pending("slice 01 — log provider keys on provider/event/recipient, not the token");
}

#[then(regex = r#"^a JSON payload describing the event is posted to the webhook endpoint$"#)]
async fn json_payload_posted_to_webhook(_world: &mut FoundryWorld) {
    pending("slice 04 — local webhook receiver observed a real POST body");
}

#[then(regex = r#"^the webhook probe made no post to the receiver$"#)]
async fn webhook_probe_made_no_post(_world: &mut FoundryWorld) {
    pending("slice 04 — probe() is host-reachability only, NO POST (N-ODD-3)");
}

#[then(regex = r#"^the delivery carries a signature header derived from the secret$"#)]
async fn delivery_carries_signature_header(_world: &mut FoundryWorld) {
    pending("slice 04 — HMAC signature header present on the POST");
}

#[then(regex = r#"^no automatic retry is attempted$"#)]
async fn no_automatic_retry(_world: &mut FoundryWorld) {
    pending("slice 05 — best-effort at-most-once, no retry in v1 (NFR-6)");
}

#[then(regex = r#"^the other active providers still deliver$"#)]
async fn other_providers_still_deliver(_world: &mut FoundryWorld) {
    pending("slice 04/05 — per-provider isolation: siblings unaffected");
}

#[then(regex = r#"^the delivery metric labels stay within their bounded sets$"#)]
async fn metric_labels_bounded(_world: &mut FoundryWorld) {
    pending("slice 03/06 — {provider,event,outcome} values stay in their closed domains");
}

#[then(regex = r#"^a cardinality check fails closed on an unbounded label value$"#)]
async fn cardinality_fails_closed(_world: &mut FoundryWorld) {
    pending("slice 03/06 — mirrored fail-closed cardinality guard (ADR-011)");
}

#[then(
    regex = r#"^the delivery metric is present on the metrics endpoint with every series at zero$"#
)]
async fn metric_present_zero_series(_world: &mut FoundryWorld) {
    pending("slice 03 — register-at-0 cross-product on first /metrics scrape");
}
