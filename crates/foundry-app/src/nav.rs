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
/// authenticated page (FR-4 / AC-03.3: never zero, never two).
///
/// ONE FIELD, FIVE VARIANTS (ui-nav-polish P1, ADR-NAV-POLISH-001). The marked
/// member is derived by EQUALITY on this sum, so a rail that marks two members —
/// or none — cannot be built here: there is no second marker to set and no bool
/// per item to forget. Each variant is the page family that OWNS the member, and
/// `Home` is also the fallback for every authed surface no primary member owns
/// (account settings, project create, instance admin, unsubscribe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    /// Dashboard `/` AND every authed surface no primary member owns
    /// (account settings, project create, instance admin, unsubscribe). Home is
    /// the app's default/hub section.
    Home,
    /// The board route family `/team/{slug}/project/{slug}` and its descendants
    /// (board, change report, issue detail).
    Board,
    /// `/teams` and its descendants (the team list, a team's own page, its
    /// members) — the destination every signed-in operator is offered.
    Teams,
    /// `/workspace/invites` — the invite form and its "invite sent" render.
    Invites,
    /// `/admin/tokens` — every machine-token render of that surface.
    Tokens,
}

impl NavSection {
    /// The member the rail may MARK, given what the rail will OFFER for this
    /// render (ui-nav-polish L2, L9 / ADR-NAV-POLISH-003, ADR-NAV-POLISH-005).
    ///
    /// ONE PLACE RECONCILES THE MARK WITH THE OFFER, and it is
    /// [`NavContext::resolved`], the one place that already resolves the rail's
    /// own identity. `Invites` and `Tokens` are the only members the rail
    /// withholds, and it withholds them exactly when its own `nav_identity`
    /// read reported no workspace admin. That read can report `false` for a
    /// render the ROUTE GATE already admitted — the gate and the rail read the
    /// fact separately, and the rail's read can fail — so a page that named
    /// `Invites` or `Tokens` for itself would otherwise mark a member its own
    /// document does not render, and the rail would serve ZERO marks (failure
    /// B1). Mapping that section to `Home`, which the rail always offers, is
    /// what keeps the marked set a non-empty subset of the offered set in
    /// EVERY render.
    ///
    /// TOTAL, PURE AND INFALLIBLE: every variant has an answer for both values
    /// of the fact, so no caller can forget a case and none can fail. The
    /// reconciliation lives here rather than in the four handlers that name a
    /// section, because one of those four would forget it.
    fn offered_mark(self, is_workspace_admin: bool) -> Self {
        match self {
            Self::Invites | Self::Tokens if !is_workspace_admin => Self::Home,
            Self::Home | Self::Board | Self::Teams | Self::Invites | Self::Tokens => self,
        }
    }
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
    /// Gates the user-section Integrations item (integrations-wayfinding W-1):
    /// the same workspace-admin fact `/workspace/integrations` checks, read live
    /// with the identity on each render (ADR-IW-002).
    pub is_workspace_admin: bool,
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
        is_workspace_admin: bool,
        csrf: String,
        active: NavSection,
        board_href: String,
    ) -> Self {
        Self {
            workspace_name,
            display_name,
            is_instance_admin,
            is_workspace_admin,
            csrf,
            active,
            board_href,
        }
    }

    /// Assemble the `Home`-section rail for an authenticated page. The acting
    /// identity comes from ONE `nav_identity` read (ADR-IW-002), which carries the
    /// workspace-admin fact with the names, so the Integrations item costs no extra
    /// round trip. On lookup failure the rail falls back to a neutral identity and
    /// hides the Integrations item (fail closed, as [`resolve_is_instance_admin`]),
    /// so the page still renders 200.
    ///
    /// The footer user-menu fields (`is_instance_admin`, `csrf`) are supplied by the
    /// caller: the handler resolves the acting user's REAL instance-admin authority
    /// (via [`resolve_is_instance_admin`], fail-closed) and its per-request CSRF
    /// double-submit token (via `ensure_csrf_cookie`), so the footer sign-out form
    /// carries a cookie-matched token and the Instance-admin item follows the user's
    /// actual authority on EVERY authed page — not only the dashboard (04-03).
    pub(crate) async fn home_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        is_instance_admin: bool,
        csrf: String,
    ) -> Self {
        Self::resolved(
            state,
            user_id,
            workspace_id,
            is_instance_admin,
            csrf,
            NavSection::Home,
        )
        .await
    }

    /// Assemble the `Board`-section rail for a board-family authenticated page
    /// (the `/team/{slug}/project/{slug}` route family: board, change report,
    /// issue detail). Identical identity resolution to [`Self::home_for`],
    /// differing only in the active section. The board family is the SOLE surface
    /// that marks `Board` current, exactly as each of the three siblings below is
    /// the sole surface that marks its own member, so exactly one primary item is
    /// ever current (FR-4: never zero, never two).
    pub(crate) async fn board_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        is_instance_admin: bool,
        csrf: String,
    ) -> Self {
        Self::resolved(
            state,
            user_id,
            workspace_id,
            is_instance_admin,
            csrf,
            NavSection::Board,
        )
        .await
    }

    /// Assemble the `Teams`-section rail for `/teams` and its descendants
    /// (ui-nav-polish P1). Same identity resolution as [`Self::home_for`],
    /// differing only in the section the page names for itself.
    pub(crate) async fn teams_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        is_instance_admin: bool,
        csrf: String,
    ) -> Self {
        Self::resolved(
            state,
            user_id,
            workspace_id,
            is_instance_admin,
            csrf,
            NavSection::Teams,
        )
        .await
    }

    /// Assemble the `Invites`-section rail for `/workspace/invites`
    /// (ui-nav-polish P1). Same identity resolution as [`Self::home_for`].
    pub(crate) async fn invites_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        is_instance_admin: bool,
        csrf: String,
    ) -> Self {
        Self::resolved(
            state,
            user_id,
            workspace_id,
            is_instance_admin,
            csrf,
            NavSection::Invites,
        )
        .await
    }

    /// Assemble the `Tokens`-section rail for `/admin/tokens`
    /// (ui-nav-polish P1). Same identity resolution as [`Self::home_for`].
    pub(crate) async fn tokens_for(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        is_instance_admin: bool,
        csrf: String,
    ) -> Self {
        Self::resolved(
            state,
            user_id,
            workspace_id,
            is_instance_admin,
            csrf,
            NavSection::Tokens,
        )
        .await
    }

    /// The rail with `active` current, for the identity [`Self::home_for`]
    /// describes: one place decides the neutral fallback for every section AND
    /// reconciles the section the page named with the members the rail will
    /// offer that identity ([`NavSection::offered_mark`], law L9). The identity
    /// is read FIRST, because the reconciliation needs the workspace-admin fact
    /// the rail itself resolved — including the `false` of the neutral fallback
    /// — and not the one the route gate read.
    async fn resolved(
        state: &crate::AppState,
        user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        is_instance_admin: bool,
        csrf: String,
        active: NavSection,
    ) -> Self {
        let identity = state
            .store
            .nav_identity(user_id, workspace_id)
            .await
            .unwrap_or_else(|err| {
                tracing::error!(%err, "nav: nav_identity failed; neutral rail identity");
                None
            })
            .unwrap_or_else(|| foundry_store::NavIdentity {
                display_name: "there".to_string(),
                workspace_name: "your workspace".to_string(),
                is_workspace_admin: false,
            });
        let board_href = resolve_board_href(state, workspace_id, user_id).await;
        Self::for_page(
            identity.workspace_name,
            identity.display_name,
            is_instance_admin,
            identity.is_workspace_admin,
            csrf,
            active.offered_mark(identity.is_workspace_admin),
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

    /// Whether the Teams primary item is the current page.
    pub fn is_teams(&self) -> bool {
        self.active == NavSection::Teams
    }

    /// Whether the Invite-a-member primary item is the current page.
    pub fn is_invites(&self) -> bool {
        self.active == NavSection::Invites
    }

    /// Whether the Machine-tokens primary item is the current page.
    pub fn is_tokens(&self) -> bool {
        self.active == NavSection::Tokens
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
/// issues for "Your projects" (`list_projects_for_workspace`, `ORDER BY p.name`,
/// member-scoped to the SESSION user so it never points at a board the team gate
/// would 404 — fix-hide-unreachable-boards) —
/// no new ordering invented — and applies the pure [`board_href_for_first_project`]
/// rule: the first project's board, else the dashboard `/`. One cheap read per
/// render; on lookup error it degrades to `/` (never 500s), mirroring the
/// dashboard's graceful-degradation posture.
pub(crate) async fn resolve_board_href(
    state: &crate::AppState,
    workspace_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> String {
    let projects = state
        .store
        .list_projects_for_workspace(workspace_id, user_id)
        .await
        .unwrap_or_else(|err| {
            tracing::error!(%err, "nav: list_projects_for_workspace failed; Board falls back to /");
            Vec::new()
        });
    board_href_for_first_project(projects.first().map(
        |(team_slug, project_slug, _name, _key_prefix)| (team_slug.as_str(), project_slug.as_str()),
    ))
}

/// Resolve whether the acting user is an INSTANCE super-admin for the shared rail's
/// footer Instance-admin item (FR-6), failing CLOSED (item absent) on lookup error —
/// the SAME fail-closed posture `signin::dashboard_root` and the instance-admin gate
/// use, so the affordance never surfaces for a user we could not verify. Reused by
/// every authed handler that assembles the rail via [`NavContext::home_for`] /
/// [`NavContext::board_for`], so the item follows the user's real authority on EVERY
/// authed page — not only the dashboard (04-03 D2).
pub(crate) async fn resolve_is_instance_admin(
    state: &crate::AppState,
    user_id: uuid::Uuid,
) -> bool {
    state
        .store
        .is_instance_admin(user_id)
        .await
        .unwrap_or_else(|err| {
            tracing::error!(%err, "nav: is_instance_admin failed; hiding Instance-admin item");
            false
        })
}

#[cfg(test)]
mod tests {
    use super::{NavContext, NavSection};

    fn nav_with(workspace_name: &str, active: NavSection) -> NavContext {
        NavContext::for_page(
            workspace_name.to_string(),
            "Ada Lovelace".to_string(),
            false,
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

    /// Behaviour 2 — the active-section predicates are total and mutually
    /// exclusive: the predicate of the named section is true and every other one
    /// is false, driven by `active` (FR-4: never zero, never two).
    #[test]
    fn active_section_selects_exactly_one_primary_predicate() {
        let home = nav_with("Acme", NavSection::Home);
        assert!(home.is_home(), "Home active → is_home() true");
        assert!(!home.is_board(), "Home active → is_board() false");

        let board = nav_with("Acme", NavSection::Board);
        assert!(board.is_board(), "Board active → is_board() true");
        assert!(!board.is_home(), "Board active → is_home() false");

        let teams = nav_with("Acme", NavSection::Teams);
        assert!(teams.is_teams(), "Teams active → is_teams() true");
        assert!(!teams.is_home(), "Teams active → is_home() false");

        let invites = nav_with("Acme", NavSection::Invites);
        assert!(invites.is_invites(), "Invites active → is_invites() true");
        assert!(!invites.is_home(), "Invites active → is_home() false");

        let tokens = nav_with("Acme", NavSection::Tokens);
        assert!(tokens.is_tokens(), "Tokens active → is_tokens() true");
        assert!(!tokens.is_home(), "Tokens active → is_home() false");
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
        for active in [
            NavSection::Home,
            NavSection::Board,
            NavSection::Teams,
            NavSection::Invites,
            NavSection::Tokens,
        ] {
            let nav = nav_with("Acme", active);
            let current = usize::from(nav.is_home())
                + usize::from(nav.is_board())
                + usize::from(nav.is_teams())
                + usize::from(nav.is_invites())
                + usize::from(nav.is_tokens());
            assert_eq!(
                current, 1,
                "exactly one primary item must be current for {active:?}, got {current}"
            );
        }
    }

    /// Behaviour 4 (ui-nav-polish L2 / L9, ADR-NAV-POLISH-005) — the mark the
    /// rail may carry is reconciled with what the rail OFFERS, over the whole
    /// closed product of five sections and the two values of the
    /// workspace-admin fact the rail's OWN identity read reported.
    ///
    /// `Invites` and `Tokens` are the two members the rail withholds from an
    /// identity the two admin pages refuse, so for that identity they fall to
    /// `Home`; every other pair keeps the section the page named. The
    /// interesting pair is the UNOBSERVABLE one (failure B1): the route gate
    /// admitted a workspace admin, the rail's `nav_identity` read then failed
    /// and fell back to the neutral identity that reports `false`, and the
    /// served rail must still mark exactly ONE member — never zero — on
    /// `/workspace/invites` and `/admin/tokens` as on every other page. No
    /// acceptance row can drive that render, which is why this unit pair is its
    /// standing check. The marked member is asserted to be an OFFERED one by
    /// construction: `Home` is in the offered set of every identity.
    #[test]
    fn the_mark_falls_to_home_when_the_rail_will_not_offer_the_named_section() {
        let cases = [
            (NavSection::Home, false, NavSection::Home),
            (NavSection::Home, true, NavSection::Home),
            (NavSection::Board, false, NavSection::Board),
            (NavSection::Board, true, NavSection::Board),
            (NavSection::Teams, false, NavSection::Teams),
            (NavSection::Teams, true, NavSection::Teams),
            (NavSection::Invites, false, NavSection::Home),
            (NavSection::Invites, true, NavSection::Invites),
            (NavSection::Tokens, false, NavSection::Home),
            (NavSection::Tokens, true, NavSection::Tokens),
        ];
        for (named, is_workspace_admin, expected) in cases {
            assert_eq!(
                named.offered_mark(is_workspace_admin),
                expected,
                "{named:?} named with is_workspace_admin={is_workspace_admin} must be marked \
                 as {expected:?}"
            );
            let nav = nav_with("Acme", named.offered_mark(is_workspace_admin));
            let current = usize::from(nav.is_home())
                + usize::from(nav.is_board())
                + usize::from(nav.is_teams())
                + usize::from(nav.is_invites())
                + usize::from(nav.is_tokens());
            assert_eq!(
                current, 1,
                "the reconciled mark for ({named:?}, {is_workspace_admin}) must still be \
                 exactly one primary item, got {current}"
            );
            let offered = is_workspace_admin
                || !matches!(nav.active, NavSection::Invites | NavSection::Tokens);
            assert!(
                offered,
                "the reconciled mark {:?} must be a member the rail offers an identity with \
                 is_workspace_admin={is_workspace_admin}",
                nav.active
            );
        }
    }
}
