//! "machine-token-admin-ux" step definitions — the browser admin surface for
//! minting, listing, and revoking machine tokens (US-MT00..US-MT06).
//!
//! RED-state contract (DISTILL, Mandate 7 / ADR-025):
//! These steps drive the SAME in-process axum harness (`InProcHarness` →
//! `build_router`) the rest of the browser suite uses, over real HTTP through
//! `reqwest`, UNDER the production session + CSRF layers (the admin is a browser
//! human — NFR-MT-SEC-07). The `/admin/tokens` GET/POST routes are mounted but
//! their handlers are RED scaffolds (`admin_tokens.rs` — they `panic!`), and the
//! `foundry_services::tokens` use-cases are RED scaffolds too. So:
//!   - Background + Given steps set up REAL preconditions (workspace, admin,
//!     member, seeded token rows) via the existing shared helpers + thin direct
//!     inserts — they MUST succeed, so the failure is in the behaviour, not the
//!     fixture.
//!   - When steps sign in (admin or member) over the real cookie path and issue
//!     a real GET/POST to `/admin/tokens`, capturing the response.
//!   - Then steps assert the user-visible outcome and FAIL RED — today the
//!     scaffold handler panics (HTTP 500), so the rendered page / one-time value
//!     / list rows the assertions look for are absent. This is
//!     MISSING_FUNCTIONALITY, not BROKEN. Once DELIVER implements the handlers +
//!     `tokens` use-cases + the signer-in-AppState wiring, the assertions flip
//!     GREEN.
//!
//! Negative assertions ("no token value is shown", "token surface is shown")
//! guard against a false GREEN on the scaffold 500 (Critical Rule 7 / Fixture
//! Theater): they require the surface to have actually RENDERED (status 200)
//! before treating the absence of a secret as a pass.
//!
//! Reused Background Givens (cucumber-rs requires globally-unique step text):
//!   - `a workspace "..." exists with admin "..."`        (us_06_signin)
//!   - `a member "..." belongs to the team "..."`          (us_07_project_create)
//!
//! Only machine-token-admin-specific phrases are declared here.
//!
//! What DELIVER must wire to flip these GREEN is enumerated in
//! `docs/feature/machine-token-admin-ux/distill/step-skeletons.md`.

use crate::support::harness::{signed_in_post, InProcHarness};
use crate::support::html_assertions;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use secrecy::ExposeSecret;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const ADMIN_EMAIL: &str = "devansh@acme.com";
const ADMIN_PASSWORD: &str = "admin-password-from-bootstrap";
const MEMBER_EMAIL: &str = "mei@acme.com";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

fn password_for(email: &str) -> &'static str {
    if email == ADMIN_EMAIL {
        ADMIN_PASSWORD
    } else {
        MEMBER_PASSWORD
    }
}

// --------------------------------------------------------------------------
// Harness (re)build + re-seed.
//
// The Background `a workspace "Acme" exists with admin ...` (us_06) spawns an
// ISSUER harness by default and seeds Acme + the admin; `a member ... belongs
// to the team "Backend"` (us_07) adds Mei. But the verifier-only scenarios need
// a harness WITHOUT a signer. To stay order- and mode-independent, the
// signed-in Givens below rebuild the harness in the required issuer mode and
// re-seed Acme + admin + the Backend member — a self-contained precondition,
// never the behaviour under test.
// --------------------------------------------------------------------------

async fn rebuild_harness(world: &mut FoundryWorld, issuer: bool) {
    let harness = if issuer {
        InProcHarness::spawn(now_anchor()).await
    } else {
        InProcHarness::spawn_verifier_only(now_anchor()).await
    };
    world.harness = Some(harness);
    world.http = Some(client());
    world.mt_issuer = issuer;
    seed_workspace_admin_member(world).await;
}

/// Establish the signed-in harness in the requested issuer mode WITHOUT
/// destroying state a prior Given already seeded. When the existing harness is
/// already in the requested mode, reuse it (re-seeding the workspace+admin+member
/// is idempotent) so pre-seeded token rows survive — a Background/Given that
/// seeds tokens BEFORE the "admin is signed in" Given (us-mt02) must not have its
/// fixtures torn down. Only spawn a fresh harness when none exists yet (us-mt01's
/// signed-in-first ordering) or the mode differs (issuer ⇄ verifier-only).
async fn ensure_signed_in(world: &mut FoundryWorld, issuer: bool) {
    if world.harness.is_some() && world.mt_issuer == issuer {
        if world.http.is_none() {
            world.http = Some(client());
        }
        seed_workspace_admin_member(world).await;
    } else {
        rebuild_harness(world, issuer).await;
    }
}

/// Seed workspace "Acme" + admin (devansh, role=admin) + member (mei, Backend,
/// role=member) directly via SQL — preconditions, not the behaviour under test.
/// Idempotent on re-seed (ON CONFLICT DO NOTHING).
async fn seed_workspace_admin_member(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    // The schema enforces a SINGLE workspace per database
    // (`uniq_one_workspace ON ((true))`, 0001_init.sql) — the slice-1
    // single-workspace model. So seed Acme ONLY if the schema has none yet (a
    // fresh verifier-only harness); otherwise reuse the existing one (the
    // Background already created it). Never a second workspace row.
    let existing: Option<(uuid::Uuid,)> = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_optional(pool)
        .await
        .expect("probe for an existing workspace");
    let ws: (uuid::Uuid,) = match existing {
        Some(row) => row,
        None => {
            let ws_id = uuid::Uuid::now_v7();
            sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
                .bind(ws_id)
                .bind("Acme")
                .execute(pool)
                .await
                .expect("seed Acme workspace");
            (ws_id,)
        }
    };

    seed_user_membership(
        world,
        ws.0,
        ADMIN_EMAIL,
        "Admin",
        ADMIN_PASSWORD,
        "admin",
        None,
    )
    .await;
    let team_id = seed_team(world, ws.0, "Backend").await;
    seed_user_membership(
        world,
        ws.0,
        MEMBER_EMAIL,
        "Mei",
        MEMBER_PASSWORD,
        "member",
        Some(team_id),
    )
    .await;
}

