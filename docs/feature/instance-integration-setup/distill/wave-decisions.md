# DISTILL Decisions — instance-integration-setup (Deliverable A)

Scope: US-IIS-01, 02, 03, 06 and 08. AC parts tagged Deliverable B (I-19, I-20, Replace, Remove,
the I-14 confirm, Start over, US-IIS-06 AC 6) are out of scope.

Read before writing:
- ✓ docs/product/journeys/journey-external-integration-plugins.yaml (step 0, its failure modes)
- ✓ docs/product/architecture/brief.md "Instance integration setup", incl. "For Acceptance
  Designer"; ADR-IIS-001 to -005
- ✓ docs/product/kpi-contracts.yaml (instance-integration-setup block)
- ✓ discuss/: user-stories.md, story-map.md, wave-decisions.md, outcome-kpis.md,
  journey-instance-integration-setup.yaml (+ visual), dor-validation.md
- ⊘ spike/ (none ran)
- ✓ design/: wave-decisions.md, upstream-changes.md, options.md
- ✓ devops/wave-decisions.md
- ✓ AGENTS.md (Deployment independence), CODING_STANDARDS.md
- ✓ docs/architecture/atdd-infrastructure-policy.md
- ✓ precedent: integrations-wayfinding distill/
- Deliverable type: application (no `deliverable_type` in `.nwave/des-config.json`)
- Language: Rust, cucumber-rs (`[lang-mode] rust`). No `tests/common/state_delta.*` port: the
  repo's Rust reading of Mandate 8 is the eip `Universe` / `capture_before` (inherited)

Reconciliation passed: 0 contradictions. The DISCUSS D7 narrowing (a different client ID needs
both secrets) was approved by the user and is Deliverable B. DEVOPS confirms it contradicts nothing
in DESIGN. One stale DESIGN brief line (pending app "in the session") touches B only:
upstream-issues.md item 6.

## Key Decisions
- [D1] One feature file, `crates/foundry-acceptance/tests/features/instance-integration-setup.feature`:
  41 scenario declarations, 59 examples, all `@pending` (after the Sentinel review, 2026-11-06).
  - Architecture of Reference: real HTTP through the production router, real Postgres, and the
    real filesystem for the key file (a temp directory per scenario). Slack is the shipped double.
  - Layer 3 throughout, so examples only, with sad paths enumerated (Mandates 9 and 11).
  - No Tier B. The chained journeys (legacy → move; file → operator key) have few, enumerable
    states. The pure decisions (key mode, offers, shapes) are DELIVER's table tests (brief.md "For
    Software Crafter").
- [D2] Lanes and tags follow DEVOPS D8.
  - In process (`InProcHarness`): most scenarios.
  - `@iis-replica`: `MultiReplicaHarness`, each replica asked at its own address, the shared
    `MockClock` advanced 1 s per poll. Used by the walking skeleton, the Generate race and the
    replica Move (US-IIS-06 AC 3). The poll keeps a 200 ms real pause: ADR-IIS-004 r8 puts
    freshness, pruning, rate limits and expiry on the `Clock`, but the reload period is a timer,
    so advancing the clock alone would not let a reload happen.
  - `@iis-subprocess`: the prebuilt `foundry` binary. Used by the standalone key file, servers
    starting together on one directory, the restart and probe, refused starts, the G1 sweep
    (real log output) and the reload alarm. The alarm runs on two processes because metrics are
    per process, so it is `@iis-subprocess` only.
  - Every scenario also carries its `@us-iis-0N` and `@kpi-iis-N` tags, and one `@walking_skeleton`
    (DISCUSS D15: 01 → 02 → 03 with an operator-set key, on two replicas).
  - No new runner exclusion: `@pending` already keeps every scenario out of every lane.
- [D3] The driver extends the eip driver rather than copying it:
  `support/instance_integration_setup.rs` and `steps/feature_instance_integration_setup.rs`.
  - Reused unchanged: the Background, "<who> reads", "gets the same not-found page", "the
    workspace integrations page reads", "the next issue … becomes", "Lena has routed", "Sam
    files", "#auth-dev shows within 10 seconds", the events-address answers and "health signals
    … healthy".
  - The eip driver gained, without weakening any assertion:
    - `InstanceSpec.public_url` and `.extra_env`, defaulting to today's values;
    - `pub(crate)` on `http`, `email_of`, `cookie`, `went_through`, `ensure_slack` and
      `database_url`;
    - `start_expecting_refusal`, factored out of `when_start_with`, whose behaviour is unchanged.
  - The Slack double accepts a scenario's client secret in the token exchange
    (`expect_client_secret`, default unchanged).
