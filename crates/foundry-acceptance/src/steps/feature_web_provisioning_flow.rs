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

use crate::support::harness::{signed_in_get, signed_in_post, InProcHarness};
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
// When — open the instance dashboard (GET, step 01-02)
// ---------------------------------------------------------------------------

/// `When the super-admin opens the instance dashboard on the web` — drive the
/// NEW session-gated `GET /admin/instance/workspaces` over real HTTP: sign in as
/// the super-admin (cookie) and GET the full dashboard page (no CSRF needed for a
/// read). Captures the rendered page for the `Then` assertions below.
#[when(regex = r#"^the super-admin opens the instance dashboard on the web$"#)]
async fn open_instance_dashboard(world: &mut FoundryWorld) {
    let super_admin = world
        .mwt6_superadmin_email
        .clone()
        .expect("super-admin seeded in the Background");
    let client = http(world);
    let outcome = signed_in_get(
        harness(world),
        &client,
        &super_admin,
        SUPERADMIN_PASSWORD,
        "/admin/instance/workspaces",
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
}

// ---------------------------------------------------------------------------
// Then — the dashboard renders the workspace list + both forms (step 01-02)
// ---------------------------------------------------------------------------

/// `Then the dashboard lists the existing workspaces` — the full-page (no-JS)
/// entry point renders a 200 page that NAMES every workspace the Background
/// seeded (the port-exposed observable: the rendered page body contains each
/// existing workspace name).
#[then(regex = r#"^the dashboard lists the existing workspaces$"#)]
async fn dashboard_lists_workspaces(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the dashboard GET must render a 200 full page; body = {:?}",
        world.last_body
    );
    let body = world
        .last_body
        .as_deref()
        .expect("a rendered dashboard page was captured");
    assert!(
        !world.mwt6_workspace_ids.is_empty(),
        "the Background must have seeded at least one workspace"
    );
    for ws_name in world.mwt6_workspace_ids.keys() {
        assert!(
            body.contains(ws_name),
            "the dashboard must list the existing workspace {ws_name:?}; got {body:?}"
        );
    }
}

