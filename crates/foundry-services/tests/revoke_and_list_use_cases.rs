//! Integration-style unit tests for `foundry_services::tokens::revoke_token`
//! and `list_tokens` (the read + kill-switch use-cases of
//! machine-token-admin-ux). They drive each use-case through its public async
//! driving-port signature against a REAL Postgres harness (@real-io) plus the
//! shipped Ed25519 test signer — the same seam shape `mint_token_use_case.rs`
//! established. No domain double is mocked inside the hexagon.
//!
//! Why these exist (mutation-coverage closure, DELIVER Phase 5): the mint
//! use-case was already covered, but `revoke_token`, `list_tokens`, and the
//! shared `resolve_team_name` helper had no in-process test. cargo-mutants
//! showed whole-function-replacement mutants surviving (`revoke_token -> Ok(())`,
//! `list_tokens -> Ok(vec![])`, `resolve_team_name -> Ok(None)/Ok(Some(..))`) —
//! each a real authz/correctness gap. These tests assert the OBSERVABLE effect
//! of each use-case so those stubs can no longer pass.
//!
//! Test budget: 3 distinct behaviours.
//!   1. revoke flips `revoked_at` (observable kill-switch effect) AND a foreign
//!      jti is NON-ENUMERABLE NotFound with no mutation (workspace isolation).
//!   2. list returns the workspace's tokens with derived status (revoked vs
//!      active) and `minted_by` resolved from `created_by` (NOT the subject).
//!   3. scope label: a team-scoped grant resolves `scope_team_name` to the real
//!      team name; a workspace grant resolves to None.

use foundry_services::tokens::{list_tokens, mint_token, revoke_token, MintInput, ScopeChoice};
use foundry_services::{Principal, ServiceError};
use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

