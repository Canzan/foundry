//! instance-integration-setup — every query on the instance integration
//! tables of migration 0033 (check-arch `integrations-instance-tables`;
//! ADR-IIS-001, ADR-IIS-003, ADR-IIS-004, ADR-IIS-005).
//!
//! The store applies plans; it never makes one, and it never sees a plaintext:
//! the secrets key arrives wrapped, the Slack secrets sealed, and every
//! re-seal is the caller's closure. Each instance-level write is one
//! transaction that first takes the instance-integrations lock exclusive
//! (before any row lock, ADR-IIS-004 rule 1), records its activity line and
//! bumps the config version. A write judged over a count - the I-14 gate over
//! the connected workspaces - reads that count inside its own transaction,
//! under that same lock, so nothing it is judged over can change between the
//! reading and the write (D10).

use super::KeyCheck;
use crate::{Store, StoreError};
use std::future::Future;
use time::OffsetDateTime;
use uuid::Uuid;

/// The instance-integrations advisory lock's name. Its id is hashed with the
/// transaction's `search_path`, so per-schema deployments and test runs never
/// contend.
pub const INSTANCE_INTEGRATIONS_LOCK: &str = "foundry.instance-integrations";

/// How many workspaces hold a live Slack connection: the one count I-14 and
/// I-15 read and the I-14 gate judges over (D10). A disconnected connection
/// posts nothing, so it counts for neither.
const CONNECTED_SLACK_WORKSPACES: &str =
    "SELECT count(DISTINCT workspace_id) FROM integration_connections
      WHERE kind = 'slack' AND state <> 'disconnected'";

/// Which instance key wrapped the stored secrets key (`wrapped_by_source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyWrapper {
    /// `FOUNDRY_INSTANCE_KEY` (or its legacy alias).
    Operator,
    /// The instance key file (ADR-IIS-002).
    File,
}

impl KeyWrapper {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Operator => "operator",
            Self::File => "file",
        }
    }

    fn from_wire(wire: &str) -> Result<Self, StoreError> {
        [Self::Operator, Self::File]
            .into_iter()
            .find(|source| source.wire_name() == wire)
            .ok_or_else(|| {
                StoreError::Sqlx(sqlx::Error::Decode(
                    format!("unknown wrapped_by_source {wire:?}").into(),
                ))
            })
    }
}

/// A secrets key for Generate and Move to store if none is. No `Debug`: it
/// carries the wrapped key.
#[derive(Clone)]
pub struct NewSecretsKey {
    pub key_id: String,
    pub wrapped_key: Vec<u8>,
    pub wrapped_by_key_id: String,
    pub wrapped_by: KeyWrapper,
    pub created_by: Option<Uuid>,
}

/// The stored secrets key row. No `Debug`: it carries the wrapped key.
#[derive(Clone, PartialEq, Eq)]
pub struct StoredSecretsKey {
    pub key_id: String,
    pub wrapped_key: Vec<u8>,
    pub wrapped_by_key_id: String,
    pub wrapped_by: KeyWrapper,
    pub rewrapped_from_path: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: OffsetDateTime,
}

#[cfg(feature = "test-support")]
impl StoredSecretsKey {
    /// The stored secrets key row the database `pool` reaches, for the
    /// generated-key test seam (check-arch `integrations-test-seam`): it
    /// takes a pool, not a `Store`, because no integrations file but the two
    /// capability seams names `Store`.
    pub async fn read_for_test_seam(pool: &sqlx::PgPool) -> Result<Option<Self>, StoreError> {
        let mut tx = pool.begin().await?;
        let stored = read_secrets_key(&mut tx).await?;
        tx.rollback().await?;
        Ok(stored)
    }
}

/// The stored secrets key wrapped again under another instance key
/// (ADR-IIS-002 r5, ADR-IIS-001 decision 8), compare-and-set on
/// `from_key_id`: the instance key it was wrapped under when read. No
/// `Debug`: it carries the wrapped key.
#[derive(Clone)]
pub struct SecretsKeyRewrap {
    pub key_id: String,
    pub from_key_id: String,
    pub wrapped_key: Vec<u8>,
    pub wrapped_by_key_id: String,
    pub wrapped_by: KeyWrapper,
    /// The key file the key was wrapped under, when the re-wrap moves it off
    /// one (I-30); `None` keeps what is recorded.
    pub rewrapped_from_path: Option<String>,
}

/// What a boot's re-wrap did.
#[derive(Debug, PartialEq, Eq)]
pub enum RewrapOutcome {
    /// The key, one system activity line and the version bump were committed
    /// together.
    Rewrapped { version: i64 },
    /// The stored key is no longer the one read (another replica re-wrapped
    /// it first): nothing changed.
    AlreadyRewrapped,
}

/// What `doctor rotate-secrets` did, or why it changed nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum RotateOutcome<E> {
    /// The re-wrap (when any), every re-sealed value (`resealed` counts
    /// credentials), the canary and the version bump were committed together.
    Rotated {
        rewrapped: bool,
        resealed: usize,
        version: i64,
    },
    /// The caller could not re-wrap the key or re-seal a value: the
    /// transaction was rolled back, so nothing changed.
    Refused(E),
    /// The re-wrap matched no row (the stored key is not the one it was made
    /// from): the transaction was rolled back, so nothing changed.
    KeyChanged,
}

/// One stored sealed value, as a re-seal closure is handed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SealedSlot {
    /// `integration_connections.sealed_credential` of this connection.
    Connection(Uuid),
    /// `instance_slack_app.sealed_client_secret`.
    SlackClientSecret,
    /// `instance_slack_app.sealed_signing_secret`.
    SlackSigningSecret,
    /// `instance_slack_app_pending.sealed_client_secret`: the submission
    /// waiting for its confirm is re-sealed with every other sealed value, so
    /// it still opens at the confirm (ADR-IIS-003 r4).
    PendingSlackClientSecret,
    /// `instance_slack_app_pending.sealed_signing_secret`.
    PendingSlackSigningSecret,
}