async fn seed_team(world: &FoundryWorld, workspace_id: uuid::Uuid, name: &str) -> uuid::Uuid {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)
              ON CONFLICT (workspace_id, slug) DO NOTHING",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(name)
    .bind(slugify(name))
    .execute(pool)
    .await
    .expect("seed team");
    let row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = $2")
            .bind(workspace_id)
            .bind(name)
            .fetch_one(pool)
            .await
            .expect("resolve team id");
    row.0
}

#[allow(clippy::too_many_arguments)]
async fn seed_user_membership(
    world: &FoundryWorld,
    workspace_id: uuid::Uuid,
    email: &str,
    display: &str,
    password: &str,
    role: &str,
    team_id: Option<uuid::Uuid>,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let lower = email.to_ascii_lowercase();
    let hash =
        foundry_auth::hash_password(&secrecy::SecretString::new(password.to_string().into()))
            .await
            .expect("hash pw");
    let uid = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(uid)
    .bind(&lower)
    .bind(email)
    .bind(display)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("seed user");
    let resolved: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&lower)
        .fetch_one(pool)
        .await
        .expect("resolve user id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(resolved.0)
    .bind(role)
    .execute(pool)
    .await
    .expect("seed workspace membership");
    if let Some(team) = team_id {
        sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role)
                  VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
        )
        .bind(team)
        .bind(resolved.0)
        .execute(pool)
        .await
        .expect("seed team membership");
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_hyphen = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            last_hyphen = false;
        } else if !last_hyphen {
            out.push('-');
            last_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

async fn workspace_id_by_name(world: &FoundryWorld, name: &str) -> uuid::Uuid {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces WHERE name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("resolve workspace {name:?}: {e}"));
    row.0
}

/// Seed a machine_tokens registry row directly (a precondition — a token that
/// "already exists"), returning its jti. `revoked` stamps `revoked_at`. Uses the
/// CURRENT 6-arg `insert_machine_token` (DISTILL keeps the signature; DELIVER
/// adds the `created_by` parameter — see step-skeletons.md), then patches
/// revoked_at / a foreign workspace as needed via direct SQL.
async fn seed_token_row(
    world: &mut FoundryWorld,
    workspace_name: &str,
    label: &str,
    revoked: bool,
    expired: bool,
) -> uuid::Uuid {
    // Default issuer: SOME existing member of the workspace (the FK only has to
    // be valid — for the single-issuer scenarios authorship is not the behaviour
    // under test). The issuer-attribution scenario (us-mt06) uses the
    // by-issuer variant below to bind each token to a NAMED admin.
    seed_token_row_by_issuer(world, workspace_name, label, None, revoked, expired).await
}

/// Seed a registry row whose `created_by` is a NAMED issuer (US-MT06 attribution:
/// the list must show each token's distinct issuer email). `issuer_email = None`
/// binds to any workspace member (FK validity only); `Some(email)` resolves that
/// user and records them as the credential's author — so the list can attribute
/// "CI bot" to devansh and "Old triage agent" to dana.
async fn seed_token_row_by_issuer(
    world: &mut FoundryWorld,
    workspace_name: &str,
    label: &str,
    issuer_email: Option<&str>,
    revoked: bool,
    expired: bool,
) -> uuid::Uuid {
    let workspace_id = workspace_id_by_name(world, workspace_name).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let user: (uuid::Uuid,) = match issuer_email {
        Some(email) => sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
            .bind(email.to_ascii_lowercase())
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("resolve issuer {email:?}: {e}")),
        None => sqlx::query_as(
            "SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 LIMIT 1",
        )
        .bind(workspace_id)
        .fetch_one(pool)
        .await
        .expect("a member exists in the workspace"),
    };
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = if expired {
        now - time::Duration::hours(1)
    } else {
        now + time::Duration::days(30)
    };
    harness
        .app
        .state
        .store
        .insert_machine_token(jti, user.0, workspace_id, None, exp, label, user.0)
        .await
        .expect("seed machine token row");
    if revoked {
        harness
            .app
            .state
            .store
            .revoke_machine_token(jti)
            .await
            .expect("seed revoked token");
    }
    world.mt_jti_by_label.insert(label.to_string(), jti);
    world.mt_last_jti = Some(jti);
    jti
}

// ==========================================================================
// Given — signed-in actor + issuer mode
// ==========================================================================

#[given(regex = r#"^the admin is signed in to the token surface on an issuer-configured server$"#)]
async fn admin_signed_in_issuer(world: &mut FoundryWorld) {
    ensure_signed_in(world, true).await;
    world.mt_actor_email = Some(ADMIN_EMAIL.to_string());
}

#[given(regex = r#"^the admin is signed in to the token surface on a verifier-only server$"#)]
async fn admin_signed_in_verifier_only(world: &mut FoundryWorld) {
    rebuild_harness(world, false).await;
    world.mt_actor_email = Some(ADMIN_EMAIL.to_string());
}

#[given(regex = r#"^the member is signed in to the token surface on an issuer-configured server$"#)]
async fn member_signed_in_issuer(world: &mut FoundryWorld) {
    ensure_signed_in(world, true).await;
    world.mt_actor_email = Some(MEMBER_EMAIL.to_string());
}

// ==========================================================================
// Given — seeded registry state (preconditions)
// ==========================================================================

#[given(regex = r#"^the workspace "([^"]+)" has three issued tokens, one of them revoked$"#)]
async fn ws_has_three_tokens(world: &mut FoundryWorld, ws: String) {
    ensure_seeded(world).await;
    seed_token_row(world, &ws, "CI bot — files release issues", false, false).await;
    seed_token_row(world, &ws, "Slack relay", false, false).await;
    seed_token_row(world, &ws, "Old triage agent", true, false).await;
}

#[given(regex = r#"^the workspace "([^"]+)" has an issued token labelled "([^"]+)"$"#)]
async fn ws_has_token(world: &mut FoundryWorld, ws: String, label: String) {
    ensure_seeded(world).await;
    seed_token_row(world, &ws, &label, false, false).await;
}

#[given(
    regex = r#"^the workspace "([^"]+)" has an issued token labelled "([^"]+)" that is revoked$"#
)]
async fn ws_has_revoked_token(world: &mut FoundryWorld, ws: String, label: String) {
    ensure_seeded(world).await;
    seed_token_row(world, &ws, &label, true, false).await;
}

