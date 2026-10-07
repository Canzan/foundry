//! The `projects::create_project` use-case (project-name-rule, DESIGN DDD-5/6/7/
//! 9/10/11, ADR-PROJECT-NAME-001/002), driven against a REAL Postgres (@real-io)
//! — the `rename_project_use_case` idiom.
//!
//! The use-case landed in slice 02 (behaviour 1a runs); the mint and the retry
//! land in slice 04, which removes the remaining ignores.
//!
//! WHY HERE AND NOT IN THE ACCEPTANCE SUITE (DESIGN OQ-D3): the fallback-address
//! race (two creates in one team taking the same key-prefix address within
//! milliseconds) is not drivable deterministically over HTTP. DESIGN's seam: the
//! use-case's internal "read siblings → check → mint → insert" step can be handed
//! a STALE sibling list, so the unique index fires on a chosen attempt. That seam
//! is the `#[doc(hidden)] pub` `create_project_with_sibling_reads`: attempt `i`
//! uses `scripted[i]` instead of reading the store while a scripted list remains,
//! then reads the store.
//!
//! Behaviours (5, six tests):
//!   1. A plain create mints the derived address (1a, slice 02) or the
//!      key-prefix fallback (1b, slice 04) and inserts the project with its lanes.
//!   2. A stale read makes the fallback address collide on insert → the use-case
//!      re-reads, re-checks, re-mints and lands on the next free address — never
//!      the uniqueness refusal (ADR-PROJECT-NAME-002 §3).
//!   3. Three stale-forced collisions exhaust the retry → FallbackSlugContention,
//!      and no row is written (D9).
//!   4. A DERIVED address colliding on insert keeps today's meaning → NotUnique,
//!      no retry, no row.
//!   5. A genuine concurrent same-name create, seen only on the re-read, gets
//!      the true uniqueness refusal.

use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

use foundry_core::{MintedSlug, ProjectName, ProjectNameError};
use foundry_services::projects::{
    create_project, create_project_with_sibling_reads, CreateProjectError, CreateProjectRequest,
};

struct Harness {
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    store: Arc<Store>,
    workspace_id: uuid::Uuid,
    /// Team "General" (seeded with project "Sandbox" / `sandbox` / GEN).
    team_id: uuid::Uuid,
}

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
    let store = Arc::new(Store::from_pool(pool));
    let workspace_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme",
            uuid::Uuid::now_v7(),
            "ops@acme.com",
            "ops@acme.com",
            "Ops",
            "phc$dummy",
            team_id,
            "General",
            "general",
            uuid::Uuid::now_v7(),
            "Sandbox",
            "sandbox",
            "GEN",
        )
        .await
        .expect("bootstrap claim");
    Harness {
        _container: container,
        store,
        workspace_id,
        team_id,
    }
}

/// A sibling already in the team, written the way any earlier create left it.
async fn seed_sibling(h: &Harness, name: &str, slug: &str, key: &str) {
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(h.team_id)
    .bind(h.workspace_id)
    .bind(name)
    .bind(slug)
    .bind(key)
    .execute(h.store.pool())
    .await
    .expect("seed sibling");
}

/// The universe a create may touch: every `(name, slug, key)` in the team, and
/// the lane count.
async fn team_projects(h: &Harness) -> Vec<(String, String, String)> {
    sqlx::query_as(
        "SELECT name, slug, key_prefix FROM projects WHERE team_id = $1 ORDER BY name, slug",
    )
    .bind(h.team_id)
    .fetch_all(h.store.pool())
    .await
    .expect("read team projects")
}

async fn lane_count(h: &Harness) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM lanes")
        .fetch_one(h.store.pool())
        .await
        .expect("count lanes");
    n
}

fn request<'a>(h: &Harness, name: ProjectName, key: &'a str) -> CreateProjectRequest<'a> {
    CreateProjectRequest {
        workspace_id: h.workspace_id,
        team_id: h.team_id,
        name,
        key_prefix: key,
    }
}

fn parsed(raw: &str) -> ProjectName {
    ProjectName::try_new(raw).unwrap_or_else(|e| panic!("{raw:?} must be a valid name: {e:?}"))
}

/// Behaviour 1a. The name is parsed BEFORE the container starts.
#[tokio::test]
async fn a_create_with_a_latin_name_keeps_its_own_address_and_gets_its_lanes() {
    let homelab = parsed("Homelab Ops");
    let h = seeded_harness().await;
    let lanes_before = lane_count(&h).await;

    let created = create_project(&h.store, request(&h, homelab, "OPS"))
        .await
        .unwrap_or_else(|e| panic!("create must succeed: {e:?}"));
    assert_eq!(created.slug, MintedSlug::Derived("homelab-ops".into()));
    assert!(team_projects(&h).await.contains(&(
        "Homelab Ops".into(),
        "homelab-ops".into(),
        "OPS".into()
    )));
    assert!(
        lane_count(&h).await > lanes_before,
        "the new project gets its lanes"
    );
}