/// What Generate or Move did, or why it changed nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum SecretsKeyOutcome<E> {
    /// The key, every re-sealed value, its canary, one activity line and the
    /// version bump were committed together.
    Stored { resealed: usize, version: i64 },
    /// A secrets key was already stored (another replica's Generate won):
    /// nothing changed.
    AlreadyStored { key_id: String },
    /// The caller could not re-seal a value: the transaction was rolled
    /// back, so nothing changed.
    Refused(E),
}

/// A Slack app for an instance admin's save. `sealed_under` is the key id
/// both secrets were sealed under: the stored secrets key's, or `None` when
/// no secrets key is stored (`Direct` mode). No `Debug`: it carries sealed
/// secrets.
#[derive(Clone)]
pub struct SlackAppSave {
    pub client_id: String,
    pub sealed_client_secret: Vec<u8>,
    pub sealed_signing_secret: Vec<u8>,
    pub sealed_under: Option<String>,
    pub saved_by: Uuid,
    pub saved_at: OffsetDateTime,
}

/// What a Slack save did.
#[derive(Debug, PartialEq, Eq)]
pub enum SlackSaveOutcome {
    Saved {
        version: i64,
    },
    /// The secrets were sealed under a key that is no longer the sealing key
    /// (the seal fence, ADR-IIS-004 rule 2): nothing changed; reload and
    /// retry once.
    StaleKey,
}

/// A Slack app held for its confirm (I-14): the app an instance admin
/// submitted while workspaces were still connected through the app in effect,
/// sealed as a save is and fenced the same way, applicable only by
/// `requested_by` and only before `expires_at` (ADR-IIS-003 r4). No `Debug`:
/// it carries sealed secrets.
#[derive(Clone)]
pub struct PendingSlackAppSave {
    pub client_id: String,
    pub sealed_client_secret: Vec<u8>,
    pub sealed_signing_secret: Vec<u8>,
    pub sealed_under: Option<String>,
    pub requested_by: Uuid,
    pub expires_at: OffsetDateTime,
}

/// The pending Slack app row. No `Debug`: it carries sealed secrets.
#[derive(Clone, PartialEq, Eq)]
pub struct PendingSlackApp {
    pub client_id: String,
    pub sealed_client_secret: Vec<u8>,
    pub sealed_signing_secret: Vec<u8>,
    pub requested_by: Uuid,
    pub expires_at: OffsetDateTime,
}

impl PendingSlackApp {
    /// Whether `actor` may still apply this held app at `now`: its own
    /// requester, before its expiry. The expiry instant itself is too late,
    /// as every later instant is (K2).
    fn applicable_by(&self, actor: Uuid, now: OffsetDateTime) -> bool {
        self.requested_by == actor && now < self.expires_at
    }
}

/// Which row a judged submission is written to, as the I-14 gate decides it
/// over the connected-workspace count read inside that write's own
/// transaction (D10). Both plans are sealed before the lock is taken,
/// because the count the gate reads is only the count that holds while the
/// lock is held; exactly one of them is written. No `Debug`: both plans
/// carry sealed secrets.
#[derive(Clone, Copy)]
pub enum SlackSubmissionPlan<'a> {
    /// Nobody is switched away from the app in effect, so the submitted app
    /// is stored at once.
    SaveAtOnce(&'a SlackAppSave),
    /// Workspaces are still connected through the app in effect, so the
    /// submitted app waits for the same admin's confirm (I-14).
    WaitForConfirm(&'a PendingSlackAppSave),
}

/// What a submission's write did: which row the gate sent it to, with that
/// write's own fenced outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum SubmissionStored {
    /// The app was stored as the saved app, so it is in effect now.
    Saved(SlackSaveOutcome),
    /// The app was held for its confirm (I-14): nothing is in effect yet.
    Held(SlackSaveOutcome),
}

/// What a confirm of the held Slack app did, or why it changed nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum PendingApplied<E> {
    /// The held app was applied as a save and the row deleted, in one
    /// transaction: the save's own outcome, so one retry rule answers for
    /// both doors.
    Applied(SlackSaveOutcome),
    /// No held app this admin may apply at this instant - none held, another
    /// admin's, or one at or past its expiry: nothing changed, and the row (if
    /// any) was left as it was.
    NotApplicable,
    /// The caller could not open the held secrets or seal them again for the
    /// saved app's row: the transaction was rolled back, so nothing changed.
    Refused(E),
}

/// What a remove of the saved Slack app did (US-IIS-05 AC 3, D8).
#[derive(Debug, PartialEq, Eq)]
pub enum RemoveOutcome {
    /// The saved app's row was deleted, one line recorded and the version
    /// bumped, in one transaction. Nothing else was touched: no connection
    /// row, no delivery, no notice.
    Removed { version: i64 },
    /// No app was saved here, so there was nothing to remove: nothing
    /// changed.
    NothingSaved,
}

/// The saved Slack app row. No `Debug`: it carries sealed secrets.
#[derive(Clone, PartialEq, Eq)]
pub struct SavedSlackApp {
    pub client_id: String,
    pub sealed_client_secret: Vec<u8>,
    pub sealed_signing_secret: Vec<u8>,
    pub saved_by: Option<Uuid>,
    pub saved_at: OffsetDateTime,
}

