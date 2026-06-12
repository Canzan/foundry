# ADR-002 — Provisioning surface (CLI vs web vs both)

## Status
**IMPLEMENTED** (2026-06-12; ratified 2026-06-11). Shipped in DELIVER phase 03 (steps 03-01..06):
`foundry doctor provision-workspace` is the v1 surface, `is_instance_admin`-gated, with structured
exit codes and proven off the bearer surface. The web `/admin/instance/…` flow remains **DEFERRED**.
FIRMS the surface half of the parent `multi-workspace-tenancy` ADR-004 (which proposed a web flow);
**this ADR overrides the parent's surface choice for v1** (CLI-first) — see "Relationship". See the
evolution doc.

## Context
OD-3 ratified instance-operator/super-admin-only provisioning, NO self-serve. The question is the
SURFACE the super-admin uses to create a workspace + seed its first admin. The deployment shape is
**self-hosted, single operator** (the parent feature's invariants: one binary, one Postgres, no
multi-team org chart). The parent ADR-004 proposed a web `/admin/instance/workspaces` flow; this
ADR re-evaluates with the operational-safety lens and the grounded finding that an operator-CLI
home already exists and is already allow-listed.

Grounding (read the code):
- `admin_cli.rs` is the `foundry doctor …` operator-CLI home. `run_restore_comment`
  (`admin_cli.rs:235-371`) is a complete precedent for a privileged subcommand that connects to
  the live DB via `DATABASE_URL`, runs sqlx in a thread-isolated tokio runtime, and returns
  structured exit codes — exactly the scaffold a `provision-workspace` subcommand needs.
- The LAYER-1e tenant-scoping guard ALREADY allow-lists `admin_cli` and `bootstrap`
  (`check_arch.rs:387-396`) — a CLI surface needs no new guard entry.
- A web flow would require a NEW authz tier (`is_instance_admin`-gated routes under
  `/admin/instance/…`), new Askama templates, CSRF wiring, and — if it lands in a new file — a
  new LAYER-1e allow-list entry.

## Options considered
- **(d) Web flow `/admin/instance/workspaces`** (session + CSRF, gated by `is_instance_admin`).
  Best UX for a non-shell operator; reuses the admin UI idiom. But: adds a new web authz tier and
  attack surface, new templates/CSRF wiring, and (if a new file) a new LAYER-1e allow-list entry —
  more surface for a once-in-a-while operator action.
- **(e) CLI subcommand `foundry doctor provision-workspace …`** (operator shell, `DATABASE_URL`).
  Smallest attack surface (shell access already implies host-level trust); off the bearer surface;
  reuses the `run_restore_comment` scaffold verbatim; ALREADY allow-listed. No new web tier. Lower
  UX for a non-technical operator, but provisioning is an infrequent operator act on a self-hosted
  box, not an end-user feature.
- **(f) `/api/v1` provisioning endpoint.** REJECTED (carried from parent ADR-004): puts a
  mint-like creation path on the bearer surface, against the no-mint boundary (`api≠mint`
  check-arch rule).
- **(g) Both CLI and web in v1.** Doubles the implementation + test surface for the first
  release; the web flow is the larger, riskier half. No need to build both at once.

## Decision
**(e) CLI-first for v1: `foundry doctor provision-workspace --name <name> --admin-email <addr>
[--as <super-admin-email>]`.** It resolves and verifies the calling super-admin via
`is_instance_admin` (fail-closed), calls the `create_workspace` use-case (ADR-003), and prints the
new workspace id + the first-admin invite link (reusing the bootstrap/invite seeding idiom). The
**web `/admin/instance/…` flow (option d) is DEFERRED** to a follow-up; if/when it lands it is
gated by `is_instance_admin` + CSRF and, if in a new file, added to the LAYER-1e allow-list.
Option (f) stays rejected.

## Consequences
- **Positive**: smallest possible attack surface (matches OD-3's operator-only intent — shell
  access already implies the host trust a super-admin needs); no new web authz tier in v1; reuses
  the `run_restore_comment` scaffold and the existing allow-list (zero new check-arch entry);
  provisioning stays off the bearer surface; ships faster.
- **Negative**: a non-shell operator cannot provision from a browser in v1 (acceptable: self-hosted
  single-operator shape; the web flow is a tracked follow-up). The CLI requires `DATABASE_URL`
  (same as `restore-comment`).
- **Security**: provisioning is reachable only by someone with host shell access AND a super-admin
  `instance_admins` row (the use-case re-checks `is_instance_admin`); never over `/api/v1`; the
  `api≠mint` boundary is preserved.

## Relationship to parent ADR-004
The parent ADR-004 proposed the WEB flow (option d) as the v1 surface. This ADR **revises that to
CLI-first (option e)** on operational-safety grounds, having grounded (i) the existing allow-listed
`admin_cli` precedent and (ii) the larger surface a web tier adds. The web flow is preserved as a
deferred follow-up, not discarded. This revision is flagged for user ratification.
</content>
