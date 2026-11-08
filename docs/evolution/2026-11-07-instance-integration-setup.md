# Evolution: instance-integration-setup, Deliverable A (an instance admin turns on Matrix and Slack from the browser, on any deploy, with no restart; keys stay wrapped by an instance key that never lives in the database)

**Finalized**: 2026-11-07 (Deliverable A only)
**Commits**: `9aa38d9` .. `5ac8f5d` (30 commits; per step below).
**Ships as**: v0.19.0. Not pushed, not released and not tagged at this finalize. Tag only on the
user's go.

**Scope**: Deliverable A, US-IIS-01, 02, 03, 06 and 08.
- **Deliverable B is not built**: Replace and Start over (US-IIS-07), Slack Replace, Remove and the
  I-14 confirm (US-IIS-05), the I-19 and I-20 links (US-IIS-04), and US-IIS-06 AC 6.
- The feature workspace `docs/feature/instance-integration-setup/` therefore **stays active**.
  Deliverable B's DELIVER runs from the same DISCUSS, DESIGN, DEVOPS and DISTILL documents.

Sources:
- `docs/feature/instance-integration-setup/discuss/`, `design/`, `devops/`, `distill/` (each
  `wave-decisions.md`), `discuss/outcome-kpis.md`
- `deliver/roadmap.json` (18 steps, approved `6e593b0`), `deliver/execution-log.json`
- `deliver/upstream-issues.md` items 1-52 (issues, deviations, review triage, gate results)
- `deliver/wave-decisions.md` (demo evidence, post-merge gate), `deliver/mutation/mutation-report.md`
- ADR-IIS-001 to -005 and brief.md "Instance integration setup" in `docs/product/architecture/`

## Summary

Before this feature the secrets key and the Slack app were environment settings: an operator edited
`FOUNDRY_SECRETS_KEY` and `SLACK_*` and restarted or rolled every replica. A standalone deploy with
neither had integrations off until someone read RELEASING. Now:
- **Instance page.** `/admin/instance/integrations` (instance admins only, uniform 404 otherwise,
  linked from the instance dashboard, I-1) shows the instance key, the secrets key, Slack and Matrix
  states, each with its I-n copy and next step.
- **Generate.** One button makes a 32-byte secrets key, create-if-absent across replicas. It is
  stored wrapped under the instance key and never shown. Matrix is ready at once (I-21, I-8).
- **Slack app.** The instance admin pastes client ID, client secret and signing secret. The app is
  shape-checked, sealed in `instance_slack_app`, and in effect for install, return and events at
  once (I-10, I-6). The environment's trio still wins when set (I-7, I-17).
- **Upgrade.** A legacy `FOUNDRY_SECRETS_KEY` instance runs in **direct mode** with today's
  behaviour. **Move** re-seals every saved credential under a new secrets key in one transaction;
  nobody reconnects (I-22).
- **Standalone key file.** With no key setting, Foundry makes `instance.key` (0600) in
  `FOUNDRY_DATA_DIR` and names it on the page for backup (I-25). An operator key set later takes
  over the file at boot (I-30).
- **Every replica.** The serving replica applies at once; the others pick up the change within one
  5 s poll of `integration_config_version`.
- **Operators.** `foundry doctor rotate-secrets` re-wraps the secrets key under a new instance key.
  The image keeps `/var/lib/foundry` as a volume, and compose mounts `foundry-data`.
- One migration (0033), no new crate.

## Business context

