//! navigation-bar-linear-ui — the shared sidebar's presentation value object.
//!
//! `NavContext` is a *presentation projection* of the already-resolved
//! authenticated session identity (workspace name, display name, admin flag,
//! per-request CSRF token) plus the handler-chosen active section and the
//! resolved Board deep-link target. It is assembled ONCE per authenticated page
//! and embedded as a single `nav` field on each page's Askama template struct,
//! so `partials/sidebar.html` reads `nav.*` instead of the page threading five
//! separate variables (DESIGN data-models.md / ADR-002). It re-fetches and
//! re-authorizes nothing.

/// The active primary sidebar item. Exactly one value is chosen per
/// authenticated page (FR-4 / AC-03.3: never zero, never two). Two variants
/// only — the rail has exactly two primary destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    /// Dashboard `/` AND every non-board authed surface (tokens, invites,
    /// project-create, instance admin). Home is the app's default/hub section.
    Home,
    /// The board route family `/team/{slug}/project/{slug}` and its descendants
    /// (board, change report, issue detail).
    Board,
}

/// Everything the shared sidebar needs, assembled ONCE per authenticated page
/// from the session context. Single documented source for every
/// shared-artifacts-registry variable — no variable has two divergent sources.
#[derive(Debug, Clone)]
pub struct NavContext {
    /// Brand (top) + footer identity — from the auth/session context.
    pub workspace_name: String,
    /// Footer identity — from the auth/session context.
    pub display_name: String,
    /// Gates the Instance-admin menu item (FR-6) — from the authorization layer.
    pub is_instance_admin: bool,
    /// Hidden `_csrf` for the footer sign-out form (BR-3) — per-request token.
    pub csrf: String,
    /// Drives the active class + `aria-current` on exactly one primary item.
    pub active: NavSection,
    /// Resolved Board deep-link target (ADR-003): the workspace's first-project
    /// board `/team/{slug}/project/{slug}` (step 04-02, via [`resolve_board_href`]),
    /// or `/` when the workspace has no projects.
    pub board_href: String,
}

impl NavContext {
    /// Assemble the nav carrier for one authenticated page from the resolved
    /// session identity values the handler already holds, the handler-chosen
    /// `active` section, and the resolved `board_href`. (There is no
    /// `SessionContext` type in this codebase yet — the handler passes the
    /// already-resolved identity fields directly; `NavContext` is a pure
    /// presentation projection over them.)
    pub fn for_page(
        workspace_name: String,
        display_name: String,
        is_instance_admin: bool,
        csrf: String,
        active: NavSection,
        board_href: String,
    ) -> Self {
        Self {
            workspace_name,
            display_name,
            is_instance_admin,
            csrf,
            active,
            board_href,
        }
    }