- [D4] Settings reach Foundry the way the operator gives them.
  - In process: `iis::boot`, today `eip::rewire` through `IntegrationSettings::from_lookup`.
    DELIVER routes `boot`, and so every in-process "restart", through the one shared boot
    function (brief.md "Boot and restart").
  - Subprocess: the process environment.
  - Every scenario gets its own `FOUNDRY_DATA_DIR`. The real `/var/lib/foundry` is never touched,
    and the Gherkin names the temp directory `/var/lib/foundry` through `eip.names`.
  - Deployment independence (AGENTS.md): the public URL is `https://foundry.example.org`, and no
    environment is named.
- [D5] Observations are made through pages, process output, `/readyz`, `/metrics` and the key
  file. There are two waivers of the policy's "no table reads", both named upstream:
  - the dump attack reads every table (US-IIS-02 AC 3);
  - the race counts `integration_secrets_key` rows (brief.md).
  Policy rows were appended to atdd-infrastructure-policy.md (driving, the key file, the
  instance tables, the test seam).
- [D6] One RED scaffold (Mandate 7): `crates/foundry-app/src/integrations/secrets_key_seam.rs`
  (`// SCAFFOLD: true`, `panic!("Not yet implemented -- RED scaffold")`).
  - It is gated on `test-support` in `integrations/mod.rs`.
  - Signature: `generated_secrets_key(pool: &PgPool, instance_key: &str) -> Option<String>`. It
    takes a pool, not `Store`, because of check-arch `integrations-store-capability`. The dump
    check offers every key-shaped value in the dump to it, because opening stays inside the
    integrations module (`integrations-secret-seam`).
  - It is the generated-key reader DISCUSS D3 and DESIGN D8 require. Nothing else needed one:
    every other door is HTTP or a process.
  - DELIVER lists it in check-arch `integrations-test-seam` and gives it a marker the Dockerfile
    refuses (DEVOPS task 1).
- [D7] Matrix connections are replaced by Slack connections as "a saved credential", because
  connecting Matrix needs Synapse (upstream-issues.md item 3). "Matrix ready" is observed as the
  Connect Matrix form on the workspace page.
- [D8] Outcomes registered as OUT-38 to OUT-42. `nwave-ai outcomes register` reformatted the
  whole registry, so it was restored and the five entries were appended by hand, as for
  OUT-36/37.
  - OUT-38 (specification): page state and offers per key state.
  - OUT-39 (operation): Generate and Move.
  - OUT-40 (operation): Slack app save.
  - OUT-41 (invariant): nothing shows or yields a key.
  - OUT-42 (invariant): one key file, never overwritten.
- [D9] The walking skeleton is `@pending` like the rest, as in every feature since ADR-025 here:
  a new page has no green end-to-end path before DELIVER.

- [D10] Sentinel review (2026-11-06, needs_revision) fixes:
  - The G1 log scan uses `eip::process_output_present`. Both log files must exist and the start
    line must be present, so a missing log can no longer pass. `process_output` and its eip
    callers are unchanged.
  - Planted values are looked for in every encoding in pages and logs, as well as in the dump.
  - Added an `@iis-replica` Move scenario, with each replica's channel picker and route history
    polled.
  - The no-access outline gained a Priya row that must reach the page.
  - The race's side effect moved to a When ("Priya connects …").
  - "Not offered" also fails on any link or button that carries the copy.
  - New assertions: the Matrix route form no longer reads W-2 (US-IIS-02 AC 2); the return
    exchange uses the saved client secret (US-IIS-03 AC 4); the takeover verifies events
    (US-IIS-06 AC 5); I-4 on the GET page (US-IIS-01 AC 5); not lost under I-26
    (US-IIS-08 AC 4).
  - US-IIS-06 AC 2's failed Move is a DELIVER store test (upstream-issues item 11).
  - The product owner removed `/api/v1/integrations` from the ACs and added I-22's n = 1 form;
    nothing here referred to either.
- [D11] **Operator persona vocabulary (exception to business-language purity).**
  - Setting names stay in the Gherkin where they are the operator's own words: the locked copy
    (I-7, I-17, I-29, I-31) names them, and so do RELEASING and the k8s README. Examples:
    "the legacy FOUNDRY_SECRETS_KEY setting", "FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY …",
    and the `health.startup.refused` line.
  - Reworded where plain words work: "the operator has not given this Foundry its public
    address" and "the warning about the unreachable address".
