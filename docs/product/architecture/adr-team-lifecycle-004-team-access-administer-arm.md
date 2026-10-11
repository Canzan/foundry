# ADR-TEAM-LIFECYCLE-004: The one team-authority policy gains an `Administer` level for delete; create stays a workspace-admin check, as `/teams/default` is

- Status: Accepted (2026-10-11)
- Feature: `team-lifecycle` (DISCUSS D2, D5, OQ-1, INV-TL-6; DESIGN DDD-15, DDD-16)
- Amends: ADR-TEAM-MEMBERSHIP-003 decision 1 (the policy table)

## Context

ADR-TEAM-MEMBERSHIP-003 made `foundry_core::TeamAccess::of(is_workspace_admin, Option<TeamRole>) →
{None, View, Manage}` the single answer to "what may this person do with this team". It is applied
only inside `foundry_services::teams`. Its table collapses "workspace admin" and "lead" into
`Manage`. This feature adds three verbs:
- **rename**: Manage, which includes a lead (OQ-1);
- **delete**: workspace admin only (D2);
- **create**: workspace admin only (D2). Create has no team yet, so there are no team facts.

The members page must render **Rename** for Manage, **Delete team** for an admin only, and **New
project** for anyone on the team (D14). An admin who is not on the team is not shown it, because
project create is gated on membership.

## Decision

1. **`TeamAccess` gains a fourth level, `Administer`**, the workspace admin. The table becomes:
   `(true, _) → Administer`, `(false, Some(Lead)) → Manage`, `(false, Some(Member)) → View`,
   `(false, None) → None`. The predicates are:
   - `can_view()`: anything but `None` (unchanged).
   - `can_manage()`: `Manage` or `Administer` (unchanged meaning for every shipped caller).
   - `can_delete()`: `Administer` only.

   The six-row example test changes its three admin rows. The only match on the enum is inside
   `team_membership.rs`. Every other caller uses the predicates, so no shipped verb changes
   behaviour.
2. **Delete** (`GET` and `POST /team/{slug}/delete`) resolves facts and access through the shipped
   `resolve_access` and requires `can_delete()`. Anything else is `TeamError::NotFound`, the
   uniform 404.
3. **Rename** requires `can_manage()`. It needs no new arm (slice 04's hypothesis).
4. **Create** (`POST /teams`) uses `Store::is_workspace_admin` in the use-case, exactly as
   ADR-TEAM-MEMBERSHIP-003 decision 3 already does for `/teams` and `/teams/default`. The store
   re-checks it under `FOR KEY SHARE` inside the create transaction (ADR-TEAM-LIFECYCLE-001 decision
   4). An instance admin with no workspace-admin row is refused like anyone else (D2).
5. **The members-page view model** carries `access` (as today) plus `on_team: bool`, which is
   `facts.role.is_some()` from the same facts read:
   - **New project** renders when `on_team`;
   - **Rename** renders when `can_manage()`;
   - **Delete team** renders when `can_delete()`.
6. **Authority is read once per request, at the start of the use-case** (team-membership DDD-22,
   INV-TL-6). The delete's locked transaction does not re-check the actor. The window is the
   milliseconds between the facts read and the lock, for an admin acting on their own workspace.

## Alternatives considered

- **Keep `TeamAccess` at three levels and check `facts.is_workspace_admin` explicitly in the delete
  use-case.** It is viable and touches no shipped test. Rejected: it is a second authority predicate
  beside the policy, the drift ADR-TEAM-MEMBERSHIP-003 was written to prevent. The members page
  would also need the raw boolean to decide whether to show **Delete team**.
- **A separate `TeamLifecyclePolicy`.** It is two policies for one aggregate, and each verb's caller
  must pick the right one.
- **Gate create through `TeamAccess`.** There is no team, and so no role. `TeamAccess::of(admin,
  None)` would answer `Administer` for an admin and `None` otherwise, which is the boolean itself
  wearing a policy's name. Kept as `is_workspace_admin`, the shipped precedent for workspace-level
  team settings.

## Consequences

- Positive: one table still answers every team verb. Delete's admin-only rule is one arm, pinned by
  the core example test.
- Positive: no shipped behaviour changes. `can_manage()` is true for exactly the same inputs as
  before.
- Negative: three rows of the shipped six-row test change their expected value (Manage →
  Administer). This is a test edit in slice 02, not a behaviour change.
- Neutral: if a future feature lets leads delete their own team, it is one arm.