/// Behaviour 1b: a name without an address takes the key prefix.
#[tokio::test]
#[ignore = "SCAFFOLD: slice 04 (DDD-9)"]
async fn a_create_without_a_latin_name_takes_the_key_prefix_address() {
    let japanese = parsed("日本語ボード");
    let h = seeded_harness().await;

    let created = create_project(&h.store, request(&h, japanese, "JP"))
        .await
        .unwrap_or_else(|e| panic!("create must succeed: {e:?}"));
    assert_eq!(created.slug, MintedSlug::KeyFallback("jp".into()));
    assert!(team_projects(&h)
        .await
        .contains(&("日本語ボード".into(), "jp".into(), "JP".into())));
}

/// Behaviour 2 (DDD-10): the first attempt reads a stale sibling list that omits
/// the existing `jp`, mints `jp`, and loses on the unique index; the retry reads
/// the store and lands on `jp-2`. Never the uniqueness refusal.
#[tokio::test]
#[ignore = "SCAFFOLD: slice 04 (DDD-10 retry)"]
async fn a_stale_fallback_address_is_retried_to_the_next_free_one() {
    let japanese = parsed("日本語ボード");
    let h = seeded_harness().await;
    seed_sibling(&h, "JP", "jp", "JPX").await;

    let created =
        create_project_with_sibling_reads(&h.store, request(&h, japanese, "JP"), vec![vec![]])
            .await
            .unwrap_or_else(|e| panic!("a lost fallback race must retry, not refuse: {e:?}"));
    assert_eq!(created.slug, MintedSlug::KeyFallback("jp-2".into()));
    assert!(team_projects(&h)
        .await
        .contains(&("日本語ボード".into(), "jp-2".into(), "JP".into())));
}

/// Behaviour 3 (DDD-10 bound): three stale reads, three collisions (`jp`, `jp-2`,
/// `jp-3` all taken) → FallbackSlugContention, and nothing is written.
#[tokio::test]
#[ignore = "SCAFFOLD: slice 04 (DDD-10 bound)"]
async fn three_lost_races_end_in_contention_and_write_nothing() {
    let japanese = parsed("日本語ボード");
    let h = seeded_harness().await;
    seed_sibling(&h, "JP", "jp", "JPA").await;
    seed_sibling(&h, "JP 2", "jp-2", "JPB").await;
    seed_sibling(&h, "JP 3", "jp-3", "JPC").await;
    let before = (team_projects(&h).await, lane_count(&h).await);

    let stale = |slugs: &[&str]| -> Vec<(String, String)> {
        slugs
            .iter()
            .map(|s| (format!("stale {s}"), (*s).to_string()))
            .collect()
    };
    let outcome = create_project_with_sibling_reads(
        &h.store,
        request(&h, japanese, "JP"),
        vec![stale(&[]), stale(&["jp"]), stale(&["jp", "jp-2"])],
    )
    .await;
    assert!(
        matches!(outcome, Err(CreateProjectError::FallbackSlugContention)),
        "three lost races must end in contention; got {outcome:?}"
    );
    assert_eq!(
        (team_projects(&h).await, lane_count(&h).await),
        before,
        "an exhausted retry must write nothing (D9)"
    );
}

/// Behaviour 4: a DERIVED address that collides on insert (the stale read hid
/// the sibling from the check) is the uniqueness refusal — no retry, no row.
#[tokio::test]
#[ignore = "SCAFFOLD: slice 04 (DDD-10 Derived arm)"]
async fn a_derived_address_collision_is_not_unique_and_writes_nothing() {
    let auth = parsed("Auth V2!");
    let h = seeded_harness().await;
    seed_sibling(&h, "Identity Platform", "auth-v2", "AUTH").await;
    let before = (team_projects(&h).await, lane_count(&h).await);

    let outcome =
        create_project_with_sibling_reads(&h.store, request(&h, auth, "AVX"), vec![vec![]]).await;
    assert!(
        matches!(
            outcome,
            Err(CreateProjectError::InvalidName(ProjectNameError::NotUnique))
        ),
        "a derived-address collision keeps today's meaning; got {outcome:?}"
    );
    assert_eq!((team_projects(&h).await, lane_count(&h).await), before);
}

/// Behaviour 5: the re-read re-runs the sibling check, so a genuine concurrent
/// create of the SAME name gets the true refusal instead of a suffix.
#[tokio::test]
#[ignore = "SCAFFOLD: slice 04 (DDD-10 re-check)"]
async fn a_concurrent_create_of_the_same_name_is_refused_on_the_retry() {
    let japanese = parsed("日本語ボード");
    let h = seeded_harness().await;
    seed_sibling(&h, "日本語ボード", "jp", "JPZ").await;
    let before = (team_projects(&h).await, lane_count(&h).await);

    let outcome =
        create_project_with_sibling_reads(&h.store, request(&h, japanese, "JP"), vec![vec![]])
            .await;
    assert!(
        matches!(
            outcome,
            Err(CreateProjectError::InvalidName(ProjectNameError::NotUnique))
        ),
        "the retry must re-check and refuse the duplicate name; got {outcome:?}"
    );
    assert_eq!((team_projects(&h).await, lane_count(&h).await), before);
}
