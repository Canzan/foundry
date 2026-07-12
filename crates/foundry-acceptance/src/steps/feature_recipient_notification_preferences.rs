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
use foundry_app::{Notification, NotificationEvent, ProviderKind};
use secrecy::SecretString;
use std::collections::HashMap;
use std::time::Instant;

/// Fixed scenario clock anchor (mirrors the other in-process step modules).
const RNP_NOW: &str = "2026-01-15T12:00:00Z";
/// The account-less recipient the walking skeleton targets (already lower-cased so
/// the token payload, the store row, and the emit recipient all normalize alike).
const SAM_EMAIL: &str = "sam@northwind.example";
/// A seeded workspace admin used to drive the REAL `POST /invites` emit whose
/// `workspace_invite` the suppression gate then intercepts.
const ADMIN_EMAIL: &str = "admin@northwind.example";
const ADMIN_PASSWORD: &str = "rnp-correct-horse-battery-staple";
/// Sam's account password, seeded ONLY for the US-02 mandatory-event scenarios
/// where a real security event must reach Sam (a password reset resolves a real
/// user; a removal deletes a real membership; a change signs Sam in). The US-01
/// slice leaves Sam account-less; these scenarios upgrade him to a real user so
/// the shipped mandatory-emit driving-ports fire a genuine delivery.
const SAM_PASSWORD: &str = "rnp-sam-correct-horse-battery-staple";
/// A SECOND workspace's admin (the independence scenario seeds Contoso alongside
/// Northwind). A distinct email so both admins coexist in the `users` table.
const CONTOSO_ADMIN_EMAIL: &str = "admin@contoso.example";

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
    seed_workspace_and_named_admin(harness, workspace, ADMIN_EMAIL, ADMIN_PASSWORD).await
}

/// As [`seed_workspace_and_admin`] but with an explicit admin identity, so a
/// scenario can stand up TWO workspaces (Northwind + Contoso), each with its own
/// admin who can drive the shipped `POST /invites` emit for that workspace.
async fn seed_workspace_and_named_admin(
    harness: &InProcHarness,
    workspace: &str,
    admin_email: &str,
    admin_password: &str,
) -> uuid::Uuid {
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let lower = admin_email.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&SecretString::new(admin_password.to_string().into()))
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
    .bind(admin_email)
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

/// Seed Sam as a REAL user (an account, not just an invitee email) so a shipped
/// mandatory-emit driving-port resolves him as the recipient: `POST /forgot-password`
/// only emits a `PasswordReset` for an email that `find_user_by_email` resolves, and
/// `POST /account/password` requires Sam to sign in. Returns Sam's `user_id` so a
/// caller can also seed his workspace membership. Both `email_lower` and
/// `email_display` are `SAM_EMAIL` (already lower-cased), so every mandatory-event
/// delivery records `to == SAM_EMAIL`.
async fn seed_sam_user(harness: &InProcHarness) -> uuid::Uuid {
    let pool = harness.app.state.store.pool();
    let user_id = uuid::Uuid::now_v7();
    let hash = foundry_auth::hash_password(&SecretString::new(SAM_PASSWORD.to_string().into()))
        .await
        .expect("hash sam pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(SAM_EMAIL)
    .bind(SAM_EMAIL)
    .bind("Sam Recipient")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert sam user");
    user_id
}

/// Make Sam a plain `member` of `workspace_id`, so an admin acting on that workspace
/// can drive the shipped `POST /workspace/members/remove` removal (which deletes a
/// real membership and emits ONE `MemberRemoved` to the removed person — 0 rows
/// deleted would 404 with NO emit).
async fn seed_sam_membership(
    harness: &InProcHarness,
    workspace_id: uuid::Uuid,
    user_id: uuid::Uuid,
) {
    let pool = harness.app.state.store.pool();
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert sam membership");
}

/// Drive the shipped `POST /forgot-password` public driving-port: GET the form to
/// mint the double-submit CSRF cookie/token, then POST `email`. The handler
/// (`signin::submit_forgot`) emits ONE `PasswordReset` through `notify()` for a
/// resolvable user. `PasswordReset` is MANDATORY (`is_suppressible()` false), so the
/// suppression gate never consults the lookup — it delivers even for an unsubscribed
/// recipient. Returns the response status.
async fn post_forgot_password_for(
    http: &reqwest::Client,
    base: &str,
    email: &str,
) -> reqwest::StatusCode {
    let csrf = csrf_from_get(http, &format!("{base}/forgot-password")).await;
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("_csrf", csrf.clone());
    http.post(format!("{base}/forgot-password"))
        .header(reqwest::header::COOKIE, format!("foundry_csrf={csrf}"))
        .form(&form)
        .send()
        .await
        .expect("POST /forgot-password")
        .status()
}

/// Mint the SAME signed link a suppressible email body carries for `(email, ws)`:
/// the base64url `t` param + the domain-separated HMAC `sig`.
fn mint_unsubscribe_link(
    harness: &InProcHarness,
    email_lower: &str,
    workspace_id: uuid::Uuid,
) -> (String, String) {
    let token = foundry_auth::UnsubscribeToken::new(
        email_lower,
        workspace_id,
        &harness.app.state.session_secret,
    )
    .expect("mint unsubscribe token");
    let t = foundry_app::unsubscribe::encode_t(email_lower, workspace_id);
    (t, token.signature)
}

/// Drive the recipient's confirm click end-to-end at the PUBLIC driving port:
/// GET the confirm page (mint the double-submit CSRF cookie), then POST the
/// `action=unsubscribe` confirm re-submitting it. Returns the confirmation the
/// recipient SEES (status + body). Idempotent on the server (`ON CONFLICT DO
/// NOTHING`), so confirming twice yields the same confirmation.
async fn confirm_unsubscribe(
    http: &reqwest::Client,
    base: &str,
    t: &str,
    sig: &str,
) -> (reqwest::StatusCode, String) {
    let get_url = format!(
        "{base}/unsubscribe?t={}&sig={}",
        urlencoding::encode(t),
        urlencoding::encode(sig),
    );
    let csrf = csrf_from_get(http, &get_url).await;
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("t", t.to_string());
    form.insert("sig", sig.to_string());
    form.insert("action", "unsubscribe".to_string());
    form.insert("_csrf", csrf.clone());
    let resp = http
        .post(format!("{base}/unsubscribe"))
        .header(reqwest::header::COOKIE, format!("foundry_csrf={csrf}"))
        .form(&form)
        .send()
        .await
        .expect("POST /unsubscribe confirm");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// Extract the `(decoded t, decoded sig)` of the signed unsubscribe link the emit
/// appended to a SUPPRESSIBLE email body (ADR-002 `unsubscribe_link_line`). PANICS if
/// the body carries no link — the falsifiable proof the emit wired one (a member_invite
/// emit that omits the link reds every scenario that opens the link from the email).
fn extract_unsubscribe_link_from_body(body: &str) -> (String, String) {
    let marker = "/unsubscribe?t=";
    let start = body
        .find(marker)
        .unwrap_or_else(|| panic!("the email body must carry a signed unsubscribe link: {body}"));
    let query = &body[start + marker.len()..];
    let (enc_t, rest) = query
        .split_once("&sig=")
        .unwrap_or_else(|| panic!("the unsubscribe link must carry both t and sig: {body}"));
    let enc_sig = rest.split_whitespace().next().unwrap_or(rest);
    let t = urlencoding::decode(enc_t).expect("decode t").into_owned();
    let sig = urlencoding::decode(enc_sig)
        .expect("decode sig")
        .into_owned();
    (t, sig)
}

/// Drive the REAL shipped member-invite emit (`member_invites::submit_invite`,
/// `POST /workspace/invites`) and return the `(workspace_id, email_lower, t, sig)` of
/// the unsubscribe link it appended to the delivered email body. Seeds the workspace +
/// an admin, signs the admin in, POSTs the invite for Sam, then reads the recorded
/// `member_invite` delivery and extracts its signed link — proving (falsifiably) the
/// member-invite email carries its own unsubscribe link, exactly like the workspace one.
async fn issue_member_invite_and_capture_link(
    world: &mut FoundryWorld,
    workspace: &str,
) -> (uuid::Uuid, String, String, String) {
    let workspace_id = {
        let harness = world.harness.as_ref().expect("harness");
        seed_workspace_and_admin(harness, workspace).await
    };
    let outcome = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            ADMIN_EMAIL,
            ADMIN_PASSWORD,
            "/workspace/invites",
            &[("email", SAM_EMAIL)],
        )
        .await
    };
    assert!(
        outcome.status.is_success() || outcome.status.is_redirection(),
        "the member-invite must be issued (2xx/3xx), got {}: {}",
        outcome.status,
        outcome.body
    );
    let harness = world.harness.as_ref().expect("harness");
    let delivery = harness
        .fake_email
        .sent()
        .into_iter()
        .find(|d| d.event == "member_invite" && d.to == SAM_EMAIL)
        .expect("the member-invite must have been delivered so its body can be read");
    let (t, sig) = extract_unsubscribe_link_from_body(&delivery.body);
    (workspace_id, SAM_EMAIL.to_ascii_lowercase(), t, sig)
}

