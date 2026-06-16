# Journey: Export a Single Workspace — Visual

> Operator data-export tooling (OD-5 / DD-MWT-09, deferred from multi-workspace-tenancy).
> Persona: **Devansh**, the self-hosting operator (the same persona who runs `foundry doctor
> backup-verify` today). SUBAGENT-authored; grounded in the SHIPPED `foundry doctor` privileged-CLI
> scaffold (`crates/foundry-app/src/admin_cli.rs`) and the SHIPPED tenant-scoping seam
> (`TENANT_TABLES`, `workspace_id` direct + transitive FK chains).

## The problem in one line

Today Devansh can back up the **whole instance** (`pg_dump -Fc`) but cannot lift **one tenant's data
out** — to archive a churned customer, hand a workspace its own data, migrate one tenant to another
instance, or take a pre-deletion snapshot. The whole-instance dump mixes every workspace together.

## Scope boundary (read first)

```
+----------------------------------------------------------------------+
| v1 = EXPORT ONLY.   per-workspace RESTORE / import is OUT (deferred). |
|                                                                      |
| Why: DD-MWT-09 — a per-workspace restore must NOT clobber a sibling, |
| an isolation-SENSITIVE write path that is the meaningfully-harder    |
| half. Export is the safe, valuable, shippable slice: archive /       |
| migrate / data-portability / pre-deletion snapshot. It only READS.   |
+----------------------------------------------------------------------+
```

## Emotional arc — Confidence Building (anxious -> focused -> trusting)

```
   START                         MIDDLE                          END
   "Did I grab the              "It's walking the              "One archive. ONLY
    right workspace?            tenant tables...                 this tenant. And I
    Will it leak another        is it really only               can PROVE it — the
    tenant's data?"             THIS one?"                       verify says OK."
        |                            |                                |
     anxious  ----------------->  focused  -------------------->  trusting / relieved
   (isolation fear)            (transparent progress)       (self-verifiable, isolation-clean)
```