- [D12] `https://foundry.canzan.dev` (the eip default public URL) and "Homelab Ops" are
  pre-existing fictional fixture data for the Canzan Labs persona company, not deployment
  environments. AGENTS.md's rule is about real hosts, so they stay. This feature's own scenarios
  use `https://foundry.example.org`.

## Scenario list

| Scenario (examples) | Tags | State |
|---|---|---|
| WS: Matrix then Slack, every replica, no restart | @walking_skeleton @driving_port @iis-replica @us-iis-01/02/03 @kpi-iis-1 @kpi-iis-2 | pending |
| No secrets key: I-25 operator, I-3, I-9, Generate, no Slack form | @us-iis-01 @driving_port @kpi-iis-2 | pending |
| Secrets key: I-2, I-5, both addresses, manifest = setup page's, I-8 | @us-iis-01 @kpi-iis-2 | pending |
| I-29 before the addresses | @us-iis-01 @kpi-iis-2 @error | pending |
| Legacy instance: I-2e, Move, I-7, no key or secret on the page | @us-iis-01 @us-iis-06 @kpi-iis-2 @kpi-iis-g1 | pending |
| Legacy, nothing saved: I-2e0, Move, Matrix ready | @us-iis-01 @us-iis-06 @kpi-iis-2 | pending |
| I-26 restore-only (no Generate, Replace, Start over, Slack form) | @us-iis-01 @us-iis-08 @kpi-iis-2 @error | pending |
| I-4: generate / move posted anyway change nothing (2) | @us-iis-01/02/06 @kpi-iis-2 @error | pending |
| I-13, and W-2 on the workspace page | @us-iis-01 @kpi-iis-2 @error | pending |
| I-27: board serves, ready, nothing offered | @us-iis-01 @us-iis-08 @kpi-iis-2 @error | pending |
| I-31: file kept, nothing offered | @us-iis-01 @us-iis-08 @kpi-iis-2 @error | pending |
| I-1 link lands on the page | @us-iis-01 @kpi-iis-g2 | pending |
| Only an instance admin: Priya reaches it; Tomás, Lena, Sam, Marco, signed out refused; posts by Tomás and signed out (9) | @us-iis-01/02/03 @kpi-iis-g2 @error | pending |
| Generate race: one key, both answers; then a connection opens on every replica | @iis-replica @us-iis-02 @kpi-iis-1 @error | pending |
| Database dump opens nothing | @us-iis-02 @us-iis-03 @kpi-iis-g1 | pending |
| A later generate keeps the key | @us-iis-02 @error | pending |
| Generate changes nothing under I-27, I-31, direct mode (3) | @us-iis-02 @us-iis-08 @error | pending |
| Generating is recorded; Matrix offered | @us-iis-02 @kpi-iis-1 | pending |
| Restart: key in effect, probe healthy | @iis-subprocess @us-iis-02 @kpi-iis-1 | pending |
| Slack save: I-10, I-6, set/set, activity, Add to Slack | @us-iis-03 @driving_port @kpi-iis-1 @kpi-iis-g1 | pending |
| Blank or malformed field (6) | @us-iis-03 @error | pending |
| Save without a secrets key: I-3 | @us-iis-03 @error | pending |
| Events verify, install uses the saved app | @us-iis-03 @kpi-iis-1 | pending |
| Add to Slack exchanges with the saved client secret | @us-iis-03 @kpi-iis-1 | pending |
| Event signed with another secret: 401 | @us-iis-03 @error | pending |
| Upgrade: direct seal keeps delivering, I-2e | @us-iis-06 @kpi-iis-5 | pending |
| Rename to FOUNDRY_INSTANCE_KEY, same value | @us-iis-06 @kpi-iis-5 | pending |
| Move: I-22 n = 2, activity, not lost, no email, delivers | @us-iis-06 @kpi-iis-5 @kpi-iis-4 | pending |
| Replica Move: I-22 n = 1, opens on every replica, history delivered on every replica | @iis-replica @us-iis-06 @kpi-iis-5 @kpi-iis-4 | pending |
| Move with n = 0 answers I-21 | @us-iis-06 @error | pending |
| Environment wins (D9): I-17, install uses env client ID | @us-iis-06 @kpi-iis-5 | pending |
| Saved app takes over when SLACK_* removed | @us-iis-06 @kpi-iis-5 | pending |
| Standalone: I-25 file, mode 0600, one-click Matrix | @iis-subprocess @us-iis-08 @kpi-iis-6 @kpi-iis-2 | pending |
| Two servers, one directory: one file, shared key | @iis-subprocess @us-iis-08 @kpi-iis-6 @error | pending |
| Key file reused on restart | @us-iis-08 @kpi-iis-6 | pending |
| Different key file: I-26, ready | @us-iis-08 @kpi-iis-2 @error | pending |
| File → operator key: I-30, system activity line, connection opens | @us-iis-08 @kpi-iis-2 @kpi-iis-g1 | pending |
| Empty names unset; same value = legacy (2) | @us-iis-08 @kpi-iis-2 @error | pending |
| Refused start: short key, conflicting names (2) | @iis-subprocess @us-iis-08 @kpi-iis-g1 @error | pending |
| G1 sweep: pages, logs, activity, database | @iis-subprocess @us-iis-02/03/08 @kpi-iis-g1 | pending |
| Reload alarm: I-26 and degraded metrics on the second server | @iis-subprocess @us-iis-08 @us-iis-01 @kpi-iis-1 @error | pending |

