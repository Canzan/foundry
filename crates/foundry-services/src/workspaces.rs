//! `foundry_services::workspaces` — workspace mutations for the instance-admin
//! surface (`instance-admin-workspace-rename`, DDD-1/9).
//!
//! The ordered behaviour (the order pins the observable 422 precedence):
//! `is_instance_admin` (defence-in-depth, the `provision_workspace` idiom) →
//! non-locking current-name pre-read → trim → no-op → empty → length → the
//! atomic audited write. The store's locked re-read is AUTHORITATIVE for the
//! no-op (DDD-3); the pre-read exists only so an untouched legacy name
//! resubmitted from the pre-filled form is a quiet success, not a 422.
//! No uniqueness arm: two workspaces may share a name (D7).

use foundry_store::{Store, WorkspaceRenameWrite};

/// The workspace-name cap in Unicode scalars (`chars().count()`, D3). `pub` so
/// the provisioning, bootstrap, and CLI paths can reuse the one rule (DDD-9).
pub const WORKSPACE_NAME_MAX_CHARS: usize = 24;

/// The rename request as the driving adapter hands it over.
pub struct RenameWorkspaceRequest<'a> {
    /// Session-resolved actor — re-gated by `is_instance_admin` inside.
    pub acting_user_id: uuid::Uuid,
    pub workspace_id: uuid::Uuid,
    /// Raw form input; trimmed inside the use-case.
    pub new_name: &'a str,
}

/// The two quiet outcomes of a rename; each carries the name the 200 head
/// fragment renders (DDD-8).
#[derive(Debug, PartialEq, Eq)]
pub enum WorkspaceRenameOutcome {
    /// Name persisted with exactly one rename record.
    Renamed { name: String },
    /// Trimmed input byte-equal to the current name — nothing written (D4).
    NoOp { name: String },
}

/// Typed refusals; the HANDLER owns the copy (D3) and the uniform 404 mapping.
#[derive(Debug)]
pub enum RenameWorkspaceError {
    /// Actor is not an instance admin → uniform 404.
    Forbidden,
    /// Unknown workspace id (or lost race with a delete) → uniform 404.
    NotFound,
    /// Trimmed name empty → 422.
    EmptyName,
    /// More than [`WORKSPACE_NAME_MAX_CHARS`] Unicode scalars → 422.
    NameTooLong,
    Store(foundry_store::StoreError),
}

/// What the pure classification decides once the current name is in hand.
#[derive(Debug, PartialEq, Eq)]
pub enum WorkspaceNameDecision {
    /// Trimmed input byte-equal to the current name — write nothing.
    NoOp { name: String },
    /// Persist this trimmed name.
    Write { name: String },
}

/// The pure ordered-check contract (DDD-9): trim → no-op (byte-equal to
/// `current_name`) → empty → over [`WORKSPACE_NAME_MAX_CHARS`] scalars. A
/// case-only change is NOT byte-equal, so it is a real rename.
pub fn classify_workspace_rename(
    raw_new_name: &str,
    current_name: &str,
) -> Result<WorkspaceNameDecision, RenameWorkspaceError> {
    let trimmed = raw_new_name.trim();
    if trimmed == current_name {
        return Ok(WorkspaceNameDecision::NoOp {
            name: trimmed.to_string(),
        });
    }
    if trimmed.is_empty() {
        return Err(RenameWorkspaceError::EmptyName);
    }
    if trimmed.chars().count() > WORKSPACE_NAME_MAX_CHARS {
        return Err(RenameWorkspaceError::NameTooLong);
    }
    Ok(WorkspaceNameDecision::Write {
        name: trimmed.to_string(),
    })
}

/// Rename a workspace's DISPLAY NAME only (D8: id, memberships and URLs are
/// untouched), recording who, from what, to what, and when in the same
/// transaction (DDD-2).
pub async fn rename_workspace(
    store: &Store,
    request: RenameWorkspaceRequest<'_>,
) -> Result<WorkspaceRenameOutcome, RenameWorkspaceError> {
    let is_admin = store
        .is_instance_admin(request.acting_user_id)
        .await
        .map_err(RenameWorkspaceError::Store)?;
    if !is_admin {
        return Err(RenameWorkspaceError::Forbidden);
    }
    let current_name = store
        .workspace_name(request.workspace_id)
        .await
        .map_err(RenameWorkspaceError::Store)?
        .ok_or(RenameWorkspaceError::NotFound)?;
    let name = match classify_workspace_rename(request.new_name, &current_name)? {
        WorkspaceNameDecision::NoOp { name } => return Ok(WorkspaceRenameOutcome::NoOp { name }),
        WorkspaceNameDecision::Write { name } => name,
    };
    let write = store
        .rename_workspace_with_audit(request.workspace_id, request.acting_user_id, &name)
        .await
        .map_err(RenameWorkspaceError::Store)?;
    match write {
        WorkspaceRenameWrite::Renamed { .. } => Ok(WorkspaceRenameOutcome::Renamed { name }),
        // The locked read is authoritative (DDD-3): a concurrent rename landed
        // this exact name first, so nothing was written.
        WorkspaceRenameWrite::Unchanged => Ok(WorkspaceRenameOutcome::NoOp { name }),
        WorkspaceRenameWrite::NotFound => Err(RenameWorkspaceError::NotFound),
    }
}