The peak tension is the **isolation fear** — an operator handing a workspace's archive to a departing
customer must KNOW no sibling tenant's rows rode along. The journey is designed so the export is
*self-verifiable* for both completeness and isolation, converting that fear into earned trust (the
same "Earned Trust" / Principle 9 spirit as `backup-verify`'s per-attachment sha256 check).

## Happy path — ASCII flow

```
[Trigger: operator needs ONE tenant's data]
        |
        v
+-- Step 1: Identify the target workspace ------------------------------+
|  $ foundry doctor list-workspaces                                     |
|  workspace-id                            slug        name             |
|  0190a1b2-...-acme                       acme        Acme Corp        |
|  0190c3d4-...-globex                     globex      Globex LLC       |
|  status: OK                                                           |
|  Feels: oriented — I can see exactly which id/slug is which          |
+----------------------------------------------------------------------+
        |
        v
+-- Step 2: Run the export --------------------------------------------+
|  $ foundry doctor export-workspace globex /backups/globex-2026-06-16.dump
|  workspace-id: 0190c3d4-...-globex                                    |
|  workspace-slug: globex                                               |
|  out-path: /backups/globex-2026-06-16.dump                           |
|  tables-exported: 10                                                  |
|  row-counts:                                                          |
|    workspaces: 1                                                      |
|    users: 7            <- members of THIS workspace only             |
|    workspace_memberships: 7                                          |
|    teams: 3                                                          |
|    team_memberships: 11                                              |
|    projects: 8                                                       |
|    issues: 412                                                       |
|    invites: 2                                                        |
|    comments: 1893                                                    |
|    machine_tokens: 4                                                 |
|  status: OK                                                          |
|  Feels: focused — transparent per-table counts, it's only this tenant|
+----------------------------------------------------------------------+
        |
        v
+-- Step 3: Verify the archive (completeness + isolation) -------------+
|  $ foundry doctor verify-export /backups/globex-2026-06-16.dump      |
|  out-path: /backups/globex-2026-06-16.dump                           |
|  archive-format: foundry per-workspace export v1                     |
|  declared-workspace: 0190c3d4-...-globex                             |
|  checks:                                                             |
|    archive is readable: YES                                          |
|    all 10 tenant tables present: YES                                 |
|    every row belongs to the declared workspace: YES  <- isolation    |
|    no rows reference a sibling workspace: YES        <- the crux      |
|  status: OK                                                          |
|  exit-code: 0                                                        |
|  Feels: trusting — I can hand this off; it's complete AND isolation-clean
+----------------------------------------------------------------------+
        |
        v
[Goal: a portable, verifiable, isolation-scoped archive of exactly one tenant]
```

## Failure paths — guide to resolution, never add frustration

```
+-- F1: Unknown workspace ---------------------------------------------+
|  $ foundry doctor export-workspace nope /backups/x.dump              |
|  foundry doctor export-workspace: no workspace matches "nope"        |
|  (looked up by id and by slug). Run `foundry doctor list-workspaces` |
|  to see available workspaces.                                        |
|  exit-code: 2 (invalid argument)                                     |
|  Feels: redirected, not blocked — told exactly how to find the id    |
+----------------------------------------------------------------------+

+-- F2: Output path unwritable / parent missing -----------------------+
|  $ foundry doctor export-workspace globex /nope/x.dump               |
|  foundry doctor export-workspace: cannot write to "/nope/x.dump":    |
|  parent directory does not exist. Create it or choose another path.  |
|  exit-code: 5 (output error) — NOTHING was read/exported             |
|  Feels: safe — it failed BEFORE touching the DB; no half-archive     |
+----------------------------------------------------------------------+

+-- F3: Partial write (disk fills / killed mid-export) -----------------+
|  Export writes to "<out>.partial", fsyncs, then atomically renames to |
|  "<out>". A killed/failed export leaves NO file at <out> (only a       |
|  discardable .partial). verify-export on a .partial-only path:        |
|  foundry doctor verify-export: archive is truncated or incomplete;    |
|  re-run the export. exit-code: 4                                      |
|  Feels: protected — a partial export can never masquerade as complete |
+----------------------------------------------------------------------+

+-- F4: Last / only workspace (single-tenant install) ------------------+
|  Exporting the sole workspace is VALID (archive / pre-migration       |
|  snapshot). It is NOT a delete — export only READS, removes nothing.  |
|  status: OK with a note: "this is the only workspace on the instance".|
|  Feels: reassured — export never implies deletion                    |
+----------------------------------------------------------------------+

+-- F5: DB unreachable -------------------------------------------------+
|  foundry doctor export-workspace: could not connect to DATABASE_URL.  |
|  exit-code: 3 (DB/infra fail) — mirrors the shipped scaffold          |
+----------------------------------------------------------------------+
```

## At-rest sensitivity warning (surfaced in the journey, not hidden)

The export contains `users.password_hash` (argon2id) and `machine_tokens` rows — the same sensitive
columns the whole-instance `pg_dump` already contains. The export is an **operator-trust artifact**:
its at-rest protection (encryption, transport, retention) is the operator's responsibility, exactly
as the shipped backup doc states for the whole-instance dump. The CLI prints a one-line reminder on
success so the operator is never surprised by what the archive holds.

```
note: this export contains password hashes and machine-token rows for
this workspace's members. Treat it as sensitive at rest (encrypt /
restrict / rotate per your policy), as you would the whole-instance dump.
```

## Why CLI (material honesty)

Shell access ⇒ host trust; the surface stays OFF the bearer API (consistent with the CLI-first
provisioning decision DD-MWT / ADR-002). A privileged data-extraction operation belongs next to its
siblings — `backup-verify`, `restore-comment`, `provision-workspace`, `grant-super-admin` — under
`foundry doctor`, with the same structured `key: value` stdout + structured exit codes the operator
already greps in cron.