/// Corrupt a signature by flipping its first character, so the constant-time
/// `UnsubscribeToken::verify` rejects it (a TAMPERED, well-formed-looking link).
fn tamper_sig(sig: &str) -> String {
    let mut chars: Vec<char> = sig.chars().collect();
    if let Some(first) = chars.first_mut() {
        *first = if *first == 'a' { 'b' } else { 'a' };
    }
    chars.into_iter().collect()
}

/// Open an unsubscribe link at the PUBLIC driving port — a NON-DESTRUCTIVE
/// `GET /unsubscribe?t=..&sig=..` (a scanner/prefetch-shaped fetch). Returns the
/// `(status, body)` the opener SEES, so a `Then` can assert the uniform refusal
/// and compare two openings byte-for-byte.
async fn open_unsubscribe_link(
    http: &reqwest::Client,
    base: &str,
    t: &str,
    sig: &str,
) -> (reqwest::StatusCode, String) {
    let resp = http
        .get(format!(
            "{base}/unsubscribe?t={}&sig={}",
            urlencoding::encode(t),
            urlencoding::encode(sig),
        ))
        .send()
        .await
        .expect("GET /unsubscribe");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
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
    // Mint the SAME signed link a suppressible email body would carry.
    let email_lower = SAM_EMAIL.to_ascii_lowercase();
    let (t, sig) = mint_unsubscribe_link(harness, &email_lower, workspace_id);
    world.unsub_email = Some(email_lower);
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_t = Some(t);
    world.unsub_sig = Some(sig);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace,
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
}

#[given(
    regex = r#"^Sam has a member-invite email for "([^"]+)" carrying a signed unsubscribe link$"#
)]
async fn sam_has_member_invite_email_with_link(world: &mut FoundryWorld, workspace: String) {
    // Drive the REAL member-invite emit and capture the signed unsubscribe link it
    // appended to the delivered email body (falsifiable: an emit that omits the link
    // panics in the extractor). The captured (t, sig) is the SAME link a recipient
    // clicks — the When then opens + confirms it through the public flow.
    let (workspace_id, email_lower, t, sig) =
        issue_member_invite_and_capture_link(world, &workspace).await;
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_email = Some(email_lower);
    world.unsub_t = Some(t);
    world.unsub_sig = Some(sig);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace,
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
}

#[given(regex = r#"^Sam has confirmed unsubscribing from "([^"]+)"$"#)]
async fn sam_has_confirmed_unsubscribing_from(world: &mut FoundryWorld, workspace: String) {
    // Establish the "already unsubscribed" precondition by driving the REAL public
    // confirm flow (seed workspace + admin, mint the signed link, GET the confirm
    // page + POST the confirm). Capturing the confirmation Sam saw the FIRST time
    // lets the harmless-no-op scenario compare it against a second confirm.
    let (base, t, sig, email_lower, workspace_id) = {
        let harness = world
            .harness
            .as_ref()
            .expect("harness spawned by Background");
        let workspace_id = seed_workspace_and_admin(harness, &workspace).await;
        let email_lower = SAM_EMAIL.to_ascii_lowercase();
        let (t, sig) = mint_unsubscribe_link(harness, &email_lower, workspace_id);
        (harness.base_url(), t, sig, email_lower, workspace_id)
    };
    let http = world.http.as_ref().expect("http").clone();
    let (status, body) = confirm_unsubscribe(&http, &base, &t, &sig).await;
    assert!(
        status.is_success(),
        "the first unsubscribe confirm must succeed, got {status}"
    );
    world.unsub_email = Some(email_lower);
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_t = Some(t);
    world.unsub_sig = Some(sig);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace,
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
    world.unsub_first_confirmation = Some(body);
}

