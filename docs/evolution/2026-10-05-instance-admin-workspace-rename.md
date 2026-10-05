# Evolution: instance-admin-workspace-rename (fix a workspace's name from the dashboard, on record)

**Finalized**: 2026-10-05
**Commits**:
- DISCUSS+DESIGN: `bac7e7f`.
- DISTILL: `ef95bc6` (23 `@pending` scenarios, 10 store scaffolds).
- DELIVER roadmap: `4194293`.
- DELIVER steps: `6954c45` → `9191ec3` (6 DES-monitored steps). Test-step fixes: `023ec2d`, `68d9b19`.
- Execution log: `e4a04c0`.
- Use-case test and mutation report: `7d3184f`.

Gates:
- DES integrity: all 6 steps complete.
- The end-of-DISTILL review (product owner, architect, acceptance) and the roadmap review approved.
- Adversarial review approved, with 1 low-severity note.
- Mutation: **14/14 viable killed (100%)**.

Feature dir PRESERVED.

**Scope**: instance super-admins rename a workspace's display name in place from
`/admin/instance/workspaces`. It is an htmx head swap with `_csrf`, and refusals return 422 inside the row. Every effective rename appends one row to a new
`workspace_rename_events` table (migration **0018**) in the same transaction. The sidebar
brand now stays on one line with an ellipsis and a `title`. No new dependency, no fake.

## Business context

Priya Raman, the instance operator, provisioned workspaces under names that were right on the day
and wrong a month later. The only fix was a hand-typed `UPDATE workspaces SET name…` in `psql`,
with no validation and no trace. Long legacy names also wrapped the sidebar brand onto two or three
lines. This feature makes the rename a product action with a record of who changed what, and when.

## Key decisions

- **D1/D2**: instance admins only. `POST /admin/instance/workspaces/{workspace_id}/rename` under the
  CSRF middleware and session layer. Non-admin, signed-out, garbled-id and unknown-id requests all
  get the byte-identical uniform 404.
- **D3**: trim, non-empty, at most **24 Unicode scalars**. The cap comes from the measured sidebar
  width (about 164px of text), lowered from 32 when OQ-2 was resolved.
- **D4**: a byte-equal resubmission is a quiet 200 with no write and **no audit row**. A case-only
  change is a real, audited rename. The no-op check precedes the 422 gates, so an over-long legacy
  name can be left as it is.
- **D5 / DDD-2,3,5 (ADR-WORKSPACE-RENAME-001)**: one transaction does `SELECT … FOR UPDATE`, re-checks
  the no-op under the lock, then `UPDATE` plus `INSERT`.
  - `old_name` comes from the locked read.
  - `CHECK (old_name <> new_name)` backs the no-op rule in the schema.
  - Record time is the database's `DEFAULT now()`.
- **D6**: the sidebar brand is a single line with an ellipsis plus a `title` holding the full name,
  at desktop width and at ≤480px.
- **D7**: no uniqueness rule; workspaces are identified by id. **D8**: display name only (no slug,
  so no URL hazard).
- **D10**: no `CHECK` on `workspaces.name`. The cap lives in the rename path only.
- **DDD-6 (ADR-WORKSPACE-RENAME-002)**: rename records are instance records and stay **out** of the
  per-workspace export. `TENANT_TABLES` stays at ten. The actor is usually a non-member and would
  break the archive's membership-bounded `users` closure.
- **DDD-11**: `Store::probe` refuses a schema without the 0018 table, so `/readyz` fails instead of
  the first rename returning 500.

## Steps completed (6/6, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Migration 0018, `Store::rename_workspace_with_audit`, probe extension; 10 store tests un-ignored | `6954c45` |
| 02-01 | `foundry_services::workspaces` classifier (proptests + 24/25 and multi-byte pairs), handler, route, head partial | `57d4ac0` |
| 02-02 | Validation edges, duplicate names allowed, markup escaping, sidebar `title` | `891a065` |
| 02-03 | Authorization refusals, member sidebar, project-row and backup guards | `0254660` |
| 03-01 | Browser: the head swaps in place; repeated refusals stay in the row | `4dfdf18` |
| 04-01 | Sidebar ellipsis CSS plus re-hash to `foundry.6ce0e2c2.css`, in one commit | `9191ec3` |

Pre-push CI: `cargo xtask ci` with `FOUNDRY_XTASK_INCLUDE_DOCKER=1` (2026-10-05): fmt, clippy, check-arch, release build, workspace tests and cargo-deny all green. Acceptance (all tags, including browser and docker-compose): 937/938 scenarios. The one failure was the known sqlx `'\0'` pool flake in a us-mt01 setup step, and `us-mt01` alone then passed 8/8.

