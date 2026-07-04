# Slice 02 — Instance-admin link (super-admin only)

**Goal**: a super-admin sees an "Instance admin" link on the dashboard; nobody else does.

**Stories**: US-03 (+ tests).

**IN scope**
- `dashboard_root` calls `Store::is_instance_admin(session user_id)`; `DashboardRoot` gains `is_instance_admin: bool`.
- Template conditionally renders a link to `/admin/instance/workspaces` in Quick actions.

**OUT of scope**
- The instance-admin surface itself (already shipped). Any other role-conditional nav.

**Learning hypothesis**: disproves "role-conditional nav needs nothing beyond the shipped
`is_instance_admin`" if that predicate is mis-scoped or insufficient. (Confidence high: predicate exists,
`lib.rs:1541`, `EXISTS(instance_admins WHERE user_id=$1)`.)

**Acceptance**: `acceptance-criteria.md` US-03 (2 scenarios: super-admin sees it; member does not — assert
absence from body, not CSS-hidden).

**Seams**: `Store::is_instance_admin` (`lib.rs:1541`); route `/admin/instance/workspaces` (`lib.rs:401`).

**Dependencies**: none (independent of slice 01). **Effort**: ~0.5 day.
