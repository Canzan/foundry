//! Issue write use-cases — the shared seam both the HTML adapter
//! (`foundry-app`) and the JSON adapter (`foundry-api`, 03-02) call.
//!
//! These REUSE the exact core write+outbox path the browser handlers use
//! (`Store::insert_issue_with_outbox`, `Store::reposition_issue_with_outbox`)
//! plus the SAME trimmed/non-empty/≤256 title validation and the lifted
//! `normalize_state` (DD10). An API write and a browser write therefore accept
//! or reject identically and store identical bytes (NFR-WEB-API-CON-02).

use crate::{BoardIssue, CreatedIssue, Principal, ServiceError};
use foundry_store::{IssueInsertError, RepositionOutcome, Store};

/// The current title + description of an issue, resolved for the edit-dialog
/// pre-fill (issue-edit-dialog). Returned by [`edit_issue_form`] AFTER the
/// `resolve_member_project` authz gate, so a foreign issue is refused before
/// any field is exposed.
#[derive(Debug, Clone)]
pub struct IssueEditView {
    pub key: String,
    pub number: i32,
    pub title: String,
    pub description_md: String,
    /// The issue's current state slug — pre-selects the edit-dialog status
    /// control (issue-status-move).
    pub state: String,
    /// The project's lanes in board order — the Status `<select>` options
    /// (board-lane-management D8): the dialog offers exactly the board's own
    /// lanes, never a static list.
    pub lanes: Vec<crate::BoardLane>,
}

/// Title length cap (chars, after trimming) — the SAME bound the browser
/// handler enforces.
const TITLE_MAX_LEN: usize = 256;

/// Description length cap (chars) — matches the DB CHECK
/// `length(description_md) <= 262144` (0001_init.sql). Enforced at the app layer
/// so an over-long description is refused CLEANLY (a validation error) before the
/// write reaches the DB CHECK, which stays as the last line of defense. The rule
/// counts CHARACTERS (`chars().count()`), mirroring the title rule and Postgres
/// `length()`, so a multi-byte description at the bound is accepted.
const DESCRIPTION_MAX_LEN: usize = 262144;

/// Enforce the description length cap (chars, `DESCRIPTION_MAX_LEN`) — the SAME
/// bound both `create_issue` and `edit_issue_details` apply so an over-long
/// description is refused CLEANLY (a validation error) before the write reaches
/// the DB CHECK, which stays as the last line of defense. Counts CHARACTERS
/// (`chars().count()`), mirroring Postgres `length()`, so a multi-byte
/// description at the bound is accepted.
fn validate_description(description: &str) -> Result<(), ServiceError> {
    if description.chars().count() > DESCRIPTION_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "description_too_long".to_string(),
            message: "Description is too long".to_string(),
        });
    }
    Ok(())
}

/// DD10 single seam, PER-PROJECT (board-lane-management 01-02): fold aliases
/// (the `normalize_state` folding survives as a private helper of this seam),
/// then require membership in the PROJECT'S lane set
/// (`Store::list_project_lanes`). Every write path — HTML dialog save, dnd
/// POST, JSON PATCH — calls THIS through [`change_issue_state`]; unknown lane
/// → `ServiceError::Validation { code: "invalid_state" }` (D8). Returns the
/// canonical lane slug actually accepted.
pub async fn validate_project_lane(
    store: &foundry_store::Store,
    project_id: uuid::Uuid,
    input: &str,
) -> Result<String, ServiceError> {
    let lane_slugs: Vec<String> = store
        .list_project_lanes(project_id)
        .await
        .map_err(|_| ServiceError::Internal)?
        .into_iter()
        .map(|lane| lane.slug)
        .collect();
    resolve_lane(input, &lane_slugs).ok_or_else(|| ServiceError::Validation {
        code: "invalid_state".to_string(),
        message: "Invalid issue state".to_string(),
    })
}

/// PURE heart of [`validate_project_lane`] (the `classify_rename` idiom —
/// property-testable without a store): fold aliases (`normalize_state`'s
/// alias-folding role), then require membership in the PROJECT'S lane-slug
/// set. Returns the canonical member slug actually accepted, `None` for an
/// unfoldable input or a folded slug the project's board does not have.
fn resolve_lane(input: &str, lane_slugs: &[String]) -> Option<String> {
    let folded = normalize_state(input)?;
    lane_slugs
        .iter()
        .find(|slug| slug.as_str() == folded)
        .cloned()
}