impl crate::Services {
    /// Delegates to [`rename_workspace`] (the provisioning idiom).
    pub async fn rename_workspace(
        &self,
        request: RenameWorkspaceRequest<'_>,
    ) -> Result<WorkspaceRenameOutcome, RenameWorkspaceError> {
        rename_workspace(&self.store, request).await
    }
}

/// Property tests over the PURE classification (its inputs are exactly what
/// the pre-read hands over, so no store double is needed at this seam).
/// Universe per case: the single decision value. Budget: 4 behaviours × 2 = 8
/// max; 4 properties + 2 exact-boundary examples.
#[cfg(test)]
mod classify_workspace_rename_properties {
    use super::*;
    use proptest::prelude::*;

    /// A current name no generated input can byte-equal (contains a char the
    /// strategies never emit).
    const CURRENT: &str = "current~name";

    proptest! {
        /// Behaviour 1 — input that trims to nothing is EmptyName.
        #[test]
        fn whitespace_only_input_is_empty(raw in "[ \t\r\n]{0,8}") {
            prop_assert!(
                matches!(
                    classify_workspace_rename(&raw, CURRENT),
                    Err(RenameWorkspaceError::EmptyName)
                ),
                "{raw:?} trims to empty and must classify EmptyName"
            );
        }

        /// Behaviour 2 — the length gate sits at 24 Unicode scalars of the
        /// TRIMMED name (3-byte scalars prove chars-not-bytes; padding proves
        /// trim-before-count).
        #[test]
        fn length_gate_counts_24_trimmed_scalars(
            scalars in 1usize..60,
            pad in "[ \t]{0,3}",
        ) {
            let name: String = "\u{65E5}".repeat(scalars);
            let raw = format!("{pad}{name}{pad}");
            let decision = classify_workspace_rename(&raw, CURRENT);
            if scalars <= WORKSPACE_NAME_MAX_CHARS {
                prop_assert_eq!(decision.ok(), Some(WorkspaceNameDecision::Write { name }));
            } else {
                prop_assert!(
                    matches!(decision, Err(RenameWorkspaceError::NameTooLong)),
                    "{scalars} scalars must classify NameTooLong"
                );
            }
        }

        /// Behaviour 3 — precedence: a trimmed input byte-equal to the current
        /// name is a quiet NoOp even past the length gate (no-op precedes it).
        #[test]
        fn byte_equal_current_is_noop_before_every_gate(
            current in "[A-Za-z][A-Za-z0-9 ]{0,80}[A-Za-z0-9]",
            pad in "[ \t]{0,3}",
        ) {
            let raw = format!("{pad}{current}{pad}");
            prop_assert_eq!(
                classify_workspace_rename(&raw, &current).ok(),
                Some(WorkspaceNameDecision::NoOp { name: current.clone() })
            );
        }

        /// Behaviour 4 — a case-only change is a real rename (byte-equality,
        /// not case-insensitive equality), persisting the trimmed input.
        #[test]
        fn case_only_change_is_written(
            current in "[a-z][a-z ]{0,20}[a-z]",
            pad in "[ \t]{0,3}",
        ) {
            let recased = current.to_uppercase();
            let raw = format!("{pad}{recased}{pad}");
            prop_assert_eq!(
                classify_workspace_rename(&raw, &current).ok(),
                Some(WorkspaceNameDecision::Write { name: recased })
            );
        }
    }

    /// Exact-boundary pin: 24 scalars is the last accepted length, 25 the
    /// first refusal (a proptest range need not sample 24 itself).
    #[test]
    fn exactly_24_accepted_25_refused() {
        let at_cap = "a".repeat(24);
        assert_eq!(
            classify_workspace_rename(&at_cap, CURRENT).ok(),
            Some(WorkspaceNameDecision::Write { name: at_cap }),
            "24 scalars sit AT the cap and must be written verbatim"
        );
        assert!(matches!(
            classify_workspace_rename(&"a".repeat(25), CURRENT),
            Err(RenameWorkspaceError::NameTooLong)
        ));
    }

    /// Multi-byte pin: 24 scalars in 28 bytes is accepted (chars, not bytes);
    /// one more scalar is refused.
    #[test]
    fn multibyte_24_scalars_in_28_bytes_accepted_25_refused() {
        let at_cap = "Ångström Øresund Societé";
        assert_eq!((at_cap.chars().count(), at_cap.len()), (24, 28));
        assert_eq!(
            classify_workspace_rename(at_cap, CURRENT).ok(),
            Some(WorkspaceNameDecision::Write {
                name: at_cap.to_string()
            })
        );
        let over_cap = "Ångström Øresund Societés";
        assert_eq!(over_cap.chars().count(), 25);
        assert!(matches!(
            classify_workspace_rename(over_cap, CURRENT),
            Err(RenameWorkspaceError::NameTooLong)
        ));
    }
}
