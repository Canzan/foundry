# DELIVER retrospective: external-integration-plugins

nWave Phase 8. This covers DELIVER from the roadmap (`45a81d3`, 2026-10-22) to the evolution
record (`90e8b82`, 2026-10-27): 27 steps, 68 commits, 136 scenarios and 225 rows. Every lane is green; G1 (a green Docker
`xtask ci`) and G3 are still open.
Sources are the orchestrator's tracker and upstream notes, `git log 45a81d3..HEAD`,
`../../../evolution/2026-10-27-external-integration-plugins.md` and `upstream-issues.md`.
Nothing is released.

Owners used below:
- **nWave**: the methodology, its agents, the DES log and the review gates.
- **Scripts**: project scripts (`scripts/eip-lanes.sh`, the mutation wrappers, xtask).
- **Harness**: the `foundry-acceptance` support code and the crate test fixtures.
- **Prompts**: the DELIVER orchestrator's dispatch prompts and templates.

Unverified links in a chain are marked *Hypothesis*.

## What went well

- **Seeded-fault proofs.** Every GREEN_ALREADY row was proven red against a seeded fault, and a
  fault that killed nothing was treated as a gap. Fault A (live-lease fence) stayed green through
  01-10 and 01-11, and that led to scenario 132 (`8f47631`).
- **Test defects went back to DISTILL.** Crafters did not edit the acceptance tests. Eighteen
  DISTILL fix commits corrected drivers, and each was proven by reverting a seeded fault.
- **The adversarial review paid for itself.** Phase 4 found four shipped defects (A1, A2, B1/B2,
  B3) and four tests that could not fail (A3, A4, B5, B6) before any push.
- **RED before the fix, in every review round.** `aa222f7` came before `717c3a5` (R1), and
  `fbe9bdc` came before `a61a399` (R2). The R3 splits (`cc5a9ce`) changed no tests.
- **The gates closed on evidence.** G2 was met at `4c43d8c`. Mutation reached about 92.9% overall
  and 83.6% for foundry-app (`12cdbbe`). DES integrity is 27 of 27. Lanes were green after every
  refactor.
- **User decisions were captured when made** (UD-1..UD-10), and nothing was pushed or tagged.

## Issues, ranked by cost

| # | Issue | Cost | Recurred |
|---|---|---|---|
| 1 | Acceptance drivers needed DISTILL fixes (AT_DRIVER_DEFECT) | 18 fix commits; 15 steps waited on a DISTILL fix, 4 were re-dispatched | yes, across all 5 phases |
| 2 | Shipped defects passed every lane | 1 review round (R1), 13 files | 4 defects |
| 3 | Fake tests (A3, A4, B5, B6) | review findings; would have hidden defects | 4 tests |
| 4 | Mutation gaps; Phase 5 kept being deferred | 181 survivors at Phase 5; 95 killed in `12cdbbe` | deferred at 9 steps, 01-06 to 05-02 |
| 5 | A busy machine | false kills, 4 failed mutation baselines, no green Docker `xtask ci` (G1 open) | throughout |
| 6 | A test contradicted a fix | blocked the R1 commit | once (the same family as A2) |
| 7 | RED evidence taken after the fix | one step's TDD claim not proven | once (05-01) |

## 1. Acceptance drivers needed repeated DISTILL fixes

**Evidence.** The driver fix commits were `7b749f3`, `fd1369e`, `c00a8fc`, `fe1fcba`, `55a88aa`,
`7c72746`, `6170eac`, `087c488`, `4e552fb`, `81d301c`, `8805294`, `76921c0`, `7628948`,
`b083e83`, `e33621c`, `d97673e`, `c1f0504` and `7c8cc67`. Fifteen steps waited on a DISTILL fix: 01-07, 01-08, 01-09, 01-10,
02-01, 02-02, 02-03, 03-01, 03-02, 03-05, 04-02, 04-03, 04-04, 05-01 and 05-02. Four were
re-dispatched: 01-10 (blocked), 02-01, 02-03 and 05-02 (05-02b).

The defects fall into four families:

