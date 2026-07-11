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

use crate::support::harness::{fresh_schema_pool_with_url, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_app::{
    LogProvider, Notification, NotificationEvent, NotificationProvider, ProviderKind, SmtpConfig,
    SmtpProvider,
};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::collections::HashMap;

/// Fixed scenario clock anchor (mirrors the other in-process step modules).
const NDP_NOW: &str = "2026-01-15T12:00:00Z";

/// Test `SESSION_SECRET` handed to the `foundry` startup subprocess (fail-fast
/// scenario). Its VALUE is asserted absent from the refusal output (no-leak).
const NDP_SESSION_SECRET: &str = "ndp-test-session-secret-must-be-at-least-32-bytes-long-yes";

/// Distinctive SMTP password used by the no-leak litmus scenario: it must never
/// surface in any recorded field, error, or debug output across a delivery cycle.
const NDP_SMTP_PASSWORD_SENTINEL: &str = "ndp-smtp-password-must-never-leak-9f3a";

/// Valid Ed25519 test public key (same literal as the slice-8 subprocess seam)
/// so the startup subprocess gets past machine-token verifier construction and
/// reaches `build_notifier`, where the unknown-provider refusal fires.
const NDP_MACHINE_TOKEN_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----\\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\\n-----END PUBLIC KEY-----";

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
async fn operator_activated_no_providers(world: &mut FoundryWorld) {
    // Unset/empty NOTIFICATION_PROVIDERS ⇒ zero active providers (Noop-
    // equivalent, BR-1/NFR-5). Spawn the harness with an EMPTY provider set so
    // `notify()` fans out to nobody — a delivery is a silent drop.
    let harness = InProcHarness::spawn_with_providers(now_anchor(), &[]).await;
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

#[given(regex = r#"^the operator has listed an unknown provider "([^"]+)"$"#)]
async fn operator_listed_unknown_provider(world: &mut FoundryWorld, name: String) {
    // Stash the typo'd channel name; the `When Foundry starts up` step hands it
    // to the real `foundry` subprocess as NOTIFICATION_PROVIDERS.
    world.ndp_unknown_provider = Some(name);
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
async fn foundry_starts_up(world: &mut FoundryWorld) {
    // Drive the REAL composition root (driving port 1): spawn the shipped
    // `foundry` binary with the operator's provider selection and capture its
    // startup outcome. `build_notifier()` validates the list against the bounded
    // ProviderKind set and aborts on an unknown name (ADR-002).
    let providers = world
        .ndp_unknown_provider
        .clone()
        .expect("a provider selection was listed by the prior Given");
    let outcome = spawn_foundry_expecting_refuse_to_start(&providers).await;
    world.ndp_startup_outcome = Some(outcome);
}

/// Spawn the shipped `foundry` binary with `NOTIFICATION_PROVIDERS=<providers>`
/// against a fresh migrated schema, and wait for it to exit. `build_notifier()`
/// runs at AppState construction — BEFORE the metrics sidecar binds — so an
/// ephemeral `METRICS_PORT=0` isolates the provider-config refusal as the sole
/// failure. Returns `(exit_code, stdout, stderr)`.
async fn spawn_foundry_expecting_refuse_to_start(providers: &str) -> (Option<i32>, String, String) {
    use std::process::Stdio;
    use std::time::Duration;
    use tokio::process::Command;

    // A fresh, already-migrated per-scenario schema so the boot reaches
    // `build_notifier` (the ONLY intended failure) rather than a DB/migration
    // error. Drop the helper pool — the subprocess opens its own.
    let (schema, pool, url) = fresh_schema_pool_with_url().await;
    pool.close().await;

    let binary_path = assert_cmd::cargo::cargo_bin("foundry");
    let mut cmd = Command::new(&binary_path);
    cmd.env("DATABASE_URL", &url)
        .env("NOTIFICATION_PROVIDERS", providers)
        .env("METRICS_PORT", "0")
        .env("FOUNDRY_PORT", "0")
        .env("METRICS_HOST", "127.0.0.1")
        .env("FOUNDRY_HOST", "127.0.0.1")
        .env("SESSION_SECRET", NDP_SESSION_SECRET)
        .env("MACHINE_TOKEN_PUBLIC_KEYS", NDP_MACHINE_TOKEN_PUBLIC_KEY)
        .env("SESSION_COOKIE_SECURE", "false")
        .env("FOUNDRY_DB_SCHEMA", &schema)
        .env("FOUNDRY_SKIP_MIGRATIONS", "1")
        .env("RUST_LOG", "info,foundry=info,sqlx=warn")
        .env("RUST_LOG_FORMAT", "pretty")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn().expect("spawn foundry startup subprocess");
    let output = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .expect("foundry startup subprocess did not exit within 30s")
        .expect("collect foundry startup subprocess output");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
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
async fn no_notification_delivered(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let deliveries = harness.fake_email.sent();
    assert!(
        deliveries.is_empty(),
        "with no providers active, zero deliveries must be recorded, got {}: {deliveries:?}",
        deliveries.len()
    );
}

#[then(regex = r#"^no error is raised$"#)]
async fn no_error_is_raised(world: &mut FoundryWorld) {
    // Best-effort no-op delivery raises nothing: the request completed with its
    // normal 200 and no failed delivery was recorded (nothing errored).
    let status = world.last_status.expect("a request was made");
    assert_eq!(
        status,
        StatusCode::OK,
        "the no-op delivery path must raise nothing — the request stays 200, got {status}"
    );
    let harness = world.harness.as_ref().expect("harness");
    let failures = harness
        .fake_email
        .recorded("log", "password_reset", "failed")
        + harness
            .fake_email
            .sent()
            .iter()
            .filter(|d| d.outcome == "failed")
            .count();
    assert_eq!(
        failures, 0,
        "no delivery failure must be recorded on the no-op path"
    );
}

#[then(regex = r#"^no delivery is attempted through the "([^"]+)" provider$"#)]
async fn no_delivery_attempted_through(world: &mut FoundryWorld, provider: String) {
    // With smtp inactive (only "log" active), the smtp provider is never wired,
    // so ZERO deliveries and ZERO recorded attempts (delivered OR failed) exist
    // for it — the inactive channel was neither constructed nor called (NFR-5).
    let harness = world.harness.as_ref().expect("harness");
    let delivered = harness.fake_email.delivered_through(&provider);
    let attempted = harness
        .fake_email
        .sent()
        .iter()
        .filter(|d| d.provider == provider)
        .count();
    assert_eq!(
        delivered, 0,
        "no delivery must occur through the inactive {provider:?} provider, got {delivered}"
    );
    assert_eq!(
        attempted, 0,
        "no attempt (delivered or failed) may be recorded for the inactive {provider:?} \
         provider, got {attempted}"
    );
}

#[then(regex = r#"^the existing notification behavior is unchanged$"#)]
async fn existing_behavior_unchanged(world: &mut FoundryWorld) {
    // Backwards-compat (NFR-5): the still-active "log" channel delivered the
    // password_reset exactly as it did before smtp existed, and the originating
    // request returned its normal 200.
    let status = world.last_status.expect("a request was made");
    assert_eq!(
        status,
        StatusCode::OK,
        "the request must return its normal 200 response, got {status}"
    );
    let harness = world.harness.as_ref().expect("harness");
    let log_delivered = harness
        .fake_email
        .recorded("log", "password_reset", "delivered");
    assert_eq!(
        log_delivered, 1,
        "the existing log delivery must be unchanged (exactly one log password_reset \
         delivered), got {log_delivered}"
    );
}

#[then(regex = r#"^startup is refused and the process exits non-zero$"#)]
async fn startup_refused_nonzero(world: &mut FoundryWorld) {
    let (code, stdout, stderr) = world
        .ndp_startup_outcome
        .as_ref()
        .expect("the startup subprocess outcome was captured");
    match code {
        Some(c) => assert_ne!(
            *c, 0,
            "startup must be refused with a non-zero exit, got {c}.\n\
             stdout:\n{stdout}\nstderr:\n{stderr}"
        ),
        None => panic!(
            "startup subprocess did not exit (no code) — expected refuse-to-start.\n\
             stdout:\n{stdout}\nstderr:\n{stderr}"
        ),
    }
}

#[then(
    regex = r#"^the startup error names the unknown provider "([^"]+)" and the known providers$"#
)]
async fn startup_error_names_unknown_and_known(world: &mut FoundryWorld, name: String) {
    let (_, stdout, stderr) = world
        .ndp_startup_outcome
        .as_ref()
        .expect("the startup subprocess outcome was captured");
    let haystack = format!("{stdout}\n{stderr}");
    assert!(
        haystack.contains(&name),
        "startup error must name the unknown provider {name:?}.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    for known in ["log", "smtp", "webhook", "email_api"] {
        assert!(
            haystack.contains(known),
            "startup error must name the known provider {known:?}.\n\
             stdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
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
async fn startup_error_no_secret(world: &mut FoundryWorld) {
    let (_, stdout, stderr) = world
        .ndp_startup_outcome
        .as_ref()
        .expect("the startup subprocess outcome was captured");
    let haystack = format!("{stdout}\n{stderr}");
    // The startup env carries a SESSION_SECRET; the refusal must never echo its
    // value (NFR-2, ADR-006 — nothing secret in logs/errors/debug output).
    assert!(
        !haystack.contains(NDP_SESSION_SECRET),
        "the startup refusal must not echo any secret value.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[then(
    regex = r#"^the "([^"]+)" value never appears in any log, error, metric label, or debug output$"#
)]
async fn secret_value_never_appears(world: &mut FoundryWorld, secret_key: String) {
    assert_eq!(
        secret_key, "SMTP_PASSWORD",
        "slice 02 covers the SMTP_PASSWORD no-leak litmus"
    );
    let harness = world.harness.as_ref().expect("harness");
    // The full delivery cycle completed through the smtp channel.
    let delivery = harness
        .fake_email
        .sent()
        .into_iter()
        .find(|d| d.provider == "smtp" && d.event == "password_reset")
        .expect("an smtp delivery for password_reset was recorded");

    // Layer 1 — no observable recorded field carries the password.
    for field in [
        &delivery.to,
        &delivery.subject,
        &delivery.body,
        &delivery.provider,
        &delivery.event,
        &delivery.outcome,
    ] {
        assert!(
            !field.contains(NDP_SMTP_PASSWORD_SENTINEL),
            "no recorded delivery field may carry the SMTP password: {field}"
        );
    }

    // Layers 2-5 — drive the SHIPPED SmtpProvider built WITH the sentinel
    // password (matching 01-01's pattern of asserting the real production
    // adapter): the SecretString redacts on Debug, the provider is not Debug,
    // and a genuine DeliveryError from a closed relay is hand-built + secret-free
    // (ADR-006). Reverting any layer re-REDs this.
    let config = SmtpConfig::from_lookup(|key| match key {
        "SMTP_HOST" => Some("127.0.0.1".to_string()),
        "SMTP_PORT" => Some("1".to_string()),
        "SMTP_USERNAME" => Some("mailer".to_string()),
        "SMTP_PASSWORD" => Some(NDP_SMTP_PASSWORD_SENTINEL.to_string()),
        "SMTP_FROM" => Some("noreply@acme.example".to_string()),
        _ => None,
    })
    .expect("sentinel smtp config parses");
    let provider = SmtpProvider::new(config).expect("smtp provider builds");
    let notification = Notification {
        event: NotificationEvent::PasswordReset,
        recipient: delivery.to.clone(),
        subject: delivery.subject.clone(),
        body: delivery.body.clone(),
    };
    let err = provider
        .deliver(&notification)
        .await
        .expect_err("a closed relay must fail the delivery");
    let rendered = format!("{err} || {err:?}");
    assert!(
        !rendered.contains(NDP_SMTP_PASSWORD_SENTINEL),
        "the SMTP password must never appear in any error or debug output: {rendered}"
    );
}

#[then(regex = r#"^no reset token appears in the delivery log line$"#)]
async fn no_reset_token_in_log_line(world: &mut FoundryWorld) {
    // The reset token rides in the notification BODY (`.../reset-password?token=
    // <raw>`). The `log` channel's line keys strictly on provider/event/recipient
    // and never interpolates the token (ADR-006). Reconstruct the delivered
    // notification and assert the SHIPPED `LogProvider::log_line` — the exact
    // string the real adapter prints — leaks neither the token nor the body.
    let harness = world.harness.as_ref().expect("harness");
    let delivery = harness
        .fake_email
        .sent()
        .into_iter()
        .find(|d| d.provider == "log" && d.event == "password_reset")
        .expect("a log delivery for password_reset was recorded");
    let token = delivery
        .body
        .split("token=")
        .nth(1)
        .and_then(|rest| rest.split(['\n', ' ']).next())
        .map(str::to_string)
        .filter(|t| !t.is_empty())
        .expect("the reset body carries a non-empty token= parameter");

    let notification = Notification {
        event: NotificationEvent::PasswordReset,
        recipient: delivery.to.clone(),
        subject: delivery.subject.clone(),
        body: delivery.body.clone(),
    };
    let line = LogProvider::log_line(&notification);
    assert!(
        line.contains(&delivery.to),
        "the delivery log line must key on the recipient: {line}"
    );
    assert!(
        !line.contains(&token),
        "the reset token must never appear in the delivery log line: {line}"
    );
    assert!(
        !line.contains(&delivery.body),
        "the notification body must never appear in the delivery log line: {line}"
    );
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