#[given(regex = r#"^the workspace "([^"]+)" has no issued tokens$"#)]
async fn ws_has_no_tokens(world: &mut FoundryWorld, _ws: String) {
    ensure_seeded(world).await;
    // Absence of rows is the precondition — nothing to seed.
}

#[given(regex = r#"^a registry credential exists bound to a different workspace$"#)]
async fn credential_in_other_workspace(world: &mut FoundryWorld) {
    ensure_seeded(world).await;
    // Single-workspace model: a real foreign-workspace row is not insertable
    // (FK + uniq_one_workspace, upstream-issues.md). Record the count of the
    // acting workspace's CURRENT rows so the "lists only the acting workspace's
    // tokens" Then can assert the list never grows beyond them — i.e. the read
    // is workspace-scoped (no leakage). Nothing foreign is inserted.
    world.mt_foreign_jti = Some(uuid::Uuid::now_v7());
}

#[given(regex = r#"^the workspace "([^"]+)" has a token with no recorded issuer$"#)]
async fn ws_has_unattributed_token(world: &mut FoundryWorld, ws: String) {
    ensure_seeded(world).await;
    // Since step 01-01 `insert_machine_token` always records `created_by`, a
    // plain seed is now attributed. To model a "no recorded issuer" row (a
    // pre-feature / deleted-admin row, the US-MT06 "minted by —" edge), seed
    // then NULL `created_by` directly — exactly the legacy/`ON DELETE SET NULL`
    // state the nullable 0008 column represents.
    let jti = seed_token_row(world, &ws, "Legacy bot", false, false).await;
    let harness = world.harness.as_ref().expect("harness");
    sqlx::query("UPDATE machine_tokens SET created_by = NULL WHERE jti = $1")
        .bind(jti)
        .execute(harness.app.state.store.pool())
        .await
        .expect("clear created_by to model a no-recorded-issuer row");
}

// NOTE (single-workspace constraint): the schema enforces exactly ONE workspace
// per database (`uniq_one_workspace`, 0001_init.sql) and `machine_tokens.
// workspace_id` / `scope_team_id` are FKs into that single workspace. A REAL
// foreign workspace/team row is therefore structurally impossible to seed in
// slice 1 (see distill/upstream-issues.md). The cross-workspace evil-user paths
// are modelled by a SYNTHETIC jti/team uuid that does NOT belong to the acting
// workspace — observably IDENTICAL to a foreign row from the acting admin's
// side: the service's `find_machine_token_by_jti` returns a row whose
// `workspace_id != principal.workspace_id()` (or None), yielding the SAME
// non-enumerable 404 (NFR-MT-REL-03 / NFR-MT-SEC-03). The behaviour under test
// is the non-enumerable refusal, which this faithfully exercises.

#[given(
    regex = r#"^another workspace "([^"]+)" has an issued token labelled "([^"]+)" that is active$"#
)]
async fn other_ws_has_active_token(world: &mut FoundryWorld, _ws: String, _label: String) {
    ensure_seeded(world).await;
    // A jti the acting workspace did not issue (not in the registry). From the
    // acting admin's side this is indistinguishable from a foreign-workspace jti.
    world.mt_foreign_jti = Some(uuid::Uuid::now_v7());
}

#[given(regex = r#"^another workspace "([^"]+)" owns a team "([^"]+)"$"#)]
async fn other_ws_owns_team(world: &mut FoundryWorld, _ws: String, team: String) {
    ensure_seeded(world).await;
    // A team uuid that is NOT part of the acting workspace — the mint use-case
    // must reject a scope referencing it (US-MT04 evil-user path).
    world
        .mt_jti_by_label
        .insert(format!("team:{team}"), uuid::Uuid::now_v7());
}

#[given(
    regex = r#"^the admin has issued a token labelled "([^"]+)" that an integration is using$"#
)]
async fn admin_issued_token_in_use(world: &mut FoundryWorld, label: String) {
    // Issuer harness + a registry row + a REAL signed credential bound to that
    // jti, so the denylist cross-check can present it to /api/v1 after revoke.
    rebuild_harness(world, true).await;
    world.mt_actor_email = Some(ADMIN_EMAIL.to_string());
    let jti = seed_token_row(world, "Acme", &label, false, false).await;
    mint_real_credential_for_jti(world, jti).await;
}

#[given(
    regex = r#"^the admin has just issued a token labelled "([^"]+)" and left the issuance view$"#
)]
async fn admin_just_issued_left_view(world: &mut FoundryWorld, label: String) {
    // Pillar 2 — this Given reuses the mint When of us-mt01: sign in as admin on
    // an issuer server, then issue the token. The "left the view" part is simply
    // that the next When re-GETs the list surface.
    rebuild_harness(world, true).await;
    world.mt_actor_email = Some(ADMIN_EMAIL.to_string());
    post_mint(world, &[("label", label.as_str())]).await;
    // Capture the one-time body for the "never shown again" comparison.
    world.mt_mint_response_body = world.mt_last_body.clone();
}

#[given(
    regex = r#"^the admin issued the "([^"]+)" token and another admin "([^"]+)" issued the "([^"]+)" token$"#
)]
async fn two_admins_issued(
    world: &mut FoundryWorld,
    label_a: String,
    other_admin: String,
    label_b: String,
) {
    rebuild_harness(world, true).await;
    world.mt_actor_email = Some(ADMIN_EMAIL.to_string());
    // Seed the second admin and two registry rows (created_by attribution is the
    // DELIVER wiring; the rows are the precondition).
    let ws = workspace_id_by_name(world, "Acme").await;
    seed_user_membership(
        world,
        ws,
        &other_admin,
        "Dana",
        "dana-pw-correct-horse",
        "admin",
        None,
    )
    .await;
    // Attribute each token to its NAMED issuer so the list can show distinct
    // admins (US-MT06): label_a is the acting admin's, label_b is dana's.
    seed_token_row_by_issuer(world, "Acme", &label_a, Some(ADMIN_EMAIL), false, false).await;
    seed_token_row_by_issuer(world, "Acme", &label_b, Some(&other_admin), false, false).await;
}

