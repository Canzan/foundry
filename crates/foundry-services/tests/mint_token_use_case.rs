//! Integration-style unit tests for `foundry_services::tokens::mint_token`
//! (step 01-01). These drive the mint use-case through its public async
//! driving-port signature against a REAL Postgres harness (@real-io) plus the
//! shipped Ed25519 test signer — exactly the seam shape `write_use_cases.rs`
//! established. The use-case orchestrates the real store (authz +
//! team-scope lookup + metadata persistence) and the real signer; no domain
//! double is mocked inside the hexagon.
//!
//! Why no separate UI acceptance run here: the us-mt01 scenarios all drive the
//! /admin/tokens browser surface (the route is built in step 01-02). These
//! tests green the SERVICE + PERSISTENCE contract (authz, TTL bounds, scope
//! mapping, sign-then-persist, created_by audit) at the driving-port seam,
//! which is the load-bearing security surface (token-admin-services.md
//! §Testability).
//!
//! Test budget: 5 distinct behaviours named in the step criteria
//! (budget = 2 × 5 = 10; 6 written, TTL bounds parametrized into one loop):
//!   1. admin happy path — returns a one-time value + persists jti + created_by.
//!   2. non-admin → Forbidden (US-MT05).
//!   3. ttl_days <= 0 → Validation{ttl_required} (parametrized: 0 and negative).
//!   4. ttl_days > MAX → Validation{ttl_over_cap}; ttl == MAX accepted (bound).
//!   5. Team scope → scope_team_id persisted; foreign team → Validation.

use foundry_services::tokens::{mint_token, MintInput, ScopeChoice, MAX_TTL_DAYS};
use foundry_services::{Principal, ServiceError};
use foundry_store::Store;
use secrecy::ExposeSecret;
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
    member_id: uuid::Uuid,
    team_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it (incl. 0008 created_by), and seed:
///   - admin "devansh" (workspace admin + Backend team lead)
///   - member "mei" (workspace member, no admin role)
///   - team "Backend" in the single workspace
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

    let member_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(member_id)
    .bind("mei@acme.com")
    .bind("mei@acme.com")
    .bind("Mei")
    .bind("x")
    .execute(store.pool())
    .await
    .expect("seed member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(store.pool())
    .await
    .expect("seed member workspace membership");

    Harness {
        _container: container,
        store,
        workspace_id,
        admin_id,
        member_id,
        team_id,
    }
}

fn admin_principal(h: &Harness) -> Principal {
    Principal::Human {
        user_id: h.admin_id,
        workspace_id: h.workspace_id,
    }
}

/// Behaviour 1 — admin happy path. Minting returns a one-time value AND
/// persists exactly one registry row whose `jti` matches the returned id and
/// whose `created_by` is the acting admin (NFR-MT-SEC-06). The minted VALUE is
/// never persisted (the registry has no value column — find returns no value).
#[tokio::test]
async fn admin_mint_returns_value_and_persists_jti_and_created_by() {
    let h = seeded_harness().await;
    let signer = foundry_auth::test_keys::signer();

    let minted = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "Release bot".to_string(),
            scope: ScopeChoice::Workspace,
            ttl_days: 30,
        },
    )
    .await
    .expect("an admin must be able to mint a token");

    // The one-time value is a non-empty JWT that the SHIPPED verifier accepts —
    // proving real signing (not a placeholder), and that it carries the minted
    // jti as the denylist key.
    let verifier = foundry_auth::test_keys::verifier();
    let recovered = verifier
        .verify(minted.value.expose_secret())
        .expect("the minted value must be a genuinely-signed, verifiable token");
    assert_eq!(
        recovered.jti, minted.jti,
        "the verified token's jti must equal the returned/persisted jti"
    );
    assert_eq!(
        recovered.sub, h.admin_id,
        "the bound principal (sub) is the acting admin in v1"
    );

    // The metadata row is persisted under the returned jti, and created_by is
    // the acting admin — the audit fact this step exists to record.
    let row = h
        .store
        .find_machine_token_by_jti(minted.jti)
        .await
        .expect("query by jti")
        .expect("mint must persist a registry row under the returned jti");
    assert_eq!(
        row.workspace_id, h.workspace_id,
        "persisted in the workspace"
    );
    assert_eq!(row.label, "Release bot", "persisted the chosen label");
    assert_eq!(
        row.scope_team_id, None,
        "workspace scope persists scope_team_id = NULL"
    );

    let created_by: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT created_by FROM machine_tokens WHERE jti = $1")
            .bind(minted.jti)
            .fetch_one(h.store.pool())
            .await
            .expect("read created_by");
    assert_eq!(
        created_by,
        Some(h.admin_id),
        "mint must record the acting admin as created_by (NFR-MT-SEC-06)"
    );
}