The user asked to turn on Slack and Matrix from the browser (2026-11-05) after integrations-wayfinding
made the "not set up" cause visible but not fixable. Deployment independence (AGENTS.md) was a hard
constraint. A compose, plain Kubernetes or single-binary deploy must reach "Matrix ready" with zero
key settings (KPI-IIS-6), and no key or secret may ever be shown or stored in plaintext
(KPI-IIS-G1, OD-12's goal kept). The research depth was light: no DIVERGE or DISCOVER ran, and the
need comes from the user's own requests, while the job was validated by external-integration-plugins.

KPIs (all measured by acceptance on each gate run; KPI-IIS-M by a manual walk after release):
- KPI-IIS-1: 0 restarts and 0 environment edits, every replica within 60 s. Covered by the walking
  skeleton on two replicas.
- KPI-IIS-2: every Deliverable A instance state shows its I-n.
- KPI-IIS-5: legacy direct seals keep opening through upgrade and Move.
- KPI-IIS-6: one click (Generate) after first start on a standalone deploy.
- KPI-IIS-G1 to G3: planted-string grep through the feature-gated test seam, the uniform 404 outline,
  and eip and wayfinding lanes 100% green.
- KPI-IIS-3 (I-19) and KPI-IIS-4 (confirm pages) belong to Deliverable B.
- KPI-IIS-M is not yet walked.

## Key decisions

- **Outer key: `FOUNDRY_INSTANCE_KEY`, with `FOUNDRY_SECRETS_KEY` as a legacy alias**
  (ADR-IIS-001).
  - The instance key wraps a stored secrets key, and the secrets key seals credentials in the
    shipped `SecretBox` format.
  - The box also opens with every held instance key, so legacy direct seals keep opening.
  - The two names with different values, or a malformed value, refuse start.
  - `FOUNDRY_INSTANCE_KEY_PREVIOUS` is a comma-separated list, used for opening only.
- **Direct mode (ruling 1e).** With only the legacy name and no stored secrets key, behaviour is
  exactly today's (I-2e, or I-2e0 when nothing is saved). Generate and Move are one use case.
  Unopenable direct seals show I-4 and withhold both buttons.
- **Key file in `FOUNDRY_DATA_DIR`** (default `/var/lib/foundry`, ADR-IIS-002).
  - `instance.key` is base64url, mode 0600. It is made by temp file, fsync, `link(2)` and
    read-back.
  - It is never overwritten, and never minted when the database already holds a secrets key or
    direct seals (Withheld: I-26 or I-4 instead).
  - An operator key replacing it holds the file key as previous and re-wraps by compare-and-set
    at boot (I-30).
  - Kubernetes ships no data volume: a per-pod `emptyDir` would mint a key per pod. Operators set
    the key in the Secret (DEVOPS D3).
- **5 s version-polled reload** (ADR-IIS-004).
  - Writes bump `integration_config_version` in the same transaction.
  - Each replica polls it every 5 s (`FOUNDRY_INTEGRATIONS_RELOAD_MS`) and reads version and state
    in one `REPEATABLE READ` snapshot.
  - Each replica writes only its own `integration_replica_states` row.
  - An unknown key id triggers an immediate reload.
- **One install path** (review H4, `9dff994`). Boot, the serving replica's post-write rebuild and
  the polled reload all go through one `swap`. It refuses a lower applied version and calls
  `raise_alert_inputs`, the one place that sets the degraded gauge, `probe_failures_total{probe_name="secrets"}`
  and the `health.degraded probe=secrets` line.
- **Seal fence.**
  - One advisory lock serialises key and app writes. Sealing writes take it shared, in the store
    transaction, and fence on the sealing key id.
  - check-arch `integrations-seal-fence` lists the allowed writers (`SEAL_FENCED_WRITERS`, now 5).
  - A writer that loses the fence reloads and retries once.
- **`foundry doctor rotate-secrets`.** The CLI re-wraps the secrets key under the current instance key
  with the server stopped (`Store::rotate_instance_key`, exclusive lock, all or nothing). The
  previous key comes from `FOUNDRY_INSTANCE_KEY_PREVIOUS`, and the output lines are quoted in
  RELEASING R-5. A lost CAS exits 3 (`RotateOutcome::KeyChanged`).
- **Slack app** (ADR-IIS-003). Single-row `instance_slack_app`, secrets sealed. The environment's
  trio wins. A saved app that does not open is not in effect (I-13). `instance_slack_app_pending`
  is Deliverable B and not in 0033 (item 6).
- **Activity** in the new `instance_integration_audit_events` (ADR-IIS-005). It is shown on the
  workspace activity list, for example "generated the secrets key" and "saved the Slack app".

## Work completed

| Step | What | Commit |
|---|---|---|
| 01-01 | Instance key settings, legacy aliases and refused starts | `9aa38d9` |
| 01-02 | Migration 0033 and the instance integrations store | `45ef951` |
| 01-03 | Key mode, credential box and the one shared boot function | `6d02c74` |
| 02-01 | Instance integrations page before a secrets key, and the dashboard link | `7f9f604` |
| 02-02 | Generate the secrets key; the sealed page and Slack's addresses | `e9bb484` |
| 02-03 | Save the Slack app, and one access rule for every door | `c787122` |
| 02-04 | Reload on every replica (walking skeleton) | `d73c59b` |
| 03-01 | Direct mode with saved credentials and the environment's app | `b0aa18d` |
| 03-02 | Slack app shape check | `ce0e3f4` |
| 03-03 | The saved Slack app in effect for events, install and return | `9b8a254` |
| 03-04 | Seal fence, the generated-key test seam, and the dump | `6404edf` |
| 04-01 | Move to a secrets key; Generate and Move withheld under I-4 | `729274c` |
| 05-01 | The Foundry-made key file at boot, and its page states | `4d9f5ef` |
| 05-02 | Key file across restarts and servers sharing a directory | `5c82d08` |
| 05-03 | Operator key takes over the file (I-30); `doctor rotate-secrets` re-wraps | `c947ba9` |
| 06-01 | Alert inputs on reload, the restart probe, the never-shown sweep | `b4e4fbc` |
| 06-02 | Image data directory and the compose volume | `efe406b` |
| 06-03 | Operator docs and examples for the new settings | `a871310` |

Other commits:
- Roadmap `6e593b0`.
- DELIVER logs and upstream notes: `5e133b7`, `03e05a2`, `e0aa5f8`, `98dfa25`, `896f9d6`.
- Review revision `9dff994`; refactor `bc9af98` (notes `158b70c`).
- Mutation `3aecfd1`, `ae73c5e` (notes `5ac8f5d`).

DES integrity: "All 18 steps have complete DES traces". It does not detect that RED was logged before
it ran in 05-02 and 05-03 (items 38, 42; see Lessons).

Demo evidence (`deliver/wave-decisions.md`): every story was observed on the real release binary
in the Lima VM, against a throwaway Postgres with a temp data directory, driven with curl. US-IIS-08,
01, 02, 03 and 06 and `doctor rotate-secrets` each gave the expected I-n lines. The planted secret
values were echoed 0 times in pages, and the key value occurred 0 times in the server log.

## Post-merge gate history

1. **Blocked** (2026-11-07, HEAD `a871310`, item 48). The IIS (59/597), eip (181/1942), wayfinding
   (54/400) and docker-compose (4/23) lanes were green. `cargo xtask ci` failed in
   `foundry-app --lib`: `key_file::tests::two_servers_racing_on_one_directory_hold_one_key`
   read "expected a held key, got Unwritable", failing 11 of 30 runs alone.
   - Cause (review H1): `probe` removed every `instance.key.tmp.*`, including a sibling replica's
     in-flight temp file on a shared volume. That replica's `link(2)` then failed NotFound and read
     as Unwritable (I-27).
2. **Adversarial review: NEEDS_REVISION** (item 47). The orchestrator triaged it:
   - B1 (Direct-mode Slack form) was refuted.
   - H1, H3 (Withheld showed I-25 "made by Foundry in {path}") and H4 (post-write install raised no
     alert inputs and was not version-CAS) were confirmed.
   - H2: the unlocked `reseal_credentials` was to be deleted.
   - B2: the mutation phase was to prove the r3 guard, the `wrapped_by_key_id` CAS and the EPERM
     mapping.
   - M1 to M5 were minor.
3. **Fixed in `9dff994`** (item 49):
   - H1: a temp file is stale only after 10 min (`STALE_TEMP`). NotFound on link reads the sibling's
     file or retries once. The race test passed 100/100 alone.
   - H4: one `swap`.
   - H3: no key-file line when Withheld.
   - H2: `reseal_credentials` removed.
   - M2: one credential tally. M3: exit 3 on a lost CAS. M1: a placeholder refusal.
   - A blank `FOUNDRY_PUBLIC_URL` now counts as unset.
   - `xtask smoke` and Docker `xtask ci` (2132 scenarios): all gates green.

## Refactor

`bc9af98` (item 50), L1-L3; L4-L6 found nothing to change.
- Boot reads the store rows once and rebuilds over that snapshot (item 40 closed).
- The key text is held in `Zeroizing<String>`.
- One `CONNECT_ATTEMPTS` constant and one fence retry path; Generate and Move share `key_door`.
- `reseal_sealed_values` renamed in ADR-IIS-004 and brief.md.
- Kept: the probe still runs under an operator key, since it reads the previous key for I-30 and
  sweeps stale temp files.
- Unit 649/649; lanes as above; smoke and ci all gates green.

## Mutation

**PASS, 90.8%**: 315 caught, 32 missed, 114 unviable. cargo-mutants 27.1.0 with `--in-diff` over
`9aa38d9^`, threshold 80%.
- Pass 1 reached only 76.1% (WARN). Pass 2 added unit tests to mod.rs, reload.rs,
  handlers/instance.rs, slack_app.rs and key_file.rs (`3aecfd1`, `ae73c5e`).
- key_file.rs went from 60.5% to 89.5% (item 41 closed). Each B2 proof is killed by a named test.
- Survivors with dispositions:
  - `handlers/instance.rs` axum bodies (12, acceptance-only);
  - `secrets_key_seam.rs` (4, acceptance seam);
  - `with_saved_slack` (5);
  - `log_secrets_verdict` (4, log only);
  - key_file (4, root-only, crash-only or equivalent);
  - store `instance.rs` (3).

## Lessons learned

- **RED logged before it ran** (items 38, 42).
  - In 05-02 the four target scenarios already passed on 05-01's code. The new guards were pinned by
    gold tests written alongside the code, never seen failing.
  - In 05-03 `RED EXECUTED FAIL` was logged before the RED run.
  - `des-verify-integrity` passed both. Mutation-prove guards whose RED was not genuine, as B2
    did, and log a phase only after its command has run.
- **Fill the disk, lose the run.** The first cargo-mutants run filled the VM's 12 GB tmpfs `/tmp`
  ("No space left on device") and reported 51 of 53 key_file mutants unviable, so the run was
  discarded. Set `TMPDIR` on disk (`~/mut/tmp`), as integrations-wayfinding also found.
- **Empty is not unset.** `FOUNDRY_PUBLIC_URL=` (empty) made I-29 read "use , which Slack cannot
  reach" and gave relative manifest URLs. `main.rs` used the raw variable while
  `IntegrationSettings` treated empty as unset. It was found only by the hand-driven demo and fixed
  in `9dff994`. Treat every optional setting the same way at one read point.
- **Cleanup must not touch a sibling's in-flight work.** The race only showed under the release-mode
  `xtask ci` (11/30). The lanes and smoke were green. Anything sweeping a shared directory needs an
  age bound.
- **Gate with an alert in mind.** Before `9dff994`, a post-write install bypassed the alert inputs.
  One install function per state change removes that class of gap.
- Outline placeholders with spaces never substitute in cucumber-rs 0.21.1 (item 29).
- A race scenario that passes with the reload deleted does not test the reload (item 22).
- Running the `@docker-compose` lane through `test-in-lima.sh` needs `cp .env.example .env` in the
  same command (item 45).

## Open items for the user

- **Copy decisions.**
  - Withheld key file: `9dff994` omits the key-file line entirely when no file was made (H3). Confirm,
    or supply a line.
  - The M1 stand-in: a save that loses the seal fence twice answers the `NOT_CONNECTED` refusal as a
    placeholder. It needs a DISCUSS copy line.
  - Misleading log reason: with an operator key set and no file, `integrations.key_file.withheld`
    logs a reason that suggests the file was withheld over sealed data. Reword or drop the line in
    that case.
- **Release time.** Add the CHANGELOG `### Migration notes` entry for 0033 (DEVOPS task 7). Also fix
  the CHANGELOG link to the old RELEASING section title "Integrations: secrets key and Slack"
  (item 46).
- **Deliverable B.** Replace and Start over, Slack Replace and Remove with the I-14 confirm, I-19 and
  I-20, `instance_slack_app_pending` in its own migration, and KPI-IIS-3 and -4.
  - Carried forward: item 10 (`key_mode` free text with no CHECK; Replace activity wording) and
    item 27 (workspace connections do not bump the config version).
  - Also carried forward: item 32 (I-13 when the environment's app wins and the saved app does not
    open has no scenario) and item 35 (fence rule reads string literals only).
- **ADRs.** ADR-IIS-001 to -005 stay Proposed pending the user's acceptance. brief.md item 5 (it
  still lists `/api/v1/integrations` as a door) is to be reworded.
