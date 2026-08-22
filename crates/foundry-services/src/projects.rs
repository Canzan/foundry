//! `foundry_services::projects` — project mutations for the instance-admin
//! surface (`instance-admin-project-rename`, ADR-PROJECT-RENAME-002).
//!
//! Contract signatures DESIGN fixed in
//! `docs/feature/instance-admin-project-rename/design/component-boundaries.md`.
//! The ordered behaviour (the order pins the observable 422 precedence):
//! `is_instance_admin` (defence-in-depth, the `provision_workspace` idiom) →
//! context fetch → trim → no-op → empty → length → duplicate → update.
//! Check-then-write; the TOCTOU window is accepted and bounded
//! (data-models.md §4).
//!
//! Duplicate rule (D4): with `t` = trimmed new name, refuse when any sibling
//! `(name, slug)` satisfies `t.to_lowercase() == name.to_lowercase()` OR
//! `foundry_core::slugify(t) == slug`. Self is excluded by the query, so a
//! case-only rename of a project onto itself is a VALID rename, and an
//! exact-match self rename is the earlier NoOp.

use foundry_store::Store;

/// The rename request as the driving adapter hands it over.
pub struct RenameProjectRequest<'a> {
    /// Session-resolved actor — re-gated by `is_instance_admin` inside
    /// (defence-in-depth, mirrors `provisioning::provision_workspace`).
    pub acting_user_id: uuid::Uuid,
    pub project_id: uuid::Uuid,
    /// Raw form input; trimmed inside the use-case.
    pub new_name: &'a str,
}

/// The two quiet outcomes of a rename.
pub enum RenameOutcome {
    /// Name persisted. Carries the trimmed stored name for the fragment.
    Renamed { name: String },
    /// Trimmed input byte-equal to the current name — nothing written (D4).
    NoOp { name: String },
}

/// Typed refusals; the HANDLER owns the mapping to user-facing copy (the three
/// exact 422 messages) and to the uniform non-enumerable 404 (D5/D6).
pub enum RenameProjectError {
    /// Actor is not an instance admin → handler renders uniform 404.
    Forbidden,
    /// Unknown project id (or lost race with a delete) → uniform 404.
    NotFound,
    /// Trimmed name empty → 422 "Project name must not be empty".
    EmptyName,
    /// More than 256 chars (Unicode scalar count, mirroring the `issues.title`
    /// CHECK semantics) → 422 "Project name must be at most 256 characters".
    NameTooLong,
    /// Case-insensitive name match OR `slugify(new)` == sibling stored slug,
    /// self excluded → 422 "Project name must be unique within the team".
    DuplicateName,
    Store(foundry_store::StoreError),
}

/// What the pure D4 classification decides once the store reads are in hand.
enum RenameDecision {
    /// Trimmed input byte-equal to the current name — write nothing.
    NoOp { name: String },
    /// Persist this trimmed name.
    Write { name: String },
}

/// The pure, store-free heart of the ordered-check contract: given the raw
/// input plus everything the store reads returned — the current name and the
/// team's OTHER projects' `(name, slug)` pairs (self excluded upstream by the
/// query) — decide trim → no-op → empty → length (256 Unicode scalars) →
/// duplicate. The order pins the observable 422 precedence.
fn classify_rename(
    raw_new_name: &str,
    current_name: &str,
    siblings: &[(String, String)],
) -> Result<RenameDecision, RenameProjectError> {
    let trimmed = raw_new_name.trim();
    if trimmed == current_name {
        return Ok(RenameDecision::NoOp {
            name: trimmed.to_string(),
        });
    }
    if trimmed.is_empty() {
        return Err(RenameProjectError::EmptyName);
    }
    if trimmed.chars().count() > MAX_NAME_SCALARS {
        return Err(RenameProjectError::NameTooLong);
    }
    if collides_with_sibling(trimmed, siblings) {
        return Err(RenameProjectError::DuplicateName);
    }
    Ok(RenameDecision::Write {
        name: trimmed.to_string(),
    })
}

/// Unicode-scalar cap on a project name — `trimmed.chars().count()`, mirroring
/// the `issues.title` CHECK semantics (data-models.md §2).
const MAX_NAME_SCALARS: usize = 256;