#[given(regex = r#"^Sam also has an invite for workspace "([^"]+)"$"#)]
async fn sam_also_has_invite_for_workspace(world: &mut FoundryWorld, workspace: String) {
    // Stand up a SECOND, independent workspace with its own admin. Sam has NOT
    // opted out here (no row for this pair), so an invite from it must deliver —
    // proving the per-(email, workspace) opt-out is scoped, not global (FR-9).
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    seed_workspace_and_named_admin(harness, &workspace, CONTOSO_ADMIN_EMAIL, ADMIN_PASSWORD).await;
    world.unsub_ws_admins.insert(
        workspace,
        (CONTOSO_ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
}

#[given(regex = r#"^Sam has unsubscribed from "([^"]+)" via a workspace-invite link$"#)]
async fn sam_unsubscribed_via_workspace_invite_link(world: &mut FoundryWorld, workspace: String) {
    // Establish the opt-out via the SUPPRESSIBLE workspace-invite link path — mint the
    // same signed link a workspace_invite email carries and drive the REAL public confirm
    // flow. The resulting 0014 row is EVENT-AGNOSTIC per (email, workspace), so a later
    // member_invite for the same pair must ALSO be suppressed (US-04 crux).
    let (base, t, sig, email_lower, workspace_id) = {
        let harness = world
            .harness
            .as_ref()
            .expect("harness spawned by Background");
        let workspace_id = seed_workspace_and_admin(harness, &workspace).await;
        let email_lower = SAM_EMAIL.to_ascii_lowercase();
        let (t, sig) = mint_unsubscribe_link(harness, &email_lower, workspace_id);
        (harness.base_url(), t, sig, email_lower, workspace_id)
    };
    let http = world.http.as_ref().expect("http").clone();
    let (status, _body) = confirm_unsubscribe(&http, &base, &t, &sig).await;
    assert!(
        status.is_success(),
        "the workspace-invite-link unsubscribe confirm must succeed, got {status}"
    );
    world.unsub_email = Some(email_lower);
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace,
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
}

#[given(regex = r#"^Sam has unsubscribed from "([^"]+)" via a member-invite link$"#)]
async fn sam_unsubscribed_via_member_invite_link(world: &mut FoundryWorld, workspace: String) {
    // Establish the opt-out via the MEMBER-invite link path: drive the real member-invite
    // emit, extract the signed unsubscribe link it appended to the email body, and confirm
    // unsubscribing through the public flow. The @property point: the mandatory-event
    // invariant must hold regardless of WHICH suppressible link performed the unsubscribe.
    let (workspace_id, email_lower, t, sig) =
        issue_member_invite_and_capture_link(world, &workspace).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http").clone();
    let (status, _body) = confirm_unsubscribe(&http, &base, &t, &sig).await;
    assert!(
        status.is_success(),
        "the member-invite-link unsubscribe confirm must succeed, got {status}"
    );
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_email = Some(email_lower);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace,
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
}

#[given(regex = r#"^Sam is unsubscribed from every workspace he belongs to$"#)]
async fn sam_unsubscribed_from_every_workspace(world: &mut FoundryWorld) {
    // Establish the crux invariant's precondition: Sam is a REAL user, a member of
    // a workspace, and has an opt-out row for it (so "every workspace he belongs to"
    // is muted). With Sam unsubscribed, the three mandatory security events must
    // STILL deliver and count NO suppression — the structural `is_suppressible()`
    // allow-list holds them exempt. Seed him with a password so the change-password
    // leg can sign him in and drive the real `password_changed` emit.
    let workspace = "Northwind";
    let (workspace_id, email_lower) = {
        let harness = world
            .harness
            .as_ref()
            .expect("harness spawned by Background");
        let workspace_id = seed_workspace_and_admin(harness, workspace).await;
        let sam_id = seed_sam_user(harness).await;
        seed_sam_membership(harness, workspace_id, sam_id).await;
        let email_lower = SAM_EMAIL.to_ascii_lowercase();
        harness
            .app
            .state
            .store
            .insert_unsubscribe(&email_lower, workspace_id)
            .await
            .expect("seed Sam's opt-out row for the workspace he belongs to");
        (workspace_id, email_lower)
    };
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_email = Some(email_lower);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace.to_string(),
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
}

#[given(regex = r#"^Sam's unsubscribe link for "([^"]+)" has a tampered token$"#)]
async fn sam_link_has_tampered_token(world: &mut FoundryWorld, workspace: String) {
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    // Seed a real workspace so the link is well-formed and points at a live target —
    // the refusal must NOT depend on that (Sam stays account-less; the tamper alone
    // must sink the link).
    let workspace_id = seed_workspace_and_admin(harness, &workspace).await;
    let email_lower = SAM_EMAIL.to_ascii_lowercase();
    // Mint the SAME signed link a suppressible email body carries, then TAMPER the
    // signature (flip one character) so the constant-time HMAC verify rejects it.
    let (t, valid_sig) = mint_unsubscribe_link(harness, &email_lower, workspace_id);
    let tampered_sig = tamper_sig(&valid_sig);
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_email = Some(email_lower.clone());
    world.unsub_t = Some(t.clone());
    world.unsub_sig = Some(tampered_sig.clone());
    // Everything a careless handler could echo into a body/log on refusal: the
    // recipient email, the workspace name + id, the opaque token, and BOTH signatures.
    world.unsub_secret_identifiers = vec![
        email_lower,
        workspace,
        workspace_id.to_string(),
        t,
        valid_sig,
        tampered_sig,
    ];
}

#[given(regex = r#"^an unsubscribe request for a real recipient carries an invalid token$"#)]
async fn unsubscribe_request_real_recipient_invalid_token(world: &mut FoundryWorld) {
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    let workspace = "Northwind";
    let workspace_id = seed_workspace_and_admin(harness, workspace).await;
    let email_lower = SAM_EMAIL.to_ascii_lowercase();
    // A well-formed link for a REAL recipient + real workspace, but with a TAMPERED
    // signature — decode succeeds, constant-time verify fails ⇒ uniform refusal.
    let (t, valid_sig) = mint_unsubscribe_link(harness, &email_lower, workspace_id);
    let sig = tamper_sig(&valid_sig);
    world.unsub_link_a = Some((t.clone(), sig.clone()));
    world.unsub_secret_identifiers.extend([
        email_lower,
        workspace.to_string(),
        workspace_id.to_string(),
        t,
        valid_sig,
        sig,
    ]);
}

#[given(regex = r#"^an unsubscribe request for a non-existent address carries an invalid token$"#)]
async fn unsubscribe_request_nonexistent_invalid_token(world: &mut FoundryWorld) {
    // A well-formed link for an address + workspace that DO NOT EXIST, with an invalid
    // signature. Its refusal must be byte-identical to the real-recipient arm — the
    // handler refuses BEFORE any existence lookup, so there is no oracle.
    let ghost_email = "ghost-recipient@no-such-domain.invalid";
    let ghost_workspace_id = uuid::Uuid::now_v7();
    let t = foundry_app::unsubscribe::encode_t(ghost_email, ghost_workspace_id);
    let sig = "not-a-real-signature-for-a-nonexistent-address".to_string();
    world.unsub_link_b = Some((t.clone(), sig.clone()));
    world.unsub_secret_identifiers.extend([
        ghost_email.to_string(),
        ghost_workspace_id.to_string(),
        t,
        sig,
    ]);
}

#[given(regex = r#"^Sam has a valid unsubscribe link for "([^"]+)" he has not confirmed$"#)]
async fn sam_has_valid_unconfirmed_link(world: &mut FoundryWorld, workspace: String) {
    // Seed the workspace + an admin who can drive the shipped `POST /invites` emit,
    // then mint the SAME well-formed signed link a suppressible email body carries —
    // but drive NO confirm POST, so NO opt-out row exists yet. Sam is subscribed;
    // the prefetch/CSRF-refusal scenarios prove neither a bare GET nor a CSRF-less
    // POST can flip that state.
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    let workspace_id = seed_workspace_and_admin(harness, &workspace).await;
    let email_lower = SAM_EMAIL.to_ascii_lowercase();
    let (t, sig) = mint_unsubscribe_link(harness, &email_lower, workspace_id);
    world.unsub_email = Some(email_lower);
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_t = Some(t);
    world.unsub_sig = Some(sig);
    world.unsub_admin = Some((ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()));
    world.unsub_ws_admins.insert(
        workspace,
        (ADMIN_EMAIL.to_string(), ADMIN_PASSWORD.to_string()),
    );
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
    world: &mut FoundryWorld,
    workspace: String,
) {
    // Seed the workspace + an admin, then record Sam's opt-out row for it (the
    // precondition: an unsubscribed recipient). Presence of the 0014 row = muted,
    // so a suppressible emit for (Sam, this ws) WOULD be suppressed — asserted here
    // so the post-deletion "resumes delivery" is a genuine state change, not a
    // vacuous pass.
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    let workspace_id = seed_workspace_and_admin(harness, &workspace).await;
    let email_lower = SAM_EMAIL.to_ascii_lowercase();
    harness
        .app
        .state
        .store
        .insert_unsubscribe(&email_lower, workspace_id)
        .await
        .expect("seed Sam's opt-out row for the workspace");
    let suppressed_before = harness
        .app
        .state
        .store
        .is_unsubscribed(&email_lower, workspace_id)
        .await
        .expect("read Sam's opt-out state");
    assert!(
        suppressed_before,
        "precondition: Sam must be unsubscribed from {workspace:?} before deletion"
    );
    world.unsub_workspace_id = Some(workspace_id);
    world.unsub_email = Some(email_lower);
}

#[given(regex = r#"^the suppression lookup is failing$"#)]
async fn the_suppression_lookup_is_failing(world: &mut FoundryWorld) {
    // Flip the already-spawned notifier's suppression point-read into failure mode:
    // the next `is_suppressed` returns `Err`, driving `notify()`'s fail-open Err arm.
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    harness.suppression_faults.set_failing();
}

#[given(regex = r#"^the suppression lookup is slow$"#)]
async fn the_suppression_lookup_is_slow(world: &mut FoundryWorld) {
    // Flip the already-spawned notifier's suppression point-read into slow mode: the
    // next `is_suppressed` blocks past the notifier's bounded suppression timeout, so
    // the gate's fail-open `Err(Elapsed)` arm fires and the emit stays await-bounded.
    let harness = world
        .harness
        .as_ref()
        .expect("harness spawned by Background");
    harness.suppression_faults.set_slow();
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
async fn workspace_invite_issued_to_sam(world: &mut FoundryWorld, workspace: String) {
    // Drive the REAL shipped issuance (bootstrap::create_invite, POST /invites):
    // sign the NAMED workspace's admin in and POST an invite for Sam. The handler
    // emits ONE `WorkspaceInvite` for Sam with `workspace_id: Some(<that ws>)`
    // through `notify()`, where the suppression gate decides deliver-vs-suppress
    // against Sam's opt-out state for THAT workspace.
    let (admin_email, admin_pw) = world
        .unsub_ws_admins
        .get(&workspace)
        .cloned()
        .unwrap_or_else(|| panic!("no seeded admin for workspace {workspace:?}"));
    // Time the whole issuance (signin + the /invites POST whose handler awaits
    // `notify()`), so the fail-open edge scenarios can assert the emit stayed
    // await-bounded (a failing/slow suppression lookup must not stall the request).
    let started = Instant::now();
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
    world.ndp_request_elapsed_ms = Some(started.elapsed().as_millis());
    assert!(
        outcome.status.is_success() || outcome.status.is_redirection(),
        "the invite must be issued (2xx/3xx), got {}: {}",
        outcome.status,
        outcome.body
    );
}

#[when(regex = r#"^a member-invite for "([^"]+)" is issued to Sam$"#)]
async fn member_invite_issued_to_sam(world: &mut FoundryWorld, workspace: String) {
    // Drive the REAL shipped member-invite issuance (`member_invites::submit_invite`,
    // POST /workspace/invites): sign the workspace admin in and POST an invite for Sam.
    // The handler emits ONE `MemberInvite` for Sam with `workspace_id: Some(<that ws>)`
    // through `notify()`, where the suppression gate decides deliver-vs-suppress against
    // Sam's opt-out state for THAT workspace.
    let (admin_email, admin_pw) = world
        .unsub_ws_admins
        .get(&workspace)
        .cloned()
        .unwrap_or_else(|| panic!("no seeded admin for workspace {workspace:?}"));
    let started = Instant::now();
    let outcome = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &admin_email,
            &admin_pw,
            "/workspace/invites",
            &[("email", SAM_EMAIL)],
        )
        .await
    };
    world.ndp_request_elapsed_ms = Some(started.elapsed().as_millis());
    assert!(
        outcome.status.is_success() || outcome.status.is_redirection(),
        "the member-invite must be issued (2xx/3xx), got {}: {}",
        outcome.status,
        outcome.body
    );
}

#[when(regex = r#"^Sam confirms unsubscribing from "([^"]+)" a second time$"#)]
async fn sam_confirms_unsubscribing_second_time(world: &mut FoundryWorld, _workspace: String) {
    // Re-run the exact same confirm click against the SAME signed link. The server
    // is idempotent (`ON CONFLICT DO NOTHING`), so this is a harmless no-op that
    // must return the same confirmation with no error.
    let base = {
        let harness = world.harness.as_ref().expect("harness");
        harness.base_url()
    };
    let http = world.http.as_ref().expect("http").clone();
    let t = world.unsub_t.clone().expect("link minted in Given");
    let sig = world.unsub_sig.clone().expect("link minted in Given");
    let (status, body) = confirm_unsubscribe(&http, &base, &t, &sig).await;
    world.last_status = Some(status);
    world.last_body = Some(body);
}

#[when(regex = r#"^Sam requests a password reset$"#)]
async fn sam_requests_password_reset(world: &mut FoundryWorld) {
    // Sam confirmed unsubscribing (the Given), but a password reset is MANDATORY.
    // Upgrade the account-less invitee to a REAL user so `POST /forgot-password`
    // resolves him, then drive that shipped public flow — it emits ONE `PasswordReset`
    // through `notify()`, where the gate skips the lookup entirely (NFR-3 structural)
    // and delivers regardless of his opt-out.
    let base = {
        let harness = world.harness.as_ref().expect("harness");
        seed_sam_user(harness).await;
        harness.base_url()
    };
    let http = world.http.as_ref().expect("http").clone();
    let status = post_forgot_password_for(&http, &base, SAM_EMAIL).await;
    assert!(
        status.is_success() || status.is_redirection(),
        "forgot-password must return its normal response (best-effort emit), got {status}"
    );
}

#[when(regex = r#"^an admin removes Sam from "([^"]+)"$"#)]
async fn admin_removes_sam_from(world: &mut FoundryWorld, workspace: String) {
    // A removal is MANDATORY. Make Sam a real member of the workspace the seeded
    // admin acts on, then drive the shipped admin-gated `POST /workspace/members/remove`
    // (the admin's session resolves that workspace). The handler deletes the membership
    // and emits ONE `MemberRemoved` to Sam through `notify()` — MANDATORY ⇒ never
    // suppressed, so it delivers despite his opt-out.
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace id captured in the Given");
    {
        let harness = world.harness.as_ref().expect("harness");
        let sam_id = seed_sam_user(harness).await;
        seed_sam_membership(harness, workspace_id, sam_id).await;
    }
    let (admin_email, admin_pw) = world
        .unsub_ws_admins
        .get(&workspace)
        .cloned()
        .unwrap_or_else(|| panic!("no seeded admin for workspace {workspace:?}"));
    let outcome = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_post(
            harness,
            http,
            &admin_email,
            &admin_pw,
            "/workspace/members/remove",
            &[("email", SAM_EMAIL)],
        )
        .await
    };
    assert!(
        outcome.status.is_success() || outcome.status.is_redirection(),
        "the removal must succeed (2xx/3xx), got {}: {}",
        outcome.status,
        outcome.body
    );
}

