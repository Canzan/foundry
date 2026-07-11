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

use crate::support::harness::{fresh_schema_pool_with_url, signed_in_post, InProcHarness};
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
use std::time::Instant;

/// Fixed scenario clock anchor (mirrors the other in-process step modules).
const NDP_NOW: &str = "2026-01-15T12:00:00Z";

/// Test `SESSION_SECRET` handed to the `foundry` startup subprocess (fail-fast
/// scenario). Its VALUE is asserted absent from the refusal output (no-leak).
const NDP_SESSION_SECRET: &str = "ndp-test-session-secret-must-be-at-least-32-bytes-long-yes";

/// Distinctive SMTP password used by the no-leak litmus scenario: it must never
/// surface in any recorded field, error, or debug output across a delivery cycle.
const NDP_SMTP_PASSWORD_SENTINEL: &str = "ndp-smtp-password-must-never-leak-9f3a";

/// The seeded member's known password, so the fan-out scenarios can sign her in
/// (as a workspace admin) to drive the REAL bootstrap + member-invite issuance
/// flows — the shipped call sites that each emit ONE notification through
/// `notify()`. Mirrors the seed in [`seed_workspace_and_member`].
const NDP_MEMBER_PASSWORD: &str = "ndp-correct-horse-battery-staple";

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
        .map(provider_kind_from_name)
        .collect()
}

/// Map one operator provider name to its [`ProviderKind`].
fn provider_kind_from_name(name: &str) -> ProviderKind {
    match name.trim() {
        "log" => ProviderKind::Log,
        "smtp" => ProviderKind::Smtp,
        "webhook" => ProviderKind::Webhook,
        "email_api" => ProviderKind::EmailApi,
        other => panic!("unknown provider kind in scenario config: {other}"),
    }
}

/// Seed the Background workspace + member (the notification recipient) so
/// `POST /forgot-password` resolves a real user and emits a `PasswordReset`.
async fn seed_workspace_and_member(harness: &InProcHarness, workspace: &str, member_email: &str) {
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let lower = member_email.to_ascii_lowercase();
    let hash =
        foundry_auth::hash_password(&SecretString::new(NDP_MEMBER_PASSWORD.to_string().into()))
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
        // Seed as `admin` so the fan-out scenarios can sign her in to drive the
        // admin-gated member-invite issuance flow (the bootstrap-invite flow needs
        // only a session). The password-reset scenarios are role-agnostic.
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
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
    world: &mut FoundryWorld,
    provider: String,
    setting: String,
) {
    // Stash the provider + the setting the operator omitted; the "Foundry starts
    // up" step lists the provider for the real `foundry` subprocess with that one
    // setting removed from its env, so `build_notifier` fails fast naming both.
    world.ndp_missing_setting = Some((provider, setting));
}

#[given(regex = r#"^the "([^"]+)" provider's endpoint is unreachable$"#)]
async fn provider_endpoint_unreachable(world: &mut FoundryWorld, provider: String) {
    // The harness was spawned by the preceding "activated providers" Given; mark
    // this provider's recording double as unreachable so a delivery through it is
    // recorded `failed` and returns a transient error the notifier contains.
    let kind = provider_kind_from_name(&provider);
    world
        .harness
        .as_ref()
        .expect("the providers were activated (harness spawned) before this step")
        .fake_email
        .set_unreachable(kind);
}

#[given(regex = r#"^the "([^"]+)" provider's endpoint hangs on connect$"#)]
async fn provider_endpoint_hangs(world: &mut FoundryWorld, provider: String) {
    // Mark this provider's recording double as hanging: a delivery through it
    // records `outcome=failed` (timeout) then blocks past the notifier's
    // per-provider timeout, so the concurrent fan-out contains the stall.
    let kind = provider_kind_from_name(&provider);
    world
        .harness
        .as_ref()
        .expect("the providers were activated (harness spawned) before this step")
        .fake_email
        .set_slow(kind);
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

/// Drive the real shipped `POST /forgot-password` flow: GET the form to mint the
/// double-submit CSRF cookie/token, then POST it. The handler (signin.rs) emits
/// ONE `PasswordReset` notification through `notifier.notify()`. Returns
/// `(status, body, elapsed_ms)` — the elapsed wall-clock of the POST alone, so a
/// caller can assert the request was not stalled by a slow provider.
async fn post_forgot_password(
    harness: &InProcHarness,
    http: &reqwest::Client,
    email: &str,
) -> (StatusCode, String, u128) {
    let base = harness.base_url();
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
    let token = raw
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let cookie_header = format!("foundry_csrf={token}");
    let mut form = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("_csrf", token);
    let started = Instant::now();
    let resp = http
        .post(format!("{base}/forgot-password"))
        .header(reqwest::header::COOKIE, cookie_header)
        .form(&form)
        .send()
        .await
        .expect("post forgot-password");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body, started.elapsed().as_millis())
}