/// What a reload rebuilds from, read in one `REPEATABLE READ` transaction
/// with the version first (ADR-IIS-004 rule 4), so the state is exactly the
/// version's. No `Debug`: it carries wrapped and sealed bytes.
#[derive(Clone)]
pub struct InstanceSnapshot {
    pub version: i64,
    pub secrets_key: Option<StoredSecretsKey>,
    pub slack_app: Option<SavedSlackApp>,
    /// The app held for its confirm (I-14): the reload ignores it, and the
    /// boot probe counts its secrets among the values a held key must open.
    pub slack_app_pending: Option<PendingSlackApp>,
    pub key_checks: Vec<KeyCheck>,
    /// Every stored connection credential (the reload reads their key ids).
    pub sealed_credentials: Vec<Vec<u8>>,
}

/// One replica's applied version and key mode (ADR-IIS-004 rule 8), stamped
/// with the caller's clock. Never a key or a key id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicaState {
    pub replica: String,
    pub applied_version: i64,
    pub key_mode: String,
    pub opens_secrets_key: bool,
    pub beat_at: OffsetDateTime,
}

/// One instance activity line as stored, newest first in a read (I-18).
/// Never a key, a secret, a key id or a length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceActivityRow {
    pub created_at: OffsetDateTime,
    /// The actor's display name; `None` for Foundry itself.
    pub actor_name: Option<String>,
    /// The stored action's wire name (`secrets_key_generated`, …).
    pub action: String,
    pub client_id: Option<String>,
    pub count: Option<i32>,
}

/// The instance activity lines this module writes (ADR-IIS-005).
#[derive(Clone, Copy)]
enum Activity {
    SecretsKeyGenerated,
    CredentialsMoved,
    SlackAppSaved,
    SlackAppReplaced,
    SlackAppRemoved,
    SecretsKeyRewrapped,
}

impl Activity {
    fn wire_name(self) -> &'static str {
        match self {
            Self::SecretsKeyGenerated => "secrets_key_generated",
            Self::CredentialsMoved => "credentials_moved",
            Self::SlackAppSaved => "slack_app_saved",
            Self::SlackAppReplaced => "slack_app_replaced",
            Self::SlackAppRemoved => "slack_app_removed",
            Self::SecretsKeyRewrapped => "secrets_key_rewrapped",
        }
    }
}

/// One activity line's facts.
struct ActivityLine<'a> {
    actor: Option<Uuid>,
    activity: Activity,
    client_id: Option<&'a str>,
    count: Option<i32>,
    at: OffsetDateTime,
}

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// The 0033 tables the probe counts.
pub(crate) const PROBE_0033_TABLES: i64 = 5;

/// The probe's view of 0033 in the active schema.
pub(crate) const PROBE_0033_SQL: &str = "
    SELECT count(*) FROM information_schema.tables
     WHERE table_schema = current_schema()
       AND table_name IN ('integration_secrets_key', 'instance_slack_app',
                          'integration_config_version', 'integration_replica_states',
                          'instance_integration_audit_events')";

