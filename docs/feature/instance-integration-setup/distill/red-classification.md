# RED classification — instance-integration-setup (Deliverable A)

## Re-run after the Sentinel review (2026-11-06)

Same lane and conditions. Result: 59 examples, 0 passed, 59 failed; 0 BROKEN; 362 of 421 steps
passed. Every step of the feature matches exactly one definition (checked over the expanded
outlines; no undefined or ambiguous step). Changed or new examples:

| Scenario (examples) | Class | Observed |
|---|---|---|
| No access, Priya row (1) | MISSING_FUNCTIONALITY | "Priya is on the instance integrations page": 404 |
| No access, other rows (8) | MISSING_FUNCTIONALITY | the not-found step passes vacuously today; "still has no secrets key" fails on Priya's 404 |
| Race, then connect (1) | MISSING_FUNCTIONALITY | both generate POSTs 404 (fails before the new When) |
| Replica Move (1) | MISSING_FUNCTIONALITY | When Move: page 404; the replicas, connection and route Givens passed |
| Return exchange with the saved secret (1) | MISSING_FUNCTIONALITY | Given generate: page 404 |
| I-4 + GET (2); recorded + Matrix form (1); takeover + events (1); different file + not lost (1); G1 sweep (1) | MISSING_FUNCTIONALITY | fail at the same step as before; the added Thens come later |

The first run is below.

## First run

Run: 2026-11-06, Lima VM `foundry-test`, `FOUNDRY_ACCEPTANCE_TAGS=instance-integration-setup`,
with `@pending` stripped from the VM's copy of the feature only. The debug `foundry` binary was
built and warmed first. Working tree: `e92de24` plus the DISTILL changes.

Result: 56 examples, 0 passed, 56 failed; 0 BROKEN. 342 of 398 steps passed: every Background
step, every key setting, the replicas, the operator's processes and every eip precondition
(connections, routes, filing). The first run hit one BROKEN step: the instance-admin Given read a
`users.email` column that does not exist. It was fixed to resolve Priya's id through the roster
and re-run (this table).

Where each failure sits:
- 29 in a Then, on the missing page or door (`/admin/instance/integrations` and its POSTs
  answer 404), a missing link, a missing refusal, or a missing file.
- 27 in a Given or When that goes through a new door or link ("Priya has generated the secrets key",
  "has saved the Slack app", "chooses …", "generates … on the first server"). The step asserts
  that the product's own page answered, and it answers 404. That is the missing behaviour, not
  setup: every earlier step passed.

| Scenario (examples) | Class | Observed |
|---|---|---|
| WS: Matrix then Slack on every replica (1) | MISSING_FUNCTIONALITY | instance page 404 (Then I-3) |
| No secrets key: I-25, I-3, I-9 (1) | MISSING_FUNCTIONALITY | page 404 |
| With a secrets key (1); I-29 (1); I-26 (1); I-13 (1) | MISSING_FUNCTIONALITY | Given generate/save: page 404 |
| Legacy instance I-2e (1); legacy I-2e0 (1) | MISSING_FUNCTIONALITY | page 404; the direct-sealed Slack connection was made by today's code |
| I-4, generate / move anyway (2) | MISSING_FUNCTIONALITY | POST door 404 |
| I-27 (1); I-31 (1) | MISSING_FUNCTIONALITY | page 404 |
| Dashboard I-1 link (1) | MISSING_FUNCTIONALITY | the dashboard has no "Integrations" link (fails in the When) |
| Only an instance admin (8) | MISSING_FUNCTIONALITY | the not-found step passes vacuously today (every door is 404); "this Foundry still has no secrets key" fails on Priya's 404. The rows become discriminating once Priya's page answers |
| Generate race (1) | MISSING_FUNCTIONALITY | both POSTs 404 |
| Dump (1); later generate (1); recorded (1); restart probe (1) | MISSING_FUNCTIONALITY | Given or When generate: page 404 |
| Generate where it does not apply (3) | MISSING_FUNCTIONALITY | POST door 404 |
| Slack save (1); blank/malformed (6); events/install (1); stranger event (1) | MISSING_FUNCTIONALITY | Given generate: page 404 |
| Save without a secrets key (1) | MISSING_FUNCTIONALITY | POST door 404 |
| Upgrade keeps delivering (1) | MISSING_FUNCTIONALITY | delivery to #auth-dev passed (today's direct seal); the I-2e line fails, page 404 |
| Rename to FOUNDRY_INSTANCE_KEY (1) | MISSING_FUNCTIONALITY | no message reaches #auth-dev: today ignores the new name, so the direct seal no longer opens (the alias is missing) |
| Move n = 2 (1); takeover (1) | MISSING_FUNCTIONALITY | Given save app: page 404 |
| Move n = 0 (1); environment wins (1) | MISSING_FUNCTIONALITY | When: page 404 |
| Standalone key file (1) | MISSING_FUNCTIONALITY | process page 404 (no file is made) |
| Two servers, one directory (1) | MISSING_FUNCTIONALITY | no instance key file in the directory |
| Reuse (1); different file (1); file → operator key (1) | MISSING_FUNCTIONALITY | Given generate: page 404 |
| Empty / same-value settings (2) | MISSING_FUNCTIONALITY | page 404 |
| Refused start: short key, conflicting names (2) | MISSING_FUNCTIONALITY | the process keeps running (killed after 20 s, exit None): today ignores FOUNDRY_INSTANCE_KEY |
| G1 sweep (1) | MISSING_FUNCTIONALITY | Given generate: page 404 |
| Reload alarm (1) | MISSING_FUNCTIONALITY | When generate on the first server: page 404 |

The generated-key test seam (`secrets_key_seam`, RED scaffold) was not reached in this run:
every scenario calling it fails earlier, at Generate. Once Generate answers, the dump and sweep
scenarios panic with "Not yet implemented -- RED scaffold" until DELIVER implements the seam.

DELIVER: un-pend story by story, walking skeleton first. Keep the no-access rows with a Priya
precondition, so they stay discriminating.
