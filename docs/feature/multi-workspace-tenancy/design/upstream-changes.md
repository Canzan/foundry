# Upstream Changes — Multi-Workspace Tenancy (DESIGN → DISCUSS)

> Per the DESIGN back-propagation contract: when a DESIGN finding refines or contradicts a DISCUSS
> assumption, flag it here with the original quoted verbatim, rather than silently diverging.
> **Neither finding contradicts a ratified OD; both REFINE the clean-migration claim** and are cheap
> to absorb. Surfaced now (DESIGN) rather than discovered at DELIVER.

## Finding 1 — There is a SECOND, application-level single-workspace guard beyond the DB index

### Original DISCUSS assumption (quoted verbatim)
From `docs/feature/multi-workspace-tenancy/discuss/wave-decisions.md` (Feature Summary + DM1):
> "Foundry is SINGLE-workspace today, enforced by `CREATE UNIQUE INDEX uniq_one_workspace ON
> workspaces ((true))` (`crates/foundry-store/migrations/0001_init.sql:15`)."
> "**Drops the single-workspace guard** and adds a workspace-resolution seam … the walking skeleton."

The DISCUSS framing consistently treats `uniq_one_workspace` as THE guard — "the single-workspace
guard" (singular) — implying that dropping the index is what makes a second workspace possible.

### The DESIGN reality (read from the 2026-06 code)
`crates/foundry-app/src/bootstrap.rs:289` `create_workspace` carries a SECOND, application-level
guard that returns **409 Conflict** for any second workspace, independent of the DB index, with the
comment:
> "Slice-1 MVP supports exactly one workspace per instance … We keep the single-workspace guard
> even after the unique index makes a second INSERT impossible, as a defence-in-depth /
> boring-monolith taste filter (cheap human-readable 409 …)."

So dropping `uniq_one_workspace` alone does NOT make a second workspace creatable — the handler
still refuses with 409 before reaching the DB.

### New assumption / rationale
The migration (US-MWT00, Slice 1) drops `uniq_one_workspace` AND the provisioning work (US-MWT07,
Slice 6) must REPLACE the hard-coded 409 in `create_workspace` with the real, `is_instance_admin`-
gated creation flow (ADR-004). The walking skeleton's "a second `workspaces` row can be created"
acceptance must create the row via a path that does NOT hit the 409 (e.g. the test fixture / the
new use-case), and the standalone 409 is removed as part of provisioning. **No story changes** — this
is an implementation reality DELIVER must honor; it is captured here so the slice-1 "second workspace
exists" check and the slice-6 provisioning flow account for the application guard, not just the index.

## Finding 2 — Two un-scoped single-row tenant reads (refines audit assumption #6)

### Original DISCUSS assumption (quoted verbatim)
From `discuss/wave-decisions.md` "Assumptions about the current single-workspace code", #6:
> "**No code currently depends on `uniq_one_workspace` for correctness** (e.g. a query that assumes
> 'the one workspace'). Assumed all reads already filter by `workspace_id`; a grep for un-scoped
> `FROM teams|projects|issues` is a DESIGN/DELIVER guard to confirm before dropping the index."

And NFR-MWT-DATA-03:
> "No query relies on 'there is exactly one workspace' for correctness; every tenant-scoped query
> filters by an explicit `workspace_id`."

### The DESIGN reality (the pre-drop audit, ADR-006)
The audit CONFIRMS the core claim — no query depends on `uniq_one_workspace` for correctness (the
only "the one workspace" query is `first_workspace()`/`SELECT … FROM workspaces LIMIT 1`, whose
sign-in call-site is replaced by membership resolution, ADR-005). BUT two single-row tenant reads
are **un-scoped** (key only by primary key, no `workspace_id`):
- `SELECT expires_at FROM invites WHERE id = $1` (`foundry-store/src/lib.rs:427`)
- `SELECT name FROM teams WHERE id = $1` (`foundry-store/src/lib.rs:546`)

Under one workspace these are harmless (every row is in the one workspace). Under multiple tenants
they are un-scoped tenant reads that the NEW tenant-scoping guard (ADR-002) would flag.

### New assumption / rationale
These two reads should be **workspace-scoped** before/with the boundary work: add `AND workspace_id
= $2` to the invite lookup; resolve the team name within the acting workspace (or scope the lookup).
This does NOT contradict assumption #6 (nothing depends on the guard) — it REFINES it: "every
tenant-scoped query filters by `workspace_id`" is the TARGET, and two reads currently miss it. Cheap
to fix; folds naturally into Slice 2 (web boundary, where the team-name read lives) and the
invite/provisioning work (Slice 6). **No story changes** — NFR-MWT-DATA-03's verify step ("a code-audit
for un-scoped reads before the guard is dropped") is exactly this; the audit is done, and these are
the two it found.

## Summary for the product owner
- Both findings are **refinements, not contradictions**; no ratified OD changes; no user story
  changes are required.
- DELIVER must: (1) remove the application-level 409 in `create_workspace` as part of provisioning
  (not just drop the DB index); (2) scope the two un-scoped reads by `workspace_id`.
- Both are captured as **OD-MWT-D9** in `wave-decisions.md` and are accounted for in the Slice 1/2/6
  component mapping.