impl Store {
    /// The config version, as each replica's reload polls it.
    pub async fn config_version(&self) -> Result<i64, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT version FROM integration_config_version WHERE id = 1")
                .fetch_one(&self.pool)
                .await?,
        )
    }

    /// Generate and Move, one use-case (ADR-IIS-001 decision 5): under the
    /// exclusive lock, store `key` unless a secrets key is stored, hand every
    /// stored sealed value to `reseal` and store what it returns (`None`
    /// keeps the value as it is), write `canary`, record one line ("moved n"
    /// for n re-sealed credentials, else "generated") and bump the version.
    /// `resealed` and the line count credentials (I-22): each connection one,
    /// the saved Slack app one. A refusal from `reseal` rolls everything back.
    pub async fn store_secrets_key<E>(
        &self,
        key: &NewSecretsKey,
        canary: &KeyCheck,
        at: OffsetDateTime,
        mut reseal: impl FnMut(SealedSlot, &[u8]) -> Result<Option<Vec<u8>>, E>,
    ) -> Result<SecretsKeyOutcome<E>, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        if let Some(key_id) = stored_key_id(&mut tx).await? {
            tx.rollback().await?;
            return Ok(SecretsKeyOutcome::AlreadyStored { key_id });
        }
        insert_secrets_key(&mut tx, key, at).await?;
        let resealed = match reseal_sealed_values(&mut tx, &mut reseal).await? {
            Ok(resealed) => resealed.credentials(),
            Err(why) => {
                tx.rollback().await?;
                return Ok(SecretsKeyOutcome::Refused(why));
            }
        };
        upsert_key_check(&mut tx, canary).await?;
        let (activity, count) = match resealed {
            0 => (Activity::SecretsKeyGenerated, None),
            n => (Activity::CredentialsMoved, Some(count_of(n))),
        };
        let line = ActivityLine {
            actor: key.created_by,
            activity,
            client_id: None,
            count,
            at,
        };
        let version = commit_recorded(tx, &line).await?;
        Ok(SecretsKeyOutcome::Stored { resealed, version })
    }

    /// A boot's re-wrap of the stored secrets key under the operator's
    /// instance key (ADR-IIS-002 r5): under the exclusive lock, compare-and-set
    /// on the key and the instance key it was read wrapped under. The winner
    /// records Foundry's system line (I-18) and bumps the version; a replica
    /// that lost changes nothing and adopts the row on its rebuild.
    pub async fn rewrap_secrets_key(
        &self,
        rewrap: &SecretsKeyRewrap,
        at: OffsetDateTime,
    ) -> Result<RewrapOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        if !update_wrapped_key(&mut tx, rewrap).await? {
            tx.rollback().await?;
            return Ok(RewrapOutcome::AlreadyRewrapped);
        }
        let line = ActivityLine {
            actor: None,
            activity: Activity::SecretsKeyRewrapped,
            client_id: None,
            count: None,
            at,
        };
        let version = commit_recorded(tx, &line).await?;
        Ok(RewrapOutcome::Rewrapped { version })
    }

    /// `doctor rotate-secrets` (ADR-IIS-001 decision 8, ADR-IIS-004 rule 1):
    /// in one transaction under the exclusive lock, hand the stored secrets
    /// key (when any) to `rewrap` and store the re-wrap it returns, hand every
    /// stored sealed value to `reseal` and store what it returns (`None`
    /// keeps the value), write `canary` and bump the version. A refusal from
    /// either rolls everything back.
    pub async fn rotate_instance_key<E>(
        &self,
        mut rewrap: impl FnMut(&StoredSecretsKey) -> Result<Option<SecretsKeyRewrap>, E>,
        mut reseal: impl FnMut(SealedSlot, &[u8]) -> Result<Option<Vec<u8>>, E>,
        canary: &KeyCheck,
    ) -> Result<RotateOutcome<E>, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        let stored = read_secrets_key(&mut tx).await?;
        let plan = match stored.as_ref().map(&mut rewrap).transpose() {
            Ok(plan) => plan.flatten(),
            Err(why) => {
                tx.rollback().await?;
                return Ok(RotateOutcome::Refused(why));
            }
        };
        if let Some(plan) = &plan {
            if !update_wrapped_key(&mut tx, plan).await? {
                tx.rollback().await?;
                return Ok(RotateOutcome::KeyChanged);
            }
        }
        let resealed = match reseal_sealed_values(&mut tx, &mut reseal).await? {
            Ok(resealed) => resealed.credentials(),
            Err(why) => {
                tx.rollback().await?;
                return Ok(RotateOutcome::Refused(why));
            }
        };
        upsert_key_check(&mut tx, canary).await?;
        let version = bump_version(&mut tx).await?;
        tx.commit().await?;
        Ok(RotateOutcome::Rotated {
            rewrapped: plan.is_some(),
            resealed,
            version,
        })
    }

    /// A Slack app save on its own (ADR-IIS-003): under the exclusive lock,
    /// refuse a save sealed under anything but the sealing key, else store the
    /// app, record one line naming its client ID ("saved", or "replaced" over
    /// a saved app) and bump the version. An instance admin's submission
    /// reaches this same write through its gate
    /// ([`Store::store_slack_submission`]), which reads the count it is judged
    /// over in the same transaction (D10).
    pub async fn save_slack_app(&self, app: &SlackAppSave) -> Result<SlackSaveOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        save_slack_app(tx, app).await
    }

    /// An instance admin's removal of the saved Slack app (US-IIS-05 AC 3,
    /// D8): one transaction under the exclusive lock that deletes the row,
    /// records one line naming the app it took away and bumps the version.
    /// It seals nothing, so there is no key it was sealed under and no fence
    /// to hold; it touches nothing else, so every connection keeps its state,
    /// its `lost_reason` and its queued deliveries. With nothing saved it
    /// removes nothing and records nothing.
    pub async fn remove_slack_app(
        &self,
        actor: Uuid,
        now: OffsetDateTime,
    ) -> Result<RemoveOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        let removed: Option<String> =
            sqlx::query_scalar("DELETE FROM instance_slack_app WHERE id = 1 RETURNING client_id")
                .fetch_optional(&mut *tx)
                .await?;
        let Some(client_id) = removed else {
            tx.rollback().await?;
            return Ok(RemoveOutcome::NothingSaved);
        };
        let line = ActivityLine {
            actor: Some(actor),
            activity: Activity::SlackAppRemoved,
            client_id: Some(&client_id),
            count: None,
            at: now,
        };
        let version = commit_recorded(tx, &line).await?;
        Ok(RemoveOutcome::Removed { version })
    }

    /// Hold the Slack app an admin submitted while workspaces are connected
    /// through the app in effect, for her confirm (I-14, ADR-IIS-003 r4):
    /// this is no instance-level change, so it takes the lock shared, fences
    /// on the sealing key (ADR-IIS-004 r2), and writes nothing a reader sees -
    /// no activity line and no version bump. A new submission overwrites the
    /// row. `Saved` reports the version as it stands, unbumped.
    pub async fn hold_pending_slack_app(
        &self,
        app: &PendingSlackAppSave,
    ) -> Result<SlackSaveOutcome, StoreError> {
        let tx = self.pool.begin().await?;
        hold_pending_slack_app(tx, app).await
    }

    /// An instance admin's submission of a Slack app, with the I-14 gate
    /// decided inside the write's own transaction (D10, ADR-IIS-004 r1): the
    /// exclusive lock first, then the connected-workspace count, then `gate` -
    /// the caller's pure decision over that count - and then the one write it
    /// chose, fenced on the sealing key as every sealed write is.
    ///
    /// The count is read under the lock every instance write takes, so a
    /// workspace that connects while a submission is in flight either is
    /// counted by the gate that judges it or waits behind that write: no app
    /// is switched under a connected workspace without the confirm I-14 asks
    /// for.
    pub async fn store_slack_submission<'plan>(
        &self,
        gate: impl FnOnce(i64) -> SlackSubmissionPlan<'plan>,
    ) -> Result<SubmissionStored, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        let connected = sqlx::query_scalar(CONNECTED_SLACK_WORKSPACES)
            .fetch_one(&mut *tx)
            .await?;
        match gate(connected) {
            SlackSubmissionPlan::SaveAtOnce(app) => {
                Ok(SubmissionStored::Saved(save_slack_app(tx, app).await?))
            }
            SlackSubmissionPlan::WaitForConfirm(app) => Ok(SubmissionStored::Held(
                hold_pending_slack_app(tx, app).await?,
            )),
        }
    }

    /// `actor`'s confirm of the held Slack app at `now` (ADR-IIS-003 r4): one
    /// transaction under the exclusive lock, so the row applied is the row
    /// read. The held app is applied only for the requester that holds it and
    /// only before its expiry; applying it deletes it, so a second confirm,
    /// another admin's confirm and a confirm at or after the expiry all find
    /// nothing to apply and change nothing.
    ///
    /// A sealed value is bound to the row it is stored in, so the held bytes
    /// are handed to `reseal`, which answers with both secrets sealed for the
    /// saved app's row; the store makes no plan and sees no plaintext. The
    /// write is then the save's own, fenced on `sealed_under` as any save is.
    /// A refusal from `reseal` rolls everything back.
    pub async fn apply_pending_slack_app<E>(
        &self,
        actor: Uuid,
        now: OffsetDateTime,
        sealed_under: Option<&str>,
        reseal: impl FnOnce(&[u8], &[u8]) -> Result<(Vec<u8>, Vec<u8>), E>,
    ) -> Result<PendingApplied<E>, StoreError> {
        let mut tx = self.pool.begin().await?;
        lock_exclusive(&mut tx).await?;
        let held = read_pending_slack_app(&mut tx, true).await?;
        let Some(held) = held.filter(|held| held.applicable_by(actor, now)) else {
            tx.rollback().await?;
            return Ok(PendingApplied::NotApplicable);
        };
        let sealed = reseal(&held.sealed_client_secret, &held.sealed_signing_secret);
        let (sealed_client_secret, sealed_signing_secret) = match sealed {
            Ok(sealed) => sealed,
            Err(why) => {
                tx.rollback().await?;
                return Ok(PendingApplied::Refused(why));
            }
        };
        delete_pending_slack_app(&mut tx).await?;
        let app = SlackAppSave {
            client_id: held.client_id,
            sealed_client_secret,
            sealed_signing_secret,
            sealed_under: sealed_under.map(str::to_string),
            saved_by: actor,
            saved_at: now,
        };
        Ok(PendingApplied::Applied(save_slack_app(tx, &app).await?))
    }

    /// How many workspaces hold a live Slack connection, as I-14 and I-15
    /// read it per request (D10). The I-14 gate reads the same count inside
    /// its own write ([`Store::store_slack_submission`]), never this one.
    pub async fn connected_slack_workspaces(&self) -> Result<i64, StoreError> {
        Ok(sqlx::query_scalar(CONNECTED_SLACK_WORKSPACES)
            .fetch_one(&self.pool)
            .await?)
    }

    /// Delete the held Slack app once it has expired (the caller's clock,
    /// never `now()`): the retention tick's prune, outside any other write.
    /// The expiry instant itself is already too late, as the confirm reads it.
    pub async fn prune_pending_slack_app(&self, now: OffsetDateTime) -> Result<u64, StoreError> {
        Ok(
            sqlx::query("DELETE FROM instance_slack_app_pending WHERE expires_at <= $1")
                .bind(now)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }

    /// The reload snapshot (ADR-IIS-004 rule 4).
    pub async fn instance_snapshot(&self) -> Result<InstanceSnapshot, StoreError> {
        self.instance_snapshot_with(|_| async {}).await
    }

    /// The reload snapshot, running `after_version_read` (given the version
    /// read) between the version read and the state reads: the gold test's
    /// seam for a write landing between them. Production passes nothing.
    pub async fn instance_snapshot_with<F, Fut>(
        &self,
        after_version_read: F,
    ) -> Result<InstanceSnapshot, StoreError>
    where
        F: FnOnce(i64) -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
        let version: i64 =
            sqlx::query_scalar("SELECT version FROM integration_config_version WHERE id = 1")
                .fetch_one(&mut *tx)
                .await?;
        after_version_read(version).await;
        let secrets_key = read_secrets_key(&mut tx).await?;
        let slack_app = read_slack_app(&mut tx).await?;
        let slack_app_pending = read_pending_slack_app(&mut tx, false).await?;
        let key_checks = sqlx::query_as::<_, (String, Vec<u8>)>(
            "SELECT key_id, sealed_canary FROM integration_key_checks ORDER BY key_id",
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|(key_id, sealed_canary)| KeyCheck {
            key_id,
            sealed_canary,
        })
        .collect();
        let sealed_credentials = sqlx::query_scalar(
            "SELECT sealed_credential FROM integration_connections
              WHERE sealed_credential IS NOT NULL ORDER BY id",
        )
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(InstanceSnapshot {
            version,
            secrets_key,
            slack_app,
            slack_app_pending,
            key_checks,
            sealed_credentials,
        })
    }

    /// The instance's latest activity lines, newest first (I-18).
    pub async fn instance_activity(
        &self,
        limit: i64,
    ) -> Result<Vec<InstanceActivityRow>, StoreError> {
        type Row = (
            OffsetDateTime,
            Option<String>,
            String,
            Option<String>,
            Option<i32>,
        );
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT a.created_at, u.display_name, a.action, a.client_id, a.count
               FROM instance_integration_audit_events a
               LEFT JOIN users u ON u.id = a.actor_id
              ORDER BY a.created_at DESC, a.id DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(created_at, actor_name, action, client_id, count)| InstanceActivityRow {
                    created_at,
                    actor_name,
                    action,
                    client_id,
                    count,
                },
            )
            .collect())
    }

    /// Upsert this replica's state row: the only write a reload makes.
    pub async fn beat_replica_state(&self, state: &ReplicaState) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO integration_replica_states
                    (replica, applied_version, key_mode, opens_secrets_key, beat_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (replica) DO UPDATE SET
                    applied_version = EXCLUDED.applied_version,
                    key_mode = EXCLUDED.key_mode,
                    opens_secrets_key = EXCLUDED.opens_secrets_key,
                    beat_at = EXCLUDED.beat_at",
        )
        .bind(&state.replica)
        .bind(state.applied_version)
        .bind(&state.key_mode)
        .bind(state.opens_secrets_key)
        .bind(state.beat_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Every replica's state, by replica name.
    pub async fn replica_states(&self) -> Result<Vec<ReplicaState>, StoreError> {
        let rows: Vec<(String, i64, String, bool, OffsetDateTime)> = sqlx::query_as(
            "SELECT replica, applied_version, key_mode, opens_secrets_key, beat_at
               FROM integration_replica_states ORDER BY replica",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(replica, applied_version, key_mode, opens_secrets_key, beat_at)| ReplicaState {
                    replica,
                    applied_version,
                    key_mode,
                    opens_secrets_key,
                    beat_at,
                },
            )
            .collect())
    }

    /// Delete the replica states that last beat before `stale_before` (the
    /// caller's clock, never `now()`); returns how many.
    pub async fn prune_replica_states(
        &self,
        stale_before: OffsetDateTime,
    ) -> Result<u64, StoreError> {
        Ok(
            sqlx::query("DELETE FROM integration_replica_states WHERE beat_at < $1")
                .bind(stale_before)
                .execute(&self.pool)
                .await?
                .rows_affected(),
        )
    }
}

