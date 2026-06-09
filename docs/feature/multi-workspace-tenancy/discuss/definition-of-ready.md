# Definition of Ready — Multi-Workspace Tenancy

> The 9-item DoR hard gate, validated with evidence. Status per item; this feature is **READY
> for DESIGN with two flagged ratifications** (OD-2, OD-3) the user should confirm before DESIGN
> commits to the resolution + provisioning mechanisms. All other items PASS.

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Every story has a `## Problem` in operator/member/reviewer language (host several teams; my data invisible to neighbours; upgrade safely). No solution prescribed in the problem. |
| 2 | User/persona with specific characteristics | PASS | 4 personas in `jobs.yaml` with real-name examples (Sasha Okonkwo, Priya Nandakumar, Marco Bianchi, Dana Whitfield) grounded in the shipped code (`is_workspace_admin`, `workspace_memberships`, the non-enumerable attachments lookup). |
| 3 | 3+ domain examples with real data | PASS | Every story has 3 `### Domain Examples` (happy / edge / error-boundary) using real workspaces "Acme"/"Globex"/"Sandbox", real members, real issues (ACME-1..3, GLOBEX-1..2). No `user123`/`test@test.com`. |
| 4 | UAT in Given/When/Then (3-7 scenarios) | PASS | Each story has 3 BDD scenarios; titles describe business/security outcomes ("A member sees only their own workspace", "A cross-workspace API call is refused non-enumerably"), not implementation. Headline isolation scenarios also in `journey.md`. |
| 5 | AC derived from UAT | PASS | Each story's `### Acceptance Criteria` is derived from its scenarios and the NFRs; isolation ACs trace to NFR-MWT-SEC-*; migration ACs to NFR-MWT-DATA-*. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS (via split) | Feature assessed OVERSIZED (4/5 oversize signals — `wave-decisions.md` Phase 2) and SPLIT into 6 thin slices, each ≤1 day with 1-2 stories and 3 scenarios per story. Slice briefs under `slices/`. |
| 7 | Technical notes: constraints/dependencies | PASS | Each story has `### Technical Notes` naming the SHIPPED primitives reused (`is_workspace_admin`, `is_team_member`, the `workspace_id` scoping, `attachments.rs` non-enumerable lookup, the `jti` denylist, `rate_limit.rs`), the migration target (`uniq_one_workspace`, `0001_init.sql:15`), and the DESIGN-owned mechanisms. |
| 8 | Dependencies resolved or tracked | PASS | Inter-story deps explicit (US-MWT01→00; 02/03/04→01; 05→02/03/04; 06→00/04; 07→00; 08→01/02/03/05). Open product decisions OD-1..OD-5 tracked in `wave-decisions.md`; OD-2 and OD-3 flagged for user ratification before DESIGN. |
| 9 | Outcome KPIs defined with measurable targets | PASS | `outcome-kpis.md`: 2 epic + 8 story KPIs, each [Who][does what][by how much][measured by][baseline]; isolation KPIs are zero-tolerance invariants verified by adversarial acceptance + a query audit, not usage telemetry. |

## Driving ports / surfaces the isolation boundary must cover (for DESIGN + DISTILL)

So DESIGN and DISTILL can verify the hexagonal boundary, the surfaces (driving ports) the
tenant boundary must cover are enumerated:

- **Web htmx tier** (`foundry-app`) — every read/write handler; admin actions; the
  session-resolved acting workspace. (US-MWT02, US-MWT04.)
- **JSON `/api/v1`** (`foundry-api`) — every read/write handler; the `MachinePrincipal`
  bearer-resolved acting workspace; token list/revoke. (US-MWT03.)
- **Machine-token auth** (`foundry-auth` + the `jti` denylist) — the token's `workspace_id`
  binding as the authoritative acting workspace; verify path unchanged. (US-MWT03.)
- **Sign-in / sessions** (tower-sessions Postgres store + the resolution seam) — exactly-one-
  workspace resolution, fail-closed, multi-membership selection. (US-MWT04.)
- **The store seam** (`foundry-store`) — the per-table `workspace_id` scoping + the
  non-enumerable lookup as the single isolation enforcement point. (US-MWT00, US-MWT05.)
- **The rate guardrail** (`crates/foundry-app/src/rate_limit.rs`) — per-principal bucket bounded
  under many tenants. (US-MWT08.)
- **Backup/restore** — whole-instance for v1 (OD-5); per-tenant export OUT of scope.

## Gate verdict

- **JTBD present**: YES — 5 jobs, dimensions + four forces + opportunity scores in `jobs.yaml`.
- **Every story has a `job_id`**: YES — US-MWT01..08 reference `mwt-job-*`; US-MWT00 is
  `infrastructure-only` with an `infrastructure_rationale` (folds into Slice 1, never standalone).
- **Every non-`@infrastructure` story has an Elevator Pitch**: YES — US-MWT01..08 each have a
  Before/After/Decision-enabled triplet naming a concrete entry point.
- **Slice composition hard gate**: PASS — every slice contains at least one user-visible value
  story; US-MWT00 (`@infrastructure`) folds into Slice 1 alongside US-MWT01 (value), never ships
  alone.
- **Scope assessment done + split approved**: YES — OVERSIZED → 6 thin slices (spine in
  `wave-decisions.md`; briefs in `slices/`).
- **DoR**: 9/9 PASS.
- **Blocking for DESIGN**: NONE for the artifact gate; but **OD-2 (user↔workspace cardinality)
  and OD-3 (provisioning authority + instance super-admin role) should be ratified by the user
  before DESIGN commits** — they shape the resolution + provisioning mechanisms. DESIGN must not
  pick these silently.
