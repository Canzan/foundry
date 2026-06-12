//! multi-workspace-provisioning — Slice 5 (US-MWT06) step definitions:
//! the existing-install upgrade-safety guarantee.
//!
//! This is the FIRST slice-05 module; step 01-01 implements ONLY the
//! idempotent-re-upgrade scenario ("Re-running the upgrade does not duplicate
//! or alter anything"). Later slice-05 steps (04-xx) extend this World glue
//! with the row-equality, carried-session/token, and no-backfill scenarios.
//!
//! Driving surface (feature header §"Driving surface"): the upgrade is
//! migration-shaped — the "actor" is the operator upgrading the binary. The
//! scenario drives the SHIPPED migration runner: it stages the PRE-feature
//! migration history (`0001`..`0008`) into a `tempfile::TempDir` (the
//! `support::test_migration` precedent), seeds representative tenant data via
//! raw inserts, then applies the canonical forward-only migrations
//! (`0009`, `0010`, `0011`) via the SAME `run_migrations_from_dir` the
//! production boot path uses — TWICE — and asserts the second apply neither
//! duplicates the workspace nor alters any tenant row.
//!
//! LAYER 3 (real adapter, @real-io): real Postgres via testcontainers + a
//! per-scenario schema; the real migration runner under its advisory-lock
//! guard. Example-based (Mandates 9 + 11) — no PBT machinery at this layer.
//! Assertions are traditional, over port-exposed observables: the workspace
//! row count + the row-level before/after snapshot equality of every tenant
//! table (matching the slices 1-4 convention; no Rust state-delta port exists).

use crate::support::harness::fresh_schema_pool_no_migrations;
use crate::support::test_migration;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_store::{run_migrations_from_dir, Store};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

/// The tenant tables whose rows the guarantee promises to leave byte-for-byte
/// unchanged across the upgrade (ADR-004 §Decision step 2, extended to the FULL
/// tenant surface for step 04-02's comprehensive row-for-row equality proof).
///
/// Every table here carries a `workspace_id` (directly or transitively via an
/// `issue_id`/`team_id`/`project_id` chain) and holds real tenant data the
/// forward-only upgrade (`0009` index drop + `0010`/`0011` additive
/// columns/table) must leave untouched. `comments` and `machine_tokens` are the
/// two tenant tables the slice-05 Background already promises ("a live signed-in
/// session and a valid machine token", comment threads on issues) but earlier
/// steps did not yet snapshot — step 04-02 makes the proof faithful by seeding
/// and diffing them too, so a rewrite/backfill/re-key of ANY tenant row reds the
/// equality proof.
const TENANT_TABLES: &[&str] = &[
    "workspaces",
    "users",
    "workspace_memberships",
    "teams",
    "team_memberships",
    "projects",
    "issues",
    "invites",
    "comments",
    "machine_tokens",
];

/// Snapshot every tenant table as an ordered list of whole-row JSON strings,
/// keyed by table name. `to_jsonb(t.*)` renders the entire row deterministically;
/// ordering by the row text makes the comparison insertion-order independent.
async fn snapshot_tenant_tables(pool: &PgPool) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for table in TENANT_TABLES {
        let sql =
            format!("SELECT to_jsonb(t.*)::text AS row_json FROM {table} t ORDER BY row_json");
        let rows = sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| panic!("snapshot {table}: {e}"));
        let row_jsons = rows
            .into_iter()
            .map(|r| r.get::<String, _>("row_json"))
            .collect();
        out.insert((*table).to_string(), row_jsons);
    }
    out
}

// ---------------------------------------------------------------------------
// Background
// ---------------------------------------------------------------------------

/// Stand up a PRE-feature single-workspace install: a fresh schema migrated to
/// the pre-feature history (`0001`..`0008`) ONLY, with one workspace + its admin.
#[given(regex = r#"^a pre-feature single-workspace install of "([^"]+)" with admin "([^"]+)"$"#)]
async fn pre_feature_install(world: &mut FoundryWorld, ws_name: String, admin: String) {
    let (schema, pool, _url) = fresh_schema_pool_no_migrations().await;

    // Stage ONLY the pre-feature migration history (0001..0008) on disk and
    // apply it via the real runner — reconstructing a real pre-0009 schema.
    let staged = test_migration::stage_subset(8).expect("stage pre-feature migrations 0001..0008");
    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("apply pre-feature migration history");

    let workspace_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&ws_name)
        .execute(&pool)
        .await
        .expect("insert pre-feature workspace");

    let admin_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Ops Admin', 'phc$dummy')",
    )
    .bind(admin_id)
    .bind(&admin)
    .execute(&pool)
    .await
    .expect("insert pre-feature admin user");

    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("insert pre-feature admin membership");

    world.mwt5_schema = Some(schema);
    world.mwt5_pool = Some(pool);
    world.mwt5_staged = Some(staged);
    world.mwt5_workspace_id = Some(workspace_id);
    world.mwt5_admin_email = Some(admin);
}

