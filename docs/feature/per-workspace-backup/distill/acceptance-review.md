# DISTILL Acceptance Review: per-workspace-backup

> Self-review against the 9 critique dimensions + the mandate-compliance checklist + traceability.
> Source Gherkin: `crates/foundry-acceptance/tests/features/us-per-workspace-backup.feature`.

## Traceability — every story + NFR → scenario

### Story-to-scenario (critique Dim 4 + Dim 8 Check A)

| Story | Scenarios | All ACs covered? |
|-------|-----------|------------------|
| **US-PWB-01** (export to portable archive) | 1 (WS, AC-01.2), 2 (AC-01.1), 3 (AC-01.3), 4 (read-only), 15 (AC-01.4) | YES — AC-01.1/.2/.3/.4 |
| **US-PWB-02** (isolation + verification, the crux) | 5 (AC-02.1), 6 (AC-02.2), 7 (AC-02.3), 8 (OD-PWB-1 users), 9 (AC-02.4 falsifiability), 10 (AC-02.5 @property) | YES — AC-02.1/.2/.3/.4/.5 + the OD-PWB-1 ratified semantics |
| **US-PWB-03** (failure paths & safety) | 11 (AC-03.1), 12 (AC-03.2), 13 (AC-03.3), 14 (AC-03.5), 15 (AC-01.4/exit3), 16 (AC-03.4), 17 (AC-03.6) | YES — AC-03.1/.2/.3/.4/.5/.6 |

All 16 ACs across the three stories are covered. Zero untraceable scenarios; zero stories with zero
scenarios. **Dim 4 / Dim 8 Check A: PASS.**

### NFR-to-scenario (the crux + safety)

| NFR / decision | Scenario(s) |
|----------------|-------------|
| **NFR-PWB-ISO-01** (the crux — all-W-rows / no-sibling) | 5, 9 (falsifiability), 10 (@property) |
| **NFR-PWB-INT-01** (path-only verify) | 6, 7, 14 |
| **NFR-PWB-ATOM-01** (atomic write) | 12, 13 |
| **NFR-PWB-SEC-01** (sensitivity disclosure) | 17 |
| **NFR-PWB-SURF-01** (off-bearer) | feature-level: CLI subprocess only, no HTTP scenario (documented in header) |
| **OD-PWB-1 / ADR-001** (users membership-bounded) | 8 |
| **OD-PWB-2 / ADR-005** (10-table completeness) | 1, 6 (acceptance side); gold test = DELIVER unit guard |
| **DRIFT-1** (id-or-name selector) | 2, 3, 11 |
| **DRIFT-2** (transitive + comment cross-check) | 7 |

The isolation crux (NFR-PWB-ISO-01) is exercised non-vacuously: scenario 5 asserts every W row present +
no sibling in a real two-workspace fixture; scenario 9 plants a sibling row and asserts verify REDs
(falsifiability — the proof bites); scenario 10 generalizes the invariant over either workspace.

### Environment-to-scenario (Dim 8 Check B)

`docs/feature/per-workspace-backup/devops/` is ABSENT (graceful degradation → WARN). There is no
multi-environment matrix for this feature: the sole environment is the per-scenario testcontainers PG16 +
tmpdir filesystem the shipped acceptance suite already provisions. Every scenario's Background establishes
its preconditions (two seeded workspaces). **Dim 8 Check B: N/A (single test environment); not a blocker.**

## Critique dimensions self-review