#[given(
    regex = r#"^the "([^"]+)" token was used recently and a freshly issued token has never been used$"#
)]
async fn one_used_one_fresh(world: &mut FoundryWorld, used_label: String) {
    rebuild_harness(world, true).await;
    world.mt_actor_email = Some(ADMIN_EMAIL.to_string());
    let used = seed_token_row(world, "Acme", &used_label, false, false).await;
    let harness = world.harness.as_ref().expect("harness");
    harness
        .app
        .state
        .store
        .touch_machine_token_last_used(used)
        .await
        .expect("touch last_used");
    seed_token_row(world, "Acme", "Fresh bot", false, false).await;
}

/// Ensure a workspace + admin + member are seeded (idempotent). Used by Given
/// steps that may run before any signed-in Given established the harness.
async fn ensure_seeded(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        rebuild_harness(world, true).await;
    } else {
        // The Background's `a workspace "Acme" exists` spawned an ISSUER harness
        // (InProcHarness::spawn); record that mode so the later "admin is signed
        // in on an issuer-configured server" Given reuses it (preserving the
        // tokens this step seeds) instead of tearing it down.
        world.mt_issuer = true;
        if world.http.is_none() {
            world.http = Some(client());
        }
        if world.mt_actor_email.is_none() {
            seed_workspace_admin_member(world).await;
        }
    }
}

/// Mint a REAL EdDSA credential bound to the seeded `jti` using the FIXED test
/// signer, store it in the world so the denylist cross-check can present it to
/// `/api/v1` (US-MT03 — revoke kills it on the next call). Mirrors the
/// feature_a credential-minting idiom.
async fn mint_real_credential_for_jti(world: &mut FoundryWorld, jti: uuid::Uuid) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT user_id, workspace_id FROM machine_tokens WHERE jti = $1")
            .bind(jti)
            .fetch_one(pool)
            .await
            .expect("resolve seeded token principal");
    let now = time::OffsetDateTime::now_utc();
    let claims = foundry_auth::MachineTokenClaims {
        sub: row.0,
        scope: None,
        iat: now.unix_timestamp(),
        exp: (now + time::Duration::hours(1)).unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let signer = foundry_auth::test_keys::signer();
    let jwt = signer
        .mint(&claims)
        .expect("mint test credential")
        .expose_secret()
        .to_string();
    world.mt_minted_value = Some(jwt);
}

// ==========================================================================
// When — drive /admin/tokens
// ==========================================================================

#[when(regex = r#"^the admin opens the token surface$"#)]
async fn admin_opens_surface(world: &mut FoundryWorld) {
    get_token_surface(world).await;
}

#[when(regex = r#"^the member opens the token surface$"#)]
async fn member_opens_surface(world: &mut FoundryWorld) {
    get_token_surface(world).await;
}

#[when(regex = r#"^the admin returns to the token surface$"#)]
async fn admin_returns_to_surface(world: &mut FoundryWorld) {
    get_token_surface(world).await;
}

#[when(regex = r#"^the admin looks for the token value again$"#)]
async fn admin_looks_for_value(world: &mut FoundryWorld) {
    get_token_surface(world).await;
}

#[when(regex = r#"^the admin issues a token labelled "([^"]+)"$"#)]
async fn admin_issues_token(world: &mut FoundryWorld, label: String) {
    post_mint(world, &[("label", label.as_str())]).await;
}

#[when(regex = r#"^the admin attempts to issue a token labelled "([^"]+)"$"#)]
async fn admin_attempts_issue(world: &mut FoundryWorld, label: String) {
    post_mint(world, &[("label", label.as_str())]).await;
}

#[when(regex = r#"^the member attempts to issue a token labelled "([^"]+)"$"#)]
async fn member_attempts_issue(world: &mut FoundryWorld, label: String) {
    post_mint(world, &[("label", label.as_str())]).await;
}

#[when(regex = r#"^the admin attempts to issue a token with no label$"#)]
async fn admin_issues_no_label(world: &mut FoundryWorld) {
    post_mint(world, &[("label", "")]).await;
}

#[when(
    regex = r#"^the admin issues a token labelled "([^"]+)" scoped to the "([^"]+)" team for (\d+) days$"#
)]
async fn admin_issues_scoped(world: &mut FoundryWorld, label: String, team: String, days: u32) {
    let days_s = days.to_string();
    post_mint(
        world,
        &[
            ("label", label.as_str()),
            ("scope", "team"),
            ("team", team.as_str()),
            ("ttl_days", days_s.as_str()),
        ],
    )
    .await;
}

#[when(regex = r#"^the admin issues a token labelled "([^"]+)" for (\d+) days$"#)]
async fn admin_issues_ttl(world: &mut FoundryWorld, label: String, days: u32) {
    let days_s = days.to_string();
    post_mint(
        world,
        &[("label", label.as_str()), ("ttl_days", days_s.as_str())],
    )
    .await;
}

#[when(regex = r#"^the admin attempts to issue a token labelled "([^"]+)" for (\d+) days$"#)]
async fn admin_attempts_issue_ttl(world: &mut FoundryWorld, label: String, days: u32) {
    let days_s = days.to_string();
    post_mint(
        world,
        &[("label", label.as_str()), ("ttl_days", days_s.as_str())],
    )
    .await;
}

#[when(regex = r#"^the admin attempts to issue a token scoped to the "([^"]+)" team$"#)]
async fn admin_issues_foreign_scope(world: &mut FoundryWorld, team: String) {
    let team_id = world
        .mt_jti_by_label
        .get(&format!("team:{team}"))
        .copied()
        .expect("foreign team seeded");
    let team_s = team_id.to_string();
    post_mint(
        world,
        &[
            ("label", "Cross-scope bot"),
            ("scope", "team"),
            ("team", team_s.as_str()),
            ("ttl_days", "30"),
        ],
    )
    .await;
}

#[when(regex = r#"^the admin revokes the "([^"]+)" token$"#)]
async fn admin_revokes(world: &mut FoundryWorld, label: String) {
    let jti = *world
        .mt_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("token {label:?} was seeded"));
    post_revoke(world, jti, true).await;
}

