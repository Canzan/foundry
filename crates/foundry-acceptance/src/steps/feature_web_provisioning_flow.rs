//! web-provisioning-flow (US-MWT07 / US-MWT08 WEB legs) step definitions.
//!
//! A NEW WEB DRIVING ADAPTER over the ALREADY-SHIPPED `Services::provision_workspace`
//! use-case + `is_instance_admin` authz. The CLI legs shipped in
//! us-mwt-slice-06-provision-and-prove; this feature adds the browser surface
//! (multi-workspace-provisioning ADR-002 D2 → realised HERE).
//!
//! Step 01-01 implements ONLY the `@walking_skeleton` scenario: a signed-in
//! instance super-admin submits the web provision form and a new isolated
//! workspace is created via the SHIPPED use-case; the rendered htmx success
//! fragment reports the new workspace id + the (informational, D5) first-admin
//! invite link.
//!
//! Driving adapter: the htmx web tier served by foundry-app over real HTTP — the
//! in-process axum router under the production session + double-submit CSRF
//! layers (mirrors `feature_mwt_slice_02_web_boundary` + the slice-06 provisioning
//! glue). REACHED at `POST /admin/instance/workspaces`, authenticated by a real
//! signed-in `foundry_session` cookie whose user is the bootstrap super-admin.
//!
//! World-state REUSE: the Background steps (`an instance claimed by super-admin …`)
//! are OWNED by `feature_mwt_slice_06_provision_and_prove` — cucumber-rs requires
//! globally-unique step text, so this module does NOT redefine them; it reads the
//! same `world.mwt6_*` slots the slice-06 Background populates (the harness + the
//! super-admin email). The walking-skeleton's `Then the new workspace "…" exists
//! and is isolated from all others` is ALSO owned by slice-06 (it reads
//! `world.mwt6_cli_exit == Some(0)` + the `mwt6_harness` pool) — so the web
//! provision `When` step below sets `mwt6_cli_exit = Some(0)` on success so that
//! SHIPPED isolation assertion runs unchanged against the web-provisioned tenant
//! (green-by-inheritance off the shipped use-case + isolation boundary).
//!
//! LAYER 3 (real adapter + real HTTP, @real-io @wiring_e2e): real Postgres via
//! testcontainers + per-scenario schema; the real tower-sessions Postgres store;
//! the real double-submit CSRF middleware; the SHIPPED `provision_workspace` tx +
//! `is_instance_admin` authz; the in-process axum router. Example-based
//! (Mandates 9 + 11) — no PBT at this layer. Assertions are traditional, over
//! port-exposed web observables: the rendered success-fragment substrings (new
//! workspace id + invite link) and the post-provision DB row presence (the shared
//! slice-06 isolation Then).

use crate::support::harness::{signed_in_post, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;

/// The password the slice-06 Background seeds the bootstrap super-admin with
/// (`instance_claimed_by_superadmin` → `create_initial_workspace` with the fixed
/// "ops-password"). The web sign-in path re-authenticates per request, so the
/// provision POST needs it.
const SUPERADMIN_PASSWORD: &str = "ops-password";

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .mwt6_harness
        .as_ref()
        .expect("the slice-06 Background must have spawned the mwt6 harness")
}

fn http(world: &mut FoundryWorld) -> reqwest::Client {
    if world.http.is_none() {
        world.http = Some(
            reqwest::Client::builder()
                .redirect(Policy::none())
                .cookie_store(false)
                .build()
                .expect("build reqwest client"),
        );
    }
    world.http.as_ref().expect("http client").clone()
}

// ---------------------------------------------------------------------------
// Background — NEW text (slice-06 owns the `an instance claimed by …` step)
// ---------------------------------------------------------------------------