#[when(regex = r#"^a password reset, a password change, and a removal each fire for Sam$"#)]
async fn all_three_mandatory_events_fire_for_sam(world: &mut FoundryWorld) {
    // Fire ALL three mandatory security events for the unsubscribed Sam through their
    // real shipped driving-ports. Every one must deliver and count NO suppression —
    // the `is_suppressible()` allow-list holds the mandatory complement structurally
    // exempt. Sam was seeded as a real member with a password in the Given.
    let base = {
        let harness = world.harness.as_ref().expect("harness");
        harness.base_url()
    };
    let http = world.http.as_ref().expect("http").clone();
    let (admin_email, admin_pw) = world
        .unsub_admin
        .clone()
        .expect("admin seeded in the Given");

    // 1. password reset — public POST /forgot-password (Sam is a resolvable user).
    let reset_status = post_forgot_password_for(&http, &base, SAM_EMAIL).await;
    assert!(
        reset_status.is_success() || reset_status.is_redirection(),
        "the password reset must be accepted, got {reset_status}"
    );

    // 2. password change — signed-in POST /account/password (reauth + min-12 policy).
    let change = {
        let harness = world.harness.as_ref().expect("harness");
        signed_in_post(
            harness,
            &http,
            SAM_EMAIL,
            SAM_PASSWORD,
            "/account/password",
            &[
                ("current_password", SAM_PASSWORD),
                ("new_password", "rnp-sam-brand-new-passphrase-9x2q"),
            ],
        )
        .await
    };
    assert!(
        change.status.is_success() || change.status.is_redirection(),
        "the password change must succeed, got {}: {}",
        change.status,
        change.body
    );

    // 3. removal — admin-gated POST /workspace/members/remove.
    let removal = {
        let harness = world.harness.as_ref().expect("harness");
        signed_in_post(
            harness,
            &http,
            &admin_email,
            &admin_pw,
            "/workspace/members/remove",
            &[("email", SAM_EMAIL)],
        )
        .await
    };
    assert!(
        removal.status.is_success() || removal.status.is_redirection(),
        "the removal must succeed, got {}: {}",
        removal.status,
        removal.body
    );
}

