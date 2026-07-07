//! US-04 rolling-upgrade — per-scenario migration staging helper.
//!
//! Per `distill/driver.md` §2d and `wave-decisions.md` §US-04 (Option B):
//!
//! The US-04 acceptance scenarios need to introduce a new migration
//! (e.g. `0099_add_dummy_column.sql`) on top of the production base
//! schema, WITHOUT modifying `crates/foundry-store/migrations/` (which
//! would poison every sibling scenario and the release build).
//!
//! Strategy:
//!   1. Open a fresh `tempfile::TempDir`.
//!   2. Copy the canonical production migrations
//!      (`crates/foundry-store/migrations/*.sql`) into the temp dir
//!      verbatim, preserving filenames so sqlx's version parser sees
//!      the same 0001..0005 base history.
//!   3. Stage a per-scenario `0099_<descriptor>.sql` alongside.
//!   4. Hand the temp dir's path to the per-replica AppState via
//!      `AppState::test_migrations_dir` so the boot path runs
//!      `foundry_store::run_migrations_from_dir` against the staged dir
//!      under the SAME advisory-lock guard the production path uses.
//!
//! When the [`TestMigrationsDir`] handle drops at scenario end the
//! tempdir is unlinked (sqlx's runtime migrator does not hold the path
//! after `run()` completes).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Handle to a per-scenario temp migrations directory. Owns the
/// `TempDir` so the on-disk files live for the scenario's lifetime.
pub struct TestMigrationsDir {
    dir: TempDir,
}

impl TestMigrationsDir {
    /// Filesystem path the staged migrations live at. Pass this to
    /// `AppState::test_migrations_dir` (then to
    /// `foundry_store::run_migrations_from_dir`).
    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}

impl std::fmt::Debug for TestMigrationsDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestMigrationsDir")
            .field("path", &self.dir.path())
            .finish()
    }
}

/// Stage the canonical production migrations into a fresh temp dir,
/// then append `extras` (each `(filename, body)`).
///
/// `extras` are written verbatim into the temp dir as siblings of the
/// base migrations. Typical use is one extra:
/// `("0099_add_dummy_column.sql", "ALTER TABLE issues ADD COLUMN ...")`.
///
/// Returns a handle whose `Drop` removes the temp dir. The caller MUST
/// keep the handle alive for the duration of the scenario.
pub fn stage(extras: &[(&str, &str)]) -> Result<TestMigrationsDir> {
    let dir = tempfile::Builder::new()
        .prefix("foundry-us04-mig-")
        .tempdir()
        .context("create tempdir for US-04 staged migrations")?;

    // Copy every production migration into the temp dir.
    let prod_dir = production_migrations_dir();
    for entry in std::fs::read_dir(&prod_dir)
        .with_context(|| format!("read production migrations dir {prod_dir:?}"))?
    {
        let entry = entry?;
        let path = entry.path();
        // Migrations are .sql files; ignore everything else (README, etc).
        let is_sql = path.extension().map(|e| e == "sql").unwrap_or(false);
        if !is_sql || !entry.file_type()?.is_file() {
            continue;
        }
        let filename = path
            .file_name()
            .with_context(|| format!("filename for {path:?}"))?
            .to_owned();
        let dst = dir.path().join(&filename);
        std::fs::copy(&path, &dst).with_context(|| format!("copy {path:?} -> {dst:?}"))?;
    }

    // Append per-scenario extras (e.g. 0099_*.sql). The version prefix
    // determines sqlx's ordering; extras must sort AFTER the base set.
    for (filename, body) in extras {
        let dst = dir.path().join(filename);
        std::fs::write(&dst, body).with_context(|| format!("write staged migration {dst:?}"))?;
    }

    Ok(TestMigrationsDir { dir })
}

