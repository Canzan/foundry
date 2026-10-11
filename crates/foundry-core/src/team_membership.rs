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
    /// Everything `Manage` may, and delete the team (ADR-TEAM-LIFECYCLE-004).
    Administer,
}

impl TeamAccess {
    /// The six-row table: a workspace admin administers it, a lead of this team
    /// manages it, a member views it, anyone else gets nothing (slice 04 gave
    /// leads `Manage`; ADR-TEAM-LIFECYCLE-004 gave admins `Administer`).
    pub fn of(is_workspace_admin: bool, role: Option<TeamRole>) -> Self {
        match (is_workspace_admin, role) {
            (true, _) => Self::Administer,
            (false, Some(TeamRole::Lead)) => Self::Manage,
            (false, Some(TeamRole::Member)) => Self::View,
            (false, None) => Self::None,
        }
    }

    /// May see the members page.
    pub fn can_view(self) -> bool {
        self != Self::None
    }

    /// May change the team's membership (and, from slice 04 of team-lifecycle,
    /// rename it).
    pub fn can_manage(self) -> bool {
        match self {
            Self::Manage | Self::Administer => true,
            Self::None | Self::View => false,
        }
    }

    /// May delete the team: a workspace admin only.
    pub fn can_delete(self) -> bool {
        match self {
            Self::Administer => true,
            Self::None | Self::View | Self::Manage => false,
        }
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
            // ADR-TEAM-LIFECYCLE-004: a workspace admin administers every team.
            (true, None, TeamAccess::Administer),
            (true, Some(TeamRole::Member), TeamAccess::Administer),
            (true, Some(TeamRole::Lead), TeamAccess::Administer),
            (false, None, TeamAccess::None),
            (false, Some(TeamRole::Member), TeamAccess::View),
            // Slice 04: a lead manages her own team.
            (false, Some(TeamRole::Lead), TeamAccess::Manage),
        ];
        for (is_admin, role, expected) in table {
            assert_eq!(
                TeamAccess::of(is_admin, role),
                expected,
                "TeamAccess::of({is_admin}, {role:?})"
            );
        }
    }

    /// One row per level: `(level, can_view, can_manage, can_delete)`.
    #[test]
    fn view_manage_and_delete_follow_the_access_level() {
        let table = [
            (TeamAccess::None, false, false, false),
            (TeamAccess::View, true, false, false),
            (TeamAccess::Manage, true, true, false),
            (TeamAccess::Administer, true, true, true),
        ];
        for (access, view, manage, delete) in table {
            assert_eq!(access.can_view(), view, "{access:?}.can_view()");
            assert_eq!(access.can_manage(), manage, "{access:?}.can_manage()");
            assert_eq!(access.can_delete(), delete, "{access:?}.can_delete()");
        }
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