/// Alias-folding helper of [`validate_project_lane`] (board-lane-management
/// 01-02: DEMOTED from the public validation seam to a private fold — the
/// per-project membership check above is the ONLY validation authority now).
/// Maps the incoming state value (which may be the human label used in feature
/// files like `"in-progress"`) onto the closed, unmintable canonical slug set
/// (D9). Never removed from the alias-folding role.
fn normalize_state(input: &str) -> Option<&'static str> {
    match input.trim().to_ascii_lowercase().as_str() {
        "backlog" => Some("backlog"),
        "todo" => Some("todo"),
        "in-progress" | "in_progress" => Some("in_progress"),
        "done" => Some("done"),
        "cancelled" | "canceled" => Some("cancelled"),
        _ => None,
    }
}

/// US-W05c create-issue use-case. Reuses `Store::insert_issue_with_outbox` and
/// the SAME title validation (trimmed, non-empty, ≤256) the browser enforces.
pub async fn create_issue(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    title: &str,
    description: &str,
) -> Result<CreatedIssue, ServiceError> {
    let (_team, project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let raw_title = title.trim();
    if raw_title.is_empty() || raw_title.chars().count() > TITLE_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "title_required".to_string(),
            message: "Title is required".to_string(),
        });
    }

    validate_description(description)?;

    let issue_id = uuid::Uuid::now_v7();
    let number = match store
        .insert_issue_with_outbox(
            issue_id,
            principal.workspace_id(),
            project.id,
            key_prefix.as_str(),
            principal.user_id(),
            raw_title,
            // Persisted verbatim, matching `edit_issue_details` (the DB CHECK
            // bounds its length); the app-level length bound arrives in slice 03.
            description,
        )
        .await
    {
        Ok(n) => n,
        Err(IssueInsertError::ProjectNotFound) => return Err(ServiceError::NotFound),
        Err(IssueInsertError::Store(_)) => return Err(ServiceError::Internal),
    };

    let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
        .map(|k| k.to_string())
        .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));

    Ok(CreatedIssue {
        key,
        number,
        // The TRIMMED title is what `insert_issue_with_outbox` persisted, so the
        // returned representation matches a subsequent read (NFR-WEB-API-CON-02).
        title: raw_title.to_string(),
        state: "backlog".to_string(),
    })
}

/// US-W05c change-state use-case, extended for card-ranking-within-status
/// (ADR-002): the board drop passes an `after` neighbour issue key (e.g.
/// `"GEN-2"`; `None`/empty ⇒ drop at the top of the column) so the write sets
/// BOTH state and rank atomically through `Store::reposition_issue_with_outbox`.
/// The JSON API + no-JS status paths pass `after = None`. Reuses the SAME
/// `resolve_member_project` authz + `normalize_state` normalisation the UI uses
/// (DD10). A foreign/missing issue OR an unresolvable `after` neighbour resolves
/// to the uniform non-enumerable NotFound (ADR-003), never a 500.
pub async fn change_issue_state(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
    new_state: &str,
    after: Option<&str>,
) -> Result<BoardIssue, ServiceError> {
    let (_team, project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    // The DD10 single seam, per-project (board-lane-management 01-02): fold
    // aliases, then require membership in THIS project's lane set. HTML
    // dialog, dnd POST and JSON PATCH all pass through here, so the three
    // surfaces accept or refuse identically (D8).
    let normalized = validate_project_lane(store, project.id, new_state).await?;

    // Resolve the `after` neighbour key → its issue number within THIS project.
    // Absent/empty ⇒ top of column. An unparseable key is a stale-client mistake
    // → refuse non-enumerably (R3), never a silent top-drop.
    let after_number = match after.map(str::trim) {
        None | Some("") => None,
        Some(key) => match key
            .rsplit_once('-')
            .and_then(|(_, n)| n.parse::<i32>().ok())
        {
            Some(n) => Some(n),
            None => return Err(ServiceError::NotFound),
        },
    };

    match store
        .reposition_issue_with_outbox(
            key_prefix.as_str(),
            number,
            &normalized,
            after_number,
            principal.user_id(),
        )
        .await
    {
        Ok(RepositionOutcome::Repositioned) => {
            let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
                .map(|k| k.to_string())
                .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));
            Ok(BoardIssue {
                key,
                number,
                title: String::new(),
                state: normalized,
            })
        }
        // Missing issue AND unresolvable neighbour BOTH refuse identically.
        Ok(RepositionOutcome::IssueNotFound | RepositionOutcome::NeighbourNotFound) => {
            Err(ServiceError::NotFound)
        }
        Err(IssueInsertError::ProjectNotFound) => Err(ServiceError::NotFound),
        Err(IssueInsertError::Store(_)) => Err(ServiceError::Internal),
    }
}

