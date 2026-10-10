//! Team membership: a person's role on a team (team-membership DDD-3).
//!
//! DISTILL 2026-10-09 created only the role type, because the store's RED
//! scaffolds take it. `TeamRole::parse` and the one authority policy
//! `TeamAccess::of(is_workspace_admin, Option<TeamRole>)` land with DELIVER step
//! 01-01 (ADR-TEAM-MEMBERSHIP-003), with their example tests.
// SCAFFOLD: true — TeamAccess is not here yet (DELIVER 01-01).

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
}
