//! `foundry_services::projects` — project mutations: the instance-admin rename
//! (`instance-admin-project-rename`, ADR-PROJECT-RENAME-002) and the team
//! member's create (`project-name-rule` DDD-5/6/7, below the rename).
//!
//! Rename — contract signatures DESIGN fixed in
//! `docs/feature/instance-admin-project-rename/design/component-boundaries.md`.
//! The ordered behaviour (the order pins the observable 422 precedence,
//! project-name-rule D5/D8/DDD-8): `is_instance_admin` (defence-in-depth, the
//! `provision_workspace` idiom) → context fetch → sibling read → trim → no-op →
//! the one project-name rule (`foundry_core::ProjectName`: empty → control →
//! length → unique within the team) → update. Check-then-write; the TOCTOU
//! window is accepted and bounded (data-models.md §4).
//!
//! Self-exclusion is this caller's job: the sibling read excludes the project's
//! own id, so a case-only rename of a project onto itself is a VALID rename, and
//! an exact-match self rename is the earlier NoOp.

use foundry_core::{
    mint_project_slug, MintedSlug, ProjectKey, ProjectKeyError, ProjectName, ProjectNameError,
};
use foundry_store::{ProjectInsertError, Store};

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

/// Typed refusals. The handler maps `Forbidden`/`NotFound` to the uniform
/// non-enumerable 404 (D5/D6) and renders `InvalidName`'s `Display` — the copy's
/// one home is `ProjectNameError` in foundry-core (DDD-4/DDD-7).
pub enum RenameProjectError {
    /// Actor is not an instance admin → handler renders uniform 404.
    Forbidden,
    /// Unknown project id (or lost race with a delete) → uniform 404.
    NotFound,
    /// The new name breaks the project-name rule → 422 with its `Display`.
    InvalidName(ProjectNameError),
    Store(foundry_store::StoreError),
}

/// What the pure classification decides once the store reads are in hand.
enum RenameDecision {
    /// Trimmed input byte-equal to the current name — write nothing.
    NoOp { name: String },
    /// Persist this trimmed name.
    Write { name: String },
}

/// The pure, store-free heart of the ordered-check contract: given the raw
/// input plus everything the store reads returned — the current name and the
/// team's OTHER projects' `(name, slug)` pairs (self excluded upstream by the
/// query) — decide trim → no-op → rule → uniqueness. The no-op precedes the
/// rule, so an untouched legacy name is never refused (D8).
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
    let name = ProjectName::try_new(trimmed).map_err(RenameProjectError::InvalidName)?;
    name.ensure_unique_among(siblings)
        .map_err(RenameProjectError::InvalidName)?;
    Ok(RenameDecision::Write {
        name: name.as_str().to_string(),
    })
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

    /// Delegates to [`create_project`].
    pub async fn create_project(
        &self,
        request: CreateProjectRequest<'_>,
    ) -> Result<CreatedProject, CreateProjectError> {
        create_project(&self.store, request).await
    }
}

/// The create request as the driving adapter hands it over (DDD-6). The name
/// is already a [`ProjectName`]: the door parses it before anything is read.
pub struct CreateProjectRequest<'a> {
    pub workspace_id: uuid::Uuid,
    pub team_id: uuid::Uuid,
    pub name: ProjectName,
    /// Handler-trimmed raw key prefix; parsed here by `ProjectKey::try_new`.
    pub key_prefix: &'a str,
}

/// A created project: its id and the address it was stored under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedProject {
    pub project_id: uuid::Uuid,
    pub slug: MintedSlug,
}

/// Typed refusals of a create (DDD-7). The handler renders `InvalidName`'s and
/// `InvalidKey`'s `Display`; a refusal writes nothing and leaves the key free.
#[derive(Debug)]
pub enum CreateProjectError {
    InvalidName(ProjectNameError),
    InvalidKey(ProjectKeyError),
    DuplicateKey,
    /// The key-prefix fallback address lost every bounded retry (DDD-10).
    FallbackSlugContention,
    Store(foundry_store::StoreError),
}