/// issue-edit-dialog edit-title+description use-case (ADR-002). Mirrors
/// [`change_issue_state`]: `resolve_member_project` authz + non-enumerable
/// NotFound → validate the title (trimmed, non-empty, ≤256 — the SAME bound
/// create enforces) → `Store::update_issue_details`. Returns the updated
/// `BoardIssue` so the handler re-renders the board card. `description_md` is
/// persisted verbatim (the DB CHECK bounds its length); v1 is last-write-wins
/// with no outbox emit (ODD-3/ODD-4).
#[allow(clippy::too_many_arguments)]
pub async fn edit_issue_details(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
    title: &str,
    description_md: &str,
) -> Result<BoardIssue, ServiceError> {
    let (_team, _project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let raw_title = title.trim();
    if raw_title.is_empty() || raw_title.chars().count() > TITLE_MAX_LEN {
        return Err(ServiceError::Validation {
            code: "title_required".to_string(),
            message: "Title is required".to_string(),
        });
    }

    // The SAME bound `create_issue` enforces, placed BEFORE the read-old→UPDATE
    // transaction so an over-long description is refused CLEANLY (validation
    // error) and the issue is left fully untouched — never a partial write and
    // never the DB-CHECK 500 (AC-03.3). The DB CHECK stays as the last line of
    // defense; the rule counts CHARACTERS, mirroring create + Postgres `length()`.
    validate_description(description_md)?;

    match store
        .update_issue_details(
            key_prefix.as_str(),
            number,
            raw_title,
            description_md,
            principal.user_id(),
        )
        .await
    {
        Ok(Some(())) => {
            let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
                .map(|k| k.to_string())
                .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));
            Ok(BoardIssue {
                key,
                number,
                // The TRIMMED title is what `update_issue_details` persisted, so
                // the re-rendered card matches a subsequent read. `state` is not
                // part of an edit (the card render only needs key + title), so it
                // is left empty exactly as `change_issue_state` leaves `title`.
                title: raw_title.to_string(),
                state: String::new(),
            })
        }
        Ok(None) => Err(ServiceError::NotFound),
        Err(IssueInsertError::ProjectNotFound) => Err(ServiceError::NotFound),
        Err(IssueInsertError::Store(_)) => Err(ServiceError::Internal),
    }
}