/// `And the dashboard offers a provision-workspace form and a grant-super-admin
/// form` — both state-changing forms are present, each POSTing to its route and
/// each carrying a valid double-submit `_csrf` field (the port-exposed observable
/// of the no-JS surface: the rendered forms' actions + hidden CSRF inputs).
#[then(regex = r#"^the dashboard offers a provision-workspace form and a grant-super-admin form$"#)]
async fn dashboard_offers_both_forms(world: &mut FoundryWorld) {
    let body = world
        .last_body
        .as_deref()
        .expect("a rendered dashboard page was captured");
    assert!(
        body.contains(r#"action="/admin/instance/workspaces""#)
            && body.contains("data-provision-form"),
        "the dashboard must offer a provision-workspace form; got {body:?}"
    );
    assert!(
        body.contains(r#"action="/admin/instance/super-admins""#)
            && body.contains("data-grant-form"),
        "the dashboard must offer a grant-super-admin form; got {body:?}"
    );
    // Each form carries a valid (non-empty) double-submit CSRF token field.
    let csrf_fields = body.matches(r#"name="_csrf""#).count();
    assert!(
        csrf_fields >= 2,
        "each of the two forms must carry a hidden _csrf field (found {csrf_fields}); got {body:?}"
    );
    assert!(
        !body.contains(r#"value=""></"#) && !body.contains(r#"name="_csrf" value="">"#),
        "the _csrf token must be a non-empty value; got {body:?}"
    );
}

// ---------------------------------------------------------------------------
// Given — an existing non-super-admin member to be granted (step 01-03)
// ---------------------------------------------------------------------------

/// `And "<email>" is an existing member who is not a super-admin` — seed an
/// ordinary `users` row + a `member` membership in the Background-claimed
/// workspace, asserting the user is NOT yet an `instance_admins` row. The grant
/// target must already be a user (you cannot grant a non-existent user — the
/// SHIPPED `user_id_by_email` resolve precedes the grant). Mirrors
/// `workspace_has_member`; the `is_instance_admin == false` precondition is the
/// state the `Then "<email>" is now a super-admin` step flips.
#[given(regex = r#"^"([^"]+)" is an existing member who is not a super-admin$"#)]
async fn member_not_super_admin(world: &mut FoundryWorld, member: String) {
    let (ws_name, workspace_id) = world
        .mwt6_workspace_ids
        .iter()
        .next()
        .map(|(name, id)| (name.clone(), *id))
        .expect("the Background must have seeded at least one workspace");
    let _ = ws_name;
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
    .expect("insert grant-target user");
    let (member_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&member_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve grant-target id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert grant-target membership");

    // Precondition: the target is NOT yet a super-admin (the state the grant flips).
    let (is_admin,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM instance_admins WHERE user_id = $1)")
            .bind(member_id)
            .fetch_one(&pool)
            .await
            .expect("probe instance_admins precondition");
    assert!(
        !is_admin,
        "the grant target {member:?} must NOT be a super-admin before the grant"
    );
}

// ---------------------------------------------------------------------------
// When — submit the web grant form (step 01-03)
// ---------------------------------------------------------------------------

/// `When the super-admin submits the grant form for "<email>"` — drive the NEW
/// web grant route over real HTTP: sign in as the super-admin (cookie + CSRF),
/// POST the grant form (`email` + `_csrf`) to `/admin/instance/super-admins`, and
/// capture the rendered confirmation fragment for the `Then` assertions.
#[when(regex = r#"^the super-admin submits the grant form for "([^"]+)"$"#)]
async fn submit_grant_form(world: &mut FoundryWorld, target_email: String) {
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
        "/admin/instance/super-admins",
        &[("email", target_email.as_str())],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body.clone());
    // Record each grant response so the idempotent-grant scenario (01-04) can
    // assert BOTH grants for the same operator confirmed.
    world
        .mwt6_grant_responses
        .push((outcome.status, outcome.body));
}

// ---------------------------------------------------------------------------
// Then — the grant is confirmed and the target is now a super-admin (step 01-03)
// ---------------------------------------------------------------------------

/// `Then the web page confirms the grant` — the port-exposed web observable: the
/// grant POST renders a 200 confirmation fragment carrying the grant-confirmation
/// marker.
#[then(regex = r#"^the web page confirms the grant$"#)]
async fn web_page_confirms_grant(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the grant form POST must render a 200 confirmation fragment; body = {:?}",
        world.last_body
    );
    let body = world
        .last_body
        .as_deref()
        .expect("a rendered grant confirmation fragment was captured");
    assert!(
        body.contains("data-grant-confirmation"),
        "the grant fragment must confirm the grant; got {body:?}"
    );
}

/// `And "<email>" is now a super-admin` — the SHIPPED `is_instance_admin` holds
/// for the granted operator (the DB-observable outcome the grant produced via the
/// SHIPPED `grant_instance_admin` path).
#[then(regex = r#"^"([^"]+)" is now a super-admin$"#)]
async fn target_is_now_super_admin(world: &mut FoundryWorld, target_email: String) {
    let pool = harness(world).app.state.store.pool().clone();
    let target_lower = target_email.to_ascii_lowercase();
    let (target_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&target_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve grant-target id");
    let is_admin = harness(world)
        .app
        .state
        .store
        .is_instance_admin(target_id)
        .await
        .expect("probe is_instance_admin after grant");
    assert!(
        is_admin,
        "the granted operator {target_email:?} must be a super-admin after the grant"
    );
}

// ---------------------------------------------------------------------------
// When/Then — granting the same operator twice is idempotent (step 01-04)
// ---------------------------------------------------------------------------

/// `And the super-admin submits the grant form for "<email>" a second time` —
/// drive the NEW web grant route a SECOND time for the SAME operator over real
/// HTTP (sign in as super-admin + CSRF, POST the same `email`). The SHIPPED grant
/// path is `INSERT … ON CONFLICT DO NOTHING`, so the second grant is a no-op that
/// must still confirm. Each response is recorded in `mwt6_grant_responses` for the
/// "confirms both times" assertion.
#[when(regex = r#"^the super-admin submits the grant form for "([^"]+)" a second time$"#)]
async fn submit_grant_form_again(world: &mut FoundryWorld, target_email: String) {
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
        "/admin/instance/super-admins",
        &[("email", target_email.as_str())],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body.clone());
    world
        .mwt6_grant_responses
        .push((outcome.status, outcome.body));
}

/// `Then the web page confirms the grant both times` — the port-exposed web
/// observable: BOTH grant POSTs rendered a 200 confirmation fragment carrying the
/// grant-confirmation marker (the second grant — a no-op on the idempotent store —
/// confirms identically to the first).
#[then(regex = r#"^the web page confirms the grant both times$"#)]
async fn web_page_confirms_grant_both_times(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_grant_responses.len(),
        2,
        "exactly two grant POSTs must have been submitted; got {:?}",
        world.mwt6_grant_responses
    );
    for (index, (status, body)) in world.mwt6_grant_responses.iter().enumerate() {
        assert_eq!(
            *status,
            StatusCode::OK,
            "grant POST #{} must render a 200 confirmation fragment; body = {body:?}",
            index + 1
        );
        assert!(
            body.contains("data-grant-confirmation"),
            "grant POST #{} must confirm the grant; got {body:?}",
            index + 1
        );
    }
}

/// `And "<email>" is recorded as a super-admin exactly once` — the DB-observable
/// idempotence outcome: after two grants for the same operator the
/// `instance_admins` table holds EXACTLY ONE row for the target (the SHIPPED
/// `INSERT … ON CONFLICT DO NOTHING` recorded no duplicate). A real `COUNT(*)`
/// against the per-scenario Postgres schema, not a synthetic probe.
#[then(regex = r#"^"([^"]+)" is recorded as a super-admin exactly once$"#)]
async fn target_recorded_super_admin_exactly_once(world: &mut FoundryWorld, target_email: String) {
    let pool = harness(world).app.state.store.pool().clone();
    let target_lower = target_email.to_ascii_lowercase();
    let (target_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&target_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve grant-target id");
    let (admin_rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM instance_admins WHERE user_id = $1")
            .bind(target_id)
            .fetch_one(&pool)
            .await
            .expect("count instance_admins rows for grant target");
    assert_eq!(
        admin_rows, 1,
        "the granted operator {target_email:?} must be recorded as a super-admin EXACTLY ONCE \
         after two grants (no duplicate instance_admins row); found {admin_rows} rows"
    );
}

// ---------------------------------------------------------------------------
// When/Then — the grant form is not a user-enumeration oracle (step 02-01)
//
// ADR-002 (g) / D2: a grant POST for an email that resolves to NO user returns
// the SAME non-committal confirmation as a grant for a real user — the grant
// form carries no oracle for whether the email belongs to a real user. The
// port-exposed web observable is the (status, body) pair; non-enumerability is
// the property that the KNOWN-email and UNKNOWN-email pairs are BYTE-IDENTICAL.
// ---------------------------------------------------------------------------

/// `When the super-admin submits the grant form for the existing email "<email>"`
/// — drive the SHIPPED web grant route over real HTTP for an email that DOES
/// resolve to a real user, recording the (status, body) response so the
/// non-enumerability assertion can compare it against the unknown-email response.
#[when(regex = r#"^the super-admin submits the grant form for the existing email "([^"]+)"$"#)]
async fn submit_grant_form_existing_email(world: &mut FoundryWorld, target_email: String) {
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
        "/admin/instance/super-admins",
        &[("email", target_email.as_str())],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body.clone());
    world
        .mwt6_grant_responses
        .push((outcome.status, outcome.body));
    world.mwt6_grant_submitted_emails.push(target_email);
}

/// `And the super-admin submits the grant form for an email that belongs to no
/// user` — drive the SHIPPED web grant route for an email that resolves to NO
/// user (`user_id_by_email` → `Ok(None)`, a silent no-op). The response is
/// recorded so the non-enumerability assertion can confirm it is byte-identical
/// to the known-email response. The address is constructed to be absent from the
/// per-scenario schema (no such user was seeded).
#[when(regex = r#"^the super-admin submits the grant form for an email that belongs to no user$"#)]
async fn submit_grant_form_unknown_email(world: &mut FoundryWorld) {
    let super_admin = world
        .mwt6_superadmin_email
        .clone()
        .expect("super-admin seeded in the Background");
    let unknown_email = "nobody-here@acme.com";
    let client = http(world);
    let outcome = signed_in_post(
        harness(world),
        &client,
        &super_admin,
        SUPERADMIN_PASSWORD,
        "/admin/instance/super-admins",
        &[("email", unknown_email)],
    )
    .await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body.clone());
    world
        .mwt6_grant_responses
        .push((outcome.status, outcome.body));
    world
        .mwt6_grant_submitted_emails
        .push(unknown_email.to_string());
}

/// `Then the two grant responses are confirmed identically` — the
/// non-enumerability property over the port-exposed web observable: once the
/// caller-supplied email (which the caller already knows — NOT an existence
/// oracle) is normalised out, the known-email and unknown-email grant POSTs
/// produced BYTE-IDENTICAL responses. The status codes match AND the
/// email-normalised body bytes match exactly — no oracle in status OR
/// confirmation TEMPLATE distinguishes a grant for a real user from one for an
/// email that belongs to no user (ADR-002 (g): the SAME non-committal "if that
/// user exists" confirmation either way).
///
/// Comparing the FULL normalised body (not merely "both 200" or "both contain the
/// marker") is what makes the assertion falsifiable: introducing any
/// existence-dependent divergence — a 404 on the unknown branch, or an extra
/// "no such user" sentence rendered only when the email did not resolve — re-REDS
/// this step. (Demonstrated during RED: see step log.)
#[then(regex = r#"^the two grant responses are confirmed identically$"#)]
async fn two_grant_responses_identical(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_grant_responses.len(),
        2,
        "exactly two grant POSTs (one known-email, one unknown-email) must have been \
         submitted; got {:?}",
        world.mwt6_grant_responses
    );
    assert_eq!(
        world.mwt6_grant_submitted_emails.len(),
        2,
        "the submitted-email log must parallel the two grant responses; got {:?}",
        world.mwt6_grant_submitted_emails
    );
    // Normalise the caller-supplied email out of each body. The response echoes
    // back the address the operator typed — information the attacker already has,
    // so it is NOT a user-existence oracle. What MUST be identical is the
    // confirmation template; what may legitimately differ is only the echoed input.
    let normalise = |body: &str, email: &str| body.replace(email, "<SUBMITTED-EMAIL>");
    let (known_status, known_body) = &world.mwt6_grant_responses[0];
    let (unknown_status, unknown_body) = &world.mwt6_grant_responses[1];
    let known_normalised = normalise(known_body, &world.mwt6_grant_submitted_emails[0]);
    let unknown_normalised = normalise(unknown_body, &world.mwt6_grant_submitted_emails[1]);
    assert_eq!(
        known_status, unknown_status,
        "the known-email and unknown-email grant responses must share the SAME status \
         (no status oracle); known = {known_status}, unknown = {unknown_status}"
    );
    assert_eq!(
        known_normalised, unknown_normalised,
        "with the caller-supplied email normalised out, the known-email and unknown-email \
         grant responses must be BYTE-IDENTICAL (no body oracle distinguishes a real user \
         from an unknown email — ADR-002 (g)); known = {known_normalised:?}, \
         unknown = {unknown_normalised:?}"
    );
}

/// `And neither response reveals whether the email belongs to a real user` — both
/// responses render the SAME non-committal confirmation marker and NEITHER leaks
/// a user-existence oracle: no "no such user" / "not found" / "does not exist"
/// negative phrasing appears in either body, and both carry the identical
/// grant-confirmation marker the known-email grant produces. This complements the
/// byte-identity check with an explicit no-negative-oracle assertion so a future
/// regression that made BOTH bodies leak (still byte-identical, but both saying
/// "no such user") would still RED here.
#[then(regex = r#"^neither response reveals whether the email belongs to a real user$"#)]
async fn neither_response_reveals_existence(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_grant_responses.len(),
        2,
        "exactly two grant POSTs must have been submitted; got {:?}",
        world.mwt6_grant_responses
    );
    for (index, (status, body)) in world.mwt6_grant_responses.iter().enumerate() {
        assert_eq!(
            *status,
            StatusCode::OK,
            "grant response #{index} must be a non-committal 200 confirmation (no \
             existence oracle in the status); body = {body:?}"
        );
        assert!(
            body.contains("data-grant-confirmation"),
            "grant response #{index} must carry the non-committal grant-confirmation \
             marker; got {body:?}"
        );
        let body_lower = body.to_ascii_lowercase();
        for oracle_phrase in [
            "no such user",
            "not found",
            "does not exist",
            "unknown user",
        ] {
            assert!(
                !body_lower.contains(oracle_phrase),
                "grant response #{index} must not leak a user-existence oracle phrase \
                 ({oracle_phrase:?}); got {body:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Non-enumerable refusal — SIGNED-OUT (web-provisioning-flow 02-02, ADR-002).
//
// A signed-OUT caller's request to EVERY /admin/instance/… route (the GET
// dashboard + both state-changing POSTs) is refused with a uniform 404 that is
// BYTE-IDENTICAL (status AND full body) to a path that never existed — NO 403,
// NO 401, NO login redirect; nothing reveals the admin surface exists. The
// `require_instance_admin` gate returns the SHIPPED `resource_not_found_page()`
// for a missing SessionUser (ADR-002 response-mapping, row 1); the control is a
// path with no route at all, which the SHIPPED router fallback also refuses with
// the SAME uniform 404 page. Green-by-inheritance behind the shipped gate +
// fallback; this scenario PROVES the byte-identity holds on every route.
//
// Falsifiability (revert-reds-it litmus, ADR-002): collapsing the gate's refusal
// into a DISTINCT response — a 401, a 303 redirect-to-sign-in, or a body that
// differs from the never-existed page — diverges from the control and re-REDS
// `every_admin_response_refused_identically`. Demonstrated during RED (step log).
// ---------------------------------------------------------------------------

/// The three `/admin/instance/…` routes an unauthorised caller probes, each with
/// the HTTP method it is served under (GET dashboard + the two state-changing
/// POSTs). The refusal MUST be byte-identical across all three (and to the
/// never-existed control) — a per-route divergence would be an enumeration oracle.
const ADMIN_INSTANCE_ROUTES: &[(&str, &str)] = &[
    ("GET", "/admin/instance/workspaces"),
    ("POST", "/admin/instance/workspaces"),
    ("POST", "/admin/instance/super-admins"),
];

/// Issue an ANONYMOUS request (no session cookie, no CSRF) for `method url`
/// against the in-process harness, returning the full (status, body) refusal
/// shape. A signed-out caller carries no credentials at all — this is the
/// adversary the non-enumerability property defends against.
async fn anonymous_request(
    world: &mut FoundryWorld,
    method: &str,
    url: &str,
) -> (StatusCode, String) {
    let base = harness(world).base_url();
    let client = http(world);
    let request = match method {
        "GET" => client.get(format!("{base}{url}")),
        "POST" => client.post(format!("{base}{url}")),
        other => panic!("unsupported anonymous method {other:?}"),
    };
    let resp = request.send().await.expect("send anonymous request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// `Given no user is signed in on the web` — the acting persona carries NO
/// session cookie. The slice-06 Background already claimed the instance + a
/// super-admin (so the surface genuinely exists for SOMEONE); this step only
/// asserts the adversary is unauthenticated and primes the http client.
#[given(regex = r#"^no user is signed in on the web$"#)]
async fn no_user_signed_in(world: &mut FoundryWorld) {
    assert!(
        world.mwt6_superadmin_email.is_some(),
        "the slice-06 Background must have claimed the instance first (so the \
         admin surface exists for the super-admin the signed-out caller cannot reach)"
    );
    let _ = http(world);
}

/// `When a signed-out caller requests each /admin/instance route on the web` —
/// drive an ANONYMOUS request to EVERY /admin/instance/… route (GET dashboard +
/// both POSTs) over real HTTP, recording each (route, status, body) refusal so
/// the `Then` can assert byte-identity against the never-existed control.
#[when(regex = r#"^a signed-out caller requests each /admin/instance route on the web$"#)]
async fn signed_out_requests_each_admin_route(world: &mut FoundryWorld) {
    for (method, url) in ADMIN_INSTANCE_ROUTES {
        let (status, body) = anonymous_request(world, method, url).await;
        world
            .mwt6_admin_surface_refusals
            .push((format!("{method} {url}"), status, body));
    }
}

/// `And a signed-out caller requests a path that never existed on the web` —
/// drive an ANONYMOUS request to a path with NO route at all (the control),
/// once per HTTP METHOD the admin routes use (GET + POST). Each admin-surface
/// refusal must be byte-identical to the never-existed-path refusal for its OWN
/// method: a GET admin route vs a never-existed GET (both the SHIPPED router
/// fallback's uniform `resource_not_found_page()`); a POST admin route vs a
/// never-existed POST (both screened identically by the SHIPPED CSRF layer that
/// runs ahead of routing). Capturing per method is what keeps the comparison
/// honest — comparing a POST refusal against a GET control would be a category
/// error, not an oracle test.
#[when(regex = r#"^a signed-out caller requests a path that never existed on the web$"#)]
async fn signed_out_requests_never_existed_path(world: &mut FoundryWorld) {
    for method in ["GET", "POST"] {
        let (status, body) =
            anonymous_request(world, method, "/this-path-has-never-existed-anywhere").await;
        world
            .mwt6_admin_never_existed
            .insert(method.to_string(), (status, body));
    }
}

/// `Then every admin-surface response is refused identically to the never-existed
/// path` — the non-enumerability core. EVERY /admin/instance/… route's refusal is
/// BYTE-IDENTICAL (status AND full body) to the never-existed-path control: a
/// uniform 404, no 403, no 401, no login redirect, no per-route divergence. The
/// control itself must be a genuine 404 (so the comparison is not vacuously
/// matching two redirects). Comparing the FULL body — not merely "both 404" — is
/// what makes the assertion falsifiable: any existence-revealing divergence on
/// any route (a 401, a 303, a distinct body) re-REDS here.
#[then(
    regex = r#"^every admin-surface response is refused identically to the never-existed path$"#
)]
async fn every_admin_response_refused_identically(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_admin_surface_refusals.len(),
        ADMIN_INSTANCE_ROUTES.len(),
        "every /admin/instance route must have been probed; got {:?}",
        world.mwt6_admin_surface_refusals
    );
    // No admin-surface refusal may reveal that the surface exists — none is a
    // login redirect (3xx) and none is a 401/403 status oracle distinct from the
    // never-existed control for the SAME method.
    for (route, status, _body) in &world.mwt6_admin_surface_refusals {
        assert!(
            !status.is_redirection(),
            "{route} answered with a redirect ({status}) — a login-redirect oracle \
             that reveals the admin surface exists (ADR-002 forbids it)"
        );
    }
    for (route, status, body) in &world.mwt6_admin_surface_refusals {
        let method = route
            .split_whitespace()
            .next()
            .expect("each recorded route is 'METHOD /path'");
        let (control_status, control_body) = world
            .mwt6_admin_never_existed
            .get(method)
            .unwrap_or_else(|| panic!("a never-existed {method} control was captured"));
        assert_eq!(
            status, control_status,
            "{route} refused with status {status} but a never-existed {method} path \
             refused with {control_status} — a status oracle (no 403, 401, or \
             redirect distinguishing the admin surface from nothing is allowed)"
        );
        assert_eq!(
            body, control_body,
            "{route} refusal body differs from the never-existed {method}-path body \
             — a body oracle that reveals the admin surface exists. \
             admin = {body:?}, never-existed = {control_body:?}"
        );
    }
}

/// `And nothing reveals that the admin surface exists` — no admin-surface refusal
/// body carries an oracle distinguishing it from a never-existed path of the same
/// method: no admin-surface vocabulary (route names, "admin", "super-admin",
/// "instance", "workspaces") leaked into any refusal body, and each body is
/// byte-identical to its same-method never-existed control. Complements the
/// status/body identity check so a regression that made BOTH the control and the
/// admin refusals name the surface (still byte-identical, but both leaking) still
/// REDS here.
#[then(regex = r#"^nothing reveals that the admin surface exists$"#)]
async fn nothing_reveals_admin_surface(world: &mut FoundryWorld) {
    assert!(
        !world.mwt6_admin_surface_refusals.is_empty(),
        "no admin-surface refusal was captured to assert on"
    );
    for (route, _status, body) in &world.mwt6_admin_surface_refusals {
        let method = route
            .split_whitespace()
            .next()
            .expect("each recorded route is 'METHOD /path'");
        let (_, control_body) = world
            .mwt6_admin_never_existed
            .get(method)
            .unwrap_or_else(|| panic!("a never-existed {method} control was captured"));
        assert_eq!(
            body, control_body,
            "{route} refusal body diverges from the never-existed {method}-path \
             control — an existence oracle; admin = {body:?}, control = {control_body:?}"
        );
        let body_lower = body.to_ascii_lowercase();
        for oracle_phrase in [
            "super-admin",
            "super admin",
            "/admin/instance",
            "instance dashboard",
            "provision",
            "grant",
        ] {
            assert!(
                !body_lower.contains(oracle_phrase),
                "{route} refusal body leaked admin-surface vocabulary \
                 ({oracle_phrase:?}) — an existence oracle; got {body:?}"
            );
        }
    }
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