Final lanes:
- iawr: 27/27 scenarios (215 steps), including 4 real-Chrome scenario runs.
- iapr: 21/21 scenarios.
- Default: 708/708 scenarios.
- Browser: 216/217 scenarios. The one failure was a card-pointer-drag timing flake under load; its tag passed 18/18 alone.

## Lessons

1. **A "no error" check written as a bare substring can never pass when the design reuses the
   marker as an id prefix.** The success head legitimately carries `id="workspace-rename-error-{id}"`,
   so `!body.contains("workspace-rename-error")` failed on every correct answer. It also meant the
   refusal check could pass on a success. The crafter escalated instead of disguising the id. Both
   checks now match the attribute `data-hx-fragment="workspace-rename-error"` (`023ec2d`). Assert on
   the attribute that carries the meaning, not on a word that may recur.
2. **"Only the head re-renders" was proved on the wrong row.** The browser step counted Canzan Labs'
   project rows, but the renamed workspace (Bailey Family) has no projects. A whole-row re-render
   would have passed. Fixed with a DOM node-identity check: expando-tag the row, its non-head
   children and the head before submitting, then assert that the row and children survive and the
   head is new (`68d9b19`). A guard needs a subject the fault can actually touch.
3. **Defence-in-depth needs its own test.** Mutation testing showed the service's `is_instance_admin`
   re-check was covered only indirectly, because the handler refuses non-admins first. Deleting the
   guard passed every suite. `rename_workspace_use_case.rs` now pins it at the service boundary
   (`7d3184f`). cargo-mutants never generates "delete the whole `if`", so a 100% score did not
   cover it. Read the surviving shape, not just the number.
4. **Probe extensions ripple into `probe_schema_scoping.rs`.** Its hand-built "healthy" schemas must
   gain each new table, as with 0007, 0008 and 0017. The roadmap omitted the file; next time list it.
5. **Much of DELIVER was enablement.** Steps 02-02 to 03-01 found the 02-01 code already met their
   criteria. The crafters reported GREEN_ALREADY honestly, and each deliberate break (the authz bypass, the
   CSRF exemption, the slot-consuming swap) showed the scenarios bite.

## Measured KPIs (no kpi-contracts.yaml in this repo; recorded here)

- **KPI-1** (renames via the UI): the UI path exists, and the dashboard → retype → head swap journey is
  demonstrated in real Chrome (scenario 21). The baseline was 0% (no path).
- **KPI-2** (exactly 1 audit entry per effective rename, 0 for no-ops and refusals): pinned by
  scenarios 1, 3–12 and 14–18 through the fail-closed universe delta, and by the store bounded-change
  and no-op tests.
- **KPI-3** (0 wrapped or overflowing brands): pinned by scenario 23. A 52-character name is one line
  at desktop (it was 63px, three lines) and at 390px (it was 42px, two lines), with no sideways page scroll.

## Permanent artifacts

- `docs/product/architecture/adr-workspace-rename-001-audited-rename-transaction.md`
- `docs/product/architecture/adr-workspace-rename-002-rename-audit-not-tenant-export.md`
- `docs/product/architecture/brief.md`: the workspace-name paragraph under "Names are labels"
- `docs/product/jobs.yaml`: `job-instance-workspace-rename`
- `docs/product/outcomes/registry.yaml`: OUT-17 (`related: [OUT-1]`)
- `docs/feature/instance-admin-workspace-rename/`: the full wave history, including
  `deliver/mutation/mutation-report.md`

## Open / deferred

- **Follow-up B** (`instance-workspace-name-rule`): the same 24-character rule for provisioning,
  bootstrap and the admin CLI through one shared validator. Then **follow-up D**:
  `CHECK … NOT VALID` on `workspaces.name`.
- **Interior control characters** (for example a newline inside a name) are accepted, as project
  rename already accepts them. They are escaped everywhere, so this is cosmetic, not a security issue.
  Rejecting them would be a new rule for both renames; it is a candidate for follow-up B.
- **Lock-test timing**: `the_old_name_on_record_is_the_one_read_under_the_lock` waits 500ms. If it
  ever flakes on a loaded CI host, replace the sleep with a lock-wait probe (`pg_locks`).
- No in-app audit viewer, no backfill, no auditing of project renames, no member-facing rename, and
  no live refresh of the admin's own sidebar (all out of scope per DISCUSS).