/// Store `app` in `tx`, which already holds the instance-integrations lock
/// exclusive: refuse a save sealed under anything but the sealing key (the
/// seal fence, ADR-IIS-004 r2), else the upsert, one line naming its client
/// ID ("saved", or "replaced" over a saved app) and the version bump,
/// committed together. The one writer of `instance_slack_app`, so an admin's
/// save and the confirm's apply are one write, not two
/// (check-arch `integrations-seal-fence`).
async fn save_slack_app(
    mut tx: Tx<'_>,
    app: &SlackAppSave,
) -> Result<SlackSaveOutcome, StoreError> {
    if stored_key_id(&mut tx).await? != app.sealed_under {
        tx.rollback().await?;
        return Ok(SlackSaveOutcome::StaleKey);
    }
    let replaced: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM instance_slack_app WHERE id = 1)")
            .fetch_one(&mut *tx)
            .await?;
    sqlx::query(
        "INSERT INTO instance_slack_app
                (id, client_id, sealed_client_secret, sealed_signing_secret, saved_by, saved_at)
         VALUES (1, $1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE SET
                client_id = EXCLUDED.client_id,
                sealed_client_secret = EXCLUDED.sealed_client_secret,
                sealed_signing_secret = EXCLUDED.sealed_signing_secret,
                saved_by = EXCLUDED.saved_by,
                saved_at = EXCLUDED.saved_at",
    )
    .bind(&app.client_id)
    .bind(&app.sealed_client_secret)
    .bind(&app.sealed_signing_secret)
    .bind(app.saved_by)
    .bind(app.saved_at)
    .execute(&mut *tx)
    .await?;
    let activity = if replaced {
        Activity::SlackAppReplaced
    } else {
        Activity::SlackAppSaved
    };
    let line = ActivityLine {
        actor: Some(app.saved_by),
        activity,
        client_id: Some(&app.client_id),
        count: None,
        at: app.saved_at,
    };
    let version = commit_recorded(tx, &line).await?;
    Ok(SlackSaveOutcome::Saved { version })
}