/// Create a project with its seeded lanes, all or nothing — the one place a
/// project row is inserted (check-arch `project-name-one-source` (b)).
///
/// Order (DDD-6, D5): sibling read → [`ProjectName::ensure_unique_among`] (the
/// same check rename runs) → `ProjectKey::try_new` → mint → insert. The
/// uniqueness check precedes the key so a taken name is reported before a key
/// problem. The fresh id is passed as the excluded sibling, so it excludes
/// nothing.
pub async fn create_project(
    store: &Store,
    request: CreateProjectRequest<'_>,
) -> Result<CreatedProject, CreateProjectError> {
    create_with_retry(store, request, Vec::new()).await
}

/// How many create attempts a key-prefix fallback address gets before the
/// create gives up with [`CreateProjectError::FallbackSlugContention`]
/// (ADR-PROJECT-NAME-002 §3). Counts the first attempt.
pub const FALLBACK_SLUG_ATTEMPTS: usize = 3;

/// [`create_project`] with the sibling read scriptable (DESIGN OQ-D3): attempt
/// `i` checks against `scripted[i]` instead of reading the store while a
/// scripted list remains, so a test can hand the check a STALE view and make
/// the unique index fire on insert. A test seam: compiled only for tests and
/// the `test-support` feature, never into a production build.
#[cfg(any(test, feature = "test-support"))]
pub async fn create_project_with_sibling_reads(
    store: &Store,
    request: CreateProjectRequest<'_>,
    scripted: Vec<Vec<(String, String)>>,
) -> Result<CreatedProject, CreateProjectError> {
    create_with_retry(store, request, scripted).await
}

/// Each attempt reads the siblings, runs the check, parses the key, mints from
/// the same read and inserts in its own transaction. Only a `KeyFallback`
/// address losing on the slug index is retried (DDD-10); a `Derived` one keeps
/// today's `NotUnique`.
async fn create_with_retry(
    store: &Store,
    request: CreateProjectRequest<'_>,
    scripted: Vec<Vec<(String, String)>>,
) -> Result<CreatedProject, CreateProjectError> {
    let mut scripted = scripted.into_iter();
    for _ in 0..FALLBACK_SLUG_ATTEMPTS {
        match attempt_create(store, &request, scripted.next()).await? {
            Attempt::Created(created) => return Ok(created),
            Attempt::FallbackTaken => continue,
        }
    }
    Err(CreateProjectError::FallbackSlugContention)
}

/// One attempt's outcome when it did not refuse.
enum Attempt {
    Created(CreatedProject),
    /// The minted `KeyFallback` address was taken between the read and the insert.
    FallbackTaken,
}

async fn attempt_create(
    store: &Store,
    request: &CreateProjectRequest<'_>,
    scripted: Option<Vec<(String, String)>>,
) -> Result<Attempt, CreateProjectError> {
    let project_id = uuid::Uuid::now_v7();
    let siblings = match scripted {
        Some(siblings) => siblings,
        None => store
            .list_team_sibling_projects(request.team_id, project_id)
            .await
            .map_err(CreateProjectError::Store)?,
    };
    request
        .name
        .ensure_unique_among(&siblings)
        .map_err(CreateProjectError::InvalidName)?;
    let key = ProjectKey::try_new(request.key_prefix).map_err(CreateProjectError::InvalidKey)?;
    let team_slugs: Vec<String> = siblings.into_iter().map(|(_, slug)| slug).collect();
    let minted = mint_project_slug(&request.name, &key, &team_slugs);
    let inserted = store
        .insert_project(
            project_id,
            request.workspace_id,
            request.team_id,
            request.name.as_str(),
            minted.as_str(),
            key.as_str(),
        )
        .await;
    match (inserted, &minted) {
        (Ok(()), _) => Ok(Attempt::Created(CreatedProject {
            project_id,
            slug: minted,
        })),
        (Err(ProjectInsertError::DuplicateSlug), MintedSlug::KeyFallback(_)) => {
            Ok(Attempt::FallbackTaken)
        }
        (Err(ProjectInsertError::DuplicateSlug), MintedSlug::Derived(_)) => {
            Err(CreateProjectError::InvalidName(ProjectNameError::NotUnique))
        }
        (Err(ProjectInsertError::DuplicateKey), _) => Err(CreateProjectError::DuplicateKey),
        (Err(ProjectInsertError::Other(err)), _) => Err(CreateProjectError::Store(err)),
    }
}