| Family | Instances |
|---|---|
| A Given returns before the system has settled | `given_filed` race in 14 (`7b749f3`) and 58 (`fe1fcba`); falls through to the Slack double (`e33621c`); Matrix path does not wait for the first send (`d97673e`); 108 counts the Given's own email (`7c8cc67`) |
| The harness wires the app differently from `main.rs` | replicas get `IntegrationsHandle::off` (`55a88aa`); Background never calls `rewire` (`b083e83`); replica recorders not merged (`76921c0`); no-email Given never reaches the app (`c1f0504`); relay rewired to the recorder, recorder drops headers (`7c8cc67`) |
| Clock or time budget | 13's 10 s Then against a 45 s lease (`c00a8fc`); 15 never moves time past the retry (`fd1369e`); frozen clock blocks pacing (`7c72746`, `6170eac`); 53's 10 s budget against an 11-12 s physical minimum (`8805294`) |
| The driver gets the existing product's contract wrong | comments driven by form posts instead of hx PATCH/DELETE (`6170eac`); lane slug not re-resolved after a rename (`087c488`); multipart upload without `x-csrf-token` (`4e552fb`); a 404 accepted as a redirect in 115 (`fe1fcba`); restore reads a deleted project's page (`7628948`); Outline headers with spaces and `\|` in Examples cells (`6170eac`, `4e552fb`) |

**5 Whys.**
1. Why did scenarios fail without a product fault? The step bodies were wrong, in the four
   families above.
2. Why were they wrong when DELIVER first ran them? DISTILL wrote the whole suite in one commit
   (`7ceb851`) before any product code existed. Its first real run was at each step's RED.
3. Why did RED not catch them sooner? RED only checks that a scenario fails. A driver defect also
   fails, so it looks like a valid RED until GREEN cannot be reached.
4. Why did one defect recur in many places? *Hypothesis, requires verification* (no per-Given
   authorship record was kept): the suite has no stated contract for a Given (wait
   until settled), for clock ownership (which component reads the frozen clock and which reads
   wall time), or for wiring parity (the harness `rewire` and `spawn_inner` against `main.rs`).
   Each Given decided them for itself. The `given_filed` family alone was fixed four times.
5. **Root cause.** DISTILL has no step that runs the drivers against the product surface that
   already exists, and no harness contract a reviewer can check. The fourth family touched only
   pre-existing surfaces (comments, lanes, attachments, sign-in), so it was checkable at DISTILL
   time.

**Forward check.** With no dry run and no contract, every new Given could repeat a family. All
four did, in at least three phases.

**Countermeasures.**
- **Harness.** Write the harness contract into the support module: (a) every Given ends with an
  explicit settled-state wait, such as `delivery_settled`; (b) one table names the clock each
  component reads; (c) one wiring function is shared by `rewire`, the replica spawn and
  `main.rs`'s composition, so they cannot drift.
- **nWave.** Add a DISTILL "driver dry run" gate: run every Given and When that touches an existing
  surface against the current product, and require them to pass before handing off.
- **nWave (review).** The acceptance-designer-reviewer checks each new Given against the contract:
  does it wait for settlement, and which clock does it use?

## 2. Shipped defects passed every lane

**Evidence.** These are the Phase 4 review findings that had passed every lane, fixed in
`717c3a5`. Defects the suite itself caught before review (the 02-01 lane snapshot, the 114 SMTP
charset `4c43d8c`, the 02-03 preview channel id) are not counted here.
- **A1.** Services `problem_cleared` checked the Slack connection for Matrix routes, so a Matrix
  route stopped by a refusal could not resume.
- **A2.** A delivery past `give_up_at` stayed claimable. `claimable_head`, the claim SQL and the
  worker had no give-up check.
- **B1/B2.** A failed Matrix save or enrol, and the `delete_route` fallback, left a room that
  another route still used.
- **B3.** Send test posted to any `form.destination`, with no `postable_destination` check.

**5 Whys.**
1. Why did the lanes pass? No scenario exercised these paths.
2. Why not? The scenarios follow the ratified user stories, which state the expected path for one
   kind. Resume and repair (03-03, 03-04) were specified and proven for Slack. 04-03 reused them
   for Matrix "unchanged" (tracker, after 04-03). Shared rooms appeared only in the DESIGN adapter
   contract. A forged Send-test destination is an attacker path, not a story.
3. Why were the gaps not added as scenarios once they were seen? New Gherkin needs the user's
   approval. The shared-room scenario was proposed at `d97673e` (DIST-30) and is still not
   ratified (PA-1). Only a store test covered it. The survivor `room_still_routed` (04-04) was
   deferred to Phase 5.