/// issue-edit-dialog pre-fill read (ADR-002). Resolves the issue's current
/// title + description for the edit dialog AFTER the `resolve_member_project`
/// authz gate, so a foreign or missing issue is refused with the uniform
/// non-enumerable NotFound (ADR-003) and no title is ever echoed for a resource
/// the caller may not see.
pub async fn edit_issue_form(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
) -> Result<IssueEditView, ServiceError> {
    let (_team, project, key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let row = store
        .issue_edit_view(key_prefix.as_str(), number)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    // The project's lane rows, board order — the dialog's Status options (D8).
    let lanes = store
        .list_project_lanes(project.id)
        .await
        .map_err(|_| ServiceError::Internal)?
        .into_iter()
        .map(|lane| crate::BoardLane {
            slug: lane.slug,
            label: lane.label,
        })
        .collect();

    let key = foundry_core::IssueKey::try_new(&key_prefix, number as u32)
        .map(|k| k.to_string())
        .unwrap_or_else(|_| format!("{}-{}", key_prefix.as_str(), number));
    Ok(IssueEditView {
        key,
        number,
        title: row.title,
        description_md: row.description_md,
        state: row.state,
        lanes,
    })
}

/// One change event as served by the program feed (issue-change-history
/// ADR-002 §2). Presentation-neutral: the JSON adapter (`foundry-api`) maps it
/// to the wire shape `{actor, field, old, new, at}`. `actor` is the acting
/// user's email (a stable identifier for integrators, matching the sibling
/// `CommentJson.author_email`); `at` is the change instant (serialized ISO-8601
/// UTC by the adapter). Read from the SAME `list_issue_changes` rows the human
/// timeline renders — one source of truth (AC-03.4).
#[derive(Debug, Clone)]
pub struct IssueChangeHistoryEntry {
    pub actor_email: String,
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub at: time::OffsetDateTime,
}

/// issue-change-history program feed (ADR-002 §2 / US-03). Mirrors the sibling
/// read use-cases: `resolve_member_project` membership authz → resolve the issue
/// by `(team, project_slug, number)` → uniform non-enumerable `NotFound` for a
/// foreign/absent issue (never a 500, watch-item R9) → read the SAME
/// `list_issue_changes` rows the human timeline renders (one source of truth,
/// AC-03.4). The store read is NEWEST-first (reading order for the human page);
/// the program feed reverses it to OLDEST→NEWEST — stable audit/stream order for
/// programs (watch-item R7).
pub async fn list_issue_change_history(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
    number: i32,
) -> Result<Vec<IssueChangeHistoryEntry>, ServiceError> {
    let (team, _project, _key_prefix) =
        resolve_member_project(store, principal, team_slug, project_slug).await?;

    let issue = store
        .find_issue_by_team_project_number(team.id, project_slug, number)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    let rows = store
        .list_issue_changes(issue.issue_id)
        .await
        .map_err(|_| ServiceError::Internal)?;

    // Store read is newest-first; the program feed is oldest→newest (R7).
    Ok(rows
        .into_iter()
        .rev()
        .map(|row| IssueChangeHistoryEntry {
            actor_email: row.actor_email,
            field: row.field,
            old_value: row.old_value,
            new_value: row.new_value,
            at: row.created_at,
        })
        .collect())
}

/// Resolve `(team, project, key_prefix)` after the SAME membership authz the
/// browser handlers run: team-by-slug → is_team_member → project-by-slug.
/// A machine credential scoped to another team is refused (Forbidden), exactly
/// as the board read use-case does (NFR-WEB-API-SEC-02).
async fn resolve_member_project(
    store: &Store,
    principal: &Principal,
    team_slug: &str,
    project_slug: &str,
) -> Result<
    (
        foundry_store::TeamRow,
        foundry_store::ProjectRow,
        foundry_core::ProjectKey,
    ),
    ServiceError,
> {
    let team = store
        .find_team_by_slug(principal.workspace_id(), team_slug)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    if let Principal::Machine {
        scope_team_id: Some(scoped_team),
        ..
    } = principal
    {
        if *scoped_team != team.id {
            return Err(ServiceError::Forbidden);
        }
    }

    let is_member = store
        .is_team_member(team.id, principal.user_id())
        .await
        .map_err(|_| ServiceError::Internal)?;
    if !is_member {
        return Err(ServiceError::Forbidden);
    }

    let project = store
        .find_project_by_slug(team.id, project_slug)
        .await
        .map_err(|_| ServiceError::Internal)?
        .ok_or(ServiceError::NotFound)?;

    let key_prefix = foundry_core::ProjectKey::try_new(&project.key_prefix)
        .map_err(|_| ServiceError::Internal)?;

    Ok((team, project, key_prefix))
}

#[cfg(test)]
mod tests {
    use super::{resolve_lane, validate_description, DESCRIPTION_MAX_LEN};
    use crate::ServiceError;
    use proptest::prelude::*;

    /// The closed, unmintable lane-slug universe (D9) — the specification's
    /// own table, restated here as the test oracle's fold reference
    /// (component-boundaries.md §3: aliases `in-progress`→`in_progress`,
    /// `canceled`→`cancelled` survive the demotion of `normalize_state`).
    const LANE_UNIVERSE: [&str; 5] = ["backlog", "todo", "in_progress", "done", "cancelled"];

    /// SPEC-pinned fold reference (not a copy of production internals): the
    /// D9 closed set + its two documented aliases, applied to the trimmed,
    /// case-insensitive input.
    fn spec_fold(input: &str) -> Option<&'static str> {
        match input.trim().to_ascii_lowercase().as_str() {
            "backlog" => Some("backlog"),
            "todo" => Some("todo"),
            "in-progress" | "in_progress" => Some("in_progress"),
            "done" => Some("done"),
            "cancelled" | "canceled" => Some("cancelled"),
            _ => None,
        }
    }

    /// Inputs a write surface may carry: canonical slugs, the documented
    /// aliases, and arbitrary garbage — each optionally noised with padding
    /// and case flips (the folding must absorb both).
    fn lane_input() -> impl Strategy<Value = String> {
        prop_oneof![
            proptest::sample::select(vec![
                "backlog",
                "todo",
                "in_progress",
                "in-progress",
                "done",
                "cancelled",
                "canceled",
            ])
            .prop_map(str::to_string),
            "[a-zA-Z_ -]{0,12}",
        ]
    }

    proptest! {
        /// board-lane-management 01-02, the DD10 per-project seam's pure
        /// heart: for ANY lane set and ANY input, the input is accepted IFF
        /// its alias-folded canonical form is a member of the lane set — and
        /// the accepted value IS that canonical member. Alias folding
        /// (`in-progress`→`in_progress`, `canceled`→`cancelled`) preserved;
        /// garbage and folded-but-absent lanes refuse identically (D8).
        #[test]
        fn lane_accepted_iff_folded_input_is_a_member(
            lanes in proptest::sample::subsequence(LANE_UNIVERSE.to_vec(), 0..=5),
            input in lane_input(),
            pad in "[ \t]{0,2}",
            shout in any::<bool>(),
        ) {
            let noisy = {
                let cased = if shout { input.to_ascii_uppercase() } else { input.clone() };
                format!("{pad}{cased}{pad}")
            };
            let lane_slugs: Vec<String> = lanes.iter().map(|s| s.to_string()).collect();

            let expected = spec_fold(&noisy)
                .filter(|folded| lanes.contains(folded))
                .map(str::to_string);

            prop_assert_eq!(
                resolve_lane(&noisy, &lane_slugs),
                expected,
                "resolve_lane({:?}, {:?}) must accept iff the folded input is a lane member",
                noisy,
                lane_slugs
            );
        }
    }

    /// A description exactly at the cap is ACCEPTED (the bound is inclusive of
    /// `DESCRIPTION_MAX_LEN`). Kills `>` → `>=` and `>` → `==`.
    #[test]
    fn description_at_the_cap_is_accepted() {
        let at_cap = "a".repeat(DESCRIPTION_MAX_LEN);
        assert!(validate_description(&at_cap).is_ok());
    }

    /// One char over the cap is REJECTED with the `description_too_long`
    /// validation code. Kills `>` → `<` and `>` → `==`.
    #[test]
    fn description_over_the_cap_is_rejected() {
        let over_cap = "a".repeat(DESCRIPTION_MAX_LEN + 1);
        match validate_description(&over_cap) {
            Err(ServiceError::Validation { code, .. }) => {
                assert_eq!(code, "description_too_long");
            }
            other => panic!("expected description_too_long validation error, got {other:?}"),
        }
    }

    /// A short/empty description is ACCEPTED — the guard must not reject
    /// well-under-cap input. Kills `>` → `<`.
    #[test]
    fn short_and_empty_descriptions_are_accepted() {
        assert!(validate_description("").is_ok());
        assert!(validate_description("a short description").is_ok());
    }

    /// The cap counts CHARACTERS, not bytes: `DESCRIPTION_MAX_LEN` multi-byte
    /// chars (2 bytes each in UTF-8) is at the cap and ACCEPTED, guarding
    /// against a `chars().count()` → `len()` regression.
    #[test]
    fn multibyte_description_at_the_cap_is_accepted() {
        let at_cap = "é".repeat(DESCRIPTION_MAX_LEN);
        assert!(validate_description(&at_cap).is_ok());
    }
}