/// D4 duplicate arm: case-insensitive match against a sibling NAME, or the
/// derived slug colliding with a sibling's STORED slug.
fn collides_with_sibling(trimmed: &str, siblings: &[(String, String)]) -> bool {
    let lowered = trimmed.to_lowercase();
    let derived_slug = foundry_core::slugify(trimmed);
    siblings
        .iter()
        .any(|(name, slug)| name.to_lowercase() == lowered || *slug == derived_slug)
}

/// Rename a project's DISPLAY NAME only (D1: `slug`, `key_prefix`, and every
/// issue key are untouched — a rename must never move a URL,
/// ADR-PROJECT-RENAME-001).
pub async fn rename_project(
    store: &Store,
    request: RenameProjectRequest<'_>,
) -> Result<RenameOutcome, RenameProjectError> {
    let is_admin = store
        .is_instance_admin(request.acting_user_id)
        .await
        .map_err(RenameProjectError::Store)?;
    if !is_admin {
        return Err(RenameProjectError::Forbidden);
    }
    let context = store
        .project_rename_context(request.project_id)
        .await
        .map_err(RenameProjectError::Store)?
        .ok_or(RenameProjectError::NotFound)?;
    let siblings = store
        .list_team_sibling_projects(context.team_id, request.project_id)
        .await
        .map_err(RenameProjectError::Store)?;
    match classify_rename(request.new_name, &context.current_name, &siblings)? {
        RenameDecision::NoOp { name } => Ok(RenameOutcome::NoOp { name }),
        RenameDecision::Write { name } => {
            let rows_affected = store
                .update_project_name(request.project_id, &name)
                .await
                .map_err(RenameProjectError::Store)?;
            if rows_affected == 0 {
                // Lost a race with a delete — the same non-enumerable refusal.
                return Err(RenameProjectError::NotFound);
            }
            Ok(RenameOutcome::Renamed { name })
        }
    }
}

impl crate::Services {
    /// Delegates to [`rename_project`] (the provisioning idiom).
    pub async fn rename_project(
        &self,
        request: RenameProjectRequest<'_>,
    ) -> Result<RenameOutcome, RenameProjectError> {
        rename_project(&self.store, request).await
    }
}

/// Property tests over the PURE classification (the domain function IS its own
/// driving port — its inputs are exactly what the store reads hand over, so no
/// store double is needed at this seam). Universe per case: the single decision
/// value (`NoOp`/`Write{name}`/typed error) — the full observable surface of a
/// pure function. Test budget: 5 distinct behaviours × 2 = 10 max; 5 written.
#[cfg(test)]
mod classify_rename_properties {
    use super::*;
    use proptest::prelude::*;

    /// A display-name-ish token with no leading/trailing whitespace — the
    /// shape the create path accepts (starts/ends alphanumeric).
    fn sibling_name() -> impl Strategy<Value = String> {
        "[A-Za-z][A-Za-z0-9 ]{0,20}[A-Za-z0-9]"
    }

    /// Sibling rows as production mints them: `(name, slugify(name))`.
    fn siblings() -> impl Strategy<Value = Vec<(String, String)>> {
        proptest::collection::vec(
            sibling_name().prop_map(|n| {
                let slug = foundry_core::slugify(&n);
                (n, slug)
            }),
            0..6,
        )
    }

    /// A current name guaranteed disjoint from every generated sibling/new
    /// name (longer than the 22-char strategy ceiling).
    const CURRENT: &str = "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ current";