4. Why did step mutation not expose A1 and B3? Mutation proves that tests detect *changes* to code
   that exists. It cannot find a check that is missing. A2 was worse: a property pinned the
   defective behaviour (see issues 3 and 6).
5. **Root cause.** Scenario coverage comes from stories, not from a variant matrix. Nothing in
   DISTILL or DELIVER enumerates kind × lifecycle operation (Slack/Matrix × resume, delete, save
   failure) or "every write door re-checks what Save checks". Generalising a Slack path to
   Matrix reused the code without reusing the proof.

**Forward check.** Each of the four defects sits in a cell such a matrix would list (Matrix ×
resume, shared room × delete or save failure, Send test × destination authority, claim × past
give-up). None of them sits on a story's main path.

**Countermeasures.**
- **nWave (DISTILL).** When a feature has more than one kind, produce a variant matrix (kind ×
  lifecycle operation, and door × authority check). Every cell needs a scenario or a named crate
  test, or an explicit "not applicable".
- **Prompts.** When a step generalises an earlier step to a new kind (as 04-03 did), require a
  variant scenario or test per reused path. "Reused unchanged" is not evidence.
- **nWave (process).** Let DISTILL land proposed safety scenarios as crate-level tests at once,
  while the Gherkin waits for approval, so a pending approval does not leave the behaviour
  untested.

## 3. Fake tests, even after the DISTILL review

**Evidence.** All four came from the DISTILL scaffold commit `7ceb851` (`git log -S` on
`stop_token_never_expires`, `project_of_destination` and the D-3 property), and the review found them
(fixed in `aa222f7`):
- **A3.** The D-3 leak property was vacuous (`core_properties` 895-917).
- **A4.** `stop_token_never_expires` was tautological (`eip_secret_box` 368-377).
- **B5.** `project_of_destination` hard-coded the oracle.
- **B6.** The scanner's GET response was discarded.

**5 Whys.**
1. Why could they not fail? Each assertion compared values that the test itself built, or
   asserted nothing about the system's answer.
2. Why did the DISTILL review pass them? The acceptance-designer-reviewer is read-only (Read,
   Glob, Grep). It reviews Gherkin and BDD quality. It cannot run a test, and nothing asks it
   whether an assertion can fail.
3. Why did DELIVER not catch them? Seeded-fault proofs were required for scenario rows that were
   GREEN_ALREADY at RED. Property tests, unit tests and harness oracles that DISTILL authored had
   no fault-proof rule. Mutation mutates product code, not tests, so a vacuous test merely leaves
   a survivor, which was then deferred (issue 4).
4. Why was that rule limited to scenarios? The rule was written for the GREEN_ALREADY case
   (a scenario that passes before any code). A test that never runs red falls through it.
5. **Root cause.** Falsifiability is proven for scenario rows only, not for every test DISTILL
   ships, and the only reviewer before DELIVER cannot execute anything.

**Countermeasures.**
- **nWave (DISTILL).** Every DISTILL-authored property, unit test and harness oracle carries a
  named seeded fault that turns it red, recorded in the feature delta beside the test. No fault,
  no hand-off.
- **nWave (review).** Add "name the fault that makes this assertion fail" to the
  acceptance-designer-reviewer checklist. Flag any assertion whose expected value comes from the
  test's own fixture.
- **Harness.** Oracles read the system's answer, never a constant the Given wrote (the B5/B6
  pattern).

## 4. Mutation gaps; Phase 5 kept being deferred

**Evidence.** Per-step results were WARN or FAIL and deferred to Phase 5: 01-06 (74.6%), 01-07
(33% in-diff), 01-08 (run B stopped), 02-01 (62.5%), 02-03 (40% after the re-run), 03-03 (28
unmeasured), 04-02 (about 74%), 04-04 (`room_still_routed`) and 05-02 (62.5%, 26 unmeasured).
Phase 5 at `cc5a9ce` measured 73.9% on own tests and 13% for services own tests, with 181
survivors, including the security mutants `require_workspace_admin -> Ok(())` and the
`finish_slack_install` state guard. `12cdbbe` killed 95, giving about 92.9% overall.

**5 Whys.**
1. Why did services score 13%? The services crate has almost no tests of its own. Its use-cases
   are reached only through foundry-app and the acceptance lanes.
2. Why? The use-cases need Postgres, and the crate has no Postgres-backed harness. The tracker
   names this twice (04-04 `room_still_routed`; Phase 5).
