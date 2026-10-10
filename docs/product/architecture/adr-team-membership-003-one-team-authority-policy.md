# ADR-TEAM-MEMBERSHIP-003: Team-page authority is one pure policy in `foundry-core`, fed by one store read and applied only inside the `foundry-services` team use-cases

- Status: Accepted (2026-10-09)
- Feature: `team-membership` (DISCUSS D2, D5, D9, D10, D11, OQ-1, OQ-2, INV-6; DESIGN DDD-3 to DDD-5, DDD-19)

## Context

The team pages add a new authority surface. A workspace admin acts on teams they are not on, and
the `lead` role grants power for the first time (slice 04). Viewing a members page needs "admin
or on the team". Managing needs "admin or lead of this team". Board access stays "on the team"
(D9), and the boards' `is_team_member` gate is untouched. Every refusal is the uniform 404 (D5).
Authority is read per request from current rows (INV-6). Instance admins get nothing implicitly
(OQ-1).

Four verbs (view, add, change role, remove) must use the same predicate, and slice 04 widens it
for all of them at once. Shipped precedents: `foundry_services::tokens` applies
`is_workspace_admin` inside the use-case, and `Services::create_project` keeps the gate in the
handler (ADR-PROJECT-NAME-001).

## Decision

1. **Pure policy.** `foundry_core::team_membership` holds:
   - `TeamRole { Lead, Member }`, parsed from the stored string, with an unknown string refused.
   - `TeamAccess { None, View, Manage }`, from one total function
     `TeamAccess::of(is_workspace_admin: bool, role: Option<TeamRole>)`.

   The table is `(true, _) → Manage`, `(false, Some(Lead)) → Manage`, `(false, Some(Member)) →
   View`, `(false, None) → None`. Until slice 04, the `Some(Lead)` arm maps to `View`. Slice 04
   changes that one arm. It has no I/O and is example-tested over all six inputs.
2. **One fact read.** `Store::team_access_facts(workspace_id, team_slug, user_id) ->
   Option<TeamAccessFacts { team_id, team_name, is_workspace_admin, role }>` is one query that
   resolves the team by (session workspace, slug) and reads both facts. `None` means there is no
   such team in this workspace.
3. **Applied in one place.** Each `foundry_services::teams` use-case (view members, add, change
   role, remove) starts with the fact read, then the policy, and returns
   `TeamError::NotFound` for an unknown team or insufficient access alike. Handlers map
   `NotFound` to `resource_not_found_page()` and never compute authority themselves. `GET /teams`
   and `POST /teams/default` use the shipped `is_workspace_admin`: the index scope is "all teams"
   for an admin and "my teams" otherwise, and the default is admin-only.
4. **Enforcement.** check-arch `team-membership-one-seam` clause (b): the store's team-write
   methods have no call site under `crates/{foundry-app,foundry-api}/src`, so a write cannot skip
   the use-case and its policy. Clause (a): SQL that writes `team_memberships` appears in no
   `.rs` outside `crates/foundry-store/src`. The compile-time layer: the policy returns an enum
   that the use-cases match exhaustively.

## Alternatives considered

- **Gate in each handler, as project create does.** Rejected: there are four routes, and slice 04
  must widen all of them together. Four copies of "admin or lead" would drift. The project-create
  gate is one membership check on one route.
- **A store method returning a ready-made `can_manage` boolean.** Rejected: it hides the policy
  in SQL, where it is testable only against Postgres, and splits the view/manage distinction into
  two queries or two booleans.
- **An axum extractor that resolves `TeamAccess` for the `/team/{slug}/members…` routes.**
  Viable. Rejected for now: it would put authority in the HTML adapter, where `foundry-api` could
  not reuse it, and an extractor's rejection path makes byte-identical 404s harder to pin than a
  handler-level `match`.

## Consequences

- Positive: one six-row truth table defines who may view and manage. Slice 04 is a one-arm change
  plus scenarios.
- Positive: board authz is untouched (D9). An admin's `Manage` grants nothing on
  `/team/{slug}/project/…`.
- Negative: one extra read per team page (the facts query). This is negligible at homelab scale.
- Neutral: a lead may remove or demote a workspace admin who is on their team. Leads are peers
  (OQ-2), and the workspace admin can reverse it.