Examples tagged `@error`: 37 of 59 (63%). Declarations by story (a scenario may carry several):
US-IIS-01 14, 02 10, 03 10, 06 10, 08 13.

KPI-IIS-2's 17 states: I-25 (operator, file), I-30, I-27, I-31, I-2, I-2e, I-2e0, I-3, I-26, I-4,
I-5, I-6, I-7, I-13, I-29, I-8 and I-9 each appear in at least one scenario. KPI-IIS-3 (I-19) and
most of KPI-IIS-4 (confirm pages) are Deliverable B. In A, KPI-IIS-4's "nothing lost, no email" is
checked on Move. KPI-IIS-G3 is the unchanged eip and wayfinding suites.

## Driving adapters (DESIGN, Deliverable A)

| Door | Scenarios |
|---|---|
| `GET /admin/instance/integrations` | every page-state scenario, the no-access outline |
| `POST …/secrets-key` (from the page, and posted directly) | WS, race, recorded, later generate, withheld outlines, no-access |
| `POST …/secrets-key/move` | Move (n = 2, n = 0), I-4 outline |
| `POST …/slack` | WS, save, blank/malformed, no key, env wins, no-access |
| `GET /admin/instance/workspaces` (I-1) | dashboard link |
| Process start (`FOUNDRY_INSTANCE_KEY`, `FOUNDRY_SECRETS_KEY`, `FOUNDRY_DATA_DIR`) | subprocess scenarios, refused starts |
| Shipped doors observed: `/workspace/integrations`, route form, `/integrations/slack/events`, `…/slack/install` and `/return`, `/readyz`, `/metrics` | as named in the steps |

## Adapter coverage

| Adapter | Real I/O | Covered by |
|---|---|---|
| Postgres (store, migration 0033) | yes | every scenario; dump and race read tables |
| Key file (`key_file.rs`) | yes, temp dir | standalone, two servers, reuse, different file, I-27, I-31, I-30 |
| Reload task (`reload.rs`) | yes | WS and race (two replicas), reload alarm (two processes) |
| Slack Web API / OAuth / Events | Slack double | connections, install redirect, signed events |
| Prometheus metrics | yes, per process | restart (healthy), reload alarm (degraded) |

## Pre-requisites
- DESIGN driving ports above; DEVOPS D8 lanes. Subprocess scenarios need the binary built and warm
  (`cargo build -p foundry-app --bin foundry`, then exec it a few times).
- Acceptance runs only with `FOUNDRY_XTASK_INCLUDE_DOCKER=1` in `cargo xtask ci` (DEVOPS D8).

## Mandate 12
Criteria 1–3 are met:
- typed vocabulary: `KeySetting`, `Restart`, `InstanceDoor`, `Button`, `Offer`, `WorkspaceOffer`,
  `Signer`, `BadStart`, `Refusal`;
- every step body is one driver call with no control flow.

Step reuse ratio (informational): 278 step lines over 60 new decorators plus the reused eip
phrases, about 4.6×.

## Upstream Changes
See upstream-issues.md: `/api/v1/integrations` does not exist, I-22 has no n = 1 form, Matrix is
replaced by Slack as the sealed credential, the stale brief line, and the alarm scenario runs on
two processes.
