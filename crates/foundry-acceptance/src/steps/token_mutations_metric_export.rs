//! Step definitions — `foundry_token_mutations_total` register-at-0 +
//! tick-on-mutation.
//!
//! Closes the last slice-8 register-at-0 gap: the per-principal
//! revoke-storm guardrail counter
//! `foundry_token_mutations_total{principal,outcome}` (emitted by
//! `RateLimiter::check` on every revoke decision) was wired and served on
//! `/metrics` but never registered/described at startup, so a fresh
//! instance scrape lacked the family until the first revoke (a Grafana
//! "no-data" panel). The startup registration in `main.rs` mints the
//! family at zero under a sentinel `system` principal.
//!
//! REUSED phrases (registered by slice-6 `handler_instrumentation.rs` —
//! NOT re-registered here):
//!   - `the operator's foundry instance is running`            (spawns the subprocess)
//!   - `the operator scrapes the metrics endpoint immediately` (caches a scrape)
//!   - `the scrape returns HTTP 200`
//!   - `the scrape body contains the line "{}"`
//!   - `the scrape body's "{}" samples carry only the label keys "{}"`
//!
//! REUSED from slice-8 `slice_8_deferred_metrics.rs`:
//!   - `the scrape body's "{}" sample is eventually at least {int} within {int} seconds`
//!     (generic monotonic-counter / gauge bounded-poll over `/metrics`)
//!
//! So this module registers ONLY the two NEW steps that exercise the live
//! emission path: the Given that mints a real management bearer + seeds a
//! revocable `machine_tokens` row, and the When that drives the real
//! `DELETE .../tokens/{jti}` against the subprocess (whose revoke decision
//! ticks the counter). The subprocess reuses the slice-1 Background schema
//! (`ensure_subprocess_running`), so the row this Given inserts via the
//! in-process harness pool is visible to the subprocess's live request.
//!
//! LAYER 3 (real adapter): example-based; the bucket-arithmetic PBT lives
//! at unit level in `crates/foundry-app/src/rate_limit.rs`.

#![allow(unused_imports)]

use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::time::Duration;

/// The main HTTP addr of the current scenario's subprocess.
fn current_main_addr(world: &FoundryWorld) -> SocketAddr {
    world
        .slice6_foundry
        .as_ref()
        .expect("the operator's foundry instance is running (subprocess spawned)")
        .main_addr
}

/// The slice-1 Background in-process harness pool — shares the schema the
/// subprocess reads, so a `machine_tokens` row inserted here is visible to
/// the subprocess's live revoke request.
fn harness_pool(world: &FoundryWorld) -> &PgPool {
    world
        .harness
        .as_ref()
        .expect("Background steps create the in-process harness")
        .app
        .state
        .store
        .pool()
}

/// Resolve `(user_id, workspace_id)` for an email against the shared
/// schema. The admin (devansh) is `is_workspace_admin`; binding the bearer
/// to the admin makes the eventual `revoke_token` authz succeed — though
/// the metric ticks regardless of the authz outcome (the rate guard fires
/// BEFORE `revoke_token`).
async fn user_and_workspace(world: &FoundryWorld, email: &str) -> (uuid::Uuid, uuid::Uuid) {
    let pool = harness_pool(world);
    let lower = email.to_ascii_lowercase();
    let user: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&lower)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("resolve user {email:?}: {e}"));
    let ws: (uuid::Uuid,) =
        sqlx::query_as("SELECT workspace_id FROM workspace_memberships WHERE user_id = $1 LIMIT 1")
            .bind(user.0)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|e| panic!("resolve workspace for {email:?}: {e}"));
    (user.0, ws.0)
}

/// Resolve `(team_slug, project_slug)` for a project name in a team.
async fn team_and_project_slug(
    world: &FoundryWorld,
    project_name: &str,
    team_name: &str,
) -> (String, String) {
    let pool = harness_pool(world);
    let row: (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug
           FROM projects p
           JOIN teams t ON t.id = p.team_id
          WHERE p.name = $1 AND t.name = $2
          LIMIT 1",
    )
    .bind(project_name)
    .bind(team_name)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| {
        panic!("resolve slug for project {project_name:?} in team {team_name:?}: {e}")
    });
    row
}

// =====================================================================
// Given — mint a real management bearer + seed a revocable token
// =====================================================================

/// Mint a REAL EdDSA bearer bound to the admin's `(user_id, workspace_id)`
/// (signed with `foundry_auth::test_keys::signer()`, whose public half is
/// the subprocess's configured `MACHINE_TOKEN_PUBLIC_KEYS`), and register a
/// `machine_tokens` row for it so the per-request jti denylist admits it.
/// The bearer's own jti is the revoke target (a revoke-self), so the
/// `DELETE` resolves to a REAL, owned token — the revoke decision ticks the
/// counter for this principal.
#[given(
    regex = r#"^a management bearer for "([^"]+)" with a revocable token in the "([^"]+)" team's "([^"]+)" project exists$"#
)]
async fn given_management_bearer_with_revocable_token(
    world: &mut FoundryWorld,
    bound_email: String,
    team_name: String,
    project_name: String,
) {
    let (user_id, workspace_id) = user_and_workspace(world, &bound_email).await;
    let (team_slug, project_slug) = team_and_project_slug(world, &project_name, &team_name).await;

    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::hours(1);

    // Register the credential's row so `Services::resolve_active_token`
    // admits the bearer (an unregistered jti would 401 before the revoke
    // and never reach the rate guard).
    world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .insert_machine_token(
            jti,
            user_id,
            workspace_id,
            None,
            exp,
            "token-mutations-metric-revoke-target",
            user_id,
        )
        .await
        .expect("register revocable machine token");

    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: None,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    let jwt = foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint management bearer")
        .expose_secret()
        .to_string();

    world.tmm_revoke_target = Some((jwt, team_slug, project_slug, jti));
}

// =====================================================================
// When — drive a real revoke against the subprocess
// =====================================================================

/// `DELETE /api/v1/teams/{team}/projects/{project}/tokens/{jti}` with the
/// minted bearer. The handler runs `MachinePrincipal` auth, then
/// `rate_guard.check_revoke(principal.user_id())` — which calls
/// `RateLimiter::check` and increments
/// `foundry_token_mutations_total{principal,outcome}` — BEFORE the
/// use-case. So the counter ticks regardless of the eventual 204/403/404.
#[when(expr = "the management bearer revokes that token over the API")]
async fn when_management_bearer_revokes_token(world: &mut FoundryWorld) {
    let (jwt, team_slug, project_slug, jti) = world
        .tmm_revoke_target
        .clone()
        .expect("a revocable token was seeded by the prior Given");
    let base = format!("http://{}", current_main_addr(world));
    let url = format!("{base}/api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}");
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build revoke http client");
    let resp = http
        .delete(&url)
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {jwt}"))
        .send()
        .await
        .expect("DELETE revoke against subprocess");
    // The status is intentionally NOT asserted: the metric ticks on the
    // rate-guard decision that fires BEFORE the use-case authz, so any
    // status (204 success / 403 / 404 non-enumerable) still produced the
    // observable counter increment the Then asserts via the scrape.
    let _ = resp.text().await;
}