/// `And "Acme" has a member "marco@acme.com"` — seed an ordinary (non-super-admin)
/// workspace member into the Background-claimed workspace, so later refusal
/// scenarios have a signed-in non-super-admin to drive (and so the walking
/// skeleton has a populated tenant to be isolated FROM). Reuses the SHIPPED store
/// rows directly (a user + a `member` membership), mirroring the slice-06 seed.
#[given(regex = r#"^"([^"]+)" has a member "([^"]+)"$"#)]
async fn workspace_has_member(world: &mut FoundryWorld, ws_name: String, member: String) {
    let workspace_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded by the Background first"));
    let pool = harness(world).app.state.store.pool().clone();

    let member_lower = member.to_ascii_lowercase();
    let pw = foundry_auth::hash_password(&SecretString::new("member-password".to_string().into()))
        .await
        .expect("hash member pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'Member', $4) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(&member_lower)
    .bind(&member)
    .bind(&pw)
    .execute(&pool)
    .await
    .expect("insert member user");
    let (member_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&member_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve member id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert member membership");
}

// ---------------------------------------------------------------------------
// Given — a signed-in super-admin on the web (NEW text)
// ---------------------------------------------------------------------------

/// `Given the super-admin is signed in on the web` — record that the acting web
/// persona is the bootstrap super-admin the slice-06 Background claimed. The web
/// sign-in itself happens per-request inside `signed_in_post` (the harness keeps
/// no cookie jar), authenticating against the SHIPPED cookie sign-in path with the
/// Background-seeded "ops-password".
#[given(regex = r#"^the super-admin is signed in on the web$"#)]
async fn super_admin_signed_in(world: &mut FoundryWorld) {
    // Confirm the Background established the super-admin (fail loud if not).
    assert!(
        world.mwt6_superadmin_email.is_some(),
        "the slice-06 Background must have claimed a super-admin first"
    );
    // Ensure the http client exists for the upcoming POST.
    let _ = http(world);
}

// ---------------------------------------------------------------------------
// When — submit the web provision form (NEW text)
// ---------------------------------------------------------------------------

/// `When the super-admin submits the provision form for workspace "<name>" with
/// first admin "<email>"` — drive the NEW web provisioning route over real HTTP:
/// sign in as the super-admin (cookie + CSRF), POST the form to
/// `/admin/instance/workspaces`, and capture the rendered success fragment.
///
/// On a 200 the web vertical succeeded end-to-end (session + CSRF → SHIPPED
/// use-case → rendered fragment); we set `mwt6_cli_exit = Some(0)` so the SHARED
/// slice-06 isolation `Then` (which gates on exit 0 + reads the `mwt6_harness`
/// pool) runs unchanged against the web-provisioned tenant.
#[when(
    regex = r#"^the super-admin submits the provision form for workspace "([^"]+)" with first admin "([^"]+)"$"#
)]
async fn submit_provision_form(world: &mut FoundryWorld, ws_name: String, admin_email: String) {
    let super_admin = world
        .mwt6_superadmin_email
        .clone()
        .expect("super-admin seeded in the Background");
    let client = http(world);
    let outcome = signed_in_post(
        harness(world),
        &client,
        &super_admin,
        SUPERADMIN_PASSWORD,
        "/admin/instance/workspaces",
        &[("name", ws_name.as_str()), ("email", admin_email.as_str())],
    )
    .await;

    world.mwt6_cli_stdout = Some(outcome.body.clone());
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
    if outcome.status == StatusCode::OK {
        // The shared slice-06 isolation Then gates on the CLI-exit slot; the web
        // vertical's "success" is a 200 rendered fragment, mapped to exit 0 so the
        // SHIPPED isolation assertion proves the web-provisioned tenant is real +
        // isolated (green-by-inheritance).
        world.mwt6_cli_exit = Some(0);
    } else {
        world.mwt6_cli_exit = Some(1);
    }
}

// ---------------------------------------------------------------------------
// Then — the rendered web fragment reports the new workspace + invite (NEW text)
// ---------------------------------------------------------------------------

/// `And the web page reports the new workspace and a first-admin invite link` —
/// the port-exposed web observable: the rendered htmx success fragment carries
/// the new workspace id (the same id the shared isolation Then resolved) and an
/// informational first-admin invite link (D5 — rendered, no sign-in via it
/// asserted).
#[then(regex = r#"^the web page reports the new workspace and a first-admin invite link$"#)]
async fn web_page_reports_workspace_and_invite(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the provision form POST must render a 200 success fragment; body = {:?}",
        world.last_body
    );
    let body = world
        .last_body
        .as_deref()
        .expect("a rendered success fragment was captured");
    let workspace_id = world
        .mwt6_provisioned_workspace_id
        .expect("the shared isolation Then resolved the new workspace id");
    assert!(
        body.contains(&workspace_id.to_string()),
        "the success fragment must report the new workspace id {workspace_id}; got {body:?}"
    );
    assert!(
        body.contains("/invites/accept?id=") && body.contains("data-first-admin-invite-link"),
        "the success fragment must report a first-admin invite link; got {body:?}"
    );
}