/// Hold `app` in `tx` for its confirm (I-14, ADR-IIS-003 r4): refuse a hold
/// sealed under anything but the sealing key (the seal fence, ADR-IIS-004 r2),
/// else the upsert of the one pending row, committed with nothing else - no
/// activity line and no version bump, because nothing a reader sees changes,
/// so `Saved` reports the version as it stands. A new submission overwrites
/// the row. The one writer of `instance_slack_app_pending`'s sealed columns
/// for a hold (check-arch `integrations-seal-fence`).
///
/// The fence takes the instance-integrations lock shared, which a hold of its
/// own needs and which the submission write already holds exclusively where
/// the gate chose this hold in the same transaction: a lock a transaction
/// already holds is granted again at once.
async fn hold_pending_slack_app(
    mut tx: Tx<'_>,
    app: &PendingSlackAppSave,
) -> Result<SlackSaveOutcome, StoreError> {
    if !seal_fence_holds(&mut tx, app.sealed_under.as_deref()).await? {
        tx.rollback().await?;
        return Ok(SlackSaveOutcome::StaleKey);
    }
    sqlx::query(
        "INSERT INTO instance_slack_app_pending
                (id, client_id, sealed_client_secret, sealed_signing_secret,
                 requested_by, expires_at)
         VALUES (1, $1, $2, $3, $4, $5)
         ON CONFLICT (id) DO UPDATE SET
                client_id = EXCLUDED.client_id,
                sealed_client_secret = EXCLUDED.sealed_client_secret,
                sealed_signing_secret = EXCLUDED.sealed_signing_secret,
                requested_by = EXCLUDED.requested_by,
                expires_at = EXCLUDED.expires_at",
    )
    .bind(&app.client_id)
    .bind(&app.sealed_client_secret)
    .bind(&app.sealed_signing_secret)
    .bind(app.requested_by)
    .bind(app.expires_at)
    .execute(&mut *tx)
    .await?;
    let version = read_config_version(&mut tx).await?;
    tx.commit().await?;
    Ok(SlackSaveOutcome::Saved { version })
}