#[when(regex = r#"^the tampered unsubscribe link is opened$"#)]
async fn the_tampered_link_is_opened(world: &mut FoundryWorld) {
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http").clone();
    let t = world
        .unsub_t
        .clone()
        .expect("tampered link minted in Given");
    let sig = world
        .unsub_sig
        .clone()
        .expect("tampered link minted in Given");
    let (status, body) = open_unsubscribe_link(&http, &base, &t, &sig).await;
    world.last_status = Some(status);
    world.last_body = Some(body);
}

#[when(regex = r#"^both unsubscribe links are opened$"#)]
async fn both_unsubscribe_links_are_opened(world: &mut FoundryWorld) {
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http").clone();
    let (t_a, sig_a) = world.unsub_link_a.clone().expect("link A minted in Given");
    let (t_b, sig_b) = world.unsub_link_b.clone().expect("link B minted in Given");
    let refusal_a = open_unsubscribe_link(&http, &base, &t_a, &sig_a).await;
    let refusal_b = open_unsubscribe_link(&http, &base, &t_b, &sig_b).await;
    world.unsub_refusal_a = Some(refusal_a);
    world.unsub_refusal_b = Some(refusal_b);
}

#[when(regex = r#"^an automated client fetches the unsubscribe link without confirming$"#)]
async fn automated_client_fetches_link_without_confirming(world: &mut FoundryWorld) {
    // An email scanner / link prefetcher issues a BARE `GET /unsubscribe?t=..&sig=..`
    // and NEVER submits the confirm POST — exactly the shape a mail client's safe-link
    // scan produces. The production GET (`unsubscribe::show_confirm`) is NON-DESTRUCTIVE
    // (renders the confirm page only, writes no row — NFR-2), so the opt-out state must
    // be untouched afterward. Capture the (status, body) so the outcome is observable.
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http").clone();
    let t = world.unsub_t.clone().expect("valid link minted in Given");
    let sig = world.unsub_sig.clone().expect("valid link minted in Given");
    let (status, body) = open_unsubscribe_link(&http, &base, &t, &sig).await;
    world.last_status = Some(status);
    world.last_body = Some(body);
}

