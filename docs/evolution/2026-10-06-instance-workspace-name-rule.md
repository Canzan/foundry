# Evolution: instance-workspace-name-rule (one workspace-name rule on every door)

**Finalized**: 2026-10-06
**Commits**:
- DISCUSS+DESIGN+DISTILL: `30fe92d`.
- DELIVER roadmap: `d8319d0`.
- DELIVER steps: `7e0b52e` → `7625316` (6 DES-monitored steps).
- Test-step fix: `a430466`.
- Refactor: `4dea2a8`.
- Survivor-killing scenarios and mutation report: `6198fd3`.

Gates:
- DES integrity: all 6 steps complete.
- The end-of-DISTILL review (product owner, architect, acceptance) approved; the architect's verdict was conditional, and its one substantive claim turned out false (see Lessons).
- The roadmap review approved.
- Adversarial review approved.
- Mutation: **50/50 viable killed (100%)**.

Feature dir PRESERVED.

**Scope**: every path that sets a workspace name applies one rule: trim, then non-empty, then no
control characters, then at most 24 characters. The four paths are:
- P1: dashboard provisioning;
- P2: `foundry doctor provision-workspace`;
- P3: the first-run bootstrap claim;
- P4: the rename shipped in v0.9.0.

All four give the same verdict and byte-identical wording. No migration, no new dependency, no fake.

## Business context

Before this feature only rename enforced a rule, and even rename let control characters through.
- Dashboard provisioning and bootstrap accepted any length.
- `foundry doctor provision-workspace` didn't even trim.
- A NUL in a name was a 500 on every door, because Postgres `text` refuses it.
- A newline forged extra `key: value` lines in the CLI output and in invite emails.

The v0.9.0 evolution doc had called interior control characters "cosmetic". Code reading in DISCUSS
corrected that.

## Key decisions

- **D4 (user, OQ-1)**: refuse Unicode Cc (`char::is_control`), the bidi embedding, override and
  isolate controls U+202A–202E and U+2066–2069, and U+2028/2029. All other format characters stay
  allowed: ZWJ (family emoji), ZWNJ (Persian and Indic spelling), soft hyphen, variation selectors.
- **D3 (user, OQ-2)**: the new message is "Workspace name must not contain control characters".
- **D5**: trim, then (rename only) the byte-equal no-op, then empty, then control, then length.
- **D6**: legacy names are never rewritten. Resubmitting one byte-for-byte is still a quiet
  success, even with a tab or over 24 characters.
- **D7/D8**: a refusal leaves nothing behind, and the refusal precedence is unchanged:
  - no workspace or user is created, and a bootstrap link is not used up;
  - non-admins still get the uniform 404 first;
  - a dead bootstrap link answers exactly as before, whatever the name;
  - the CLI exits 2 before it reads `DATABASE_URL`.
- **DDD-1/2/3 (ADR-WORKSPACE-NAME-001)**: `foundry_core::WorkspaceName::try_new` is the one rule.
  The `Display` of `WorkspaceNameError` is the only source of the wording, and `cargo xtask check-arch`
  (`workspace-name-one-source`) fails the build if the literal appears in an adapter crate.
- **DDD-5**: `ProvisionRequest.workspace_name` is a `WorkspaceName`, so the type is the
  defence-in-depth.
- **DDD-9 (ADR-WORKSPACE-NAME-002)**: bootstrap checks the name before hashing the password and
  before the claim. Only on a refusal does it make a non-consuming link-status read.
- **DDD-7**: the store's `create_initial_workspace` stays a test-seeding seam. check-arch forbids
  production callers.
- **D11**: there is no database CHECK (follow-up D), and project names keep today's rule.

## Steps completed (6/6, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | `WorkspaceName` value object, the D4 set, the copy as `Display`; 13 boundary tests | `7e0b52e` |
| 01-02 | Rename composes the rule (`InvalidName`); check-arch clause (a) | `792f3c7` |
| 02-01 | Bootstrap checks the name before the claim; 422 claim page; check-arch clause (b) | `e1e102f` |
| 03-01 | Typed `ProvisionRequest`; 422 dashboard re-render; CLI call site moved before `DATABASE_URL` | `b5d4af3` |
| 04-01 | CLI: `--name ""` gets the rule, refusal exits 2, success prints the trimmed name | `af76c6a` |
| 05-01 | Parity outlines: same verdict and same words at all four doors | `7625316` |

