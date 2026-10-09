# Evolution: fix-hide-unreachable-boards (don't offer boards a member can't open)

**Finalized**: 2026-10-09 · **Commits**: roadmap `22117d2`, regression test `22ae6cf` (RED), fix `cf21e01` (GREEN). DES integrity: both steps complete. Mutation 20/24 (83.3%).

## Defect

On dev, the operator got a 404 on `/team/general/project/sandbox`. The operator signs in through Keycloak as `jeff@unintelligent-design.us`, which differs from the bootstrap account `jeffabailey@gmail.com`. So OIDC provisioning created a **second account**: a workspace member, but on no team. The dashboard's "Your projects" list and the rail Board link listed every project in the workspace (`list_projects_for_workspace`, no membership filter). The board itself is gated by team membership (`is_team_member`), which answers with the uniform 404. Invite-accept and OIDC provisioning add only a workspace membership, so every such member was offered boards they couldn't open.

The bootstrap account was never changed. It is still lead of team `general`, workspace admin and instance admin, and the board opens when signed in with it.

## Fix (user-chosen: "hide boards you can't open")

- `Store::list_projects_for_workspace(workspace_id, user_id)` now requires the user and joins `team_memberships`, so the unfiltered read is gone. A new `Store::is_on_any_team`.
- The dashboard and `nav::resolve_board_href` pass the session user. The rail Board link falls back to `/` when the list is empty.
- A member on no team sees "You're not on any team yet — ask a workspace admin to add you." and no create-project link. A team member whose teams have no projects keeps the existing copy.
- The instance-admin listings stay unfiltered. No migration.

Lanes: fix-hub 2/2, keycloak-sso 46/46 (expectations unchanged), navigation-bar 33/33, dashboard-enhancements 8/8, invites 30/30 + 18/18, default 896/896.

## Lessons

1. **A 404 that is correct can sit on a link that is wrong.** The gate was right; the UI advertised a different authorization model than the one enforced.
2. **SSO with a different email creates a new account.** Link-only matching by verified email can't relate the operator's Keycloak identity to the bootstrap account.

## Follow-ups

- **A way to join teams:** a workspace-admin "add to team" action, or a default-team policy for invite and provisioning. This is a product decision.
- **Show the signed-in account,** and let an operator link a Keycloak identity to an existing account.
- **Test gaps from mutation:**
  - the no-team state should assert the absence of the create link;
  - a scenario for a team member whose teams have no projects;
  - a rail Board-link check on a page other than `/` (this gap predates the fix).