/// Seed representative tenant data: a member, a team + membership, a project,
/// two issues, and an invite — so the upgrade has real rows to leave untouched.
#[given(regex = r#"^"([^"]+)" has members, teams, projects, issues, and invites$"#)]
async fn install_has_tenant_data(world: &mut FoundryWorld, _ws_name: String) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let workspace_id = world.mwt5_workspace_id.expect("workspace seeded");

    let member_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Member', 'phc$dummy')",
    )
    .bind(member_id)
    .bind("member@acme.com")
    .execute(&pool)
    .await
    .expect("insert member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert member membership");

    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .expect("insert team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(member_id)
        .execute(&pool)
        .await
        .expect("insert team membership");

    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Apollo', 'apollo', 'APL')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .expect("insert project");

    let mut first_issue_id = None;
    for (number, title) in [(1_i32, "First issue"), (2_i32, "Second issue")] {
        let issue_id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(workspace_id)
        .bind(number)
        .bind(title)
        .bind(member_id)
        .execute(&pool)
        .await
        .expect("insert issue");
        first_issue_id.get_or_insert(issue_id);
    }

    // A comment thread on the first issue — a tenant row whose byte-for-byte
    // survival the equality proof must cover (step 04-02 extends the snapshot to
    // `comments`). `body_markdown` is the raw input; `body_html` the rendered
    // output — the upgrade must touch neither.
    let comment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, 'A pre-feature comment', '<p>A pre-feature comment</p>')",
    )
    .bind(comment_id)
    .bind(workspace_id)
    .bind(first_issue_id.expect("at least one issue was seeded"))
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert comment");

    let invite_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO invites (id, workspace_id, invitee_email, created_by, expires_at)
              VALUES ($1, $2, 'newcomer@acme.com', $3, now() + interval '7 days')",
    )
    .bind(invite_id)
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert invite");
}

/// A live session + machine token from before the upgrade. Step 04-02 seeds a
/// REAL `machine_tokens` row (the "valid machine token" the Background promises)
/// so the comprehensive row-for-row equality proof covers the `machine_tokens`
/// tenant table too — the registry row (a `jti` bound to the admin + workspace 1)
/// must survive the upgrade byte-for-byte. The carried-session/token RESOLUTION
/// proof is a later scenario; here the token is tenant data the upgrade must not
/// rewrite, re-key, or revoke.
#[given(regex = r#"^"([^"]+)" has a live signed-in session and a valid machine token$"#)]
async fn install_has_session_and_token(world: &mut FoundryWorld, _ws_name: String) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let workspace_id = world.mwt5_workspace_id.expect("workspace seeded");
    let admin_email = world.mwt5_admin_email.clone().expect("admin email seeded");

    let admin_id =
        sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM users WHERE email_lower = $1")
            .bind(&admin_email)
            .fetch_one(&pool)
            .await
            .expect("look up the seeded admin's id for the machine token");

    let jti = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO machine_tokens (jti, user_id, workspace_id, expires_at, label)
              VALUES ($1, $2, $3, now() + interval '30 days', 'pre-feature CI token')",
    )
    .bind(jti)
    .bind(admin_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .expect("insert pre-feature machine token");

    // Capture the carried credential's identity so the resolution proof (sc 3) can
    // look up the SAME token + session after the upgrade — no re-issue, no re-binding.
    world.mwt5_machine_token_jti = Some(jti);
}

// ---------------------------------------------------------------------------
// Scenario 5: Re-running the upgrade does not duplicate or alter anything
// ---------------------------------------------------------------------------

/// Apply the canonical forward-only migration set (`0009`, `0010`, `0011`) ONCE
/// — the first upgrade — then snapshot every tenant table for later comparison.
#[given(regex = r#"^the install has already been upgraded to multi-workspace support$"#)]
async fn already_upgraded(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let staged = world.mwt5_staged.as_ref().expect("staged dir present");

    // Add the forward-only migrations (0009, 0010, 0011) to the staged dir, then
    // apply the now-canonical set. The pre-feature history is already applied, so
    // only 0009/0010/0011 run.
    test_migration::add_forward_only_to(staged.path())
        .expect("stage forward-only migrations 0009/0010/0011");
    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("apply the forward-only upgrade");

    world.mwt5_snapshot_after_first = snapshot_tenant_tables(&pool).await;
}

/// Apply the SAME canonical migration set a second time. Idempotent
/// (`IF EXISTS` / `IF NOT EXISTS`): no SQL re-runs, nothing changes.
#[when(regex = r#"^the upgrade is applied a second time$"#)]
async fn upgrade_applied_again(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let staged = world.mwt5_staged.as_ref().expect("staged dir present");

    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("re-applying the upgrade must not error");
}

/// The workspace is neither duplicated (still exactly one) nor altered (its id
/// unchanged from seed time).
#[then(regex = r#"^the workspace is neither duplicated nor altered$"#)]
async fn workspace_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");

    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert_eq!(
        count, 1,
        "the upgrade must NOT duplicate the workspace — exactly one must remain"
    );

    let id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("read workspace id");
    assert_eq!(
        id, expected_id,
        "the existing workspace's identity must be unchanged by the re-upgrade"
    );
}