    /// Flip the case of every alphabetic char — same name under the
    /// case-insensitive rule, different bytes.
    fn flip_case(s: &str) -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else if c.is_ascii_uppercase() {
                    c.to_ascii_lowercase()
                } else {
                    c
                }
            })
            .collect()
    }

    proptest! {
        /// Behaviour 1 — whatever the siblings, input that trims to nothing
        /// is refused as EmptyName.
        #[test]
        fn whitespace_only_input_is_empty(raw in "[ \t\r\n]{0,8}", sibs in siblings()) {
            let decision = classify_rename(&raw, CURRENT, &sibs);
            prop_assert!(
                matches!(decision, Err(RenameProjectError::EmptyName)),
                "{raw:?} trims to empty and must classify EmptyName"
            );
        }

        /// Behaviour 2 — the length gate sits at 256 UNICODE SCALARS of the
        /// TRIMMED name (multi-byte scalars prove chars-not-bytes; padding
        /// proves trim-before-count).
        #[test]
        fn length_gate_counts_256_trimmed_scalars(
            scalars in 1usize..400,
            pad in "[ \t]{0,3}",
        ) {
            let name: String = "\u{65E5}".repeat(scalars); // 3 UTF-8 bytes each
            let raw = format!("{pad}{name}{pad}");
            let decision = classify_rename(&raw, CURRENT, &[]);
            if scalars <= 256 {
                match decision {
                    Ok(RenameDecision::Write { name: written }) => prop_assert_eq!(
                        written, name, "the persisted name must be the trimmed input"
                    ),
                    _ => prop_assert!(false, "{scalars} scalars must be accepted"),
                }
            } else {
                prop_assert!(
                    matches!(decision, Err(RenameProjectError::NameTooLong)),
                    "{scalars} scalars must classify NameTooLong"
                );
            }
        }

        /// Behaviour 3 — ordered-check precedence: a trimmed input byte-equal
        /// to the current name is a quiet NoOp even when a sibling collides
        /// outright and even past the length gate (no-op precedes both).
        #[test]
        fn byte_equal_current_is_noop_before_every_gate(
            current in "[A-Za-z][A-Za-z0-9 ]{0,300}[A-Za-z0-9]",
            pad in "[ \t]{0,3}",
        ) {
            let colliding = vec![(current.clone(), foundry_core::slugify(&current))];
            let raw = format!("{pad}{current}{pad}");
            let decision = classify_rename(&raw, &current, &colliding);
            match decision {
                Ok(RenameDecision::NoOp { name }) => prop_assert_eq!(
                    name, current, "NoOp must carry the current name"
                ),
                _ => prop_assert!(false, "byte-equal input must be a NoOp, never a refusal"),
            }
        }

        /// Behaviour 4 — the D4 duplicate rule over arbitrary sibling sets:
        /// a case-mangled sibling NAME and a punctuation-mangled name whose
        /// SLUG collides with a sibling's stored slug are both refused.
        #[test]
        fn sibling_name_or_slug_collision_is_duplicate(
            sibs in siblings(),
            pick in any::<proptest::sample::Index>(),
        ) {
            prop_assume!(!sibs.is_empty());
            let (target_name, _) = &sibs[pick.index(sibs.len())];
            // Case arm: same letters, different case.
            let case_mangled = flip_case(target_name);
            prop_assert!(
                matches!(
                    classify_rename(&case_mangled, CURRENT, &sibs),
                    Err(RenameProjectError::DuplicateName)
                ),
                "{case_mangled:?} case-matches sibling {target_name:?} and must be refused"
            );
            // Slug arm: different name, colliding derived slug ('!' slugs away).
            let slug_mangled = format!("{target_name}!");
            prop_assert!(
                matches!(
                    classify_rename(&slug_mangled, CURRENT, &sibs),
                    Err(RenameProjectError::DuplicateName)
                ),
                "{slug_mangled:?} slug-collides with sibling {target_name:?} and must be refused"
            );
        }

        /// Behaviour 5 — non-colliding names pass, including the case-only
        /// self rename (self is NOT in the sibling set — the query excludes
        /// it), and the persisted name is the trimmed input.
        #[test]
        fn fresh_and_case_only_self_names_are_written(
            current_base in sibling_name(),
            sib_bases in proptest::collection::vec(sibling_name(), 0..5),
            pad in "[ \t]{0,3}",
        ) {
            // Disjoint namespaces: current "cur …", siblings "sib …" — no
            // cross name/slug collision is possible by construction.
            let current = format!("cur {current_base}");
            let sibs: Vec<(String, String)> = sib_bases
                .iter()
                .map(|b| {
                    let n = format!("sib {b}");
                    let slug = foundry_core::slugify(&n);
                    (n, slug)
                })
                .collect();
            let recased = flip_case(&current);
            prop_assume!(recased != current);
            let raw = format!("{pad}{recased}{pad}");
            match classify_rename(&raw, &current, &sibs) {
                Ok(RenameDecision::Write { name }) => prop_assert_eq!(
                    name, recased,
                    "a case-only self rename is VALID and persists the trimmed input"
                ),
                _ => prop_assert!(false, "a non-colliding name must be written"),
            }
        }
    }
}