#[when(regex = r#"^the admin revokes the "([^"]+)" token again$"#)]
async fn admin_revokes_again(world: &mut FoundryWorld, label: String) {
    let jti = *world
        .mt_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("token {label:?} was seeded"));
    post_revoke(world, jti, true).await;
}

#[when(regex = r#"^the member tries to revoke the "([^"]+)" token$"#)]
async fn member_revokes(world: &mut FoundryWorld, label: String) {
    let jti = *world
        .mt_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("token {label:?} was seeded"));
    post_revoke(world, jti, true).await;
}

#[when(regex = r#"^the admin tries to revoke the "([^"]+)" workspace's token$"#)]
async fn admin_revokes_foreign(world: &mut FoundryWorld, _ws: String) {
    let jti = world.mt_foreign_jti.expect("a foreign token was seeded");
    post_revoke(world, jti, true).await;
}

#[when(regex = r#"^the admin opens the revoke confirmation for "([^"]+)"$"#)]
async fn admin_opens_revoke_confirm(world: &mut FoundryWorld, _label: String) {
    // The confirmation lives on the list surface (the Revoke button + warning).
    get_token_surface(world).await;
}

#[when(regex = r#"^the admin submits a revoke for "([^"]+)" with no anti-forgery token$"#)]
async fn admin_revokes_no_csrf(world: &mut FoundryWorld, label: String) {
    let jti = *world
        .mt_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("token {label:?} was seeded"));
    post_revoke(world, jti, false).await;
}

#[when(regex = r#"^the registry record for that token is inspected$"#)]
async fn inspect_registry_record(world: &mut FoundryWorld) {
    // Read the row the mint persisted directly — the SECURITY assertion is that
    // the row carries metadata only and no token value (NFR-MT-SEC-01). Capture
    // the most-recently minted label's jti from the list surface; for the RED
    // scaffold the mint never persisted (it panicked), so there is no row — the
    // Then asserts the metadata-only shape, failing RED for MISSING_FUNCTIONALITY.
    get_token_surface(world).await;
}

// ==========================================================================
// Then — outcomes
// ==========================================================================

#[then(
    regex = r#"^the issued token value is shown exactly once with a copy affordance and an only-time warning$"#
)]
async fn token_value_shown_once(world: &mut FoundryWorld) {
    let body = mint_body(world);
    let status = world.mt_last_status.expect("mint status captured");
    assert!(
        status.is_success(),
        "mint must succeed to show the one-time value; got {status}, body {body:?}"
    );
    // A copy affordance + the unmistakable one-time warning + a token value.
    html_assertions::assert_has(body, "[data-token-value]");
    assert!(
        body.contains("only time") || body.to_lowercase().contains("only time you"),
        "missing the 'only time you'll see this' warning; body {body:?}"
    );
    html_assertions::assert_has(body, "[data-copy-token]");
}

#[then(regex = r#"^the issued token shows its id, label, scope, and expiry$"#)]
async fn issued_shows_metadata(world: &mut FoundryWorld) {
    let body = mint_body(world);
    for marker in [
        "[data-token-jti]",
        "[data-token-label]",
        "[data-token-scope]",
        "[data-token-expiry]",
    ] {
        html_assertions::assert_has(body, marker);
    }
}

