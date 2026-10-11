//! Warnings a boot must log about the migrations it just applied
//! (team-lifecycle, user decision 2026-10-11, DELIVER 02-01).
//!
//! Migration 0023 creates `team_lifecycle_events` with foreign keys to
//! `workspaces` and `users` (ADR-TEAM-LIFECYCLE-003). From then on,
//! `pg_restore --clean` of a dump taken BEFORE 0023 over the upgraded database
//! fails: it cannot drop `workspaces_pkey` / `users_pkey`. The user kept the
//! foreign keys; the supported path is to restore such a dump into a fresh
//! database (or drop `team_lifecycle_events` first). The boot warns once, when
//! it UPGRADES a database through 0023 ("handled during upgrading as a
//! warning"), and the CHANGELOG says the same. A fresh install, which applies
//! 1..23 in one boot, has no older dump and does not warn.
//!
//! No new `MigrationReport` field is needed: `already_applied` already lists
//! the versions present before the boot, so "upgrade" is `applied` containing
//! 23 while `already_applied` is non-empty.
//!
//! Written by the acceptance designer (2026-10-11), implemented in DELIVER
//! 02-01. [`UPGRADE_WARNING_0023`] is the one source of the text;
//! `Store::migrate` logs each returned warning once at `warn`. Pinned by
//! `tests/team_lifecycle_migrations.rs`.

use crate::MigrationReport;

/// The boot's warning when it applies migration 0023.
pub const UPGRADE_WARNING_0023: &str = "Migration 0023 added team_lifecycle_events, which \
references workspaces and users. A database dump taken before 0023 can no longer be restored \
over this database with pg_restore --clean: restore it into a fresh database, or drop \
team_lifecycle_events first.";

/// The warnings for this boot, in version order: [`UPGRADE_WARNING_0023`]
/// exactly once when this boot applied 0023 (`report.applied` contains 23) on
/// top of migrations already present before it (`report.already_applied` is
/// non-empty); nothing for a fresh install, a boot that applied nothing, or a
/// boot that did not apply 0023.
pub fn upgrade_warnings(report: &MigrationReport) -> Vec<&'static str> {
    let upgraded_through_0023 = report.applied.contains(&23) && !report.already_applied.is_empty();
    if upgraded_through_0023 {
        vec![UPGRADE_WARNING_0023]
    } else {
        Vec::new()
    }
}