/// Every tenant row is byte-for-byte identical to the post-first-upgrade
/// snapshot — the re-upgrade rewrote nothing.
#[then(regex = r#"^every tenant row remains exactly as it was after the first upgrade$"#)]
async fn tenant_rows_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let after_second = snapshot_tenant_tables(&pool).await;
    let after_first = &world.mwt5_snapshot_after_first;

    for table in TENANT_TABLES {
        assert_eq!(
            after_second.get(*table),
            after_first.get(*table),
            "re-applying the upgrade must leave every row of `{table}` exactly as it was"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 1 (walking skeleton): Upgrading a single-workspace install keeps it
// working as workspace 1.
//
// The thinnest end-to-end migration-then-resolve proof: stand up a pre-feature
// single-workspace install (Background), snapshot every tenant table, apply the
// forward-only upgrade (`0009`/`0010`/`0011`) via the SAME `run_migrations_from_dir`
// the production boot path uses, then assert the existing workspace IS workspace 1
// with its identity unchanged, every tenant row is byte-for-byte unchanged, and a
// carried-over user (NULL active workspace + sole membership) signs in and resolves
// to workspace 1 via the SHIPPED `resolve_active_workspace` seam — proving the
// no-backfill resolution (ADR-004 / D4) holds across the upgrade.
// ---------------------------------------------------------------------------

/// Apply the canonical forward-only upgrade ONCE — the operator-upgrade event.
/// Snapshot every tenant table FIRST (the pre-upgrade state the data-safety proof
/// compares against), then add `0009`/`0010`/`0011` to the staged dir and apply
/// the now-canonical set via the real runner (only the new migrations run, the
/// pre-feature history is already applied).
#[when(regex = r#"^the install is upgraded to multi-workspace support$"#)]
async fn install_is_upgraded(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let staged = world.mwt5_staged.as_ref().expect("staged dir present");

    world.mwt5_snapshot_before_upgrade = snapshot_tenant_tables(&pool).await;

    // Capture the existing member's board view (the issues + projects they can
    // see) BEFORE the upgrade, through the SHIPPED membership-gated scoped-read
    // seam — the same chain a returning user's board hits. Step 04-05's
    // regression proof (NFR-MWT-REL-02) compares this against the post-upgrade
    // read to prove nothing a returning user sees changed.
    world.mwt5_pre_upgrade_board = Some(member_board_view(&pool, Resolver::SoleMembership).await);

    test_migration::add_forward_only_to(staged.path())
        .expect("stage forward-only migrations 0009/0010/0011");
    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("apply the forward-only upgrade");
}

/// Columns the forward-only upgrade ADDITIVELY introduces (nullable, no rewrite)
/// — they do not exist in the pre-feature schema, so a row-level before/after
/// EQUALITY proof over tenant DATA (ADR-004 / D4) must compare the rows over the
/// columns that carried data before the upgrade, ignoring these additions. The
/// no-backfill invariant (active_workspace_id stays NULL) is proven separately by
/// the sign-in resolution step, not by row-shape equality.
///
/// `table -> additively-introduced column`:
/// - `users.active_workspace_id` (added by `0010_active_workspace.sql`).
const ADDITIVE_UPGRADE_COLUMNS: &[(&str, &str)] = &[("users", "active_workspace_id")];

/// Project a snapshot onto the tenant-data columns that existed BEFORE the
/// upgrade: strip any additively-introduced column (e.g. `active_workspace_id`)
/// from each row-JSON so a before/after comparison reflects DATA equality, not the
/// additive schema change the upgrade is allowed to make.
fn project_pre_upgrade_columns(
    snapshot: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    snapshot
        .iter()
        .map(|(table, rows)| {
            let stripped = rows
                .iter()
                .map(|row_json| strip_additive_columns(table, row_json))
                .collect();
            (table.clone(), stripped)
        })
        .collect()
}

/// Remove the additive-upgrade keys for `table` from one row-JSON object string.
fn strip_additive_columns(table: &str, row_json: &str) -> String {
    let mut value: serde_json::Value = serde_json::from_str(row_json)
        .unwrap_or_else(|e| panic!("parse row json for {table}: {e}"));
    if let Some(object) = value.as_object_mut() {
        for (additive_table, column) in ADDITIVE_UPGRADE_COLUMNS {
            if *additive_table == table {
                object.remove(*column);
            }
        }
    }
    value.to_string()
}

/// The existing workspace IS the first workspace and its id is unchanged from
/// seed time — the upgrade neither duplicated nor re-identified it.
#[then(
    regex = r#"^the existing workspace becomes the first workspace with its identity unchanged$"#
)]
async fn existing_workspace_is_first(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");
    let store = Store::from_pool(pool.clone());

    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert_eq!(
        count, 1,
        "the upgrade must leave exactly one workspace — the existing single workspace"
    );

    let first = store
        .first_workspace()
        .await
        .expect("read first workspace")
        .expect("the existing workspace must still be present after the upgrade");
    assert_eq!(
        first.0, expected_id,
        "the existing workspace's identity must be unchanged — it IS workspace 1"
    );
}

