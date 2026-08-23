# Evolution — board-lane-management (boards render the operator's lanes, not the tool's)

**Finalized**: 2026-08-23
**Commits**: DISCUSS+DESIGN+DISTILL docs landed earlier waves; DELIVER `1f100bf` → `dbd7a05`
(6 DES-monitored TDD steps across 4 slices), refactor `dd5fbf1`/`2ce06a5`/`9ce3477` (L1/L2/L1;
L3–L6 no-change with rationale), mutant-killers `3ab37e9`/`5f0b4ae`/`ad834a4`/`55a0a75`,
mutation report `eef2182`. Trunk-based; DES integrity exit 0; adversarial review APPROVED
(0 defects); mutation **98/110 viable killed (89.1%**, gate 80%**)** with survivor analysis.
Post-merge gate: fresh 24/24 scenarios (137/137 steps) ×2, 2026-08-23. Feature dir PRESERVED.
**Not pushed.**
**Scope**: lanes become per-project data (migration 0015: `lanes` table, grandfather seed,
CHECK/DEFAULT dropped, composite FK `fk_issues_lane`); new projects seed exactly Backlog,
In-Progress, Done; any lane is deletable from the board behind a counted dialog — move all N
cards to a chosen surviving lane or delete all N permanently, one atomic transaction. No wire
surface changed a byte; one new migration (0015); one new dev-dependency (proptest in
foundry-store); no fake.

## Business context

Priya Raman, the self-hosting operator, has a three-stage workflow — yet every board foundry
created rendered four hardcoded columns, with "Todo" sitting permanently dead between Backlog
and In-Progress on every board she made. Worse (the D1 premise correction): `cancelled` was
DB-valid and API-settable but rendered on **no** board column — cancelled issues like AUTH-9
were invisible everywhere except search, and nothing in the schema prevented more cards from
vanishing that way. This feature makes lanes hers: existing boards keep their columns
(stranded cancelled cards surface into a Cancelled lane — a fix, not a regression), new boards
start at her three stages, and slimming a board is an on-board action where the fate of every
card is her explicit, counted decision.

## Key decisions (D1–D11)

- **D1 (premise corrections)** — the code contradicted the working brief: four hardcoded
  columns (not the five DB states), `cancelled` settable but rendered nowhere, no keyboard
  state-move binding. All requirements were built on the corrected picture.
- **D2** — lanes become per-project **data**; the CHECK enum + `DEFAULT_COLUMNS` const cannot
  express a deletable, per-project lane set.
- **D3** — per project, not per team/workspace (everything lane-adjacent was already
  project-scoped).
- **D4** — new defaults: Backlog, In-Progress, Done, in that order ("Todo" dropped).
- **D5** — grandfathering, zero-surprise: existing projects keep their four lanes; a Cancelled
  lane is granted only where ≥1 cancelled issue exists; migration never deletes.
- **D6** — a board keeps ≥1 lane (last-lane delete refused inline); new issues land in the
  leftmost lane; no lane is otherwise special.
