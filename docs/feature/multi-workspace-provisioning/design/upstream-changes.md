# Upstream Changes — multi-workspace-provisioning (DESIGN findings)

> Two DESIGN findings that refine inherited assumptions. Neither blocks DESIGN; both are recorded
> so DISTILL/DELIVER (and a future reader of the parent feature) do not rediscover or trip on them.
> Per the nw-design back-propagation contract: original quoted, new assumption + rationale stated.

## Finding 1 — Provisioning surface revised: parent ADR-004 chose web; this feature chooses CLI-first

**Original assumption** (parent `docs/feature/multi-workspace-tenancy/design/adr-004-instance-super-admin-role.md`, Decision):
> "**Surface: (d) a web flow under `/admin/instance/workspaces`** (session + CSRF), gated by
> `is_instance_admin`… (e) the CLI path is noted as a follow-up convenience, not v1-required."

**New assumption** (this feature, `adr-002-provisioning-surface.md`, D2):
The v1 provisioning surface is **the operator CLI** (`foundry doctor provision-workspace …`,
option e); the **web flow (option d) is DEFERRED** to a follow-up.

**Rationale**: grounding the shipped code surfaced that (i) `admin_cli.rs` already provides a
complete privileged-subcommand scaffold (`run_restore_comment`: live-DB sqlx, thread-isolated
runtime, structured exit codes) and (ii) `admin_cli` + `bootstrap` are ALREADY on the LAYER-1e
allow-list (`check_arch.rs:387-396`), so a CLI surface needs no new web authz tier, no new
templates/CSRF wiring, and no new check-arch allow-list entry. For a self-hosted single-operator
deployment, the CLI is the smaller attack surface and the faster v1. The web flow is preserved as a
tracked follow-up, not discarded. **Flagged for user ratification (wave-decisions.md, open decision
#1).**

## Finding 2 — The application-level 409 `create_workspace` guard is STILL PRESENT (evolution doc overstated the guard-drop)

**Original claim** (`docs/evolution/2026-06-11-multi-workspace-tenancy.md`, "What shipped"):
> "`0009_multi_workspace.sql` **drops `uniq_one_workspace`** … and removes the application 409
> guard"

**Actual code state** (`crates/foundry-app/src/bootstrap.rs:301-333`, `create_workspace`):
The handler STILL returns `409 CONFLICT` ("Only one workspace per instance") for any second
workspace, via `state.store.workspace_count()`. The DB index was dropped by `0009`; the
**application-level 409 handler was NOT removed**.

**New assumption** (this feature): `bootstrap.rs:301` `create_workspace` is the EXTEND/replace point
for provisioning (Reuse #1). For v1 the CLI provisioning path is the active surface; the web
`create_workspace` 409 is replaced (gated by `is_instance_admin`) only when the deferred web flow
lands (ADR-002).

**Rationale**: this is a documentation/code drift in the PARENT feature's evolution note, not a
defect in shipped behaviour (the 409 is harmless defense-in-depth today — no one can reach a
second-workspace web POST without it). Recorded so DELIVER does not assume the guard is already
gone, and so the parent evolution doc can be corrected if desired. **Do NOT modify the parent
feature's docs from this feature** — this note is the record.

## Impact
- No change to any inherited NFR or user story (US-MWT06/07/08 stand).
- DELIVER should expect the `bootstrap.rs:301` 409 to be present and treat it as the provisioning
  EXTEND point.
- The parent evolution doc's "removes the application 409 guard" line is inaccurate; correcting it
  is OPTIONAL and belongs to the parent feature's owner, not this feature.
</content>