Pre-push CI: `cargo xtask ci` with `FOUNDRY_XTASK_INCLUDE_DOCKER=1` (2026-10-06): all gates green. That covers fmt, clippy, check-arch, the release build, workspace tests, cargo-deny, and acceptance on all tags including browser and docker-compose: 999/999 scenarios, 6900 steps, with no flakes.

Final lanes:
- iwnr: 61/61 scenarios, 344 steps, including 3 real-Chrome scenarios.
- Neighbour lanes, all green: iawr 27, iapr 21, us-05 23, bootstrap-enum-oracle 4, web-provisioning-flow 11, mwt-slice-06 9.
- Default lane: 761/761 at step 05-01.

## Lessons

1. **A snapshot test must create its fixtures before the "before" snapshot.** The parity step minted
   its bootstrap link after capturing the universe, so every correct refusal (link left live) read as
   "a new live link appeared". The step was fixed in `a430466`. The crafter proved the doors were
   right before blaming the step: copy parity held on every row.
2. **A trimming read helper hides a trimming bug.** The dashboard "trimmed name" check read the
   success text through `texts()`, which trims, so an untrimmed echo passed. Compare the raw text when
   whitespace is the subject.
3. **Mutation testing finds the flags nobody leaves out.** The 92% first pass left survivors on the
   `--as` / `--admin-email` presence checks and on the live-link claim form. Every suite always passed
   those flags, and only ever GET a dead link. Six scenarios closed them to 100%.
4. **Reviewer claims need checking.** The DESIGN reviewer flagged the scaffold as missing the bidi
   boundaries; it had all of them. It also advised hashing the password before the name check, which
   contradicts ADR-002. Neither was acted on. The thin DELIVER review's live-vs-dead "oracle" question
   was answered by reasoning: a plain GET already distinguishes the two link states.
5. **One value type removes a whole class of drift.** Once the copy is `WorkspaceNameError::Display`
   and check-arch forbids the literal elsewhere, the parity scenarios found nothing to fix at any door.

## Measured KPIs (no kpi-contracts.yaml; recorded here)

- **KPI-1** (0 rule-violating names created or renamed): pinned at every door by the refusal scenarios
  and the parity outline.
- **KPI-2** (4/4 doors give identical verdict and copy): the parity outlines, with 8 refusal rows,
  5 accept rows and NUL at the 3 web doors. The baseline was 1/4.
- **KPI-3** (0 bootstrap links or first-admin emails burned by a refusal): pinned by "remains
  unconsumed" plus the corrected retry with the same link and email.
- **KPI-4** (0 name-caused 500s): NUL is now a 422 with the control copy at every web door. The
  baseline was a 500 everywhere.

## Permanent artifacts

- `docs/product/architecture/adr-workspace-name-001-one-rule-as-core-value-object.md`
- `docs/product/architecture/adr-workspace-name-002-bootstrap-name-check-before-claim.md`
- `docs/product/architecture/brief.md`: the "Names are labels" paragraph now covers every write path
- `docs/product/jobs.yaml`: `job-instance-workspace-naming`
- `docs/product/outcomes/registry.yaml`: OUT-18 (`related: [OUT-17]`)
- `docs/feature/instance-workspace-name-rule/`: the full wave history, including
  `deliver/mutation/mutation-report.md`

## Open / deferred

- **Follow-up D**: `CHECK (…) NOT VALID` on `workspaces.name`. It closes hand-typed `psql` writes.
- **Project names** still accept control characters; the same rule could follow as its own change.
- **Two CLI usage messages**: the dispatcher says "missing required flags." while
  `run_provision_workspace` says "--name, --admin-email and --as are required." A scenario now pins
  the dispatcher's wording. Unifying them is a small design choice; the scenario must change with it.
- **Legacy names with a newline**: browsers strip line breaks from text inputs, so submitting the
  untouched rename form for such a name becomes a real, audited rename to the cleaned name (D6
  caveat, accepted).
