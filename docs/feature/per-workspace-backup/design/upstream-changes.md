# Upstream Changes / Drift: per-workspace-backup

> Discrepancies between DISCUSS-wave assumptions and the SHIPPED code, surfaced during DESIGN's
> existing-system analysis. None blocks the design; each is reconciled with a recommended resolution.

## DRIFT-1 (HIGH for acceptance-designer) — `workspaces` has NO `slug` column

**DISCUSS assumes:** the user stories and acceptance criteria repeatedly select workspaces by slug
(`globex`, `acme`) and `list-workspaces` "shows id + slug + name" (FR-1, AC-01.1, US-PWB-01 examples).

**Shipped reality:** `crates/foundry-store/migrations/0001_init.sql` defines
`workspaces (id, name, created_at)` — NO `slug`. Only `teams` and `projects` have a `slug` column.
`Store::list_workspaces()` returns `(id, name)` only (`crates/foundry-store/src/lib.rs:561`).

**Impact:** `list-workspaces` cannot show a slug; `export-workspace <slug>` cannot resolve a slug
column.

**Recommended resolution (no schema change):** the selector accepts `<id>` OR an exact, case-
insensitive `<name>` match. `list-workspaces` prints `id` + `name` (the columns that exist). The ACs'
literal `globex`/`acme` tokens are treated by acceptance-designer as the SELECTOR token (id or name),
NOT as a `slug` column lookup. Ambiguous name -> exit 2 listing the matches. This keeps the feature
read-only (no `0012` migration to add a slug).

**Alternative (deferred, NOT recommended for v1):** add a `workspaces.slug` column via a forward-only
additive migration + backfill from name. Rejected for v1 — it turns a read-only feature into a
schema-changing one and is unnecessary (id + name selection satisfies every AC's intent).

**Action:** acceptance-designer to interpret slug tokens as selector tokens; note in the `.feature`
that selection is by id-or-name. No code/schema change to workspaces.

## DRIFT-2 (MEDIUM, already flagged in shared-artifacts-registry) — `comments` / `team_memberships` scoping

**DISCUSS assumes:** `comments` and `team_memberships` reach the workspace "only transitively"
(US-PWB-02 example 2, AC-02.3).

**Shipped reality:** `comments` HAS a denormalized direct `workspace_id` column
(`0004_comments.sql:21`). Only `team_memberships` is genuinely transitive-only (via `team_id`, no
`workspace_id` — `0001_init.sql:43`).

**Impact:** the transitive-isolation AC (AC-02.3) is still valid and necessary, but `comments` is
scoped DIRECTLY by export; the transitive check on `comments` becomes a CROSS-CHECK (does the
denormalized `comments.workspace_id` agree with `issues.workspace_id` via `comment.issue_id`?) rather
than the sole resolution path.

**Recommended resolution:** keep AC-02.3's transitive isolation check — apply it to
`team_memberships` (genuinely transitive) and AS A CROSS-CHECK to `comments` (catch a denormalized
`workspace_id` that disagrees with the issue's — itself a corruption worth detecting). Documented in
architecture.md Section 5. No schema or AC rewrite needed; acceptance-designer notes the cross-check.

## DRIFT-3 (LOW, informational) — the `admin_cli` backup-verify table list differs

**Shipped reality:** `admin_cli.rs::run_backup_verify` counts `[workspaces, users, teams,
team_memberships, workspace_memberships, projects, issues, comments, issue_attachments, session,
outbox]` — it INCLUDES `issue_attachments`, `session`, `outbox` and OMITS `invites`, `machine_tokens`.

**Impact:** the per-workspace tenant-table set (the 10 from slice-05 `TENANT_TABLES`) MUST NOT be
copied from `run_backup_verify`. They serve different purposes (whole-instance restore-verify vs.
per-tenant export). `issue_attachments` is notably ABSENT from the slice-05 `TENANT_TABLES` set.

**Recommended resolution:** pin the per-workspace set as its own constant (OD-PWB-2 / ADR-005),
gold-tested, explicitly NOT sourced from `admin_cli.rs`. Documented.

**Open question for DISTILL (LOW):** `issue_attachments` exists in the schema (`0005`) and the
backup-verify list but is NOT in the slice-05 `TENANT_TABLES` (10). If issue attachments are tenant
data that should travel with a workspace export, the set may need an 11th table. **Recommended:
follow slice-05's authoritative 10-table set for v1** (it is the ratified tenant surface for the
migration-guarantee proof); flag `issue_attachments` inclusion as a possible follow-up if operators
report attachments missing from exports. The gold test will make any future addition explicit.
