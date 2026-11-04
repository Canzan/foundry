# ADR-IW-002: The workspace-admin fact rides the sidebar's existing identity read

Status: Proposed (integrations-wayfinding DESIGN, 2026-11-03)

## Context

US-IW-01 shows Integrations in the sidebar to workspace admins on every
authenticated page. `NavContext` (`nav.rs`) carries only `is_instance_admin`.
`NavContext::home_for` and `board_for` already make two reads per render:
`dashboard_greeting` (display name and workspace name) and
`resolve_board_href`. The caller makes a third, `is_instance_admin`.

## Decision

Add `Store::nav_identity(user_id, workspace_id) -> Option<NavIdentity>` with
fields `{ display_name, workspace_name, is_workspace_admin }`. It is one query,
`dashboard_greeting`'s plus
`EXISTS (SELECT 1 FROM workspace_memberships WHERE workspace_id = $2 AND user_id = $1 AND role = 'admin')`.

- `home_for` and `board_for` read it instead of `dashboard_greeting`. On
  error they fall back to the neutral identity with `is_workspace_admin =
  false` (fail closed, as `resolve_is_instance_admin` does).
- `NavContext` gains `is_workspace_admin`, and `for_page` takes it. The
  `signin.rs` direct `for_page` callers pass the value they hold:
  `dashboard_root` already computes it (v0.18.1).
- `dashboard_greeting` is left in place for its other callers.

## Alternatives considered

- **Call `is_workspace_admin` inside `home_for` and `board_for`.** Simplest,
  but it adds one round trip on every authenticated page.
- **Store the role in the session at sign-in.** It goes stale when an admin is
  demoted (the session outlives the role), which breaks the "no link to a 404"
  guardrail (KPI-IW-G1) in exactly the case that matters.

## Consequences

- There is no extra round trip per page.
- The sidebar entry follows the role live, on the next render after a change.
- W-1 is not marked as the current page. The user-section items (Settings,
  Keyboard shortcuts, Instance admin) never are; Home stays current on those
  pages (navigation-bar-linear-ui FR-4: exactly one primary item current). This
  changes US-IW-01 AC 3 (see `docs/feature/integrations-wayfinding/design/upstream-changes.md`).