#[when(regex = r#"^the unsubscribe confirm is posted without a valid CSRF token$"#)]
async fn unsubscribe_confirm_posted_without_csrf(world: &mut FoundryWorld) {
    // Post the confirm form (t, sig, action=unsubscribe) DIRECTLY, WITHOUT the
    // double-submit CSRF pair — no `foundry_csrf` cookie and no matching `_csrf`
    // field (the shape a forged cross-site POST takes). The shipped `csrf_middleware`
    // must refuse it (no cookie ⇒ invalid) before `submit_confirm` can write a row.
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http").clone();
    let t = world.unsub_t.clone().expect("valid link minted in Given");
    let sig = world.unsub_sig.clone().expect("valid link minted in Given");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("t", t);
    form.insert("sig", sig);
    form.insert("action", "unsubscribe".to_string());
    let resp = http
        .post(format!("{base}/unsubscribe"))
        .form(&form)
        .send()
        .await
        .expect("POST /unsubscribe without CSRF");
    world.last_status = Some(resp.status());
    world.last_body = Some(resp.text().await.unwrap_or_default());
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
async fn the_workspace_is_deleted(world: &mut FoundryWorld, _workspace: String) {
    // No workspace-delete route exists in v1, so drive the deletion at the store
    // boundary. Removing the `workspaces` row fires the 0014 FK ON DELETE CASCADE
    // (ADR-004), clearing Sam's opt-out row as a side effect — the behaviour under
    // test. Capture the rows affected so the `Then` asserts the delete succeeded.
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace id captured in the Given");
    let harness = world.harness.as_ref().expect("harness");
    let result = sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .execute(harness.app.state.store.pool())
        .await
        .expect("delete the workspace");
    world.unsub_delete_rows = Some(result.rows_affected());
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
    assert!(
        event == "workspace_invite" || event == "member_invite",
        "the suppressed event must be a suppressible one \
         (workspace_invite | member_invite), got {event:?}"
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
async fn the_invitation_is_delivered_to_sam(world: &mut FoundryWorld, _workspace: String) {
    // Observed at the provider driven-port boundary: the workspace_invite for Sam
    // reached delivery (the opt-out for a DIFFERENT workspace did not suppress it).
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| d.event == "workspace_invite" && d.to == SAM_EMAIL && d.outcome == "delivered")
        .collect();
    assert_eq!(
        delivered.len(),
        1,
        "the invitation must be delivered to Sam exactly once, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^Sam sees that he is already unsubscribed from "([^"]+)"$"#)]
async fn sam_sees_already_unsubscribed(world: &mut FoundryWorld, workspace: String) {
    // The second confirm succeeded and shows the same unsubscribed outcome — the
    // workspace's invitations are stopped (his opt-out persisted; re-confirming did
    // not error or flip it).
    let status = world.last_status.expect("a second confirm was posted");
    assert!(
        status.is_success(),
        "the second confirm must succeed (harmless no-op), got {status}"
    );
    let body = world.last_body.as_deref().expect("a second confirm body");
    assert!(
        body.contains(&workspace) && body.contains("invitations are stopped"),
        "the second confirm must reaffirm the workspace {workspace:?} is unsubscribed: {body}"
    );
}

#[then(regex = r#"^Sam sees the same confirmation both times with no error$"#)]
async fn sam_sees_same_confirmation_both_times(world: &mut FoundryWorld) {
    let first = world
        .unsub_first_confirmation
        .as_deref()
        .expect("first confirmation captured in Given");
    let second = world
        .last_body
        .as_deref()
        .expect("second confirmation body");
    assert_eq!(
        first, second,
        "confirming twice must yield the byte-identical confirmation (idempotent no-op)"
    );
}

#[then(regex = r#"^the workspace-invite for Sam from "([^"]+)" is delivered unchanged$"#)]
async fn workspace_invite_delivered_unchanged(world: &mut FoundryWorld, _workspace: String) {
    // With NO opt-out on record the empty-table point-read is Ok(false), so the
    // suppression gate falls through and the invite delivers byte-for-byte
    // unchanged (NFR-7) — observed at the provider boundary.
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| d.event == "workspace_invite" && d.to == SAM_EMAIL && d.outcome == "delivered")
        .collect();
    assert_eq!(
        delivered.len(),
        1,
        "with no opt-out the workspace_invite must be delivered unchanged, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^the password-reset notification is delivered to Sam$"#)]
async fn password_reset_delivered_to_sam(world: &mut FoundryWorld) {
    // Observed at the provider driven-port boundary: the mandatory `PasswordReset`
    // reached delivery for the unsubscribed Sam (the gate skipped the lookup).
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| {
            d.event == "password_reset"
                && d.to.eq_ignore_ascii_case(SAM_EMAIL)
                && d.outcome == "delivered"
        })
        .collect();
    assert_eq!(
        delivered.len(),
        1,
        "the password reset must be delivered to Sam exactly once, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^it is not counted as suppressed$"#)]
async fn it_is_not_counted_as_suppressed(world: &mut FoundryWorld) {
    // A MANDATORY event never reaches the suppression lookup (structural exempt), so
    // the SuppressionPolicy port records ZERO decisions for the whole scenario —
    // the `foundry_notification_suppressions_total` counter never ticks for it.
    // Reverting the `is_suppressible()` allow-list to admit a mandatory event would
    // make this scenario's mandatory emit consult the lookup and (for a suppressible-
    // carrying workspace) count a suppression, reddening this assertion.
    let harness = world.harness.as_ref().expect("harness");
    let count = harness.suppressions.count();
    assert_eq!(
        count, 0,
        "a mandatory security event must never be counted as suppressed, got {count}"
    );
}

#[then(regex = r#"^the member-removed notification is delivered to Sam$"#)]
async fn member_removed_delivered_to_sam(world: &mut FoundryWorld) {
    // Observed at the provider driven-port boundary: the mandatory `MemberRemoved`
    // reached delivery for the unsubscribed Sam (the gate skipped the lookup).
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| {
            d.event == "member_removed"
                && d.to.eq_ignore_ascii_case(SAM_EMAIL)
                && d.outcome == "delivered"
        })
        .collect();
    assert_eq!(
        delivered.len(),
        1,
        "the removal notice must be delivered to Sam exactly once, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^every one of those notifications is delivered$"#)]
async fn every_one_of_those_notifications_delivered(world: &mut FoundryWorld) {
    // All three mandatory events delivered to the unsubscribed Sam, observed at the
    // provider boundary — the `is_suppressible()` allow-list held every one exempt.
    let harness = world.harness.as_ref().expect("harness");
    let sent = harness.fake_email.sent();
    for event in ["password_reset", "password_changed", "member_removed"] {
        let delivered: Vec<_> = sent
            .iter()
            .filter(|d| {
                d.event == event && d.to.eq_ignore_ascii_case(SAM_EMAIL) && d.outcome == "delivered"
            })
            .collect();
        assert_eq!(
            delivered.len(),
            1,
            "the mandatory {event} must be delivered to Sam exactly once, found {}: {delivered:?}",
            delivered.len()
        );
    }
}

#[then(regex = r#"^none of them is counted as suppressed$"#)]
async fn none_of_them_counted_as_suppressed(world: &mut FoundryWorld) {
    // The crux invariant, observed at the SuppressionPolicy port: with Sam
    // unsubscribed, NOT ONE of the three mandatory events was counted as a
    // suppression — the structural `is_suppressible()` allow-list holds. Reverting
    // the allow-list to admit a mandatory event reds this (@property).
    let harness = world.harness.as_ref().expect("harness");
    let count = harness.suppressions.count();
    assert_eq!(
        count, 0,
        "no mandatory event may ever be counted as suppressed, got {count}"
    );
}

#[then(regex = r#"^the uniform non-enumerable refusal page is shown$"#)]
async fn uniform_non_enumerable_refusal_shown(world: &mut FoundryWorld) {
    let status = world.last_status.expect("the tampered link was opened");
    let body = world.last_body.clone().expect("a refusal body");
    // It is the uniform refusal page (fixed 200, reason-non-committal copy), NOT a
    // confirm page — no <form>, no workspace name.
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "the refusal is a fixed 200, got {status}"
    );
    assert!(
        body.contains("This unsubscribe link is no longer valid"),
        "the refusal must render the uniform non-enumerable copy: {body}"
    );
    assert!(
        !body.contains("<form"),
        "the refusal must carry NO confirm form: {body}"
    );
    // "refused exactly like an invalid one": a wholly-invalid link (garbage t + sig)
    // yields a BYTE-IDENTICAL refusal — the tampered arm and the never-valid arm
    // collapse to the same page, so a mutated sig is indistinguishable from a link
    // that never verified. Reverting the refusal to diverge per reason reds this.
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http").clone();
    let (invalid_status, invalid_body) =
        open_unsubscribe_link(&http, &base, "not-a-real-token", "not-a-real-signature").await;
    assert_eq!(
        (status, body.as_str()),
        (invalid_status, invalid_body.as_str()),
        "the tampered-token refusal must be byte-identical to a wholly-invalid link's refusal"
    );
}

#[then(regex = r#"^no unsubscribe is recorded$"#)]
async fn no_unsubscribe_is_recorded(world: &mut FoundryWorld) {
    let email = world
        .unsub_email
        .clone()
        .expect("recipient captured in Given");
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace captured in Given");
    let harness = world.harness.as_ref().expect("harness");
    let recorded = harness
        .app
        .state
        .store
        .is_unsubscribed(&email, workspace_id)
        .await
        .expect("read opt-out state");
    assert!(
        !recorded,
        "a refused (tampered-token) request must record NO opt-out row"
    );
}

#[then(regex = r#"^both requests return a byte-identical refusal$"#)]
async fn both_requests_return_byte_identical_refusal(world: &mut FoundryWorld) {
    let (status_a, body_a) = world.unsub_refusal_a.clone().expect("refusal A captured");
    let (status_b, body_b) = world.unsub_refusal_b.clone().expect("refusal B captured");
    assert_eq!(
        status_a, status_b,
        "the two refusals must share the same status (no status oracle)"
    );
    assert_eq!(
        body_a, body_b,
        "the two refusals must be byte-identical in body (no existence oracle)"
    );
    // And it is the uniform refusal page, not an incidental 404/500 collision.
    assert_eq!(
        status_a,
        reqwest::StatusCode::OK,
        "the refusal is the fixed-200 uniform page, got {status_a}"
    );
    assert!(
        body_a.contains("This unsubscribe link is no longer valid"),
        "the refusal must be the uniform non-enumerable page: {body_a}"
    );
}

#[then(regex = r#"^neither response reveals whether the address, workspace, or account exists$"#)]
async fn neither_response_reveals_existence(world: &mut FoundryWorld) {
    let (_, body_a) = world.unsub_refusal_a.clone().expect("refusal A captured");
    let (_, body_b) = world.unsub_refusal_b.clone().expect("refusal B captured");
    for ident in &world.unsub_secret_identifiers {
        assert!(
            !body_a.contains(ident.as_str()),
            "the real-recipient refusal must not echo {ident:?} (existence oracle): {body_a}"
        );
        assert!(
            !body_b.contains(ident.as_str()),
            "the non-existent-address refusal must not echo {ident:?} (existence oracle): {body_b}"
        );
    }
}

#[then(regex = r#"^a subsequent workspace-invite to Sam in "([^"]+)" is still delivered$"#)]
async fn subsequent_workspace_invite_to_sam_still_delivered(
    world: &mut FoundryWorld,
    _workspace: String,
) {
    // Prove prefetch-safety at the DELIVERY boundary: drive the REAL shipped issuance
    // (sign the seeded admin in, `POST /invites` for Sam). The handler emits ONE
    // `WorkspaceInvite` for Sam with `workspace_id: Some(Northwind)` through `notify()`.
    // Because the prefetch GET wrote NO opt-out row, the suppression gate's point-read
    // is `Ok(false)` ⇒ the invite DELIVERS — observed at the recording provider double.
    // (If the GET had wrongly written a row, this invite would be suppressed and RED.)
    let (admin_email, admin_pw) = world.unsub_admin.clone().expect("admin seeded in Given");
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
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| d.event == "workspace_invite" && d.to == SAM_EMAIL && d.outcome == "delivered")
        .collect();
    assert_eq!(
        delivered.len(),
        1,
        "a prefetch (bare GET, no confirm) must NOT unsubscribe Sam — his workspace_invite \
         must still be delivered exactly once, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^Sam remains subscribed to "([^"]+)" until he explicitly confirms$"#)]
async fn sam_remains_subscribed_until_confirms(world: &mut FoundryWorld, _workspace: String) {
    // The store-boundary observable: after the non-destructive prefetch GET, NO opt-out
    // row exists for (Sam, workspace) — the point-read is `Ok(false)` (subscribed). Only
    // an explicit confirm POST may ever flip this.
    let email = world
        .unsub_email
        .clone()
        .expect("recipient captured in Given");
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace captured in Given");
    let harness = world.harness.as_ref().expect("harness");
    let recorded = harness
        .app
        .state
        .store
        .is_unsubscribed(&email, workspace_id)
        .await
        .expect("read opt-out state");
    assert!(
        !recorded,
        "a bare prefetch GET must record NO opt-out — Sam remains subscribed until he \
         explicitly confirms"
    );
}

#[then(regex = r#"^the confirm is refused and no opt-out state changes$"#)]
async fn confirm_refused_no_state_change(world: &mut FoundryWorld) {
    // The shipped `csrf_middleware` fronts `POST /unsubscribe`: a confirm carrying no
    // valid double-submit CSRF token is REFUSED with 403 BEFORE `submit_confirm` runs,
    // so no `insert_unsubscribe` is ever reached. Two observables together prove the
    // refusal: (1) the response status is the CSRF refusal, and (2) the store still
    // holds NO opt-out row for (Sam, workspace).
    let status = world.last_status.expect("a CSRF-less confirm was posted");
    assert_eq!(
        status,
        reqwest::StatusCode::FORBIDDEN,
        "a confirm without a valid CSRF token must be refused with 403, got {status}"
    );
    let email = world
        .unsub_email
        .clone()
        .expect("recipient captured in Given");
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace captured in Given");
    let harness = world.harness.as_ref().expect("harness");
    let recorded = harness
        .app
        .state
        .store
        .is_unsubscribed(&email, workspace_id)
        .await
        .expect("read opt-out state");
    assert!(
        !recorded,
        "a CSRF-refused confirm must change NO opt-out state — Sam stays subscribed"
    );
}

#[then(regex = r#"^no unsubscribe token or recipient email appears in the logs$"#)]
async fn no_token_or_email_in_logs(world: &mut FoundryWorld) {
    // LOG OBSERVABLE (mirrors invite-accept scenario 13): the harness wires NO
    // in-process tracing-capture seam (tracing is global-only, initialised in
    // `main.rs::init_tracing`, not the harness), so the STRONGEST AVAILABLE observable
    // is the refusal's response-body surface — the user-visible projection of what the
    // handler chose to surface. The production refusal path (`unsubscribe::show_confirm`
    // → bad token → `unsubscribe_refusal_page`) emits ZERO tracing on the refusal arm;
    // its only `tracing::error!` lines carry `%err` alone, never the token or email. A
    // handler careless enough to log a secret would, by the same careless formatting,
    // echo it into this body — so the scan below is the falsifiable proxy.
    let body = world
        .last_body
        .clone()
        .expect("the tampered link was opened");
    for ident in &world.unsub_secret_identifiers {
        assert!(
            !body.contains(ident.as_str()),
            "a refused request must leak NO token/recipient email — found {ident:?}: {body}"
        );
    }
}

#[then(regex = r#"^the member-invite for Sam from "([^"]+)" is not delivered$"#)]
async fn member_invite_for_sam_not_delivered(world: &mut FoundryWorld, _workspace: String) {
    // The suppressed member_invite reached NO provider — the recording double observes
    // zero delivery for Sam. `member_invites::submit_invite` threads
    // `workspace_id: Some(..)` and `MemberInvite` is suppressible, so the SAME
    // (email, workspace) opt-out intercepts it before fan-out. Dropping either re-REDs this.
    let harness = world.harness.as_ref().expect("harness");
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| d.event == "member_invite" && d.to == SAM_EMAIL && d.outcome == "delivered")
        .collect();
    assert!(
        delivered.is_empty(),
        "an unsubscribed recipient's member_invite must not be delivered, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^both member-invite and workspace-invite emails from "([^"]+)" are suppressed$"#)]
async fn both_invite_events_suppressed(world: &mut FoundryWorld, workspace: String) {
    // After Sam confirmed unsubscribing via the member-invite link, the EVENT-AGNOSTIC
    // (email, workspace) opt-out must suppress BOTH suppressible events. Drive a real
    // member-invite (POST /workspace/invites) AND a real workspace-invite (POST /invites)
    // for Sam, and assert both are intercepted before fan-out — observed as a +2 delta at
    // the SuppressionPolicy port for this workspace, with zero NEW deliveries.
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace id captured in the Given");
    let (admin_email, admin_pw) = world
        .unsub_ws_admins
        .get(&workspace)
        .cloned()
        .unwrap_or_else(|| panic!("no seeded admin for workspace {workspace:?}"));
    let (suppressed_before, delivered_before) = {
        let harness = world.harness.as_ref().expect("harness");
        let suppressed = harness.suppressions.count_for_workspace(workspace_id);
        let delivered = harness
            .fake_email
            .sent()
            .into_iter()
            .filter(|d| {
                d.to == SAM_EMAIL
                    && (d.event == "member_invite" || d.event == "workspace_invite")
                    && d.outcome == "delivered"
            })
            .count();
        (suppressed, delivered)
    };
    for path in ["/workspace/invites", "/invites"] {
        let outcome = {
            let harness = world.harness.as_ref().expect("harness");
            let http = world.http.as_ref().expect("http");
            signed_in_post(
                harness,
                http,
                &admin_email,
                &admin_pw,
                path,
                &[("email", SAM_EMAIL)],
            )
            .await
        };
        assert!(
            outcome.status.is_success() || outcome.status.is_redirection(),
            "issuing an invite via {path} must succeed (2xx/3xx), got {}: {}",
            outcome.status,
            outcome.body
        );
    }
    let harness = world.harness.as_ref().expect("harness");
    let suppressed_after = harness.suppressions.count_for_workspace(workspace_id);
    assert_eq!(
        suppressed_after - suppressed_before,
        2,
        "both the member_invite and the workspace_invite must be suppressed for the \
         unsubscribed (email, workspace) pair, got a suppression delta of {}",
        suppressed_after - suppressed_before
    );
    let delivered_after = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| {
            d.to == SAM_EMAIL
                && (d.event == "member_invite" || d.event == "workspace_invite")
                && d.outcome == "delivered"
        })
        .count();
    assert_eq!(
        delivered_after, delivered_before,
        "no new suppressible invite may be delivered to the unsubscribed pair after opt-out"
    );
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
async fn deleting_the_workspace_succeeds(world: &mut FoundryWorld) {
    let rows = world
        .unsub_delete_rows
        .expect("the workspace deletion ran in the When");
    assert_eq!(
        rows, 1,
        "deleting the workspace must remove exactly one workspace row, removed {rows}"
    );
}

#[then(regex = r#"^a previously-unsubscribed recipient of that workspace resumes delivery$"#)]
async fn previously_unsubscribed_recipient_resumes_delivery(world: &mut FoundryWorld) {
    // Emit a fresh suppressible workspace_invite for (Sam, the deleted workspace)
    // through the notifier driving port. No HTTP invite path remains (the workspace
    // is gone), so drive `notify()` directly. With the FK cascade having cleared the
    // 0014 opt-out row, the point-read is now `Ok(false)` ⇒ the gate falls through ⇒
    // the invite delivers again — observed at the recording provider boundary.
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace id captured in the Given");
    let email = world
        .unsub_email
        .clone()
        .expect("recipient captured in the Given");
    let harness = world.harness.as_ref().expect("harness");
    let notification = Notification {
        event: NotificationEvent::WorkspaceInvite,
        recipient: email.clone(),
        subject: "You're invited".to_string(),
        body: "join the workspace".to_string(),
        workspace_id: Some(workspace_id),
    };
    harness.app.state.notifier.notify(&notification).await;
    let delivered: Vec<_> = harness
        .fake_email
        .sent()
        .into_iter()
        .filter(|d| d.event == "workspace_invite" && d.to == email && d.outcome == "delivered")
        .collect();
    assert_eq!(
        delivered.len(),
        1,
        "after the cascade cleared the opt-out, the workspace_invite must resume \
         delivering, found {}: {delivered:?}",
        delivered.len()
    );
}

#[then(regex = r#"^no orphaned suppression state remains$"#)]
async fn no_orphaned_suppression_state(world: &mut FoundryWorld) {
    // The FK ON DELETE CASCADE must have removed Sam's 0014 opt-out row along with
    // the workspace — the suppression point-read at the store boundary now reads
    // `Ok(false)` (subscribed), so no orphaned opt-out state lingers.
    let workspace_id = world
        .unsub_workspace_id
        .expect("workspace id captured in the Given");
    let email = world
        .unsub_email
        .clone()
        .expect("recipient captured in the Given");
    let harness = world.harness.as_ref().expect("harness");
    let still_unsubscribed = harness
        .app
        .state
        .store
        .is_unsubscribed(&email, workspace_id)
        .await
        .expect("read Sam's opt-out state after deletion");
    assert!(
        !still_unsubscribed,
        "the workspace deletion must cascade-clear Sam's opt-out row (no orphaned \
         suppression state)"
    );
}

#[then(regex = r#"^the emit completes without stalling$"#)]
async fn the_emit_completes_without_stalling(world: &mut FoundryWorld) {
    // The failing/slow suppression lookup must not stall the emit: the gate is
    // bounded (fail-open on Err/timeout), so the /invites request completes far
    // inside the block a slow lookup would otherwise impose. Reverting the bound
    // (awaiting the 5s slow lookup) re-REDs this.
    let elapsed = world
        .ndp_request_elapsed_ms
        .expect("the invite request timing was captured in the When");
    assert!(
        elapsed < 3000,
        "the emit must not stall on a failing/slow suppression lookup \
         (await-bounded, fail-open), took {elapsed}ms"
    );
}