#[when(regex = r#"^a member requests a password reset for "([^"]+)"$"#)]
async fn member_requests_password_reset_for(world: &mut FoundryWorld, email: String) {
    let (status, body, elapsed) = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        post_forgot_password(harness, http, &email).await
    };
    world.last_status = Some(status);
    world.last_body = Some(body);
    world.ndp_request_elapsed_ms = Some(elapsed);
}

#[when(regex = r#"^Foundry starts up$"#)]
async fn foundry_starts_up(world: &mut FoundryWorld) {
    // Drive the REAL composition root (driving port 1): spawn the shipped
    // `foundry` binary with the operator's provider selection and capture its
    // startup outcome. `build_notifier()` validates the list against the bounded
    // ProviderKind set and per-provider required settings, aborting on an unknown
    // name OR a missing required setting (ADR-002).
    let outcome = if let Some(unknown) = world.ndp_unknown_provider.clone() {
        // Unknown-provider case: list the typo'd name; the notifier bails naming
        // it and the known set before any per-provider settings are read.
        spawn_foundry_expecting_refuse_to_start(&unknown, &[]).await
    } else if let Some((provider, setting)) = world.ndp_missing_setting.clone() {
        // Missing-setting case: list a KNOWN provider but ensure the named
        // required setting is absent from the subprocess env, so the notifier
        // fails fast naming the provider AND the missing key.
        spawn_foundry_expecting_refuse_to_start(&provider, &[setting.as_str()]).await
    } else {
        panic!("a startup precondition Given must run before 'Foundry starts up'");
    };
    world.ndp_startup_outcome = Some(outcome);
}

/// Spawn the shipped `foundry` binary with `NOTIFICATION_PROVIDERS=<providers>`
/// against a fresh migrated schema, and wait for it to exit. `build_notifier()`
/// runs at AppState construction — BEFORE the metrics sidecar binds — so an
/// ephemeral `METRICS_PORT=0` isolates the provider-config refusal as the sole
/// failure. Any key in `remove_settings` is stripped from the subprocess env so
/// the missing-required-setting fail-fast can be provoked deterministically
/// (the parent env is otherwise inherited). Returns `(exit_code, stdout, stderr)`.
async fn spawn_foundry_expecting_refuse_to_start(
    providers: &str,
    remove_settings: &[&str],
) -> (Option<i32>, String, String) {
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
    for key in remove_settings {
        cmd.env_remove(key);
    }
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
async fn bootstrap_invite_issued_for(world: &mut FoundryWorld, email: String) {
    // Drive the REAL shipped issuance (bootstrap::create_invite, POST /invites):
    // sign the seeded member in and POST the invite form. The handler emits ONE
    // `WorkspaceInvite` notification to the invitee through `notify()`.
    let member = world
        .ndp_member
        .clone()
        .expect("Background seeded a member");
    let outcome = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &member,
            NDP_MEMBER_PASSWORD,
            "/invites",
            &[("email", email.as_str())],
        )
        .await
    };
    assert!(
        outcome.status.is_success() || outcome.status.is_redirection(),
        "the bootstrap invite must be issued (2xx/3xx), got {}: {}",
        outcome.status,
        outcome.body
    );
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
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
async fn all_three_existing_notifications_fire(world: &mut FoundryWorld) {
    // Fire all THREE shipped notification call sites, each emitting ONE
    // notification through `notify()`: the forgot-password flow (password_reset),
    // the bootstrap issuance (workspace_invite), and the admin-gated member
    // issuance (member_invite). Each fans out to every active provider.
    let member = world
        .ndp_member
        .clone()
        .expect("Background seeded a member");

    // 1. password_reset — POST /forgot-password.
    let (status, body, elapsed) = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        post_forgot_password(harness, http, &member).await
    };
    world.last_status = Some(status);
    world.last_body = Some(body);
    world.ndp_request_elapsed_ms = Some(elapsed);

    // 2. workspace_invite — POST /invites (signed-in bootstrap issuance).
    let bootstrap = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &member,
            NDP_MEMBER_PASSWORD,
            "/invites",
            &[("email", "invitee-bootstrap@acme.example")],
        )
        .await
    };
    assert!(
        bootstrap.status.is_success() || bootstrap.status.is_redirection(),
        "the bootstrap invite must be issued, got {}: {}",
        bootstrap.status,
        bootstrap.body
    );

    // 3. member_invite — POST /workspace/invites (admin-gated member issuance).
    let member_invite = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &member,
            NDP_MEMBER_PASSWORD,
            "/workspace/invites",
            &[("email", "invitee-member@acme.example")],
        )
        .await
    };
    assert!(
        member_invite.status.is_success() || member_invite.status.is_redirection(),
        "the member invite must be issued, got {}: {}",
        member_invite.status,
        member_invite.body
    );
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

