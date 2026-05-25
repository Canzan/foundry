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
