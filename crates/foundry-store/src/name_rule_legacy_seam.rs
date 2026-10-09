//! The legacy seam (name-db-checks DDD-10, test-support only): store a row "that
//! predates the name rule" — the state a pre-0019 door or a restore leaves.
//!
//! In ONE transaction it switches off the table's two name-rule triggers BY
//! NAME, runs the caller's write, switches both back on and commits. A failed
//! write rolls the whole transaction back, so the triggers are never left off.
//! The ALTER is transactional and holds its table lock until commit, so no other
//! session ever sees the rule absent.
//!
//! Never `DISABLE TRIGGER ALL` (it would skip the foreign keys and needs
//! superuser), never `session_replication_role`. check-arch
//! `name-rule-legacy-seam` keeps every trigger switch in this file and every
//! call out of the production doors.

use sqlx::postgres::{PgArguments, PgQueryResult};
use sqlx::query::Query;
use sqlx::{PgPool, Postgres};

/// The tables that carry the name rule (closed set).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRuleTable {
    Workspaces,
    Projects,
}

impl NameRuleTable {
    fn table(self) -> &'static str {
        match self {
            Self::Workspaces => "workspaces",
            Self::Projects => "projects",
        }
    }

    /// `ALTER TABLE` statements switching both name triggers to `state`.
    fn switch(self, state: &str) -> [String; 2] {
        let t = self.table();
        ["on_insert", "on_rename"]
            .map(|kind| format!("ALTER TABLE {t} {state} TRIGGER {t}_name_rule_{kind}"))
    }
}

/// Run `write` on `table` with its two name-rule triggers switched off, all in
/// one transaction; both are enabled again before the commit.
pub async fn seed_row_predating_name_rule<'q>(
    pool: &PgPool,
    table: NameRuleTable,
    write: Query<'q, Postgres, PgArguments>,
) -> Result<PgQueryResult, sqlx::Error> {
    let mut tx = pool.begin().await?;
    for off in table.switch("DISABLE") {
        sqlx::query(&off).execute(&mut *tx).await?;
    }
    let written = write.execute(&mut *tx).await?;
    for on in table.switch("ENABLE") {
        sqlx::query(&on).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(written)
}
