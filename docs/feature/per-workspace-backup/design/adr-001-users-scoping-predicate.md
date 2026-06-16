# ADR-001: The `users` scoping predicate under multi-membership (OD-PWB-1)

## Status
Accepted

## Context
The export must select "all of workspace W's tenant data, none of any sibling's" (NFR-PWB-ISO-01).
Eight of the ten tenant tables resolve to W cleanly (direct `workspace_id` or transitive `team_id`).
`users`, however, is a GLOBAL identity table (`0001_init.sql:17`) with NO `workspace_id`: a single
user may be a member of Acme AND Globex (the shipped multi-membership model, `resolve_active_workspace`
honours `active_workspace_id` across memberships). So "which users belong to W?" is genuinely
ambiguous, and a naive isolation rule ("every archived row resolves to exactly W") would flag a
multi-membership user as a cross-tenant leak.

## Decision
Define the `users` scope **membership-bounded**:

- Export includes the `users` rows where `id IN (SELECT user_id FROM workspace_memberships WHERE
  workspace_id = W)`, plus the `workspace_memberships` edges for W.
- Verify confirms each archived `users` row IS a member of W. It does NOT fail because that user is
  also a member of a sibling workspace.
- Isolation in the strict "resolves to exactly W, zero siblings" sense applies to the
  workspace-OWNED resources (tables 1-9), never to the shared `users` identity.

## Alternatives Considered
1. **Strict "every row resolves to exactly W" for `users` too.** Rejected: would red on any
   multi-membership user, making a perfectly valid export of W unverifiable whenever a user belongs
   to two workspaces. Also wrong domain semantics — the user IS a legitimate member of W.
2. **Omit `users` from the export; ship only membership edges + user ids.** Rejected: the archive
   would NOT be self-contained — a recipient could not reconstruct W (no display names, no
   `password_hash` for a migration). The whole point is a faithful, standalone tenant dump.
3. **Strip multi-membership users down to a W-only projection (e.g. blank other-workspace data).**
   Rejected: `users` rows carry no per-workspace columns — there is nothing workspace-specific to
   strip. The row IS the global identity.

## Consequences
- Positive: matches the shipped multi-membership domain model; a valid W export always verifies;
  the archive is self-contained.
- Positive: the isolation crux stays sharp where it matters (owned resources) without false leaks.
- Negative: a W archive may contain a user identity (incl. `password_hash`) for someone who also
  belongs to a sibling — surfaced by the NFR-PWB-SEC-01 sensitivity note (operator-trust artifact).
- Negative: verify needs TWO predicate shapes (owned-by-W for 1-9, member-of-W for `users`) — the
  ONE place the predicate is not uniform; documented explicitly in architecture.md Section 5.
