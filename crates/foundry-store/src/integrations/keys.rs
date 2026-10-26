//! The credential keys' check values and `doctor rotate-secrets` (DDR-9,
//! ADR-EIP-004 decision 4, C-11).

use crate::{Store, StoreError};
use uuid::Uuid;

/// One key's check value (`integration_key_checks`, DDR-9): a fixed canary
/// sealed under the key `key_id` names, so the boot probe can prove a
/// configured key is the one that wrote it. No `Debug`: it carries sealed
/// bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyCheck {
    pub key_id: String,
    pub sealed_canary: Vec<u8>,
}

/// What `doctor rotate-secrets` did (C-11), or why it changed nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum ResealOutcome<E> {
    /// Every stored credential was sealed again and the canary written.
    Resealed { credentials: usize },
    /// The caller could not re-seal a credential: the transaction was rolled
    /// back, so nothing changed.
    Refused(E),
}

impl Store {
    /// Every stored sealed credential (the boot probe reads only their key
    /// ids; ADR-EIP-004 decision 4).
    pub async fn sealed_credentials(&self) -> Result<Vec<Vec<u8>>, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT sealed_credential FROM integration_connections
              WHERE sealed_credential IS NOT NULL ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Every key's check value, in key id order.
    pub async fn key_checks(&self) -> Result<Vec<KeyCheck>, StoreError> {
        let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
            "SELECT key_id, sealed_canary FROM integration_key_checks ORDER BY key_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(key_id, sealed_canary)| KeyCheck {
                key_id,
                sealed_canary,
            })
            .collect())
    }

    /// Record a key's check value unless that key already has one (every
    /// replica of a rollout may race to write it; the first stays).
    pub async fn record_key_check(&self, check: &KeyCheck) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO integration_key_checks (key_id, sealed_canary) VALUES ($1, $2)
             ON CONFLICT (key_id) DO NOTHING",
        )
        .bind(&check.key_id)
        .bind(&check.sealed_canary)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `doctor rotate-secrets` (C-11, ADR-EIP-004 decision 5): in one
    /// transaction, replace every stored sealed credential with what `reseal`
    /// makes of it (given the row id the value is bound to) and write
    /// `canary` as its key's check value. A refusal from `reseal` rolls the
    /// whole transaction back. The store never sees a plaintext.
    pub async fn reseal_credentials<E>(
        &self,
        mut reseal: impl FnMut(Uuid, &[u8]) -> Result<Vec<u8>, E>,
        canary: &KeyCheck,
    ) -> Result<ResealOutcome<E>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, Vec<u8>)> = sqlx::query_as(
            "SELECT id, sealed_credential FROM integration_connections
              WHERE sealed_credential IS NOT NULL ORDER BY id FOR UPDATE",
        )
        .fetch_all(&mut *tx)
        .await?;
        for (id, sealed) in &rows {
            let resealed = match reseal(*id, sealed) {
                Ok(resealed) => resealed,
                Err(why) => {
                    tx.rollback().await?;
                    return Ok(ResealOutcome::Refused(why));
                }
            };
            sqlx::query("UPDATE integration_connections SET sealed_credential = $2 WHERE id = $1")
                .bind(id)
                .bind(resealed)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "INSERT INTO integration_key_checks (key_id, sealed_canary) VALUES ($1, $2)
             ON CONFLICT (key_id) DO UPDATE SET sealed_canary = EXCLUDED.sealed_canary",
        )
        .bind(&canary.key_id)
        .bind(&canary.sealed_canary)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ResealOutcome::Resealed {
            credentials: rows.len(),
        })
    }
}
