//! `foundry_services::workspaces` — workspace mutations for the instance-admin
//! surface (`instance-admin-workspace-rename`, DDD-1/9).
//!
//! The ordered behaviour (the order pins the observable 422 precedence):
//! `is_instance_admin` (defence-in-depth, the `provision_workspace` idiom) →
//! non-locking current-name pre-read → trim → byte-equal no-op → the ONE
//! workspace-name rule ([`WorkspaceName::try_new`], instance-workspace-name-rule
//! DDD-4) → the atomic audited write. The store's locked re-read is
//! AUTHORITATIVE for the no-op (DDD-3); the pre-read exists only so an untouched
//! legacy name resubmitted from the pre-filled form is a quiet success, not a
//! 422 — even one the rule would now refuse (D6).
//! No uniqueness arm: two workspaces may share a name (D7).

use foundry_core::{WorkspaceName, WorkspaceNameError};
use foundry_store::{Store, WorkspaceRenameWrite};

/// The workspace-name cap, re-exported from its one home in `foundry-core` so
/// the shipped `foundry_services::workspaces::WORKSPACE_NAME_MAX_CHARS` path
/// stays valid.
pub use foundry_core::WORKSPACE_NAME_MAX_CHARS;

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

/// Typed refusals. The copy of an [`InvalidName`](Self::InvalidName) refusal is
/// its [`WorkspaceNameError`]'s `Display` (D3); the handler owns only the
/// uniform 404 mapping.
#[derive(Debug)]
pub enum RenameWorkspaceError {
    /// Actor is not an instance admin → uniform 404.
    Forbidden,
    /// Unknown workspace id (or lost race with a delete) → uniform 404.
    NotFound,
    /// The trimmed name breaks the workspace-name rule → 422.
    InvalidName(WorkspaceNameError),
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

/// The pure ordered-check contract (DDD-4): trim → no-op (byte-equal to
/// `current_name`, so an untouched legacy name is never refused, D6) →
/// [`WorkspaceName::try_new`]. A case-only change is NOT byte-equal, so it is a
/// real rename.
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
    let name = WorkspaceName::try_new(trimmed).map_err(RenameWorkspaceError::InvalidName)?;
    Ok(WorkspaceNameDecision::Write {
        name: name.as_str().to_string(),
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
/// Universe per case: the single decision value. Budget: 5 behaviours × 2 = 10
/// max; 5 properties + 3 exact-boundary examples.
#[cfg(test)]
mod classify_workspace_rename_properties {
    use super::*;
    use proptest::prelude::*;

    /// A current name no generated input can byte-equal (contains a char the
    /// strategies never emit).
    const CURRENT: &str = "current~name";

    /// A sample of the D4 refused set: Cc (incl. TAB, NUL, DEL, NEL), a bidi
    /// override, a bidi isolate, and the line/paragraph separators.
    fn refused_char() -> impl Strategy<Value = char> {
        prop::sample::select(vec![
            '\t', '\0', '\u{7F}', '\u{85}', '\u{1B}', '\u{202A}', '\u{202E}', '\u{2066}',
            '\u{2069}', '\u{2028}', '\u{2029}',
        ])
    }

    fn is_invalid(
        decision: &Result<WorkspaceNameDecision, RenameWorkspaceError>,
        expected: WorkspaceNameError,
    ) -> bool {
        matches!(decision, Err(RenameWorkspaceError::InvalidName(e)) if *e == expected)
    }

    proptest! {
        /// Behaviour 1 — input that trims to nothing is refused as Empty.
        #[test]
        fn whitespace_only_input_is_empty(raw in "[ \t\r\n]{0,8}") {
            let decision = classify_workspace_rename(&raw, CURRENT);
            prop_assert!(
                is_invalid(&decision, WorkspaceNameError::Empty),
                "{raw:?} trims to empty and must be refused as Empty, got {decision:?}"
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
                    is_invalid(&decision, WorkspaceNameError::TooLong),
                    "{scalars} scalars must be refused as TooLong, got {decision:?}"
                );
            }
        }

        /// Behaviour 3 — an interior refused character is ControlCharacter at
        /// any length: the control arm precedes the length arm (named fault 6),
        /// and edge whitespace is trimmed before it is looked at (fault 4).
        #[test]
        fn interior_refused_char_is_control_at_any_length(
            head in "[A-Za-z]{1,30}",
            refused in refused_char(),
            tail in "[A-Za-z]{1,30}",
            pad in "[ \t\r\n]{0,3}",
        ) {
            let raw = format!("{pad}{head}{refused}{tail}{pad}");
            let decision = classify_workspace_rename(&raw, CURRENT);
            prop_assert!(
                is_invalid(&decision, WorkspaceNameError::ControlCharacter),
                "{raw:?} must be refused as ControlCharacter, got {decision:?}"
            );
        }

        /// Behaviour 4 — precedence: a trimmed input byte-equal to the current
        /// name is a quiet NoOp before every gate — past the length cap, and for
        /// a legacy name holding a refused character (D6, named fault 7).
        #[test]
        fn byte_equal_current_is_noop_before_every_gate(
            head in "[A-Za-z][A-Za-z0-9 ]{0,40}",
            refused in prop::option::of(refused_char()),
            tail in "[A-Za-z0-9 ]{0,40}[A-Za-z0-9]",
            pad in "[ \t]{0,3}",
        ) {
            let current = match refused {
                Some(c) => format!("{head}{c}{tail}"),
                None => format!("{head}{tail}"),
            };
            let raw = format!("{pad}{current}{pad}");
            prop_assert_eq!(
                classify_workspace_rename(&raw, &current).ok(),
                Some(WorkspaceNameDecision::NoOp { name: current.clone() })
            );
        }

        /// Behaviour 5 — a case-only change is a real rename (byte-equality,
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
        assert!(is_invalid(
            &classify_workspace_rename(&"a".repeat(25), CURRENT),
            WorkspaceNameError::TooLong
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
        assert!(is_invalid(
            &classify_workspace_rename(over_cap, CURRENT),
            WorkspaceNameError::TooLong
        ));
    }

    /// Legacy pin (D6): the untouched legacy tab name is a NoOp; one edited
    /// character away it is refused for the tab.
    #[test]
    fn legacy_tab_name_untouched_is_noop_edited_is_refused() {
        let legacy = "Canzan\tLabs";
        assert_eq!(
            classify_workspace_rename(legacy, legacy).ok(),
            Some(WorkspaceNameDecision::NoOp {
                name: legacy.to_string()
            })
        );
        assert!(is_invalid(
            &classify_workspace_rename("Canzan\tLabz", legacy),
            WorkspaceNameError::ControlCharacter
        ));
    }
}
