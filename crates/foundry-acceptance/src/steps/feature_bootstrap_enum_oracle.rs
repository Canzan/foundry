//! bootstrap-claim-enumeration-oracle (US-01/02/03) step definitions.
//!
//! Closes the DOWNSTREAM enumeration oracle on the bootstrap claim POST: an
//! email-uniqueness collision (SQLSTATE 23505 on the users insert) currently
//! surfaces as a 500 — distinguishable from the 303 success — and burns the
//! single-use token. This module drives the SAME real `POST /bootstrap` driving
//! port the US-05 walking skeleton uses (the in-process axum harness over real
//! HTTP + real Postgres), and adds ONLY the collision-specific Given/When/Then;
//! the shared token-refusal steps (Background mint, prior claim, expired/unknown
//! submit, byte-identity + reveal-nothing Thens, dashboard redirect) are REUSED
//! verbatim from `us_05_bootstrap.rs` (cucumber-rs matches steps globally across
//! every registered step fn), keeping one vocabulary across the two files
//! (Pillars 1 + 2).
//!
//! LAYER 3 (real adapter + real HTTP, @real-io): example-based (Mandates 9 + 11)
//! — no PBT at this layer; assertions are traditional, over port-exposed
//! observables: the HTTP status + full body of the refusal (byte-identity), the
//! 303 + workspace/instance-admin seed at the driven-port (Postgres) boundary,
//! and the token's `used_at` after a collision (the atomic-rollback proof).

use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

fn sha256(s: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().into()
}

/// Translate a Gherkin token name (minted in the Background) back to its raw
/// value; a never-minted literal is used verbatim (the Unknown arm).
fn raw_token(world: &FoundryWorld, name: &str) -> String {
    world
        .minted_tokens
        .get(name)
        .cloned()
        .unwrap_or_else(|| name.to_string())
}

/// POST a bootstrap claim form for `token_name` with the given `email` (and fixed
/// remaining fields) against the REAL `/bootstrap` endpoint, capturing the full
/// (status, headers, body) into the world's `last_*` slots.
async fn post_claim(world: &mut FoundryWorld, token_name: &str, email: &str, workspace_name: &str) {
    let raw = raw_token(world, token_name);
    let url = format!("/bootstrap?token={}", urlencoding::encode(&raw));
    let mut form: HashMap<&str, &str> = HashMap::new();
    form.insert("email", email);
    form.insert("password", "correct horse battery staple");
    form.insert("display_name", "Claimant");
    form.insert("workspace_name", workspace_name);

    let http = world.http.as_ref().expect("http client").clone();
    let base = world.harness.as_ref().expect("harness").base_url();
    let resp = http
        .post(format!("{base}{url}"))
        .form(&form)
        .send()
        .await
        .expect("submit bootstrap claim");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
}

// --- US-01: colliding-email claim (drives the collision arm) ------------------

/// `a visitor submits the bootstrap claim for "<token>" using the already-registered
/// email "<email>"` — drive a claim whose email ALREADY maps to a user (created by a
/// prior successful claim). Registered as BOTH a `When` (the US-01 non-enumerability
/// action) and a `Given` (the US-02 chained precondition that sets up the collision
/// before the token/retry is observed). Pushes the captured (status, body) onto
/// `bootstrap_refusals` in the SAME shape the reused us-05 expired/unknown submit
/// steps use, so the shared byte-identity Then compares all three arms.
#[given(
    regex = r#"^a visitor submits the bootstrap claim for "([^"]+)" using the already-registered email "([^"]+)"$"#
)]
#[when(
    regex = r#"^a visitor submits the bootstrap claim for "([^"]+)" using the already-registered email "([^"]+)"$"#
)]
async fn submit_collision_claim(world: &mut FoundryWorld, token_name: String, email: String) {
    post_claim(world, &token_name, &email, "Collision WS").await;
    let status = world.last_status.expect("status captured");
    let body = world.last_body.clone().unwrap_or_default();
    world.bootstrap_refusals.push((status, body));
}

// --- US-02: token reusability (the atomic-rollback proof) ---------------------

/// `the bootstrap token "<token>" remains unconsumed` — the DB-observable
/// atomic-rollback outcome: after a colliding claim the token's `used_at` is STILL
/// NULL (the claim+create rolled back together), so exactly one UNCONSUMED row
/// exists for its hash. RED today: the token is claimed BEFORE the create runs, so
/// a collision leaves `used_at` set (the token is burned).
#[then(regex = r#"^the bootstrap token "([^"]+)" remains unconsumed$"#)]
async fn token_remains_unconsumed(world: &mut FoundryWorld, token_name: String) {
    let hash = sha256(&raw_token(world, &token_name));
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let (unconsumed,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM bootstrap_tokens WHERE token_hash = $1 AND used_at IS NULL",
    )
    .bind(hash.as_slice())
    .fetch_one(pool)
    .await
    .expect("count the unconsumed bootstrap token");
    assert_eq!(
        unconsumed, 1,
        "the bootstrap token {token_name:?} must remain UNCONSUMED after a colliding \
         claim (used_at NULL — the claim+create rolled back atomically); found \
         {unconsumed} unconsumed rows for its hash"
    );
}

/// `the visitor retries "<token>" with the fresh email "<email>" and workspace
/// "<workspace>"` — the user-facing recovery: reuse the SAME (now-unconsumed) token
/// with a corrected, unregistered email. Captures the response into `last_*` so the
/// reused us-05 dashboard-redirect Then + the instance-admin Then can observe the
/// success. RED today: the token was burned by the prior collision, so this is
/// refused (200) instead of redirecting (303).
#[when(
    regex = r#"^the visitor retries "([^"]+)" with the fresh email "([^"]+)" and workspace "([^"]+)"$"#
)]
async fn retry_with_fresh_email(
    world: &mut FoundryWorld,
    token_name: String,
    email: String,
    workspace_name: String,
) {
    post_claim(world, &token_name, &email, &workspace_name).await;
}

// --- US-03: happy-path seed regression (instance admin) -----------------------

/// `the workspace "<name>" exists with a first instance admin` — the D1 seed
/// regression guard: the named workspace exists exactly once AND its admin was
/// seeded as an `instance_admins` row in the SAME claim transaction (the operator
/// is both workspace admin and the first instance super-admin). Reads the REAL
/// per-scenario Postgres at the driven-port boundary. This is the seed that must
/// stay green through the store rewire (and, on the US-02 retry, the observable
/// that the reused token minted a real workspace).
#[then(regex = r#"^the workspace "([^"]+)" exists with a first instance admin$"#)]
async fn workspace_has_first_instance_admin(world: &mut FoundryWorld, ws_name: String) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let (ws_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspaces WHERE name = $1")
        .bind(&ws_name)
        .fetch_one(pool)
        .await
        .expect("count the seeded workspace");
    assert_eq!(
        ws_count, 1,
        "the workspace {ws_name:?} must exist exactly once after the claim; found \
         {ws_count} rows"
    );

    let (admin_instance_rows,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM instance_admins ia
           JOIN workspace_memberships wm ON wm.user_id = ia.user_id
           JOIN workspaces w ON w.id = wm.workspace_id
          WHERE w.name = $1 AND wm.role = 'admin'",
    )
    .bind(&ws_name)
    .fetch_one(pool)
    .await
    .expect("count the seeded first instance admin");
    assert!(
        admin_instance_rows >= 1,
        "the {ws_name:?} claim must seed its admin as the first instance super-admin \
         (an instance_admins row); found {admin_instance_rows}"
    );
}