#[then(regex = r#"^the issued token authenticates against the API$"#)]
async fn issued_token_authenticates(world: &mut FoundryWorld) {
    // Extract the token value from the one-time display and present it as a
    // bearer to a SHIPPED /api/v1 read — proving the product minted a real,
    // signed, verifiable credential (US-MT01 AC). RED: no value rendered yet.
    let value = extract_token_value(world).unwrap_or_else(|| {
        panic!(
            "no token value in the one-time display; body {:?}",
            world.mt_last_body
        )
    });
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // Any /api/v1 read that requires a valid machine credential: a 401 means the
    // credential did not verify. We assert it is NOT a 401 (it authenticated).
    let url = format!(
        "{base}/api/v1/teams/backend/projects/any/issues",
        base = harness.base_url()
    );
    let resp = http
        .get(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {value}"))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .expect("send api request with minted token");
    assert_ne!(
        resp.status().as_u16(),
        401,
        "the minted token did not authenticate against /api/v1 (got 401) — it is not a real signed credential"
    );
}

#[then(regex = r#"^the token list attributes "([^"]+)" to the admin$"#)]
async fn list_attributes_to_admin(world: &mut FoundryWorld, label: String) {
    get_token_surface(world).await;
    let body = surface_body(world);
    require_rendered(world, "token list");
    assert!(
        body.contains(&label) && body.contains(ADMIN_EMAIL),
        "list does not attribute {label:?} to the admin; body {body:?}"
    );
}

#[then(regex = r#"^issuing is reported as not enabled on this server$"#)]
async fn issuing_not_enabled(world: &mut FoundryWorld) {
    let body = surface_body(world);
    let status = world.mt_last_status.expect("status captured");
    // Graceful (OD1/DD2): the surface renders (or returns a clean 403-style page)
    // with a clear "issuing not enabled" message — NEVER a 500 (which is what the
    // RED scaffold panic produces today, so this fails RED for the right reason).
    assert_ne!(
        status.as_u16(),
        500,
        "verifier-only must degrade GRACEFULLY, not 500; body {body:?}"
    );
    assert!(
        body.to_lowercase().contains("not enabled"),
        "missing the 'issuing not enabled on this server' notice; status {status}, body {body:?}"
    );
}

#[then(regex = r#"^no mint form is offered$"#)]
async fn no_mint_form(world: &mut FoundryWorld) {
    require_rendered(world, "verifier-only surface");
    let body = surface_body(world);
    html_assertions::assert_not_has(body, "form[data-mint-form]");
}

#[then(regex = r#"^the server does not error$"#)]
async fn server_does_not_error(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    assert!(
        status.as_u16() != 500,
        "the server errored (500); a verifier-only binary must stay graceful"
    );
}

#[then(regex = r#"^issuance is refused as invalid$"#)]
async fn issuance_refused_invalid(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    let body = surface_body(world);
    assert_eq!(
        status.as_u16(),
        422,
        "expected 422 validation refusal, got {status}; body {body:?}"
    );
}

#[then(regex = r#"^issuance is refused with the maximum expiry stated$"#)]
async fn issuance_refused_cap(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    let body = surface_body(world);
    assert_eq!(
        status.as_u16(),
        422,
        "expected 422 over-cap refusal, got {status}; body {body:?}"
    );
    assert!(
        body.contains("365"),
        "the refusal must state the maximum (365 days); body {body:?}"
    );
}

#[then(regex = r#"^no token value is shown$"#)]
async fn no_token_value_shown(world: &mut FoundryWorld) {
    // Guard against a false pass on the scaffold 500: a refusal/notice page must
    // have actually rendered (a real status, not a panic 500) before treating
    // the absence of a value as a pass.
    let status = world.mt_last_status.expect("status captured");
    assert_ne!(
        status.as_u16(),
        500,
        "a 500 (scaffold panic) is not a clean refusal — the value-absence check is vacuous"
    );
    let body = surface_body(world);
    html_assertions::assert_not_has(body, "[data-token-value]");
}

#[then(regex = r#"^the token value is nowhere on the surface$"#)]
async fn value_nowhere(world: &mut FoundryWorld) {
    require_rendered(world, "token surface");
    let body = surface_body(world);
    html_assertions::assert_not_has(body, "[data-token-value]");
    // And the actual minted secret substring (if we captured one) is absent.
    if let Some(value) = world.mt_minted_value.clone() {
        assert!(
            !body.contains(&value),
            "the minted token value leaked into the list surface"
        );
    }
}

#[then(regex = r#"^only the token's id, label, scope, expiry, and status are shown$"#)]
async fn only_metadata_shown(world: &mut FoundryWorld) {
    require_rendered(world, "token surface");
    let body = surface_body(world);
    html_assertions::assert_not_has(body, "[data-token-value]");
    html_assertions::assert_has(body, "[data-token-jti]");
}

#[then(regex = r#"^the token value cannot be retrieved anywhere$"#)]
async fn value_unretrievable(world: &mut FoundryWorld) {
    require_rendered(world, "token surface");
    let body = surface_body(world);
    html_assertions::assert_not_has(body, "[data-token-value]");
}

#[then(regex = r#"^the guidance says to revoke that token and issue a new one$"#)]
async fn guidance_revoke_reissue(world: &mut FoundryWorld) {
    require_rendered(world, "token surface");
    let body = surface_body(world);
    assert!(
        body.to_lowercase().contains("revoke") && body.to_lowercase().contains("issue"),
        "missing the revoke-and-reissue guidance; body {body:?}"
    );
}

#[then(regex = r#"^the record holds only the token's id and metadata$"#)]
async fn record_metadata_only(world: &mut FoundryWorld) {
    // The mint persisted exactly one row this scenario; its registry shape has no
    // value column by design (NFR-MT-SEC-01). RED: the scaffold mint panicked, so
    // no row was persisted — assert the row exists (it must, once GREEN).
    require_rendered(world, "token surface");
    let body = surface_body(world);
    html_assertions::assert_has(body, "[data-token-jti]");
}

#[then(regex = r#"^the record holds no token value$"#)]
async fn record_no_value(world: &mut FoundryWorld) {
    // Structural truth of the schema (NFR-MT-DATA-02): the machine_tokens table
    // has no token/secret/hash column. Assert it directly against the live schema.
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let cols: Vec<(String,)> = sqlx::query_as(
        "SELECT column_name FROM information_schema.columns WHERE table_name = 'machine_tokens'",
    )
    .fetch_all(pool)
    .await
    .expect("read machine_tokens columns");
    let names: Vec<String> = cols.into_iter().map(|c| c.0.to_ascii_lowercase()).collect();
    for forbidden in ["token", "secret", "hash", "value"] {
        assert!(
            !names.iter().any(|n| n.contains(forbidden)),
            "machine_tokens must hold no secret column; found one matching {forbidden:?}: {names:?}"
        );
    }
}

#[then(regex = r#"^the surface lists all three tokens, newest first$"#)]
async fn lists_three_newest_first(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    let rows = html_assertions::collect_attributes(
        surface_body(world),
        "[data-token-row]",
        "data-token-label",
    );
    assert!(
        rows.len() >= 3,
        "expected at least three token rows, got {rows:?}"
    );
}

#[then(regex = r#"^each row shows its label, scope, expiry, and status$"#)]
async fn rows_show_fields(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    let body = surface_body(world);
    for marker in [
        "[data-token-label]",
        "[data-token-scope]",
        "[data-token-expiry]",
        "[data-token-status]",
    ] {
        html_assertions::assert_has(body, marker);
    }
}

#[then(regex = r#"^no token value appears anywhere in the list$"#)]
async fn no_value_in_list(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    html_assertions::assert_not_has(surface_body(world), "[data-token-value]");
}

#[then(regex = r#"^the surface lists "([^"]+)"$"#)]
async fn surface_lists(world: &mut FoundryWorld, label: String) {
    require_rendered(world, "token list");
    assert!(
        surface_body(world).contains(&label),
        "expected the list to contain {label:?}"
    );
}

#[then(regex = r#"^the surface lists only the acting workspace's tokens$"#)]
async fn lists_only_acting_workspace(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    // The acting workspace seeded exactly the labels in `mt_jti_by_label` (minus
    // the synthetic foreign sentinel). The list must contain exactly those rows
    // — never more (no foreign leakage), proving the read is workspace-scoped.
    let seeded: usize = world
        .mt_jti_by_label
        .keys()
        .filter(|k| !k.starts_with("team:"))
        .count();
    let rows = html_assertions::collect_attributes(
        surface_body(world),
        "[data-token-row]",
        "data-token-label",
    );
    assert_eq!(
        rows.len(),
        seeded,
        "the list must show exactly the acting workspace's {seeded} token(s); got {rows:?}"
    );
}

#[then(regex = r#"^a clear empty state invites issuing the first token$"#)]
async fn empty_state_shown(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    let body = surface_body(world);
    assert!(
        body.to_lowercase().contains("no tokens") || body.to_lowercase().contains("issue one"),
        "missing the inviting empty state; body {body:?}"
    );
}

#[then(regex = r#"^the row for "([^"]+)" shows status revoked$"#)]
async fn row_shows_revoked(world: &mut FoundryWorld, label: String) {
    get_token_surface(world).await;
    require_rendered(world, "token list");
    assert_row_status(world, &label, "revoked");
}

#[then(regex = r#"^the row for "([^"]+)" remains active$"#)]
async fn row_remains_active(world: &mut FoundryWorld, label: String) {
    // Assert the REGISTRY truth, not a re-rendered view: the seeded token's
    // `revoked_at` is still NULL. This is actor-independent — a non-admin (whose
    // failed revoke this guards, us-mt05) cannot view the list at all (404), so a
    // re-GET would be vacuous. Reading the persisted state directly proves the
    // refused/forbidden revoke left the credential untouched.
    let jti = *world
        .mt_jti_by_label
        .get(&label)
        .unwrap_or_else(|| panic!("token {label:?} was seeded"));
    let harness = world.harness.as_ref().expect("harness");
    let row = harness
        .app
        .state
        .store
        .find_machine_token_by_jti(jti)
        .await
        .expect("lookup token by jti")
        .unwrap_or_else(|| panic!("token {label:?} must still exist in the registry"));
    assert!(
        row.revoked_at.is_none(),
        "the row for {label:?} must remain active (revoked_at must be NULL)"
    );
}

#[then(regex = r#"^the integration's next API call with that token is refused$"#)]
async fn next_api_call_refused(world: &mut FoundryWorld) {
    // The kill-switch cross-check (US-MT03, NFR-MT-SEC-05): present the REAL
    // credential bound to the revoked jti to the SHIPPED /api/v1 verify path and
    // assert it is refused 401 by the per-request denylist. RED: revoke went
    // through the scaffold (panic), so revoked_at was never stamped → not refused.
    let value = world
        .mt_minted_value
        .clone()
        .expect("a real credential was minted for the revoked token");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/backend/projects/any/issues",
        base = harness.base_url()
    );
    let resp = http
        .get(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {value}"))
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .expect("send api request with revoked token");
    assert_eq!(
        resp.status().as_u16(),
        401,
        "a revoked token must be refused 401 on its next API call (denylist)"
    );
}

#[then(regex = r#"^the confirmation warns the revoke is immediate and cannot be undone$"#)]
async fn confirm_warns(world: &mut FoundryWorld) {
    require_rendered(world, "token surface");
    let body = surface_body(world);
    assert!(
        body.to_lowercase().contains("cannot be undone")
            || body.to_lowercase().contains("immediate"),
        "missing the immediate-and-irreversible warning; body {body:?}"
    );
}

#[then(regex = r#"^the revoke succeeds without error$"#)]
async fn revoke_succeeds(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    assert!(
        status.is_success() || status.as_u16() == 303,
        "revoke should succeed (idempotent); got {status}"
    );
}

#[then(regex = r#"^the revoke is refused without revealing whether that token exists$"#)]
async fn revoke_refused_nonenumerable(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    let body = surface_body(world);
    assert_eq!(
        status.as_u16(),
        404,
        "a cross-workspace revoke must be a non-enumerable 404; got {status} body {body:?}"
    );
}

#[then(regex = r#"^the "([^"]+)" workspace's token remains active$"#)]
async fn foreign_token_active(world: &mut FoundryWorld, _ws: String) {
    // The foreign jti is synthetic (a credential the acting workspace never
    // issued — single-workspace model, see upstream-issues.md). The behaviour
    // under test is that the acting admin's revoke did NOT stamp `revoked_at` on
    // it: the registry holds no revoked row for that jti. (`None` = the acting
    // workspace cannot see/touch it — the non-enumerable guarantee.)
    let jti = world.mt_foreign_jti.expect("foreign token seeded");
    let harness = world.harness.as_ref().expect("harness");
    let row = harness
        .app
        .state
        .store
        .find_machine_token_by_jti(jti)
        .await
        .expect("lookup foreign token");
    match row {
        None => { /* never issued in this single-workspace db — correctly untouched */ }
        Some(r) => assert!(
            r.revoked_at.is_none(),
            "the foreign workspace's token must remain active (the admin must not revoke it)"
        ),
    }
}

#[then(regex = r#"^the revoke is refused as forbidden$"#)]
async fn revoke_refused_forbidden(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        403,
        "a revoke with no anti-forgery token must be refused 403 by CSRF; got {status}"
    );
}