/// Property tests over the PURE classification (the domain function IS its own
/// driving port — its inputs are exactly what the store reads hand over, so no
/// store double is needed at this seam). Universe per case: the single decision
/// value (`NoOp`/`Write{name}`/typed error) — the full observable surface of a
/// pure function. Test budget: 8 distinct behaviours × 2 = 16 max; 8 written.
#[cfg(test)]
mod classify_rename_properties {
    use super::*;
    use foundry_core::ProjectNameError;
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

    /// One character of the refused set (D3): Unicode Cc (TAB, NUL, DEL, …),
    /// the bidi controls, and the line/paragraph separators.
    fn refused_char() -> impl Strategy<Value = char> {
        prop_oneof![
            Just('\t'),
            Just('\u{0}'),
            Just('\u{7F}'),
            Just('\u{1B}'),
            Just('\u{202A}'),
            Just('\u{202E}'),
            Just('\u{2066}'),
            Just('\u{2069}'),
            Just('\u{2028}'),
            Just('\u{2029}'),
        ]
    }

    /// A current name guaranteed disjoint from every generated sibling/new
    /// name (longer than the 22-char strategy ceiling).
    const CURRENT: &str = "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ current";

    fn is_invalid(
        decision: &Result<RenameDecision, RenameProjectError>,
        expected: ProjectNameError,
    ) -> bool {
        matches!(decision, Err(RenameProjectError::InvalidName(e)) if *e == expected)
    }

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
        /// is refused as InvalidName(Empty).
        #[test]
        fn whitespace_only_input_is_empty(raw in "[ \t\r\n]{0,8}", sibs in siblings()) {
            let decision = classify_rename(&raw, CURRENT, &sibs);
            prop_assert!(
                is_invalid(&decision, ProjectNameError::Empty),
                "{raw:?} trims to empty and must classify InvalidName(Empty)"
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
                    is_invalid(&decision, ProjectNameError::TooLong),
                    "{scalars} scalars must classify InvalidName(TooLong)"
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
        /// a case-mangled sibling NAME, and a punctuation-mangled name whose
        /// derived SLUG collides with a sibling's STORED slug, are both
        /// refused. Punctuation in ANY position — leading, trailing, and
        /// replacing every space — with the letter case flipped on top.
        /// `slugify` collapses each non-alphanumeric run to one '-' and
        /// strips the ends, so every such mangle derives the sibling's stored
        /// slug byte-for-byte, while the punctuation guarantees the NAME arm
        /// cannot be the one firing (sibling names carry none).
        #[test]
        fn sibling_name_or_slug_collision_is_not_unique(
            sibs in siblings(),
            pick in any::<proptest::sample::Index>(),
            lead in "[!?.,;:*#@&+=]{0,2}",
            sep in "[!?.,;:*#@&+=]{1,3}",
            trail in "[!?.,;:*#@&+=]{1,2}",
        ) {
            prop_assume!(!sibs.is_empty());
            let (target_name, _) = &sibs[pick.index(sibs.len())];
            let case_mangled = flip_case(target_name);
            prop_assert!(
                is_invalid(
                    &classify_rename(&case_mangled, CURRENT, &sibs),
                    ProjectNameError::NotUnique
                ),
                "{case_mangled:?} case-matches sibling {target_name:?} and must be refused"
            );
            let slug_mangled = format!(
                "{lead}{}{trail}",
                flip_case(target_name).replace(' ', &sep)
            );
            prop_assert!(
                is_invalid(
                    &classify_rename(&slug_mangled, CURRENT, &sibs),
                    ProjectNameError::NotUnique
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

        /// Behaviour 6 — a refused character anywhere inside the trimmed name
        /// is InvalidName(ControlCharacter), and control is checked BEFORE
        /// length (the over-long name carrying one still reads "control")
        /// and BEFORE uniqueness (its slug collides with a sibling's).
        #[test]
        fn an_interior_refused_char_is_control_before_length_and_uniqueness(
            left in "[A-Za-z]{1,10}",
            right in "[A-Za-z]{1,10}",
            bad in refused_char(),
            long in any::<bool>(),
        ) {
            let left = if long { "a".repeat(300) } else { left };
            let raw = format!("{left}{bad}{right}");
            // The slug ignores the refused char's run, so the sibling below
            // collides on the slug arm.
            let sibling_name = format!("{left} {right}");
            let sibs = vec![(sibling_name.clone(), foundry_core::slugify(&sibling_name))];
            prop_assert!(
                is_invalid(&classify_rename(&raw, CURRENT, &sibs), ProjectNameError::ControlCharacter),
                "{raw:?} carries a refused char and must classify InvalidName(ControlCharacter)"
            );
        }

        /// Behaviour 7 — a legacy stored name the rule would now refuse (a
        /// refused char, or past 256 scalars) re-submitted byte-equal is a
        /// quiet NoOp (D8: the no-op precedes the rule).
        #[test]
        fn an_untouched_legacy_name_is_noop(
            left in "[A-Za-z]{1,10}",
            right in "[A-Za-z]{1,10}",
            bad in refused_char(),
            scalars in 257usize..400,
            pad in "[ ]{0,3}",
        ) {
            for legacy in [format!("{left}{bad}{right}"), "x".repeat(scalars)] {
                let raw = format!("{pad}{legacy}{pad}");
                match classify_rename(&raw, &legacy, &[]) {
                    Ok(RenameDecision::NoOp { name }) => prop_assert_eq!(
                        name, legacy, "NoOp must carry the stored legacy name"
                    ),
                    _ => prop_assert!(false, "an untouched legacy name must be a NoOp"),
                }
            }
        }

        /// Behaviour 8 — the edited legacy name is NOT a no-op: one byte off a
        /// stored TAB name and the rule applies (ControlCharacter).
        #[test]
        fn an_edited_legacy_control_name_is_refused(
            left in "[A-Za-z]{1,10}",
            right in "[A-Za-z]{1,10}",
            extra in "[A-Za-z]",
        ) {
            let legacy = format!("{left}\t{right}");
            let edited = format!("{legacy}{extra}");
            prop_assert!(
                is_invalid(&classify_rename(&edited, &legacy, &[]), ProjectNameError::ControlCharacter),
                "{edited:?} differs from the stored name and must meet the rule"
            );
        }
    }

    /// Exact-boundary pin for the length gate (behaviour 2's edge): EXACTLY
    /// 256 scalars is the last ACCEPTED length, 257 the first refusal.
    /// # bypass: exact boundary pin — single-example by design
    #[test]
    fn exactly_256_scalars_accepted_257_refused() {
        let at_cap = "a".repeat(256);
        match classify_rename(&at_cap, CURRENT, &[]) {
            Ok(RenameDecision::Write { name }) => assert_eq!(
                name, at_cap,
                "256 scalars sit AT the cap and must be written verbatim"
            ),
            _ => panic!("a 256-scalar name must be accepted (the cap is inclusive)"),
        }
        assert!(
            is_invalid(
                &classify_rename(&"a".repeat(257), CURRENT, &[]),
                ProjectNameError::TooLong
            ),
            "257 scalars must classify InvalidName(TooLong)"
        );
    }
}
