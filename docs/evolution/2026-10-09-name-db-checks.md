# Evolution: name-db-checks (the name rules, enforced in the database on name writes)

**Finalized**: 2026-10-09
**Commits**:
- DISCUSS+DESIGN+DISTILL: `d5587cc`.
- DELIVER roadmap: `b37ac8e`.
- DELIVER steps: `f2176bd` → `5ca06d1` (9 DES-monitored steps; paused once on the user's request
  while the machine was busy).
- Test-strength fixes: `5d8a855`, `7fb29e9`, `a46a331`.
- Docs fix: `25cddc8`.

Gates:
- DES integrity: all 9 steps complete.
- End-of-DISTILL review approved, two reviewers with recorded conditions.
- Roadmap review approved; its sizing blocker was overruled with the rationale recorded in the
  roadmap.
- Adversarial review approved.
- Rust mutation 100%: check-arch 27/27, seam 3/3 viable, boot entry 1/1.
- SQL hand-mutation table: every mutation killed or proven equivalent.

Feature dir PRESERVED. Nothing was pushed until finalize, because migrations freeze once the
auto-deploying dev instance applies them.

**Scope**: a workspace or project name written to the database by anything other than the app's
doors (psql, scripts, a restore of new data) now meets the same rule the app enforces. The name must
be trimmed, non-empty, free of control and bidi characters, and at most 24 or 256 characters. The
database raises SQLSTATE 23514 naming the broken arm (`<table>_name_<arm>`). Uniqueness stays in
the app.

## Business context

The two earlier name features made every app door enforce one rule, but psql and other tools
bypassed it. DISCUSS confirmed that a plain `CHECK … NOT VALID` is a trap. Postgres re-checks a
CHECK on every UPDATE of the row, and every new issue updates `projects.next_issue_number`, so a
legacy project name would 500 every issue filed on it. No existing test exercised that path, so the
whole suite would have passed and production would have failed.

## Key decisions

- **OQ-1 (user): a trigger on name writes on both tables**, not CHECK. It fires `BEFORE INSERT` and
  `BEFORE UPDATE OF name … WHEN (OLD.name IS DISTINCT FROM NEW.name)`, so legacy rows keep every
  capability and no other column's update ever meets the rule.
- **OQ-3 (user): the read-only legacy count** on dev and prod was 0 violations. Both databases are
  UTF8.
- **DDD-1..3**: one IMMUTABLE verdict function, `foundry_name_rule_violation(name, max_chars)`,
  applies Rust's order with explicit code-point sets: 25 White_Space points for trim, and the refused
  ranges. There is no `\s`.
- **DDD-4..6**: `foundry_enforce_name_rule()` raises 23514 with CONSTRAINT `<table>_name_<arm>`, the
  native message, SCHEMA, a HINT naming the mirrored Rust type, and no DETAIL, so names are never
  logged.
- **DDD-7/8**: migrations 0019 and 0020 are idempotent. 0019 refuses a non-UTF8 database, and both
  self-check edge cases at apply time.
- **DDD-10..12**: a test-support legacy seam disables only the two name triggers in one
  transaction. check-arch `name-rule-legacy-seam` keeps it, and any `DISABLE TRIGGER` or
  `session_replication_role`, out of production code. A GUC switch was rejected because any role
  could set it from the psql prompt.
- **DDD-15**: forward-only. The rollback floor is v0.11.0. The undo and re-arm SQL is documented.

Pre-push CI: `cargo xtask ci` with `FOUNDRY_XTASK_INCLUDE_DOCKER=1` (2026-10-09): all gates green. That covers fmt, clippy, check-arch, the release build, workspace tests, cargo-deny, and acceptance on all tags including browser and docker-compose: 1130/1130 scenarios, 7638 steps, with no flakes.

## Steps completed (9/9)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | check-arch `name-rule-legacy-seam` (gate before seam) | `f2176bd` |
| 01-02 | Migration 0019, legacy seam, fixtures F2/F7, workspace parity | `028d15f` |
| 01-03 | Hand-typed workspace names refused; fit names stored exactly | `5ac0ff8` |
| 01-04 | Legacy workspaces keep working | `ea32504` |
| 02-01 | Migration 0020, fixtures F3–F6, project parity, in-database store tests | `decf01c` |
| 02-02 | Hand-typed project names refused; fit names stored | `d32449e` |
| 02-03 | Legacy projects keep every capability (OPS-8 filed on "Homelab\tOps") | `eb111f5` |
| 03-01 | Restore, backup-verify, previous-release boot, undo and re-arm | `4fbf2a2` |
| 03-02 | CHANGELOG migration notes, ADR amendments, burn-down, mutation report | `5ca06d1` |

## Lessons

1. **A CHECK constraint is a row rule, not a write rule.** NOT VALID skips the scan, not the
   re-check. Asking "which UPDATEs touch these rows?" before choosing the mechanism found the
   show-stopper that no test would have caught.
2. **A setup step can absorb the fault it should expose.** The legacy-project Given bumped the issue
   counter with a direct UPDATE, so "rule on every UPDATE" failed in setup, not at filing. Moving the
   counter into the seam insert put the failure where it means something.
3. **Assert what an error says, not just that it happened.** Without the UTF8 guard, 0019 still
   failed on SQL_ASCII, with the same SQLSTATE. Only the exact message distinguished the guard from
   an accident. The same applied to the boot entry: "returns Ok" let a stub pass, and asserting the
   MigrationReport killed it.
4. **Some SQL mutations are equivalent, and saying so is the result.** Dropping `OF name` while WHEN
   stays changes nothing observable.
5. **Machine load is a test input.** Timing-budget scenarios (us-09, us-06) failed under a load
   average around 55. Pausing on the user's request beat filing flakes as regressions.
6. **A changelog command must be checked against the cluster.** The drafted log search used a pod
   label that doesn't exist; `kubectl get pods --show-labels` corrected it.

## Measured KPIs

- **KPI-1** (rule-breaking names never rise above the upgrade baseline): the baseline is 0 on dev and
  prod. The CHANGELOG ships the post-upgrade count query.
- **KPI-2** (0 app/DB disagreements over at least 10k generated names per table): 10,000 per table at
  both caps, plus exact boundary pairs; 0 disagreements.
- **KPI-3** (100% of legacy-row behaviours still pass): scenarios 6–8 and 13–17, plus the store
  legacy tests.
- **KPI-4** (0 name-caused 500s): the CHANGELOG ships the log search for after release.

## Permanent artifacts

- `docs/product/architecture/adr-name-db-001-name-write-trigger.md`; amendments in ADR-WORKSPACE-NAME-001
  and ADR-PROJECT-NAME-001
- `docs/product/architecture/brief.md`: "Names are labels", the database rule
- `docs/product/jobs.yaml`: `job-name-rule-below-the-app`
- `docs/product/outcomes/registry.yaml`: OUT-20
- `docs/feature/name-db-checks/`: the full wave history, including `deliver/mutation/mutation-report.md`

## Open / deferred

- **Constraint names on `projects`**: the store's slug-vs-key classification still relies on Postgres
  auto-generated names (a project-name-rule follow-up).
- **The app answers 500** if the database ever refuses a write that reached it. That should never
  happen while the rules agree; KPI-4 watches for it.
- **Lookalike names** using allowed invisible characters (ZWSP) are out of scope.