/// Take the instance-integrations lock exclusive for the transaction.
async fn lock_exclusive(tx: &mut Tx<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "SELECT pg_advisory_xact_lock(
                    hashtextextended($1 || '|' || current_setting('search_path'), 0))",
    )
    .bind(INSTANCE_INTEGRATIONS_LOCK)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// The seal fence of a write that stores a sealed value but is no
/// instance-level write (ADR-IIS-004 rule 2): take the instance-integrations
/// lock shared, before any row lock, and say whether `sealed_under` is still
/// the sealing key — the stored secrets key's id, or `None` when none is
/// stored (`Direct` mode). `false` means the caller rolls back with a
/// stale-key refusal.
pub(super) async fn seal_fence_holds(
    tx: &mut Tx<'_>,
    sealed_under: Option<&str>,
) -> Result<bool, sqlx::Error> {
    sqlx::query(
        "SELECT pg_advisory_xact_lock_shared(
                    hashtextextended($1 || '|' || current_setting('search_path'), 0))",
    )
    .bind(INSTANCE_INTEGRATIONS_LOCK)
    .execute(&mut **tx)
    .await?;
    Ok(stored_key_id(tx).await?.as_deref() == sealed_under)
}

/// Write `canary` as its key's check value, replacing any.
async fn upsert_key_check(tx: &mut Tx<'_>, canary: &KeyCheck) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO integration_key_checks (key_id, sealed_canary) VALUES ($1, $2)
         ON CONFLICT (key_id) DO UPDATE SET sealed_canary = EXCLUDED.sealed_canary",
    )
    .bind(&canary.key_id)
    .bind(&canary.sealed_canary)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Store `rewrap` if the stored key is still `rewrap.key_id` wrapped under
/// `rewrap.from_key_id`; whether it was.
async fn update_wrapped_key(
    tx: &mut Tx<'_>,
    rewrap: &SecretsKeyRewrap,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE integration_secrets_key
            SET wrapped_key = $3, wrapped_by_key_id = $4, wrapped_by_source = $5,
                rewrapped_from_path = COALESCE($6, rewrapped_from_path)
          WHERE id = 1 AND key_id = $1 AND wrapped_by_key_id = $2",
    )
    .bind(&rewrap.key_id)
    .bind(&rewrap.from_key_id)
    .bind(&rewrap.wrapped_key)
    .bind(&rewrap.wrapped_by_key_id)
    .bind(rewrap.wrapped_by.wire_name())
    .bind(rewrap.rewrapped_from_path.as_deref())
    .execute(&mut **tx)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// The config version as it stands in `tx`, bumped by nobody.
async fn read_config_version(tx: &mut Tx<'_>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT version FROM integration_config_version WHERE id = 1")
        .fetch_one(&mut **tx)
        .await
}

/// The held Slack app, row-locked when `for_update` (the confirm reads and
/// applies it in one transaction).
async fn read_pending_slack_app(
    tx: &mut Tx<'_>,
    for_update: bool,
) -> Result<Option<PendingSlackApp>, sqlx::Error> {
    type Row = (String, Vec<u8>, Vec<u8>, Uuid, OffsetDateTime);
    let sql = if for_update {
        "SELECT client_id, sealed_client_secret, sealed_signing_secret, requested_by, expires_at
           FROM instance_slack_app_pending WHERE id = 1 FOR UPDATE"
    } else {
        "SELECT client_id, sealed_client_secret, sealed_signing_secret, requested_by, expires_at
           FROM instance_slack_app_pending WHERE id = 1"
    };
    let row: Option<Row> = sqlx::query_as(sql).fetch_optional(&mut **tx).await?;
    Ok(row.map(
        |(client_id, sealed_client_secret, sealed_signing_secret, requested_by, expires_at)| {
            PendingSlackApp {
                client_id,
                sealed_client_secret,
                sealed_signing_secret,
                requested_by,
                expires_at,
            }
        },
    ))
}