/// Stage ONLY the canonical production migrations whose numeric version
/// prefix is `<= max_version` into a fresh temp dir.
///
/// Used by the slice-05 migration-guarantee harness to reconstruct a
/// PRE-feature migration history (`0001`..`0008`, `max_version = 8`) so
/// the forward-only set (`0009`/`0010`/`0011`) can then be applied on top
/// against the SAME `run_migrations_from_dir` runner the production boot
/// path uses — proving the upgrade is additive and idempotent without
/// touching `crates/foundry-store/migrations/`.
///
/// Returns a handle whose `Drop` removes the temp dir.
pub fn stage_subset(max_version: u64) -> Result<TestMigrationsDir> {
    let dir = tempfile::Builder::new()
        .prefix("foundry-mwt05-mig-")
        .tempdir()
        .context("create tempdir for staged pre-feature migrations")?;

    let prod_dir = production_migrations_dir();
    for entry in std::fs::read_dir(&prod_dir)
        .with_context(|| format!("read production migrations dir {prod_dir:?}"))?
    {
        let entry = entry?;
        let path = entry.path();
        let is_sql = path.extension().map(|e| e == "sql").unwrap_or(false);
        if !is_sql || !entry.file_type()?.is_file() {
            continue;
        }
        let filename = path
            .file_name()
            .with_context(|| format!("filename for {path:?}"))?
            .to_owned();
        // The numeric prefix before the first '_' is the sqlx version.
        let version: u64 = filename
            .to_string_lossy()
            .split('_')
            .next()
            .and_then(|n| n.parse().ok())
            .with_context(|| format!("parse migration version from {filename:?}"))?;
        if version > max_version {
            continue;
        }
        let dst = dir.path().join(&filename);
        std::fs::copy(&path, &dst).with_context(|| format!("copy {path:?} -> {dst:?}"))?;
    }

    Ok(TestMigrationsDir { dir })
}

/// Copy the multi-workspace-era forward-only migrations (versions `0009`,
/// `0010`, `0011` — the migrations that INTRODUCE multi-workspace support) into
/// an EXISTING staged dir.
///
/// Companion to [`stage_subset`]: stage the pre-feature history first
/// (`stage_subset(8)`), apply it, then `add_forward_only_to(dir)` to drop the
/// `0009`/`0010`/`0011` upgrade migrations alongside and apply the now-canonical
/// set — exactly the operator-upgrade sequence the slice-05 guarantee proves.
///
/// The version range is BOUNDED to `9..=11` on purpose. This test's guarantee is
/// "the multi-workspace upgrade preserves every tenant `issues` row byte-for-byte
/// (`to_jsonb(t.*)`) — no rewrite, backfill, or re-key." LATER feature migrations
/// are NOT part of "the multi-workspace upgrade" and legitimately mutate rows —
/// e.g. `0012_issue_position` adds + backfills `issues.position` (card-ranking),
/// `0013_issue_change_events` adds a table — so sweeping them in here would make
/// the byte-for-byte proof spuriously fail. Those migrations carry their own
/// feature acceptance; this helper stays scoped to the multi-workspace set.
pub fn add_forward_only_to(dir: &Path) -> Result<()> {
    let prod_dir = production_migrations_dir();
    let mut copied_versions = Vec::new();
    for entry in std::fs::read_dir(&prod_dir)
        .with_context(|| format!("read production migrations dir {prod_dir:?}"))?
    {
        let entry = entry?;
        let path = entry.path();
        let is_sql = path.extension().map(|e| e == "sql").unwrap_or(false);
        if !is_sql || !entry.file_type()?.is_file() {
            continue;
        }
        let filename = path
            .file_name()
            .with_context(|| format!("filename for {path:?}"))?
            .to_owned();
        let version: u64 = filename
            .to_string_lossy()
            .split('_')
            .next()
            .and_then(|n| n.parse().ok())
            .with_context(|| format!("parse migration version from {filename:?}"))?;
        // Only the multi-workspace-era migrations (0009..=0011). Later feature
        // migrations (0012 position backfill, 0013 change-events, …) are not
        // part of "the multi-workspace upgrade" and would break the
        // byte-for-byte tenant-data guarantee — see the fn doc.
        if !(9..=11).contains(&version) {
            continue;
        }
        let dst = dir.join(&filename);
        std::fs::copy(&path, &dst).with_context(|| format!("copy {path:?} -> {dst:?}"))?;
        copied_versions.push(version);
    }

    // The canonical forward-only upgrade set is `0009`, `0010`, AND the
    // feature's additive `0011_instance_admins.sql` (ADR-003/004, D6). The
    // slice-05 guarantee is the upgrade-safety PROOF for ALL THREE; until
    // `0011` ships, the "upgrade" the scenario applies is incomplete and the
    // guarantee is unproven — so staging the canonical set MUST fail. This is
    // the genuine RED for step 01-01 (DISTILL RED-state contract: "0011
    // MISSING → idempotence unproven until built").
    if !copied_versions.contains(&11) {
        anyhow::bail!(
            "canonical forward-only migration set is incomplete: \
             0011_instance_admins.sql is missing from {prod_dir:?}"
        );
    }
    Ok(())
}

/// Resolve the absolute path to `crates/foundry-store/migrations` from
/// this crate's `CARGO_MANIFEST_DIR`. The workspace layout is fixed:
/// `foundry-acceptance` and `foundry-store` are siblings under
/// `crates/`.
fn production_migrations_dir() -> PathBuf {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    PathBuf::from(manifest)
        .join("..")
        .join("foundry-store")
        .join("migrations")
}