/// Behaviour 2 — non-admin is refused (US-MT05). A workspace member without the
/// admin role gets Forbidden and NO row is written.
#[tokio::test]
async fn non_admin_mint_is_forbidden_and_persists_nothing() {
    let h = seeded_harness().await;
    let signer = foundry_auth::test_keys::signer();
    let member = Principal::Human {
        user_id: h.member_id,
        workspace_id: h.workspace_id,
    };

    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM machine_tokens")
        .fetch_one(h.store.pool())
        .await
        .expect("count before");

    let result = mint_token(
        &h.store,
        &signer,
        &member,
        MintInput {
            label: "Sneaky bot".to_string(),
            scope: ScopeChoice::Workspace,
            ttl_days: 30,
        },
    )
    .await;
    match result {
        Err(ServiceError::Forbidden) => {}
        Err(other) => panic!("non-admin mint must be Forbidden, got {other:?}"),
        Ok(_) => panic!("a non-admin must not be able to mint a token"),
    }

    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM machine_tokens")
        .fetch_one(h.store.pool())
        .await
        .expect("count after");
    assert_eq!(before, after, "a refused mint must persist no registry row");
}

/// Behaviour 3 — TTL is required: ttl_days <= 0 (zero and negative) is refused
/// as Validation{ttl_required} BEFORE any signing/persistence (OD4: no
/// never-expires option). Parametrized over the equivalence class.
#[tokio::test]
async fn non_positive_ttl_is_refused_as_ttl_required() {
    let h = seeded_harness().await;
    let signer = foundry_auth::test_keys::signer();

    for ttl in [0_i64, -1, -365] {
        let result = mint_token(
            &h.store,
            &signer,
            &admin_principal(&h),
            MintInput {
                label: "No-ttl bot".to_string(),
                scope: ScopeChoice::Workspace,
                ttl_days: ttl,
            },
        )
        .await;
        match result {
            Err(ServiceError::Validation { code, .. }) => assert_eq!(
                code, "ttl_required",
                "non-positive ttl_days={ttl} must be refused with ttl_required"
            ),
            Err(other) => {
                panic!("ttl_days={ttl}: expected ttl_required Validation, got {other:?}")
            }
            Ok(_) => panic!("ttl_days={ttl} must be refused as ttl_required, not minted"),
        }
    }
}

/// Behaviour 4 — TTL cap: ttl_days > MAX is refused as Validation{ttl_over_cap}
/// with the cap stated; ttl_days == MAX (boundary "at the cap") is accepted.
#[tokio::test]
async fn over_cap_ttl_is_refused_but_exactly_at_cap_is_accepted() {
    let h = seeded_harness().await;
    let signer = foundry_auth::test_keys::signer();

    let result = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "Too-long bot".to_string(),
            scope: ScopeChoice::Workspace,
            ttl_days: MAX_TTL_DAYS + 1,
        },
    )
    .await;
    match result {
        Err(ServiceError::Validation { code, message }) => {
            assert_eq!(code, "ttl_over_cap", "the refusal code must name the cap");
            assert!(
                message.contains("365"),
                "the cap message must state the maximum (got {message:?})"
            );
        }
        Err(other) => panic!("expected ttl_over_cap Validation, got {other:?}"),
        Ok(_) => panic!("ttl beyond the cap must be refused, not minted"),
    }

    // Boundary: exactly at the cap is accepted and mints.
    let minted = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "At-cap bot".to_string(),
            scope: ScopeChoice::Workspace,
            ttl_days: MAX_TTL_DAYS,
        },
    )
    .await
    .expect("ttl exactly at the cap must be accepted");
    assert!(
        h.store
            .find_machine_token_by_jti(minted.jti)
            .await
            .expect("query")
            .is_some(),
        "an at-cap mint persists its registry row"
    );
}

/// Behaviour 5 — scope mapping (DD9). A Team scope to a team that BELONGS to the
/// workspace persists scope_team_id = that team; a foreign team id is refused as
/// Validation{scope_team_not_in_workspace}.
#[tokio::test]
async fn team_scope_persists_team_and_foreign_team_is_refused() {
    let h = seeded_harness().await;
    let signer = foundry_auth::test_keys::signer();

    let minted = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "Team-scoped bot".to_string(),
            scope: ScopeChoice::Team(h.team_id),
            ttl_days: 30,
        },
    )
    .await
    .expect("a team in the workspace is a valid scope");
    let row = h
        .store
        .find_machine_token_by_jti(minted.jti)
        .await
        .expect("query")
        .expect("row");
    assert_eq!(
        row.scope_team_id,
        Some(h.team_id),
        "a Team scope persists scope_team_id = the chosen team"
    );

    // A team id that is NOT in the acting workspace is refused (evil-user path).
    let foreign_team = uuid::Uuid::now_v7();
    let result = mint_token(
        &h.store,
        &signer,
        &admin_principal(&h),
        MintInput {
            label: "Foreign-team bot".to_string(),
            scope: ScopeChoice::Team(foreign_team),
            ttl_days: 30,
        },
    )
    .await;
    match result {
        Err(ServiceError::Validation { code, .. }) => assert_eq!(
            code, "scope_team_not_in_workspace",
            "a foreign team scope is refused with the scoped code"
        ),
        Err(other) => panic!("expected scope_team_not_in_workspace Validation, got {other:?}"),
        Ok(_) => panic!("a team outside the workspace must be refused, not minted"),
    }
}