/// The three shipped notification events that fan out through the abstraction.
const NDP_EXISTING_EVENTS: [&str; 3] = ["password_reset", "workspace_invite", "member_invite"];

#[then(regex = r#"^each notification is delivered through the "([^"]+)" provider$"#)]
async fn each_notification_delivered_through(world: &mut FoundryWorld, provider: String) {
    // Every one of the three emitted notifications reached this provider exactly
    // once with outcome `delivered` (fan-out completeness through the abstraction).
    let harness = world.harness.as_ref().expect("harness");
    for event in NDP_EXISTING_EVENTS {
        let count = harness.fake_email.recorded(&provider, event, "delivered");
        assert_eq!(
            count, 1,
            "each notification must be delivered through the {provider:?} provider \
             (event {event}), got {count}"
        );
    }
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
async fn each_delivery_recorded_per_provider_and_event(world: &mut FoundryWorld) {
    // The bounded cross-product of active providers × the three events is each
    // recorded exactly once with outcome `delivered` (per-provider, per-event
    // observability — the metric-emit contract at the recorder boundary).
    let harness = world.harness.as_ref().expect("harness");
    for provider in ["log", "smtp"] {
        for event in NDP_EXISTING_EVENTS {
            let count = harness.fake_email.recorded(provider, event, "delivered");
            assert_eq!(
                count, 1,
                "delivery must be recorded once for provider={provider} event={event} \
                 outcome=delivered, got {count}"
            );
        }
    }
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
async fn request_returns_without_waiting(world: &mut FoundryWorld) {
    // Normal 200 response …
    let status = world.last_status.expect("a request was made");
    assert_eq!(
        status,
        StatusCode::OK,
        "forgot-password must return its normal 200 response, got {status}"
    );
    // … and it did NOT stall on the hanging provider. The slow double blocks 5s;
    // the concurrent fan-out bounds the emit path to ~one per-provider timeout
    // (the harness sets 500ms), so the whole request completes well under that
    // block. Reverting the timeout (awaiting the hang) re-REDs this.
    let elapsed = world
        .ndp_request_elapsed_ms
        .expect("the request timing was captured");
    assert!(
        elapsed < 3000,
        "the request must not stall on the slow provider (await-bounded to ~one \
         timeout window), took {elapsed}ms"
    );
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
    world: &mut FoundryWorld,
    provider: String,
    setting: String,
) {
    let (_, stdout, stderr) = world
        .ndp_startup_outcome
        .as_ref()
        .expect("the startup subprocess outcome was captured");
    let haystack = format!("{stdout}\n{stderr}");
    assert!(
        haystack.contains(&provider),
        "startup error must name provider {provider:?}.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        haystack.contains(&setting),
        "startup error must name the missing setting {setting:?}.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
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