3. Why were per-step scores below the gate accepted? Steps measured in-diff mutants against crate
   tests within a 60-minute cap. Survivors were often killable only by scenarios that were still
   pending or owned by later steps. Each WARN was accepted with "re-measure at Phase 5", and no
   step owned the survivor.
4. Why did that reach Phase 5 with 181 survivors, two of them security-relevant? A deferral had no
   ledger and no owner, so survivors piled up. Security mutants were not treated differently.
5. **Root cause.** Accepting a mutation WARN needs no named owner and no due step. A crate whose
   only tests live in other crates cannot be measured cheaply at step time.

**Countermeasures.**
- **Prompts.** Accepting a WARN must name the step or test that will kill each survivor, and that
  goes in a survivors ledger that the step's hand-off updates. A security-relevant survivor
  (authority, CSRF, OAuth state, secrets) is never deferred.
- **Harness.** Add a Postgres-backed test harness for `foundry-services` use-cases, so services can
  be measured with its own tests (and `room_still_routed` killed).
- **Scripts.** Measure mutation through the consumer suites by default (crate + foundry-app +
  review tests), as Phase 5 did. Own-crate figures alone were misleading (lesson 5 in the
  evolution doc).

## 5. A busy machine

**Evidence.** PortNotExposed on 5432 made four mutation baselines fail ("cargo test failed in an
unmutated tree": `mutC2`, `mutD`, `mutL`, `rerunV` in the Phase 5 scratch). Infrastructure
failures counted as kills, so a classifier (`phase5/tainted.py`) was written to find caught
mutants whose failures were all infrastructure (PortNotExposed, SSLRequest, "could not resize
shared memory", connection reset, pool timeout) and re-run them. The 05-02 `xtask ci` ran 1997
of 2003 rows; the 6 us-03 shm failures passed alone, and the run stopped twice on PortNotExposed
and a connection reset at load average about 65. `eip_pre` refused because of foreign listeners
(another app's helper, a leftover `http.server`). The bench was skewed by load averages of 16-65.
The orchestrator reports readings up to about 250 during Phase 5; those are not in the saved logs.
Docker Desktop and colima were swapped during 04-03.

**5 Whys.**
1. Why the false kills and failures? Postgres containers failed to expose a port, ran out of
   shared memory, or reset connections.
2. Why? Other projects' test servers, containers and sessions were running on the same host at the
   same time. The tracker records leftover mailpit and postgres containers "not ours to kill", a
   canzan-lift session asked to restart its stack, and foreign 5xxxx listeners. Our own load
   contributed too: parallel mutation tracks (A-D) and lanes each started Postgres containers.
   *Hypothesis:* the split between foreign and own load was not measured; no per-process load
   snapshot was taken.
3. Why did that corrupt the results instead of stopping the run? Neither the lanes nor
   cargo-mutants check the host before starting. cargo-mutants counts any non-zero test exit as a
   kill, and the classifier was a one-off script in the scratchpad.
4. Why was it a one-off? Busy-host handling lives in memory notes and per-session judgement, not
   in the project's scripts.
5. **Root cause.** The project scripts assume a dedicated host. They have no load or port
   preflight and no way to tell an infrastructure failure from a test failure.

**Countermeasures.**
- **Scripts.** Move `tainted.py`'s classifier into the mutation wrapper. Re-run tainted kills
  automatically and report them separately. Fail the run, not the mutant, when the baseline fails
  on infrastructure.
- **Scripts.** Add a preflight to `eip-lanes.sh` and the mutation wrapper: refuse, or wait, when
  the 1-minute load is above the core count, or when foreign listeners or containers hold the
  test port ranges. Print what holds them (`lsof`, `docker ps`).
- **Scripts.** Scale `--jobs` for cargo-mutants and lane concurrency to the measured load, and cap
  Postgres containers per run (shm pressure).
- **Prompts.** G1 (the green Docker `xtask ci`) runs only after the preflight reports a quiet host.

## 6. An older test contradicted the fix

**Evidence.** R1's A2 fix (`717c3a5`) made a delivery at or past `give_up_at` unclaimable. The
store test `sweeps_apply_exactly_the_plan_ids` claimed past `give_up_at`, so it went red and
blocked the commit. The property at `core_properties` 249-279 asserted the same defective
behaviour. Both came from `7ceb851` (DISTILL; `git log -S` on the test names). Quinn fixed the store test (claim the fourth
delivery at t0, fault-proven) before R1 committed.

**5 Whys.**
1. Why did the commit block? A gold test expected a claim that A2 forbids.
2. Why did a gold test expect that? DISTILL wrote it as a fixture for the sweep and did not
   consider give-up while setting up the claim. The DESIGN invariant (never claim past give-up)
   was not stated as a test.
3. Why did the conflict appear only at commit time? The R1 RED tests (`aa222f7`) were added as
   `#[ignore]`. Nobody ran the existing suites against the intended invariant before the crafter
   implemented it.
4. Why did 01-07, which built give-up, not find it? A crafter does not edit tests (SLIM scope), so
   the test acted as the oracle and the code was written to satisfy it. 01-07 built the expiry
   sweep ahead of its owner step (03-01) against that test.
5. **Root cause.** Gold tests are treated as the specification, but nothing checks them against
   the DESIGN invariants. When a review changes an invariant, nothing first searches for tests
   that encode the old behaviour.

**Countermeasures.**
- **Prompts.** A RED dispatch for a review finding that changes an invariant must list the
  existing tests that assert the old behaviour (search the invariant's functions, then run the
  suites against a sketch of the fix), and update them in the same RED commit.
- **nWave (DISTILL).** State each DESIGN invariant (here: no claim at or past `give_up_at`) as one
  named test or property, and review fixtures against that list.

## 7. RED evidence taken after the fix (05-01)

**Evidence.** The tracker records that 05-01's RED was taken after implementing (`5b65501`). The
DES log still shows `RED EXECUTED PASS` at 18:02:45Z, GREEN at 18:26:43Z and COMMIT at
18:31:11Z. It looks the same as every other step. In this log "PASS" on a RED entry means that the phase
gate passed (only 01-10's blocked attempt is logged FAIL), so the value says nothing about the
order.

**5 Whys.**
1. Why was RED not proven? The failing run was captured after product code existed.
2. Why did the DES log accept it? A DES entry records phase, status, result and time. It does not
   record the tree state or point at a failing run, so a RED entry written later looks valid.
3. Why did the crafter go out of order? *Hypothesis, requires verification:* 05-01 was flagged
   `sizing_review_needed` (prompt-0501) and touched many layers. The crafter built across them
   before recording the RED. The crafter's report was not kept.
4. Why did the orchestrator find it only afterwards? The orchestrator's acceptance check reads the
   crafter's report and the DES phases. Nothing compares the RED time with when the product code
   changed.
5. **Root cause.** RED is a self-reported claim with no proof tied to the tree, and large steps
   make it tempting to skip.

**Countermeasures.**
- **nWave (DES).** A RED entry records the tree hash and the path of the failing-run log, and the
  DES check refuses a RED whose tree already has production changes for that step
  (`git diff --stat -- 'crates/*/src'` empty against the step's base).
- **Prompts.** Split any step flagged `sizing_review_needed` before dispatch, and ask for the RED
  log path in the hand-off.
- **Prompts.** When RED was not proven, mark it NOT_PROVEN in the record, as the evolution doc's
  lesson 6 says.

## Cross-check

The seven root causes do not contradict each other. Two of them link:
- Issue 3 (DISTILL tests not fault-proven) and issue 6 (gold tests not checked against invariants)
  both trace back to `7ceb851`. Together they explain A2: a test pinned the defect, so the lanes
  stayed green (issue 2).
- Issue 4's deferrals left A3/A4 and `room_still_routed` as survivors rather than findings.

## Not analysed here

- The migration "deadlock detected" flake: it happens on the pre-feature base too, so it is not a
  DELIVER cause. It is open before the tag (`upstream-issues.md` §5).
- FU-11 (degraded mode may not refuse route writes): a possible gap, not yet confirmed
  (`upstream-issues.md` §3c).

## Top countermeasures

1. **Fault-proof every DISTILL test, and check gold tests against DESIGN invariants** (nWave
   DISTILL plus the reviewer checklist). This addresses issues 3, 6 and part of 2.
2. **A harness contract and a driver dry run in DISTILL** (harness plus nWave). This addresses
   issue 1, the most frequent cost.
3. **Host preflight and an infrastructure-failure classifier in the lane and mutation scripts**
   (scripts). This addresses issue 5 and unblocks G1.

Also cheap: a survivors ledger with named owners and no deferral of security mutants (prompts,
issue 4); a variant matrix for multi-kind features (nWave, issue 2); RED tied to a tree hash
(nWave DES, issue 7).