| Dim | Check | Verdict |
|-----|-------|---------|
| 1 — Happy-path bias | Error/edge ≥40% | PASS — 6 `@error` (9,11,12,13,14,15) + 2 safety/boundary (4,16) = 8/17 = 47% |
| 2 — GWT compliance | Single When, Given context, observable Then | PASS — each scenario one user action; scenario 10 is a Scenario Outline (boundary variation), GWT-clean |
| 3 — Business language purity | No technical jargon in titles/steps | PASS — see grep evidence below; "archive", "row count", "workspace", "status: OK" are domain/operator terms (the operator literally reads `status: OK` on stdout — it is the user-facing contract, not jargon) |
| 4 — Coverage completeness | All stories + ACs | PASS — see traceability table |
| 5 — WS user-centricity | User goal, observable Then | PASS — see walking-skeleton.md litmus |
| 6 — Priority validation | Largest bottleneck addressed | PASS — the isolation crux (the operator's peak-tension fear) gets 3 scenarios (5,9,10) incl. falsifiability |
| 7 — Observable behavior assertions | No internal-state asserts | PASS — every Then asserts exit code, stdout content, file presence/absence, or unchanged source data; no private fields, no "row inserted in DB" framing. "every row in the archive belongs to W" is an observable property of the produced artifact, asserted via verify-export's report lines |
| 8 — Traceability coverage | Story + env mapping | PASS (Check A) / N/A (Check B, single env) |
| 9 — WS boundary proof | Strategy declared, real I/O, adapter integration | PASS — Strategy C-equivalent (`@real-io`) declared in walking-skeleton.md; every driven adapter has a real-I/O scenario (Mandate 6 table in test-scenarios.md) |

### Dim 3 evidence (business language purity)

The scenario titles + steps use operator-domain language. The only literal that looks "technical" is
`status: OK` and the exit codes — these are the OPERATOR-FACING CONTRACT (the operator greps `status:` in
cron and branches on exit code exactly as for the shipped `backup-verify`), so they are domain vocabulary
for this persona, not leaked implementation detail. No `SELECT`, `JSON`, `HTTP`, `tar`, `sqlx`,
`testcontainers`, `to_jsonb`, or table-internal column names appear in any scenario title or step — those
live only in the `#` header comments (design context) and will live in the DELIVER step bodies. "tenant
tables" is the operator's mental model (kinds of data that belong to a tenant), used in the ACs verbatim.

### Dim 7 evidence (observable behavior)

Sampled Then steps and their observable nature:
- `an archive file exists at that path` → filesystem observable (the user's deliverable artifact).
- `the output reports a row count for all 10 tenant tables` → stdout observable.
- `the command exits with code 0` → process exit code (the cron contract).
- `every row in the archive belongs to "Globex LLC"` → property of the produced archive, asserted through
  the verify-export report (an observable user outcome: the proof the operator runs), NOT a DB query of
  internal state.
- `no file exists at that path` / `no archive file is created` → filesystem observable (atomicity/safety).
- `the message identifies a row resolving to a workspace other than the declared one` → stderr/stdout
  observable (the falsifiability proof the operator reads).

No scenario asserts a mock call, a private field, or an internal DB row by direct query at the acceptance
layer. The two-workspace seeding + "still exist unchanged" checks (scenarios 4, 16) are observable
read-only guarantees, verified through the same operator-facing surface.

## Mandate compliance evidence

- **CM-A (Mandate 1 — hexagonal boundary)**: tests enter through the driving port (the `foundry` CLI
  subprocess), never an internal component. The step glue (DELIVER) will invoke
  `assert_cmd::Command::cargo_bin("foundry")` + the shipped `Store` seam, never instantiate a parser/
  formatter directly. Off-bearer (ADR-006 allow-list, no new line).
- **CM-B (Mandate 2 — business language)**: Gherkin uses operator-domain terms; technical detail confined
  to header comments + (future) step bodies. See Dim 3 evidence.
- **CM-C (Mandate 3 — user journey completeness)**: every scenario is a complete operator journey
  (trigger → processing → observable outcome → value). The WS + isolation + failure scenarios each deliver
  business value (a verifiable archive, a proven-clean archive, a guided recovery).
- **CM-D (Mandate 4 — pure function extraction)**: the scope predicate is extracted to ONE place
  (`Store::export_workspace`, predicate strings = the `workspace_scope_predicate` shared artifact) and is
  unit-testable at the store seam (layer 1-2). Fixture parametrization (scenario 10 outline) applies only
  to the selector token, not to environment variants.
- **CM-E (Mandate 8 — universe-bound assertion)**: N/A at LAYER-3 with no Rust state-delta port; LAYER-3
  uses traditional assertions over port-exposed observables (consistent with slices 1-6). Documented in
  test-scenarios.md Phase-0 log.
- **CM-F (Mandate 9 — PBT mode layer-dependent)**: COMPLIANT — zero PBT machinery in these LAYER-3
  scenarios. The `@property` scenario 10 is EXAMPLE-PINNED (concrete Acme/Globex fixture), with the
  generative amplification explicitly deferred to layer-1-2 store tests (test-scenarios.md).
- **CM-G (Mandate 10 — two-tier acceptance)**: Tier A only. Tier B is NOT warranted — the journey is not a
  ≥3-chained-scenario state machine over a domain-rich input space; it is a config-shaped batch
  read/verify CLI. No `test_per_workspace_backup_state_machine.rs`. Justified.
- **CM-H (Mandate 11 — integration sad paths example-based)**: COMPLIANT — every failure mode (exit 2/3/4/5,
  atomic write, planted sibling) is a named example-based scenario; no PBT explosion on the slow subprocess
  layer.

## RED-state / compile-safety (Mandate 7)

- The crate COMPILES: the feature file is Gherkin text; no new undefined-symbol reference added to any
  `.rs`; `acceptance.rs` untouched (no new force-link). → NOT BROKEN.
- Scaffold note: for cucumber-rs, the RED-ready "scaffold" is the step glue authored in DELIVER, not a
  production stub committed in DISTILL (matching the shipped slice-05/06 precedent — DISTILL ships Gherkin,
  DELIVER ships glue + production code in the same cycle). Genuine RED = MISSING_FUNCTIONALITY at runtime
  (unknown subcommand / absent `Store::export_workspace`), classified at DELIVER RED-phase entry.
- `@pending` is already wired into all three `acceptance.rs` run lanes (default / all / docker-compose) to
  exclude pending scenarios — so the 16 `@pending` scenarios do not run until DELIVER unskips them one at a
  time. The single `@walking_skeleton @wiring_e2e` scenario is the first DELIVER cycle target.

## Self-review checklist (Dimension 9 + Mandate 7)

- [x] 1. WS strategy declared (walking-skeleton.md — Strategy C-equivalent, `@real-io`)
- [x] 2. WS scenarios tagged correctly (`@real-io` via feature-level tag; `@walking_skeleton @wiring_e2e`)
- [x] 3. Every driven adapter has a `@real-io` scenario (Mandate 6 table)
- [x] 4. No InMemory doubles (N/A — all real I/O)
- [x] 5. Container preference documented (testcontainers PG16, inherited from policy)
- [x] 6. Mandate 7 — crate compiles; RED is runtime MISSING_FUNCTIONALITY (glue + prod in DELIVER)
- [x] 10. Driving adapter — all 3 subcommands exercised via subprocess (coverage table)
- [x] 11. ≥1 `@real-io` scenario per driven adapter
- [x] 14. No timing assertions in the feature file (none present)

## Decision: APPROVED for handoff to DELIVER

All 9 critique dimensions PASS (Dim 8 Check B N/A). All applicable mandates COMPLIANT. Traceability complete
(16/16 ACs, 3/3 stories, the isolation crux + falsifiability + all failure paths + completeness +
sensitivity covered). Error/edge ratio 47% ≥ 40%. Non-vacuous (real two-workspace fixtures, real seeded
rows, planted-sibling falsifiability). One `@walking_skeleton`, 16 `@pending`. No cargo run performed
(per instruction). Open decisions all carry recommended options (auto-accepted by orchestrator) and are
DELIVER-scoped, not blockers.
