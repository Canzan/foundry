//! Workspace membership: a person's role in a workspace, the
//! `workspace_memberships.role` column.

/// A person's role in a workspace (`'admin'` / `'member'`, migration 0001's
/// CHECK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspaceRole {
    Admin,
    Member,
}

impl WorkspaceRole {
    /// The stored value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Member => "member",
        }
    }

    /// Parse the stored (and CLI) value. Exact match only: anything other than
    /// `"admin"` or `"member"` is refused with `None`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "admin" => Some(Self::Admin),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkspaceRole;

    #[test]
    fn each_role_round_trips_through_its_stored_value() {
        for role in [WorkspaceRole::Admin, WorkspaceRole::Member] {
            assert_eq!(WorkspaceRole::parse(role.as_str()), Some(role));
        }
    }

    #[test]
    fn anything_but_the_exact_stored_values_is_refused() {
        for raw in ["", "Admin", "ADMIN", " admin", "lead", "owner"] {
            assert_eq!(WorkspaceRole::parse(raw), None, "{raw:?}");
        }
    }
}