/// Every tenant row is byte-for-byte identical to the pre-upgrade snapshot — the
/// forward-only migrations rewrote, moved, or cross-wired nothing.
#[then(regex = r#"^all of its tenant data is present and unchanged$"#)]
async fn all_tenant_data_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let after = snapshot_tenant_tables(&pool).await;

    // Compare DATA: project both snapshots onto the columns that existed before the
    // upgrade so the additive-only `active_workspace_id` (added by 0010, NULL, no
    // rewrite) is not mistaken for a data change. ADR-004 / D4: forward-only, no
    // tenant row rewritten — the additive nullable column is the schema change, not
    // a data change.
    let after_data = project_pre_upgrade_columns(&after);
    let before_data = project_pre_upgrade_columns(&world.mwt5_snapshot_before_upgrade);

    for table in TENANT_TABLES {
        assert_eq!(
            after_data.get(*table),
            before_data.get(*table),
            "the upgrade must leave every data row of `{table}` unchanged"
        );
    }
}

/// The carried-over admin signs in and works exactly as before: the SHIPPED
/// sign-in seam (`find_user_by_email`) still finds them, and the SHIPPED
/// `resolve_active_workspace` maps them — NULL active workspace + sole membership
/// — to workspace 1 deterministically, with no value written (ADR-004 / D4).
#[then(regex = r#"^"([^"]+)" signs in and works exactly as before$"#)]
async fn admin_signs_in_as_before(world: &mut FoundryWorld, admin_email: String) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");
    let store = Store::from_pool(pool.clone());

    let user = store
        .find_user_by_email(&admin_email.to_ascii_lowercase())
        .await
        .expect("look up the carried-over admin")
        .unwrap_or_else(|| panic!("admin {admin_email:?} must still exist after the upgrade"));

    let resolved = store
        .resolve_active_workspace(user.id)
        .await
        .expect("resolve the carried-over admin's active workspace")
        .unwrap_or_else(|| panic!("admin {admin_email:?} must resolve to a workspace"));
    assert_eq!(
        resolved.0, expected_id,
        "the carried-over admin must resolve to workspace 1 (NULL-active + sole-membership)"
    );

    // No backfill (ADR-004 / D4): the upgrade leaves `active_workspace_id` NULL —
    // resolution maps the sole-membership user to workspace 1 without writing a value.
    let active_workspace_id = sqlx::query_scalar::<_, Option<uuid::Uuid>>(
        "SELECT active_workspace_id FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .expect("read the admin's active_workspace_id");
    assert!(
        active_workspace_id.is_none(),
        "the upgrade must NOT backfill active_workspace_id — it stays NULL (ADR-004 / D4)"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: No tenant data is lost or changed by the upgrade.
//
// The comprehensive row-for-row before/after EQUALITY proof across EVERY tenant
// table (NFR-MWT-DATA-01, ADR-004 / D4). Snapshot every tenant table BEFORE the
// upgrade, apply the forward-only migrations, snapshot AFTER, assert each table's
// rows are byte-for-byte identical (additive columns projected out) and the
// existing workspace's identity is unchanged.
//
// The proof is FALSIFIABLE by construction: it diffs whole-row JSON for every
// row of every tenant table, so ANY rewrite — a backfill writing
// `active_workspace_id` into a DATA-bearing column, a re-keyed primary key, a
// re-timestamped row, a moved/cross-wired foreign key — changes a row-JSON and
// reds the equality assertion. (Verified during RED by injecting a rewrite, see
// step 04-02 DELIVER log.)
// ---------------------------------------------------------------------------

/// Record the before-snapshot of EVERY tenant table — the baseline the equality
/// proof compares against after the upgrade. Captures the real pre-upgrade
/// database state row-for-row (ADR-004 §Decision step 2). The `When` step
/// re-captures the identical pre-upgrade state at its start (nothing mutates
/// between this `Given` and the upgrade), so this records into the same
/// `mwt5_snapshot_before_upgrade` slot the proof reads.
#[given(regex = r#"^a recorded snapshot of all the workspace's data before the upgrade$"#)]
async fn recorded_snapshot_before_upgrade(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    world.mwt5_snapshot_before_upgrade = snapshot_tenant_tables(&pool).await;
}

/// Every tenant row is present and unchanged afterward: re-snapshot every tenant
/// table after the upgrade and assert each table's rows are byte-for-byte
/// identical to the before-snapshot (additive-upgrade columns projected out so
/// the nullable `active_workspace_id` added by `0010` is not mistaken for a data
/// change). A missing, added, rewritten, re-keyed, or re-ordered-content row in
/// ANY tenant table reds this — the equality proof has no blind spot across the
/// full tenant surface.
#[then(regex = r#"^every tenant row is present and unchanged afterward$"#)]
async fn every_tenant_row_present_and_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let after = snapshot_tenant_tables(&pool).await;

    let after_data = project_pre_upgrade_columns(&after);
    let before_data = project_pre_upgrade_columns(&world.mwt5_snapshot_before_upgrade);

    for table in TENANT_TABLES {
        let before_rows = before_data.get(*table);
        let after_rows = after_data.get(*table);

        // Present: the upgrade neither dropped nor duplicated any row.
        assert_eq!(
            after_rows.map(Vec::len),
            before_rows.map(Vec::len),
            "the upgrade must leave the same number of rows in `{table}` — none lost, none added"
        );
        // Unchanged: every row is byte-for-byte identical to the before-snapshot.
        assert_eq!(
            after_rows, before_rows,
            "the upgrade must leave every row of `{table}` byte-for-byte unchanged \
             (no rewrite, backfill, or re-key)"
        );
    }
}

/// The existing workspace's identity is unchanged: exactly one workspace, and its
/// id is the same id captured at seed time. The upgrade neither duplicated nor
/// re-identified the existing single workspace — it IS workspace 1.
#[then(regex = r#"^the existing workspace's identity is unchanged$"#)]
async fn existing_workspace_identity_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");

    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert_eq!(
        count, 1,
        "the upgrade must leave exactly one workspace — the existing single workspace"
    );

    let id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("read workspace id");
    assert_eq!(
        id, expected_id,
        "the existing workspace's identity must be unchanged across the upgrade"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Existing sessions and machine tokens still resolve after the
// upgrade (NFR-MWT-DATA-02, ADR-004 / D4).
//
// A session and a machine token that PREDATE the upgrade keep working and
// resolve to workspace 1 — proving the NULL-active + sole-membership resolution
// path holds across the forward-only upgrade with NO re-issue or re-binding.
//
// Both credentials were seeded in the Background BEFORE any upgrade:
//   - the session leg is the admin user carried over (resolved via the SHIPPED
//     `resolve_active_workspace`, the same seam a returning user's session hits);
//   - the API leg is the machine token bound to (admin, workspace 1) via its
//     `workspace_id` column (looked up via the SHIPPED `find_machine_token_by_jti`,
//     the same seam the per-request verify path hits).
//
// Green-by-inheritance: the upgrade adds a nullable column + an empty table and
// drops a guard — it neither rewrites the membership the session resolves through
// nor re-keys the token's `workspace_id` binding. So the SHIPPED resolution seam,
// unchanged, still maps both carried credentials to workspace 1.
//
// FALSIFIABILITY (demonstrated during RED, then restored): mutating the carried
// token's `workspace_id` to a different workspace, or revoking it, or deleting the
// admin's sole membership, reds the corresponding Then — proving the proof bites.
// ---------------------------------------------------------------------------

/// The carried credentials exist BEFORE the upgrade: the Background seeded a
/// signed-in admin (the session leg) and a machine token bound to workspace 1 (the
/// API leg). This step confirms the carried credential is present pre-upgrade — the
/// precondition the post-upgrade resolution proof carries forward unchanged.
#[given(regex = r#"^an active session and a valid machine token from before the upgrade$"#)]
async fn active_session_and_valid_token(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let jti = world
        .mwt5_machine_token_jti
        .expect("the Background seeded a pre-upgrade machine token");
    let store = Store::from_pool(pool.clone());

    // The session leg: the carried admin must be a real signed-in user pre-upgrade.
    let admin_email = world.mwt5_admin_email.clone().expect("admin email seeded");
    store
        .find_user_by_email(&admin_email.to_ascii_lowercase())
        .await
        .expect("look up the carried-over admin pre-upgrade")
        .expect("the carried session's admin must exist before the upgrade");

    // The API leg: the carried machine token must be present + active pre-upgrade.
    let token = store
        .find_machine_token_by_jti(jti)
        .await
        .expect("look up the carried machine token pre-upgrade")
        .expect("the carried machine token must exist before the upgrade");
    assert!(
        token.revoked_at.is_none(),
        "the carried machine token must be valid (not revoked) before the upgrade"
    );
}

/// The carried session still resolves to workspace 1 after the upgrade: the SHIPPED
/// `resolve_active_workspace` seam (the session leg) maps the carried admin — NULL
/// active workspace + sole membership — to workspace 1, with no re-issue or
/// re-binding. The membership the session resolves through survived the upgrade
/// byte-for-byte, so resolution is unchanged.
#[then(regex = r#"^the carried session still resolves to the first workspace$"#)]
async fn carried_session_resolves_to_first(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");
    let admin_email = world.mwt5_admin_email.clone().expect("admin email seeded");
    let store = Store::from_pool(pool.clone());

    let user = store
        .find_user_by_email(&admin_email.to_ascii_lowercase())
        .await
        .expect("look up the carried-over admin after the upgrade")
        .expect("the carried session's admin must still exist after the upgrade");

    let resolved = store
        .resolve_active_workspace(user.id)
        .await
        .expect("resolve the carried session's active workspace after the upgrade")
        .expect("the carried session must still resolve to a workspace");
    assert_eq!(
        resolved.0, expected_id,
        "the carried session must still resolve to workspace 1 after the upgrade \
         (NULL-active + sole-membership, no re-binding)"
    );
}

/// The carried machine token still acts on workspace 1 after the upgrade: the
/// SHIPPED `find_machine_token_by_jti` verify-path seam (the API leg) returns the
/// SAME `jti` seeded before the upgrade, still bound to workspace 1 via its
/// `workspace_id` column, still valid (not revoked) — no re-issue or re-binding.
/// The token row survived the forward-only upgrade byte-for-byte, so the credential
/// the operator already holds keeps acting on the first workspace.
#[then(regex = r#"^the carried machine token still acts on the first workspace$"#)]
async fn carried_token_acts_on_first(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");
    let jti = world
        .mwt5_machine_token_jti
        .expect("the Background seeded a pre-upgrade machine token");
    let store = Store::from_pool(pool.clone());

    let token = store
        .find_machine_token_by_jti(jti)
        .await
        .expect("verify the carried machine token after the upgrade")
        .expect("the carried machine token must still exist after the upgrade — no re-issue");
    assert_eq!(
        token.jti, jti,
        "the carried machine token must be the SAME credential — not re-issued"
    );
    assert_eq!(
        token.workspace_id, expected_id,
        "the carried machine token must still act on workspace 1 — its binding is unchanged"
    );
    assert!(
        token.revoked_at.is_none(),
        "the upgrade must not revoke the carried machine token — it stays valid"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: An upgraded user resolves to workspace 1 without their active
// workspace being written (D4 / ADR-004 — the no-backfill finding made
// OBSERVABLE).
//
// The no-backfill guarantee says the upgrade achieves correct resolution WITHOUT
// rewriting any user row: an upgraded user whose `active_workspace_id` was never
// chosen keeps it NULL across the upgrade, and the SHIPPED resolution seam still
// maps them — NULL-active + sole-membership — to workspace 1. This step proves the
// OBSERVABLE no-backfill state: after resolution (run REPEATEDLY) the user's
// `active_workspace_id` is read back from the REAL database and is still NULL.
//
// Green-by-inheritance: resolution is READ-ONLY (a `SELECT`), so it cannot write
// `active_workspace_id`; the upgrade adds the column NULL and never backfills it.
//
// FALSIFIABILITY (demonstrated during RED, then restored): make resolution
// persist/backfill the resolved workspace into `active_workspace_id` (e.g. follow
// `resolve_active_workspace` with a `set_active_workspace`) → the post-resolution
// "stays NULL" assertion REDs, proving the proof bites.
// ---------------------------------------------------------------------------

/// The "actor" of this scenario: an upgraded user who NEVER chose an active
/// workspace before the upgrade. The Background's admin `ops@acme.com` is exactly
/// such a user — in the PRE-feature schema the `active_workspace_id` column does
/// not even exist yet (`0010` adds it), so by construction the user never chose an
/// active workspace. Confirm the precondition observably: the column is ABSENT
/// pre-upgrade, and the user is a sole-membership member (the NULL-active +
/// sole-membership path the no-backfill finding rests on).
#[given(regex = r#"^a user whose active workspace was never chosen before the upgrade$"#)]
async fn user_never_chose_active_workspace(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let admin_email = world.mwt5_admin_email.clone().expect("admin email seeded");

    // Pre-feature: there is no active-workspace column to choose into — the user
    // cannot have chosen one. (`0010` introduces `users.active_workspace_id`.)
    let column_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
             SELECT 1 FROM information_schema.columns
              WHERE table_name = 'users' AND column_name = 'active_workspace_id'
                AND table_schema = current_schema()
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("probe whether the active_workspace_id column exists pre-upgrade");
    assert!(
        !column_exists,
        "precondition: pre-feature, the active-workspace column does not exist — the user \
         could never have chosen an active workspace"
    );

    // And the user is a sole-membership member — the NULL-active + sole-membership
    // resolution path the no-backfill finding (D4) relies on.
    let membership_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM workspace_memberships m
            JOIN users u ON u.id = m.user_id
           WHERE u.email_lower = $1",
    )
    .bind(&admin_email)
    .fetch_one(&pool)
    .await
    .expect("count the upgraded user's memberships pre-upgrade");
    assert_eq!(
        membership_count, 1,
        "precondition: the upgraded user must be a member of exactly one workspace"
    );
}

/// That user resolves to the first workspace: the SHIPPED `resolve_active_workspace`
/// maps the upgraded user — NULL active workspace + sole membership — to workspace 1
/// deterministically. Resolve REPEATEDLY (three times) and assert every resolution
/// yields workspace 1 (AC 4: re-resolving keeps yielding the first workspace).
#[then(regex = r#"^that user resolves to the first workspace$"#)]
async fn upgraded_user_resolves_to_first(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");
    let admin_email = world.mwt5_admin_email.clone().expect("admin email seeded");
    let store = Store::from_pool(pool.clone());

    let user = store
        .find_user_by_email(&admin_email.to_ascii_lowercase())
        .await
        .expect("look up the upgraded user after the upgrade")
        .expect("the upgraded user must still exist after the upgrade");

    // Re-resolve repeatedly: each resolution must deterministically yield workspace 1
    // (NULL-active + sole-membership). Repetition proves resolution is stable and
    // never drifts to a different workspace across calls.
    for attempt in 1..=3 {
        let resolved = store
            .resolve_active_workspace(user.id)
            .await
            .unwrap_or_else(|e| panic!("resolve attempt {attempt}: {e}"))
            .unwrap_or_else(|| panic!("resolve attempt {attempt}: must resolve to a workspace"));
        assert_eq!(
            resolved.0, expected_id,
            "resolve attempt {attempt}: the upgraded user must resolve to workspace 1 \
             (NULL-active + sole-membership)"
        );
    }
}

/// Their active-workspace choice remains UNWRITTEN: read `active_workspace_id` back
/// from the REAL database AFTER resolution ran (repeatedly) and assert it is still
/// NULL — the OBSERVABLE proof of the no-backfill decision (D4 / ADR-004). The
/// upgrade wrote no row and resolution (a read-only SELECT) backfilled nothing, so
/// the user's unwritten active-workspace state is exactly as the upgrade left it.
/// Verified against the real DB, not inferred from the resolution code.
#[then(regex = r#"^their active-workspace choice remains unwritten$"#)]
async fn active_workspace_choice_remains_unwritten(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let admin_email = world.mwt5_admin_email.clone().expect("admin email seeded");

    let active_workspace_id = sqlx::query_scalar::<_, Option<uuid::Uuid>>(
        "SELECT active_workspace_id FROM users WHERE email_lower = $1",
    )
    .bind(&admin_email)
    .fetch_one(&pool)
    .await
    .expect("read the upgraded user's active_workspace_id after resolution");
    assert!(
        active_workspace_id.is_none(),
        "no-backfill (D4 / ADR-004): after resolution the user's active_workspace_id must \
         remain UNWRITTEN (NULL) — resolution wrote nothing, the upgrade backfilled nothing"
    );
}

// ---------------------------------------------------------------------------
// Scenario 6: Existing sign-in and workspace behaviour is unchanged after the
// upgrade (NFR-MWT-REL-02 — the regression proof).
//
// The single-workspace experience is the ONE-MEMBERSHIP SPECIAL CASE of
// multi-workspace, not a separate code path: an existing member signs in, the
// SHIPPED `resolve_active_workspace` maps them (NULL-active + sole-membership)
// to workspace 1, and the SHIPPED membership-gated scoped-read chain returns
// EXACTLY the issues + projects they saw before — nothing added, removed, or
// reordered. Proven through the shipped sign-in + scoped-read seams, NOT by
// inspecting internals.
//
// Green-by-inheritance: the upgrade adds a nullable column + an empty role table
// and drops a guard — it rewrites no membership, project, or issue row the board
// read traverses, so the read is unchanged. The pre-upgrade board view is
// captured in the `When` step (before any migration runs); this scenario asserts
// the post-upgrade view is byte-identical.
//
// FALSIFIABILITY (demonstrated during RED, then restored): the upgrade altering
// the member's visible issues/projects (e.g. an added/removed/re-titled issue),
// or sign-in landing on a different workspace, reds the corresponding Then —
// proving the regression proof bites.
// ---------------------------------------------------------------------------

/// The existing member ("member@acme.com") whose returning experience the
/// regression proof carries across the upgrade. They were seeded as a
/// sole-membership member of workspace 1 in the Background, with a team
/// membership, a project, and issues to see.
const EXISTING_MEMBER_EMAIL: &str = "member@acme.com";

/// How to resolve the existing member's acting workspace when capturing their
/// board view. The PRE-upgrade schema has no `active_workspace_id` column (it is
/// added by `0010`), so the shipped `resolve_active_workspace` (which SELECTs
/// that column) cannot run yet — pre-upgrade we resolve the sole-membership
/// workspace directly. POST-upgrade we drive the SHIPPED `resolve_active_workspace`
/// seam, the same path a returning user's session hits. The scoped-read chain the
/// board view traverses is identical either way — only the acting-workspace
/// resolver differs, mirroring the schema's forward-only evolution.
enum Resolver {
    /// Pre-upgrade: the active-workspace column does not exist yet; resolve the
    /// member's sole-membership workspace directly.
    SoleMembership,
    /// Post-upgrade: drive the SHIPPED `resolve_active_workspace` seam.
    Shipped,
}

/// Read the existing member's board view through the membership-gated scoped-read
/// seam: resolve their acting workspace (sole-membership ⇒ workspace 1), then
/// traverse the SAME `find_team_by_slug` → `is_team_member` →
/// `find_project_by_slug` → `list_issues_by_project` chain a returning user's
/// board hits. Returns the (issue titles, project names) the member is permitted
/// to see, sorted so the comparison reflects VISIBLE-SET equality, not row order.
async fn member_board_view(pool: &PgPool, resolver: Resolver) -> (Vec<String>, Vec<String>) {
    let store = Store::from_pool(pool.clone());

    let user_id = store
        .user_id_by_email(EXISTING_MEMBER_EMAIL)
        .await
        .expect("look up the existing member")
        .unwrap_or_else(|| panic!("existing member {EXISTING_MEMBER_EMAIL:?} must exist"));

    // Resolve the acting workspace. Post-upgrade this is the SHIPPED sign-in /
    // resolution seam (NULL-active + sole-membership ⇒ workspace 1, no value
    // written — the one-membership special case). Pre-upgrade the active-workspace
    // column does not exist yet, so we resolve the sole-membership workspace
    // directly — the same workspace the shipped seam will resolve to afterward.
    let acting_workspace_id = match resolver {
        Resolver::Shipped => match store
            .resolve_active_workspace(user_id)
            .await
            .expect("resolve the existing member's acting workspace")
        {
            Some((id, _name)) => id,
            None => return (Vec::new(), Vec::new()),
        },
        Resolver::SoleMembership => {
            match sqlx::query_scalar::<_, uuid::Uuid>(
                "SELECT workspace_id FROM workspace_memberships WHERE user_id = $1",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await
            .expect("resolve the existing member's sole-membership workspace pre-upgrade")
            {
                Some(id) => id,
                None => return (Vec::new(), Vec::new()),
            }
        }
    };

    // SHIPPED membership-gated scoped-read chain — a non-member sees nothing,
    // the lookups are scoped to the acting workspace.
    let Some(team) = store
        .find_team_by_slug(acting_workspace_id, "core")
        .await
        .expect("find team by slug scoped to the acting workspace")
    else {
        return (Vec::new(), Vec::new());
    };
    if !store
        .is_team_member(team.id, user_id)
        .await
        .expect("team membership gate")
    {
        return (Vec::new(), Vec::new());
    }
    let Some(project) = store
        .find_project_by_slug(team.id, "apollo")
        .await
        .expect("find project by slug scoped to the team")
    else {
        return (Vec::new(), Vec::new());
    };

    let mut issue_titles: Vec<String> = store
        .list_issues_by_project(project.id)
        .await
        .expect("scoped issue read")
        .into_iter()
        .map(|row| row.title)
        .collect();
    issue_titles.sort();

    let project_names = vec![project.name];

    (issue_titles, project_names)
}

/// An existing member signs in and lands on the first workspace: the SHIPPED
/// sign-in seam finds them and `resolve_active_workspace` maps them — NULL-active
/// + sole-membership — to workspace 1, the same workspace they always landed on.
/// Proven through the shipped seam, with the active-workspace value left
/// UNWRITTEN (the single-workspace experience is the one-membership special case,
/// not a separate code path).
#[then(regex = r#"^an existing member signs in and lands on the first workspace$"#)]
async fn existing_member_lands_on_first(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");
    let store = Store::from_pool(pool.clone());

    let user = store
        .find_user_by_email(EXISTING_MEMBER_EMAIL)
        .await
        .expect("look up the existing member after the upgrade")
        .unwrap_or_else(|| panic!("existing member {EXISTING_MEMBER_EMAIL:?} must still exist"));

    let resolved = store
        .resolve_active_workspace(user.id)
        .await
        .expect("resolve the existing member's active workspace after the upgrade")
        .expect("the existing member must resolve to a workspace");
    assert_eq!(
        resolved.0, expected_id,
        "the existing member must land on workspace 1 after the upgrade \
         (the one-membership special case of multi-workspace)"
    );

    let active_workspace_id = sqlx::query_scalar::<_, Option<uuid::Uuid>>(
        "SELECT active_workspace_id FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .expect("read the existing member's active_workspace_id");
    assert!(
        active_workspace_id.is_none(),
        "sign-in must not be a separate code path that backfills active_workspace_id — \
         it stays UNWRITTEN (the single-workspace experience is the sole-membership special case)"
    );
}

/// The existing member sees EXACTLY the issues and projects they saw before: the
/// post-upgrade board view (read through the SHIPPED resolution + scoped-read
/// seam) is byte-identical to the pre-upgrade view captured in the `When` step —
/// nothing added, removed, or re-titled.
#[then(regex = r#"^the existing member sees exactly the issues and projects they saw before$"#)]
async fn member_board_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let before = world
        .mwt5_pre_upgrade_board
        .clone()
        .expect("the pre-upgrade board view was captured in the upgrade step");

    let after = member_board_view(&pool, Resolver::Shipped).await;

    let (before_issues, before_projects) = &before;
    let (after_issues, after_projects) = &after;

    // The member must actually see something before the upgrade — otherwise the
    // equality below would be a vacuous "empty == empty" pass.
    assert!(
        !before_issues.is_empty(),
        "precondition: the existing member must see issues before the upgrade \
         (otherwise the regression proof is vacuous)"
    );

    assert_eq!(
        after_issues, before_issues,
        "the upgrade must leave the member's visible issues EXACTLY as before — \
         nothing added, removed, or re-titled"
    );
    assert_eq!(
        after_projects, before_projects,
        "the upgrade must leave the member's visible projects EXACTLY as before"
    );
}

/// Nothing about the single-workspace experience has changed: there is still
/// exactly one workspace, and the existing member's full board view (issues +
/// projects, read through the SHIPPED seam) is unchanged across the upgrade — no
/// behavioural change is observable at the sign-in or scoped-read surface.
#[then(regex = r#"^nothing about the single-workspace experience has changed$"#)]
async fn single_workspace_experience_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let before = world
        .mwt5_pre_upgrade_board
        .clone()
        .expect("the pre-upgrade board view was captured in the upgrade step");

    // Still a single-workspace install — the upgrade did not turn the existing
    // experience into a multi-workspace one for this member.
    let workspace_count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert_eq!(
        workspace_count, 1,
        "the single-workspace experience is preserved — still exactly one workspace"
    );

    // And the member's whole board view is unchanged across the upgrade.
    let after = member_board_view(&pool, Resolver::Shipped).await;
    assert_eq!(
        after, before,
        "no behavioural change is observable at the member's sign-in or scoped-read \
         surface — the single-workspace experience is unchanged"
    );
}