- **D7** — delete-time prompt, verbatim: N ≥ 1 cards → dialog with the live count and exactly
  two actions ("Move all N to …" with survivor picker, leftmost preselected, or "Delete all N
  permanently" — hard cascade, copy states permanence); empty lane → confirm-only; ×/Esc
  cancels; moved cards append at the destination bottom preserving order, one 0013 status
  event each, same transaction.
- **D8** — every ripple surface (board render, dnd targets, dialog options, API validation,
  report labels, new-issue landing) derives from lane data; unknown lane on any write = 422.
- **D9** — out of scope: add, rename, reorder — deletion alone serves the validated job;
  consequence carried in dialog copy.
- **D10** — team-membership gate, uniform non-enumerable 404, `_csrf` on the mutating POST;
  no new role axis. DESIGN refinements: the dialog GET is a safe read; the lane-route
  404-vs-board-page-403 asymmetry is chosen and pinned.
- **D11** — walking skeleton: slice 01 swaps the foundation under byte-identical renders, with
  the cancelled-card surfacing as its user-demonstrable outcome.

## Steps completed (6/6, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Migration 0015 + lane-driven board render (grandfather, cancelled surfacing, fixture sweep) | `1f100bf` |
| 01-02 | One lane-validation seam (`validate_project_lane`); static lane lists deleted; check-arch rule | `92b5712` |
| 02-01 | Three-lane creation seed; leftmost landing (in-tx, retry-once); truthful `CreatedIssue.state` echo | `d6d9d55` |
| 03-01 | Delete-lane dialog GET + confirm POST: empty-lane arm, last-lane 422, authz/CSRF, OOB column swap | `dee4298` |
| 04-01 | Two-fate write path: move-all / delete-all in one guarded transaction (ADR-BOARD-LANE-002) | `7ea963c` |
| 04-02 | Browser lane: counted fate dialog end-to-end, declarative dialog close; zero `@pending` closeout | `dbd7a05` |

Post-merge gate: fresh 24/24 scenarios (137/137 steps) run twice, 2026-08-23 —
22 HTTP/API/migration + 2 `@needs-browser` (chromedriver/Chrome pinned at 151).

## Lessons

1. **Enum→data via a composite FK on the slug kept every wire surface byte-stable.**
   `issues.state` stayed the slug carrier and lane slugs carried over 1:1, so dnd POST
   bodies, `/api/v1` payloads, 0013 event values, `data-column` attributes and every URL
   were untouched while the schema's identity model changed underneath. The whole
   migration risk collapsed into one relation, one FK, and a byte-identical render gate.
2. **The FK is the strand-guard.** `fk_issues_lane (project_id, state) → lanes` turns
   "no laneless card" from an application promise into a schema fact: the lane DELETE
   blocks while cards reference it, forcing the fate into the same transaction, and the
   mid-decision race window degrades to a bounded retry instead of a lost card. The FK ADD
   doubling as a live-data validation probe made the migration self-checking.
3. **A schema shift under a mature suite has a fixture-sweep cost — budget it.** Dropping
   the CHECK and the column DEFAULT rippled into every suite that seeded projects/issues
   via raw SQL: 20 acceptance step modules plus 4 store test files needed lane rows or
   explicit state in one step (01-01). DISTILL pre-registering the sweep as a known suite
   impact is what kept it a planned chore instead of a mid-step surprise.
4. **The 04-01 hook-conduct flag: investigate, never work around.** During 04-01 the DES
   pre-write hook logged repeated `json_parse_error` protocol anomalies (control characters
   in the hook payload) and fell back to `allow` — degraded write monitoring, flagged as a
   conduct concern. The correction was procedural, not evasive: the step's writes were
   validated after the fact (`COMMIT_VERIFIED`, subagent-stop conduct PASSED for 04-01,
   `des-verify-integrity` exit 0). Root cause is hook-payload encoding, recorded for the
   DES maintainers.
5. **Retry-envelope mutants need fault injection — recorded as accepted survivors.** 9 of
   the 12 mutation survivors are attempt-boundary arithmetic in the store's retry envelope,
   observable only when a retryable transient fires at the exact attempt boundary. The
   mitigation pattern that worked: extract the retry *classifiers* as pure functions
   (100% pinned by fast unit tests), kill retry-forever via the persistent-failure test's
   timeout, and let the crossing-race gold test exercise the path non-deterministically.
   Deterministic kills would require in-transaction fault injection — deliberately not
   built for a homelab-scale surface.

## Measured KPI baselines (no kpi-contracts.yaml in this repo — recorded here)

- **KPI-1** (≥50% of pre-existing projects trimmed within 30 days): the trim is now
  **possible** — baseline before this feature was 0% possible (lane set hardcoded). The
  full trim journey is demonstrated (empty-Todo delete + counted move-fate delete);
  actual adoption is measured from first real use by SQL over lane rows.
- **KPI-2** (zero invisible issues, permanently): now a **schema fact** — `fk_issues_lane`
  structurally refuses a laneless card; the zero-laneless guard query returns 0 after
  every scenario run and the migration surfaced every previously stranded card.
- **KPI-3** (100% of new projects at exactly 3 lanes): pinned by the suite — the
  creation-seed contract test and the three-defaults chain through the real create port.
- **KPI-4** (0 cards lost without the explicit counted choice): pinned by scenarios
  #19/#21/#22 — deletion happens only through the counted permanent choice, cancel leaves
  everything byte-identical, and a card filed mid-decision is included in the fate at
  confirm time.

## Permanent artifacts

- `docs/product/architecture/adr-board-lane-001-issues-linkage-state-fk.md`
- `docs/product/architecture/adr-board-lane-002-two-fate-delete-transaction.md`
- `docs/product/architecture/brief.md` — "Lanes are per-project data; the lane FK is the
  no-stranded-card invariant" section
- `docs/product/jobs.yaml` — `job-board-lane-shaping`
- `docs/product/outcomes/registry.yaml` — OUT-3 (board view from lane data), OUT-4
  (two-fate lane delete), OUT-5 (≥1 lane + zero-laneless FK invariant)
- `docs/architecture/atdd-infrastructure-policy.md` — lane-delete driving row +
  driven-internal lanes/migration-oracle rows
- `docs/feature/board-lane-management/` — full wave history incl. `feature-delta.md`
  (DISCUSS/DISTILL/DELIVER), `design/`, `slices/`, and `deliver/mutation/mutation-report.md`

## Open / deferred

- **Add / rename / reorder lanes** — the pre-registered successor feature (D9): deletion
  is one-way without add; the natural next slice once an operator over-trims.
- **WIP limits, lane colors, per-lane settings** — adjacent kanban capability unlocked by
  lanes-as-data; explicitly out of scope.
- **Role-gated lane administration** — D10 kept the team-membership gate; a lead-only axis
  would be a new authz concept.
- **Single-issue delete affordance** — US-BLM-04 shipped the first user-facing issue
  deletion, but only as a lane fate.
- **`lanes` missing from `TENANT_TABLES`** (the pre-registered backlog note): the
  per-workspace export/backup covers ten tenant tables and does not include lane rows —
  a workspace restore into a fresh instance would trip `fk_issues_lane` (issues referencing
  lanes that were never restored). Must be added to `TENANT_TABLES`/`verify_export` before
  the next backup-restore exercise.
- **Retry-envelope survivors** — 9 fault-injection-only mutants accepted with analysis
  (mutation-report.md); a store-level pause-seam test remains optional per DISTILL oracle
  discipline #7.