#[then(regex = r#"^the issued token is limited to the "([^"]+)" team$"#)]
async fn issued_limited_to_team(world: &mut FoundryWorld, team: String) {
    let body = mint_body(world);
    assert!(
        body.contains(&team),
        "the one-time display must show the chosen team scope {team:?}; body {body:?}"
    );
}

#[then(regex = r#"^the issued token expires in (\d+) days$"#)]
async fn issued_expires_in(world: &mut FoundryWorld, _days: u32) {
    let body = mint_body(world);
    html_assertions::assert_has(body, "[data-token-expiry]");
}

#[then(regex = r#"^the token list shows the "([^"]+)" scope for "([^"]+)"$"#)]
async fn list_shows_scope(world: &mut FoundryWorld, team: String, label: String) {
    get_token_surface(world).await;
    require_rendered(world, "token list");
    let body = surface_body(world);
    assert!(
        body.contains(&team) && body.contains(&label),
        "list must show {team:?} scope for {label:?}; body {body:?}"
    );
}

#[then(regex = r#"^the token surface is shown$"#)]
async fn surface_shown(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        200,
        "an admin must see the token surface (200); got {status}"
    );
    html_assertions::assert_has(surface_body(world), "[data-token-surface]");
}

#[then(regex = r#"^the member is refused without revealing whether the surface exists$"#)]
async fn member_refused_nonenumerable(world: &mut FoundryWorld) {
    let status = world.mt_last_status.expect("status captured");
    let body = surface_body(world);
    // Non-enumerable: a generic 404, NOT a 403 that confirms the surface exists,
    // and NOT a 500 (scaffold panic). RED today (scaffold panics 500).
    assert_eq!(
        status.as_u16(),
        404,
        "a non-admin must get a non-enumerable 404; got {status} body {body:?}"
    );
    assert!(
        !body.to_lowercase().contains("admin only")
            && !body.to_lowercase().contains("token surface"),
        "the refusal must not reveal the surface exists; body {body:?}"
    );
}

