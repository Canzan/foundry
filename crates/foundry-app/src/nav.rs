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
    /// Resolved Board deep-link target (ADR-003). Provisional `/` in the walking
    /// skeleton; real first-project resolution lands in step 04-02.
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
}
