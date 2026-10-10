//! Team membership: a person's role on a team, and the one team-authority policy
//! (team-membership DDD-3, ADR-TEAM-MEMBERSHIP-003).
//!
//! `TeamAccess::of(is_workspace_admin, Option<TeamRole>)` is the single answer to
//! "what may this person do with this team?". It has no I/O; the facts come from
//! one store read and the policy is applied only inside `foundry_services::teams`.

/// A person's role on a team: the `team_memberships.role` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeamRole {
    Lead,
    Member,
}

impl TeamRole {
    /// The stored value (`'lead'` / `'member'`, migration 0001's CHECK).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lead => "lead",
            Self::Member => "member",
        }
    }

    /// Parse the stored (and form) value. Exact match only: anything other than
    /// `"lead"` or `"member"` is refused with `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "lead" => Some(Self::Lead),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

/// What a person may do with a team's membership (DDD-3, Authorization Matrix).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeamAccess {
    /// Not theirs to see: the uniform 404.
    None,
    /// See the members page, without controls.
    View,
    /// See the members page and change it.
    Manage,
}

impl TeamAccess {
    /// The six-row table. Slices 01-03: a lead gets `View`; slice 04 flips that
    /// one arm to `Manage`.
    pub fn of(is_workspace_admin: bool, role: Option<TeamRole>) -> Self {
        match (is_workspace_admin, role) {
            (true, _) => Self::Manage,
            (false, Some(TeamRole::Lead)) => Self::View,
            (false, Some(TeamRole::Member)) => Self::View,
            (false, None) => Self::None,
        }
    }

    /// May see the members page.
    pub fn can_view(self) -> bool {
        self != Self::None
    }

    /// May change the team's membership.
    pub fn can_manage(self) -> bool {
        self == Self::Manage
    }
}

// bypass: `TeamAccess::of` is total over a finite 2 x 3 domain, so the six-row
// table IS the exhaustive property; `TeamRole::parse` is an exact-string contract.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_access_six_row_table() {
        let table = [
            (true, None, TeamAccess::Manage),
            (true, Some(TeamRole::Member), TeamAccess::Manage),
            (true, Some(TeamRole::Lead), TeamAccess::Manage),
            (false, None, TeamAccess::None),
            (false, Some(TeamRole::Member), TeamAccess::View),
            // Slice 01-03: a lead views; slice 04 makes this Manage.
            (false, Some(TeamRole::Lead), TeamAccess::View),
        ];
        for (is_admin, role, expected) in table {
            assert_eq!(
                TeamAccess::of(is_admin, role),
                expected,
                "TeamAccess::of({is_admin}, {role:?})"
            );
        }
    }

    #[test]
    fn view_and_manage_follow_the_access_level() {
        assert!(!TeamAccess::None.can_view());
        assert!(!TeamAccess::None.can_manage());
        assert!(TeamAccess::View.can_view());
        assert!(!TeamAccess::View.can_manage());
        assert!(TeamAccess::Manage.can_view());
        assert!(TeamAccess::Manage.can_manage());
    }

    #[test]
    fn role_parse_accepts_only_the_stored_words() {
        assert_eq!(TeamRole::parse("lead"), Some(TeamRole::Lead));
        assert_eq!(TeamRole::parse("member"), Some(TeamRole::Member));
        assert_eq!(TeamRole::parse("Lead"), None);
        assert_eq!(TeamRole::parse(""), None);
        assert_eq!(TeamRole::parse("admin"), None);
    }

    #[test]
    fn role_round_trips_through_its_stored_word() {
        for role in [TeamRole::Lead, TeamRole::Member] {
            assert_eq!(TeamRole::parse(role.as_str()), Some(role));
        }
    }
}