- **KPI-IIS-M.** The manual walk on a deployed instance after release.

## Permanent artifacts

Migrated at this finalize (copied; originals kept in the feature workspace):
- `docs/ux/instance-integration-setup/journey-instance-integration-setup.yaml`
- `docs/ux/instance-integration-setup/journey-instance-integration-setup-visual.md`

  Neither carries a "FUTURE DESIGN" label, so no label changed. Step 0 is also in the SSOT journey
  `docs/product/journeys/journey-external-integration-plugins.yaml` (applied in DISCUSS).

Already permanent, not duplicated:
- `docs/product/architecture/adr-iis-001-two-tier-keys.md`, `adr-iis-002-instance-key-file.md`,
  `adr-iis-003-slack-app-saved-by-instance-admin.md`,
  `adr-iis-004-live-reconfiguration-across-replicas.md`,
  `adr-iis-005-instance-integration-activity.md`. This repo keeps ADRs there, so nothing goes to
  `docs/adrs/`. Supersession notes are on ADR-EIP-004, ADR-EIP-005 and ADR-IW-001.
- `docs/product/architecture/brief.md` "Instance integration setup" holds the architecture. It is
  the SSOT, not migrated.
- `docs/product/kpi-contracts.yaml` (instance-integration-setup block) and
  `docs/product/outcomes/registry.yaml`.
- RELEASING.md (R-1, R-2, R-5), `deploy/k8s/` README, Secret and ConfigMap examples, `Dockerfile`,
  `docker-compose.yml` (06-02, 06-03).
- `crates/foundry-acceptance/tests/features/instance-integration-setup.feature`.

Not migrated (none exists for this feature):
- `design/architecture-design.md`, `component-boundaries.md`, `technology-stack.md`,
  `data-models.md`; `design/adrs/`; `distill/walking-skeleton.md`. The walking skeleton is the
  "walking skeleton" block of the feature file above (driven green by step 02-04).

Kept in the feature workspace (active for Deliverable B): `discuss/` (user stories, story map, DoR,
outcome KPIs), `design/` (`options.md`, `upstream-changes.md`), `devops/`, `distill/`
(`red-classification.md`, `upstream-issues.md`), `deliver/` (roadmap, execution log, upstream issues,
mutation report).