struct Harness {
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    store: Store,
    workspace_id: uuid::Uuid,
    admin_id: uuid::Uuid,
    team_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it, and seed an admin "devansh" (workspace
/// admin + Backend team lead) and team "Backend". The admin gates on
/// `revoke_token`/`list_tokens` are already covered by the baseline `delete !`
/// kills, so this harness only needs the admin + a team for scope labelling.
async fn seeded_harness() -> Harness {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let base = format!("postgres://postgres:postgres@{host}:{port}/postgres");

    let opts = PgConnectOptions::from_str(&base)
        .expect("parse base url")
        .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Store::from_pool(pool);

    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme Eng",
            admin_id,
            "devansh@acme.com",
            "devansh@acme.com",
            "Devansh",
            "x",
            team_id,
            "Backend",
            "backend",
            project_id,
            "Auth v2",
            "auth-v2",
            "AUTH",
        )
        .await
        .expect("seed initial workspace");

    Harness {
        _container: container,
        store,
        workspace_id,
        admin_id,
        team_id,
    }
}

/// A principal that is an admin BUT carries a different `workspace_id` than the
/// one real workspace. The deployment is single-tenant (`uniq_one_workspace`),
/// so a token "in another workspace" cannot exist as a row; the real isolation
/// surface is a principal presenting a foreign/stale `workspace_id`. The user is
/// the seeded admin so authz (`is_workspace_admin(other_ws, admin)`) is exercised
/// against the wrong workspace — which is itself the isolation boundary.
fn foreign_workspace_principal(h: &Harness) -> Principal {
    Principal::Human {
        user_id: h.admin_id,
        workspace_id: uuid::Uuid::now_v7(),
    }
}

fn admin_principal(h: &Harness) -> Principal {
    Principal::Human {
        user_id: h.admin_id,
        workspace_id: h.workspace_id,
    }
}

async fn mint_workspace_token(h: &Harness, label: &str) -> uuid::Uuid {
    let signer = foundry_auth::test_keys::signer();
    mint_token(
        &h.store,
        &signer,
        &admin_principal(h),
        MintInput {
            label: label.to_string(),
            scope: ScopeChoice::Workspace,
            ttl_days: 30,
        },
    )
    .await
    .expect("admin mint")
    .jti
}

/// Behaviour 1 — revoke has an OBSERVABLE kill-switch effect and is
/// workspace-isolated. Revoking a token owned by the acting workspace stamps
/// `revoked_at` (the row transitions from active to revoked — this kills a
/// `revoke_token -> Ok(())` stub that would skip the store write). A jti that
/// belongs to ANOTHER workspace is refused as the SAME non-enumerable NotFound
/// and is NOT mutated (no cross-workspace oracle, NFR-MT-SEC-03).
#[tokio::test]
async fn revoke_flips_revoked_at_and_foreign_jti_is_non_enumerable_notfound() {
    let h = seeded_harness().await;
    let jti = mint_workspace_token(&h, "Revoke-me bot").await;

    // Precondition: the freshly minted token is active (revoked_at is NULL).
    let before = h
        .store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query")
        .expect("row exists");
    assert!(
        before.revoked_at.is_none(),
        "a freshly minted token must start active (revoked_at NULL)"
    );

    revoke_token(&h.store, &admin_principal(&h), jti)
        .await
        .expect("an admin may revoke a token in their workspace");

    // Observable effect: revoked_at is now set. A `revoke_token -> Ok(())` stub
    // never reaches the store write, so this assertion fails under that mutant.
    let after = h
        .store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query")
        .expect("row still present (denylist semantics retain the row)");
    assert!(
        after.revoked_at.is_some(),
        "revoke must stamp revoked_at — the kill-switch the denylist reads"
    );

    // Workspace isolation / non-enumerability: a jti that the acting workspace
    // does NOT own yields NotFound (never an oracle). In this single-tenant
    // deployment (`uniq_one_workspace`) the realisable "not yours" case is an
    // unknown jti: `find_machine_token_by_jti` returns None, `belongs` is false,
    // and the use-case refuses with NotFound — exercising the `!belongs` branch.
    match revoke_token(&h.store, &admin_principal(&h), uuid::Uuid::now_v7()).await {
        Err(ServiceError::NotFound) => {}
        Err(other) => panic!("an unknown jti must be NotFound (non-enumerable), got {other:?}"),
        Ok(()) => panic!("revoking a jti the workspace does not own must not succeed"),
    }

    // And a principal presenting a foreign/stale workspace_id is refused (the
    // admin gate is evaluated against the wrong workspace) and does NOT mutate
    // the real token — proving the kill-switch is bound to the acting workspace.
    let foreign = foreign_workspace_principal(&h);
    match revoke_token(&h.store, &foreign, jti).await {
        Err(ServiceError::Forbidden) | Err(ServiceError::NotFound) => {}
        Err(other) => panic!("a foreign-workspace principal must be refused, got {other:?}"),
        Ok(()) => panic!("a foreign-workspace principal must not revoke this workspace's token"),
    }
    let still = h
        .store
        .find_machine_token_by_jti(jti)
        .await
        .expect("query")
        .expect("row");
    assert_eq!(
        still.revoked_at, after.revoked_at,
        "a refused cross-workspace revoke must not change the token's revoked_at"
    );
}

/// Behaviour 2 — list returns the workspace's tokens with derived status and
/// `minted_by` resolved from `created_by` (the ISSUER). This kills a
/// `list_tokens -> Ok(vec![])` stub (which would return nothing) and exercises
/// the revoked/active status derivation and the per-row issuer resolution.
/// (The deployment is single-tenant — `uniq_one_workspace` — so a second
/// workspace cannot exist; the workspace-scoping filter is asserted by the
/// @real-io acceptance lane, not here.)
#[tokio::test]
async fn list_returns_workspace_tokens_with_status_and_minted_by() {
    let h = seeded_harness().await;
    let active_jti = mint_workspace_token(&h, "Active bot").await;
    let revoked_jti = mint_workspace_token(&h, "Revoked bot").await;
    revoke_token(&h.store, &admin_principal(&h), revoked_jti)
        .await
        .expect("revoke the second token");

    let views = list_tokens(&h.store, &admin_principal(&h))
        .await
        .expect("an admin may list the workspace's tokens");

    assert_eq!(
        views.len(),
        2,
        "the list returns exactly the two minted tokens (kills the empty-vec stub)"
    );

    let active = views
        .iter()
        .find(|v| v.jti == active_jti)
        .expect("the active token appears in the list");
    assert!(
        !active.revoked,
        "an un-revoked token derives revoked = false"
    );
    assert_eq!(
        active.minted_by.as_deref(),
        Some("devansh@acme.com"),
        "minted_by resolves from created_by (the issuing admin), not the subject"
    );

    let revoked = views
        .iter()
        .find(|v| v.jti == revoked_jti)
        .expect("the revoked token still appears in the list");
    assert!(
        revoked.revoked,
        "a revoked token derives revoked = true (status from revoked_at)"
    );
}

/// Behaviour 3 — scope label resolution (`resolve_team_name`, DD9). A
/// team-scoped grant resolves `scope_team_name` to the team's REAL name; a
/// whole-workspace grant resolves to None. This kills the
/// `resolve_team_name -> Ok(None)` stub (team grant would lose its name) and the
/// `Ok(Some("xyzzy"))` / `Ok(Some(""))` stubs (the name must be the real one).
#[tokio::test]
async fn scope_team_name_resolves_to_the_real_team_for_a_team_grant_and_none_for_workspace() {
    let h = seeded_harness().await;
    let signer = foundry_auth::test_keys::signer();

    let team_scoped = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "Backend bot".to_string(),
            scope: ScopeChoice::Team(h.team_id),
            ttl_days: 30,
        },
    )
    .await
    .expect("a team in the workspace is a valid scope");
    assert_eq!(
        team_scoped.scope_team_name.as_deref(),
        Some("Backend"),
        "a team-scoped grant resolves scope_team_name to the team's real name"
    );

    let workspace_scoped = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "Whole-ws bot".to_string(),
            scope: ScopeChoice::Workspace,
            ttl_days: 30,
        },
    )
    .await
    .expect("workspace scope mints");
    assert_eq!(
        workspace_scoped.scope_team_name, None,
        "a whole-workspace grant carries no team name"
    );

    // The list view resolves the same team name for the team-scoped row.
    let views = list_tokens(&h.store, &admin_principal(&h))
        .await
        .expect("list");
    let team_view = views
        .iter()
        .find(|v| v.jti == team_scoped.jti)
        .expect("team-scoped token in list");
    assert_eq!(
        team_view.scope_team_name.as_deref(),
        Some("Backend"),
        "the list view resolves scope_team_name to the real team name too"
    );
}