    /// Assemble the `Home`-section rail for a batch-migrated authenticated page
    /// (navigation-bar-linear-ui step 02-01) by resolving the acting identity
    /// through the SHIPPED `dashboard_greeting` seam — the SAME presentation
    /// projection the dashboard uses (`signin::dashboard_root`) — falling back
    /// to the neutral greeting on lookup failure so the page still renders 200
    /// (never 500s), mirroring the dashboard's graceful degradation. Board-family
    /// active-state (02-02) and the resolved first-project deep-link (04-02)
    /// refine `active`/`board_href` in later slices; the footer-only fields
    /// (`is_instance_admin`, `csrf`) are inert in the current rail — there is no
    /// footer user menu in `partials/sidebar.html` yet — so they default here and
    /// are wired when that footer lands.
    pub(crate) async fn home_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
    ) -> Self {
        let (display_name, workspace_name) = state
            .store
            .dashboard_greeting(user_id, workspace_id)
            .await
            .unwrap_or_else(|err| {
                tracing::error!(%err, "nav: dashboard_greeting failed; neutral rail identity");
                None
            })
            .unwrap_or_else(|| ("there".to_string(), "your workspace".to_string()));
        let board_href = resolve_board_href(state, workspace_id).await;
        Self::for_page(
            workspace_name,
            display_name,
            false,
            String::new(),
            NavSection::Home,
            board_href,
        )
    }

    /// Assemble the `Board`-section rail for a board-family authenticated page
    /// (navigation-bar-linear-ui step 02-02 — the `/team/{slug}/project/{slug}`
    /// route family: board, change report, issue detail). Identical identity
    /// resolution to [`Self::home_for`] — the SAME `dashboard_greeting` seam and
    /// the SAME neutral fallback so a lookup failure still renders 200 — differing
    /// only in the handler-chosen active section (`Board`). This is the
    /// deterministic active rule (DESIGN data-models.md): the board family is the
    /// SOLE surface that marks `Board` current; every other authed page uses
    /// `home_for`, so exactly one primary item is ever current (FR-4: never zero,
    /// never two). The resolved first-project deep-link (`board_href`) stays the
    /// provisional `/` until step 04-02 refines it, matching `home_for`.
    pub(crate) async fn board_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
    ) -> Self {
        let (display_name, workspace_name) = state
            .store
            .dashboard_greeting(user_id, workspace_id)
            .await
            .unwrap_or_else(|err| {
                tracing::error!(%err, "nav: dashboard_greeting failed; neutral rail identity");
                None
            })
            .unwrap_or_else(|| ("there".to_string(), "your workspace".to_string()));
        let board_href = resolve_board_href(state, workspace_id).await;
        Self::for_page(
            workspace_name,
            display_name,
            false,
            String::new(),
            NavSection::Board,
            board_href,
        )
    }

    /// Uppercased first character of the workspace name, for the brand monogram.
    /// `"?"` when the workspace name is empty.
    pub fn monogram(&self) -> String {
        self.workspace_name
            .chars()
            .next()
            .map(|first| first.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string())
    }

    /// Whether the Home primary item is the current page.
    pub fn is_home(&self) -> bool {
        self.active == NavSection::Home
    }

    /// Whether the Board primary item is the current page.
    pub fn is_board(&self) -> bool {
        self.active == NavSection::Board
    }
}

/// Pure ADR-003 rule: the Board primary-nav deep-link is the workspace's FIRST
/// project board `/team/{team_slug}/project/{project_slug}`; when the workspace has
/// ZERO projects it degrades to the dashboard `/`, whose empty-state hosts the
/// "create your first project" affordance (no dead link, no 404). Kept pure (no
/// store) so the href formatting + the zero-project fallback are unit-pinned without
/// a live DB — both [`resolve_board_href`] and the dashboard feed it their
/// already-loaded first-project `(team_slug, project_slug)` pair.
pub(crate) fn board_href_for_first_project(first: Option<(&str, &str)>) -> String {
    match first {
        Some((team_slug, project_slug)) => format!("/team/{team_slug}/project/{project_slug}"),
        None => "/".to_string(),
    }
}

/// Resolve the Board primary-nav deep-link target (ADR-003) for an authed render.
/// Reuses the SAME deterministic first-project query family the dashboard already
/// issues for "Your projects" (`list_projects_for_workspace`, `ORDER BY p.name`) —
/// no new ordering invented — and applies the pure [`board_href_for_first_project`]
/// rule: the first project's board, else the dashboard `/`. One cheap read per
/// render; on lookup error it degrades to `/` (never 500s), mirroring the
/// dashboard's graceful-degradation posture.
pub(crate) async fn resolve_board_href(
    state: &crate::AppState,
    workspace_id: uuid::Uuid,
) -> String {
    let projects = state
        .store
        .list_projects_for_workspace(workspace_id)
        .await
        .unwrap_or_else(|err| {
            tracing::error!(%err, "nav: list_projects_for_workspace failed; Board falls back to /");
            Vec::new()
        });
    board_href_for_first_project(projects.first().map(
        |(team_slug, project_slug, _name, _key_prefix)| (team_slug.as_str(), project_slug.as_str()),
    ))
}