async fn delete_pending_slack_app(tx: &mut Tx<'_>) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM instance_slack_app_pending WHERE id = 1")
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn stored_key_id(tx: &mut Tx<'_>) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT key_id FROM integration_secrets_key WHERE id = 1")
        .fetch_optional(&mut **tx)
        .await
}

async fn insert_secrets_key(
    tx: &mut Tx<'_>,
    key: &NewSecretsKey,
    at: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO integration_secrets_key
                (id, key_id, wrapped_key, wrapped_by_key_id, wrapped_by_source,
                 created_by, created_at)
         VALUES (1, $1, $2, $3, $4, $5, $6)",
    )
    .bind(&key.key_id)
    .bind(&key.wrapped_key)
    .bind(&key.wrapped_by_key_id)
    .bind(key.wrapped_by.wire_name())
    .bind(key.created_by)
    .bind(at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// What a re-seal changed.
#[derive(Debug, Default)]
struct Resealed {
    connections: usize,
    app_secrets: usize,
    /// The held app's secrets re-sealed. No credential of I-22's count
    /// (ADR-IIS-001): nothing is connected through an app nobody confirmed.
    pending_secrets: usize,
}

impl Resealed {
    /// The credentials re-sealed (I-22): each connection one, the saved
    /// Slack app one however many of its secrets were.
    fn credentials(&self) -> usize {
        self.connections + usize::from(self.app_secrets > 0)
    }
}

/// Hand every stored sealed value (connections, the saved Slack app, then the
/// app held for its confirm) to `reseal`, row-locked, and store each value it
/// returns. The outer error is the database's; the inner, the caller's refusal
/// (the caller rolls back).
async fn reseal_sealed_values<E>(
    tx: &mut Tx<'_>,
    reseal: &mut impl FnMut(SealedSlot, &[u8]) -> Result<Option<Vec<u8>>, E>,
) -> Result<Result<Resealed, E>, sqlx::Error> {
    let mut resealed = Resealed::default();
    let connections: Vec<(Uuid, Vec<u8>)> = sqlx::query_as(
        "SELECT id, sealed_credential FROM integration_connections
          WHERE sealed_credential IS NOT NULL ORDER BY id FOR UPDATE",
    )
    .fetch_all(&mut **tx)
    .await?;
    for (id, sealed) in &connections {
        match reseal(SealedSlot::Connection(*id), sealed) {
            Err(why) => return Ok(Err(why)),
            Ok(None) => {}
            Ok(Some(value)) => {
                sqlx::query(
                    "UPDATE integration_connections SET sealed_credential = $2 WHERE id = $1",
                )
                .bind(id)
                .bind(value)
                .execute(&mut **tx)
                .await?;
                resealed.connections += 1;
            }
        }
    }
    let app: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT sealed_client_secret, sealed_signing_secret FROM instance_slack_app
          WHERE id = 1 FOR UPDATE",
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((client_secret, signing_secret)) = app {
        for (slot, sealed, sql) in [
            (
                SealedSlot::SlackClientSecret,
                client_secret,
                "UPDATE instance_slack_app SET sealed_client_secret = $1 WHERE id = 1",
            ),
            (
                SealedSlot::SlackSigningSecret,
                signing_secret,
                "UPDATE instance_slack_app SET sealed_signing_secret = $1 WHERE id = 1",
            ),
        ] {
            match reseal(slot, &sealed) {
                Err(why) => return Ok(Err(why)),
                Ok(None) => {}
                Ok(Some(value)) => {
                    sqlx::query(sql).bind(value).execute(&mut **tx).await?;
                    resealed.app_secrets += 1;
                }
            }
        }
    }
    let pending: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT sealed_client_secret, sealed_signing_secret FROM instance_slack_app_pending
          WHERE id = 1 FOR UPDATE",
    )
    .fetch_optional(&mut **tx)
    .await?;
    let Some((client_secret, signing_secret)) = pending else {
        return Ok(Ok(resealed));
    };
    for (slot, sealed, sql) in [
        (
            SealedSlot::PendingSlackClientSecret,
            client_secret,
            "UPDATE instance_slack_app_pending SET sealed_client_secret = $1 WHERE id = 1",
        ),
        (
            SealedSlot::PendingSlackSigningSecret,
            signing_secret,
            "UPDATE instance_slack_app_pending SET sealed_signing_secret = $1 WHERE id = 1",
        ),
    ] {
        match reseal(slot, &sealed) {
            Err(why) => return Ok(Err(why)),
            Ok(None) => {}
            Ok(Some(value)) => {
                sqlx::query(sql).bind(value).execute(&mut **tx).await?;
                resealed.pending_secrets += 1;
            }
        }
    }
    Ok(Ok(resealed))
}

/// An instance write's end: record `line`, bump the version and commit; the
/// new version.
async fn commit_recorded(mut tx: Tx<'_>, line: &ActivityLine<'_>) -> Result<i64, StoreError> {
    record_activity(&mut tx, line).await?;
    let version = bump_version(&mut tx).await?;
    tx.commit().await?;
    Ok(version)
}

async fn record_activity(tx: &mut Tx<'_>, line: &ActivityLine<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO instance_integration_audit_events
                (id, actor_id, action, client_id, count, created_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(Uuid::now_v7())
    .bind(line.actor)
    .bind(line.activity.wire_name())
    .bind(line.client_id)
    .bind(line.count)
    .bind(line.at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn bump_version(tx: &mut Tx<'_>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE integration_config_version SET version = version + 1 WHERE id = 1
         RETURNING version",
    )
    .fetch_one(&mut **tx)
    .await
}

async fn read_secrets_key(tx: &mut Tx<'_>) -> Result<Option<StoredSecretsKey>, StoreError> {
    type Row = (
        String,
        Vec<u8>,
        String,
        String,
        Option<String>,
        Option<Uuid>,
        OffsetDateTime,
    );
    let row: Option<Row> = sqlx::query_as(
        "SELECT key_id, wrapped_key, wrapped_by_key_id, wrapped_by_source,
                rewrapped_from_path, created_by, created_at
           FROM integration_secrets_key WHERE id = 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    row.map(
        |(
            key_id,
            wrapped_key,
            wrapped_by_key_id,
            source,
            rewrapped_from_path,
            created_by,
            created_at,
        )| {
            Ok(StoredSecretsKey {
                key_id,
                wrapped_key,
                wrapped_by_key_id,
                wrapped_by: KeyWrapper::from_wire(&source)?,
                rewrapped_from_path,
                created_by,
                created_at,
            })
        },
    )
    .transpose()
}

async fn read_slack_app(tx: &mut Tx<'_>) -> Result<Option<SavedSlackApp>, sqlx::Error> {
    type Row = (String, Vec<u8>, Vec<u8>, Option<Uuid>, OffsetDateTime);
    let row: Option<Row> = sqlx::query_as(
        "SELECT client_id, sealed_client_secret, sealed_signing_secret, saved_by, saved_at
           FROM instance_slack_app WHERE id = 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(
        |(client_id, sealed_client_secret, sealed_signing_secret, saved_by, saved_at)| {
            SavedSlackApp {
                client_id,
                sealed_client_secret,
                sealed_signing_secret,
                saved_by,
                saved_at,
            }
        },
    ))
}

/// An activity line's count: the number of sealed values, far below `i32`.
fn count_of(n: usize) -> i32 {
    i32::try_from(n).unwrap_or(i32::MAX)
}