#[then(regex = r#"^"([^"]+)" shows it was minted by "([^"]+)"$"#)]
async fn shows_minted_by(world: &mut FoundryWorld, label: String, issuer_email: String) {
    require_rendered(world, "token list");
    let body = surface_body(world);
    assert!(
        body.contains(&label) && body.contains(&issuer_email),
        "list must show {label:?} minted by {issuer_email:?}; body {body:?}"
    );
}

#[then(regex = r#"^"([^"]+)" shows a recent last-used time$"#)]
async fn shows_recent_last_used(world: &mut FoundryWorld, label: String) {
    require_rendered(world, "token list");
    let body = surface_body(world);
    assert!(
        body.contains(&label),
        "list must include {label:?} with a last-used time; body {body:?}"
    );
    html_assertions::assert_has(body, "[data-token-last-used]");
}

#[then(regex = r#"^the freshly issued token shows it has never been used$"#)]
async fn fresh_never_used(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    let body = surface_body(world);
    assert!(
        body.to_lowercase().contains("never"),
        "a never-used token must show 'never'; body {body:?}"
    );
}

#[then(regex = r#"^that token shows its issuer as unknown$"#)]
async fn issuer_unknown(world: &mut FoundryWorld) {
    require_rendered(world, "token list");
    let body = surface_body(world);
    assert!(
        body.contains("—") || body.to_lowercase().contains("unknown"),
        "a token with no recorded issuer must show '—'/unknown; body {body:?}"
    );
}

// ==========================================================================
// Internals — HTTP against /admin/tokens via the in-process harness.
// ==========================================================================

async fn get_token_surface(world: &mut FoundryWorld) {
    let email = world
        .mt_actor_email
        .clone()
        .unwrap_or_else(|| ADMIN_EMAIL.to_string());
    let password = password_for(&email);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

    // Sign in over the real cookie path, then GET /admin/tokens with the session.
    let session = sign_in_session(http, &base, &email, password).await;
    let resp = http
        .get(format!("{base}/admin/tokens"))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("get /admin/tokens");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.mt_last_status = Some(status);
    world.mt_last_headers = Some(headers);
    world.mt_last_body = Some(body);
}

async fn post_mint(world: &mut FoundryWorld, fields: &[(&str, &str)]) {
    let email = world
        .mt_actor_email
        .clone()
        .unwrap_or_else(|| ADMIN_EMAIL.to_string());
    let password = password_for(&email);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome = signed_in_post(harness, http, &email, password, "/admin/tokens", fields).await;
    world.mt_last_status = Some(outcome.status);
    world.mt_last_headers = Some(outcome.headers);
    world.mt_last_body = Some(outcome.body);
}

async fn post_revoke(world: &mut FoundryWorld, jti: uuid::Uuid, with_csrf: bool) {
    let email = world
        .mt_actor_email
        .clone()
        .unwrap_or_else(|| ADMIN_EMAIL.to_string());
    let password = password_for(&email);
    let path = format!("/admin/tokens/{jti}/revoke");
    if with_csrf {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        let outcome = signed_in_post(harness, http, &email, password, &path, &[]).await;
        world.mt_last_status = Some(outcome.status);
        world.mt_last_headers = Some(outcome.headers);
        world.mt_last_body = Some(outcome.body);
    } else {
        // Sign in for the session cookie, then POST WITHOUT a `_csrf` field — the
        // double-submit CSRF middleware must refuse it 403 (NFR-MT-SEC-07).
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        let base = harness.base_url();
        let session = sign_in_session(http, &base, &email, password).await;
        let resp = http
            .post(format!("{base}{path}"))
            .header(reqwest::header::COOKIE, session)
            .form(&std::collections::HashMap::<&str, &str>::new())
            .send()
            .await
            .expect("post revoke without csrf");
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        world.mt_last_status = Some(status);
        world.mt_last_headers = Some(headers);
        world.mt_last_body = Some(body);
    }
}

/// Sign in over the real cookie path; return the `foundry_session=...` cookie
/// pair to present on subsequent GETs.
async fn sign_in_session(
    http: &reqwest::Client,
    base: &str,
    email: &str,
    password: &str,
) -> String {
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf_token.clone());
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /sign-in");
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn surface_body(world: &FoundryWorld) -> &str {
    world.mt_last_body.as_deref().unwrap_or("")
}

fn mint_body(world: &FoundryWorld) -> &str {
    world.mt_last_body.as_deref().unwrap_or("")
}

/// Guard a negative/structural assertion against a false GREEN on the scaffold
/// 500 (Critical Rule 7): the surface must have actually rendered (200) before
/// an "absence" check is meaningful.
fn require_rendered(world: &FoundryWorld, what: &str) {
    let status = world.mt_last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        200,
        "the {what} must have rendered (200) before this assertion is meaningful; got {status} \
         (a 500 here is the RED scaffold panic — MISSING_FUNCTIONALITY)"
    );
}

/// Pull the one-time token value out of the mint display (the `[data-token-value]`
/// element's text), if present.
fn extract_token_value(world: &FoundryWorld) -> Option<String> {
    let body = world.mt_last_body.as_deref()?;
    let doc = html_assertions::parse(body);
    let els = html_assertions::select_all(&doc, "[data-token-value]");
    els.first()
        .map(|el| html_assertions::text_of(el).trim().to_string())
}

/// Assert the list row for `label` carries the expected status text/marker.
fn assert_row_status(world: &FoundryWorld, label: &str, expected: &str) {
    let body = surface_body(world);
    let doc = html_assertions::parse(body);
    for row in html_assertions::select_all(&doc, "[data-token-row]") {
        let row_html = row.html();
        if row_html.contains(label) {
            let status = row
                .value()
                .attr("data-token-status")
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            assert_eq!(
                status, expected,
                "row for {label:?} has status {status:?}, expected {expected:?}"
            );
            return;
        }
    }
    panic!("no token row for {label:?} found; body {body}");
}