#[cfg(test)]
mod tests {
    use super::{NavContext, NavSection};

    fn nav_with(workspace_name: &str, active: NavSection) -> NavContext {
        NavContext::for_page(
            workspace_name.to_string(),
            "Ada Lovelace".to_string(),
            false,
            "csrf-token".to_string(),
            active,
            "/".to_string(),
        )
    }

    /// Behaviour 1 — the brand monogram is the uppercased first grapheme of the
    /// workspace name, and a safe `"?"` placeholder when the name is empty.
    /// Table-driven over the equivalence classes (already-upper, lower-cased,
    /// non-ASCII, empty).
    #[test]
    fn monogram_is_the_uppercased_first_character_or_placeholder_when_empty() {
        let cases = [("Acme", "A"), ("acme", "A"), ("ålborg", "Å"), ("", "?")];
        for (workspace_name, expected) in cases {
            let nav = nav_with(workspace_name, NavSection::Home);
            assert_eq!(
                nav.monogram(),
                expected,
                "monogram of {workspace_name:?} must be {expected:?}"
            );
        }
    }

    /// Behaviour 2 — the active-section predicates are a total, mutually
    /// exclusive pair: exactly one of `is_home()` / `is_board()` is true, driven
    /// by `active` (FR-4: never zero, never two).
    #[test]
    fn active_section_selects_exactly_one_primary_predicate() {
        let home = nav_with("Acme", NavSection::Home);
        assert!(home.is_home(), "Home active → is_home() true");
        assert!(!home.is_board(), "Home active → is_board() false");

        let board = nav_with("Acme", NavSection::Board);
        assert!(board.is_board(), "Board active → is_board() true");
        assert!(!board.is_home(), "Board active → is_home() false");
    }

    /// Behaviour 3 (04-02 ADR-003) — the Board deep-link is the workspace's FIRST
    /// project board route `/team/{team_slug}/project/{project_slug}`, and degrades
    /// to the dashboard `/` when the workspace has ZERO projects (whose empty-state
    /// hosts the "create your first project" affordance — no dead link, no 404).
    /// Pins the pure href-formatting + zero-project fallback rule in milliseconds,
    /// without a live Postgres: `resolve_board_href` feeds this pure rule the
    /// already-loaded first-project `(team_slug, project_slug)` pair.
    #[test]
    fn board_href_is_the_first_projects_board_or_dashboard_fallback_when_empty() {
        let cases = [
            (
                Some(("general", "sandbox")),
                "/team/general/project/sandbox",
            ),
            (Some(("core", "atlas")), "/team/core/project/atlas"),
            (None, "/"),
        ];
        for (first, expected) in cases {
            assert_eq!(
                super::board_href_for_first_project(first),
                expected,
                "board href for first project {first:?} must be {expected:?}"
            );
        }
    }

    /// Behaviour 2 (totality guard, 02-02 Earned Trust) — for EVERY `NavSection`
    /// variant the primary-item predicates partition cleanly: the count of
    /// current primary items is exactly ONE, never zero, never two (FR-4 /
    /// AC-03.3). Iterating every variant makes this the unit-level regression net
    /// behind the acceptance sweep's "exactly one primary item is current"
    /// property — a future third variant (or a predicate wired to return true for
    /// both) that broke the never-zero/never-two invariant would red HERE, in
    /// milliseconds, instead of only under a live-Postgres acceptance run.
    #[test]
    fn every_nav_section_marks_exactly_one_primary_item_current() {
        for active in [NavSection::Home, NavSection::Board] {
            let nav = nav_with("Acme", active);
            let current = usize::from(nav.is_home()) + usize::from(nav.is_board());
            assert_eq!(
                current, 1,
                "exactly one primary item must be current for {active:?}, got {current}"
            );
        }
    }
}
