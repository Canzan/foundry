# Evolution: project-name-rule (one project-name rule on both doors, and a URL for every project)

**Finalized**: 2026-10-07
**Commits**:
- DISCUSS+DESIGN+DISTILL: `15f516a`.
- DELIVER roadmap: `7842e53`.
- DELIVER steps: `845a915` → `4cefd57` (7 DES-monitored steps).
- Test-gap fixes: `c1aa43a`.
- Refactor: `a728447`.
- Survivor tests and mutation report: `fca6452`.

Gates:
- DES integrity: all 7 steps complete.
- The end-of-DISTILL review (product owner, architect, acceptance) approved. The architect's verdict was conditional; its three points were resolved in DESIGN.
- The roadmap review approved.
- Adversarial review approved.
- Mutation: **62/63 viable killed (98.4%)**; the survivor is an equivalent mutant (93.7% before the survivor tests).

Feature dir PRESERVED.

**Scope**: both places that set a project name apply one rule: trim, then non-empty, then no control
characters (the workspace rule's set, now one shared check), then at most 256 characters, then
unique within the team. The two places are W1, project create (any team member), and W2, the
instance-admin rename. Create gained the length and control checks and rename's uniqueness check.
A project whose name has no Latin letter or digit now gets a working URL: its lower-cased key prefix,
with `-2`, `-3` and so on on a clash. No migration, no new dependency, no fake.

## Business context

Create was looser than rename. It had no length limit and no control-character check, and it checked
uniqueness by slug only, so it could mint a case-insensitive duplicate of a renamed project (the
project-rename D7 gap). Worse, a name with no ASCII letter or digit ("日本語ボード", "🚀") slugified to
`""`. The project was created and listed, but its URL `/team/backend/project/` matched no route, and a
second such project was wrongly refused as a duplicate.

## Key decisions

- **D3/DDD-1**: the refused-character check moves to `foundry_core::name_chars`
  (`pub(crate) is_refused_name_char`). WorkspaceName and ProjectName both call it, and a property test
  pins that they agree on every character.
- **D4/DDD-4**: `ProjectNameError {Empty, ControlCharacter, TooLong, NotUnique}`. Its `Display` is the
  only copy, enforced by check-arch `project-name-one-source` clause (a). The new message is "Project
  name must not contain control characters".
- **D5**: create checks trim → empty → control → length → uniqueness → key; rename checks trim → no-op
  → empty → control → length → uniqueness. Authorization refusals still come first on both doors (D10).
- **D6 / OQ-2 (user)**: one uniqueness definition, a case-insensitive name or a non-empty derived slug
  equal to a sibling's slug. The empty-slug skip means an empty address matches nothing. This closes D7.
- **D15 / OQ-1, OQ-3 (user)**: the key-prefix fallback slug (`jp`, `jp-2`, …). It never refuses, never
  produces `-1` and never produces `""`. Latin names keep today's slugs.
- **DDD-5**: `Services::create_project` is the one place a slug is minted. check-arch clause (b) forbids
  `insert_project(` in the app and API crates.
- **DDD-10 (ADR-PROJECT-NAME-002)**: a `KeyFallback` slug that loses a race retries with a fresh sibling
  read, up to 3 attempts. Exhaustion gives `FallbackSlugContention`, a 500 whose log carries only the
  team id and attempt count. A `Derived` collision keeps today's refusal.
- **D16**: no data rewrite. Legacy `""` slugs are left alone and block nothing.

## Steps completed (7/7, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Shared predicate; `ProjectName` value object | `845a915` |
| 01-02 | Rename composes the rule; one copy source; check-arch (a) | `c576a7a` |
| 02-01 | `create_project` use-case; `ProjectName` on create; `DuplicateSlug`; check-arch (b) | `3f6116d` |
| 02-02 | Create guards and the browser refusal | `3aec0db` |
| 03-01 | One uniqueness check on both doors; empty-slug skip (closes D7) | `4773c8c` |
| 04-01 | `mint_project_slug`; bounded retry; `FallbackSlugContention` | `e2b348d` |
| 05-01 | Two-door parity outlines; burn-down; logging review | `4cefd57` |

Pre-push CI: `cargo xtask ci` with `FOUNDRY_XTASK_INCLUDE_DOCKER=1` (2026-10-07): all gates green. That covers fmt, clippy, check-arch, the release build, workspace tests, cargo-deny, and acceptance on all tags including browser and docker-compose: 1073/1073 scenarios, 7300 steps, with no flakes.

Final lanes:
- pnr: 74/74, including 2 real-Chrome scenarios.
- Neighbour lanes: iapr 21, iwnr 61, us-07 30, us-r01 2, blm 24, form-error-display-contract 6, us-05 23.
- Default lane: 838/839 at 05-01. The one failure was the known sqlx `'\0'` flake, and the rerun passed.

## Lessons

1. **A warm-up command must not be able to stop a lane.** The roadmap's `foundry --help` (and
   `--version`) exits 2, because the CLI has no such subcommand, so `build && warm && lane` never ran
   the lane. The warm-up is now `build && { warm || true; }`. Tolerating only the exec keeps a failed
   build fatal.
2. **A page-wide selector is a weak oracle.** The create refusal check matched any `p.error`. It is now
   scoped to the slot adjacent to the create form. That slot is a sibling of the form, not inside it,
   which is why a descendant selector would have matched nothing.
3. **Defer a guard and its test together.** The empty-slug skip was deliberately ported without the skip
   in 01-01, so its scenario stayed red for the right reason until 03-01 added both.
4. **cargo-mutants can't reach `Result`-returning orchestration without `Default`.** All services
   mutants were unviable, so the retry bound was pinned by hand: a test now fails if the bound drops
   below 3 as well as if it rises above.
5. **Reviewer claims still need checking.** The DESIGN reviewer said `ProjectName` could not call a
   crate-private function; `pub(crate)` is visible crate-wide.

## Measured KPIs

- **KPI-1** (0 rule-violating names created or renamed): pinned on both doors by the refusal scenarios
  and the parity outline (11 rows).
- **KPI-2** (both doors give the same verdict and copy): the parity outlines. The baseline differed on
  length, control characters and uniqueness.
- **KPI-3** (a refused create writes nothing; the key stays free): "corrected retry reuses the same key".
- **KPI-4** (0 name-caused 500s): NUL is now a 422 on both doors.
- **KPI-5** (0 empty-slug projects after release; every create redirect reaches a board): the
  fallback-slug scenarios, which also open the report.

## Permanent artifacts

- `docs/product/architecture/adr-project-name-001-one-rule-and-create-use-case.md`
- `docs/product/architecture/adr-project-name-002-fallback-slug-mint.md`
- `docs/product/architecture/brief.md`: "Names are labels; slugs are identity", updated
- `docs/product/jobs.yaml`: `job-project-naming`
- `docs/product/outcomes/registry.yaml`: OUT-19 (`related: [OUT-1, OUT-18]`) and OUT-1's amended note
- `docs/feature/project-name-rule/`: the full wave history, including `deliver/mutation/mutation-report.md`

## Open / deferred

- **Legacy `slug = ''` projects**: not repaired (D16). If dogfood finds any, repairing them is a user
  decision. It is safe, because no URL to them ever worked.
- **D17 residual**: once "日本語ボード" holds `jp`, a project named "JP" is refused as not unique.
- **Constraint names**: the store tells slug clashes from key clashes by Postgres's auto-generated
  constraint names. Naming them explicitly needs a migration.
- **Lookalikes**: an allowed invisible character such as ZWSP can make two names look identical.
- **Database CHECKs** on `projects.name` and `workspaces.name` (follow-up D).
