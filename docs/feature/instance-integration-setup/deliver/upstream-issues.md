# Upstream issues found in DELIVER — instance-integration-setup (Deliverable A)

Found while planning the roadmap (2026-11-06). None blocks a step; each has a planned resolution.

## For DESIGN (brief.md, ADRs)

1. **Test seam vs table rule.** `secrets_key_seam.rs` takes a `PgPool` (check-arch
   `integrations-store-capability`), but `integrations-instance-tables` allows SQL on
   `integration_secrets_key` only in `foundry-store/src/integrations/instance.rs`. Resolution:
   a pool-taking reader in the store module, called by the seam (step 03-04).
2. **Table rule scope.** ADR-IIS-001 says SQL on the new tables appears only in `instance.rs`, but
   the acceptance driver reads them (DISTILL D5 waivers: race count, dump). Resolution: the rule
   covers production `src` only (step 01-02).
3. **Does reload write?** brief.md's contract-shape table says reload "writes nothing"; ADR-IIS-004
   r8 has each replica upsert its own `integration_replica_states` row. Resolution: reload writes
   only its own replica-state row; the store test asserts no other write.
4. **`doctor rotate-secrets` has no story.** ADR-IIS-001 decision 8 re-wraps it; RELEASING R-5
   (DEVOPS task 5, Deliverable A) describes it. Resolution: step 05-03.
5. **brief.md still lists `/api/v1/integrations`** as an observed door; the product owner removed
   it from the ACs. Reword at finalize.
6. **`instance_slack_app_pending`** is listed in ADR-IIS-001's table rule and ADR-IIS-004's
   snapshot but is Deliverable B. Resolution: not in migration 0033; B adds it in its own
   migration.

## For DEVOPS

7. **No-key scenarios and the real data directory.** Once the key file is wired in, eip and
   wayfinding scenarios without a key setting would default to `/var/lib/foundry`. Resolution:
   the harness gives every scenario a temp `FOUNDRY_DATA_DIR` (step 05-01).
8. **Task 1 split.** check-arch `integrations-test-seam` needs the Dockerfile marker grep as soon
   as the seam is listed, so the grep lands in 03-04 and the data directory and `VOLUME` in 06-02.

## Found while executing steps

9. **01-02 touched a file outside its plan.** `foundry-store/tests/probe_schema_scoping.rs`'s
   healthy fixture now creates the 0033 tables, since the probe asks for them. No assertion
   changed. The 0033 probe is its own query (`PROBE_0033_SQL` in `instance.rs`) so the
   instance-tables rule keeps SQL on those tables in one module.
10. **Left open for later steps.** `integration_replica_states.key_mode` is free text with no
    CHECK; 02-04 may close it once the wire names are fixed. Slack save records
    `slack_app_saved` for a first save and `slack_app_replaced` over an existing app;
    Deliverable B's Replace flow may want different wording.
11. **01-03 touched three handlers outside its plan.** `handlers/matrix_setup.rs`,
    `slack_setup.rs` and `workspace.rs` now seal through `integrations.sealing_box()`, so
    sealing follows the mode (criterion 3) instead of always using the instance key.
12. **Replica harness still bypasses the boot function.** `multi_replica_harness::install_integrations`
    builds `Integrations::new` with no probe, so replicas judge the mode as if nothing were
    stored. Step 02-04 (reload on every replica) must route it through `Integrations::boot`.
13. **Boot probe scope.** A held key's canary is checked only when a stored value needs that
    key, as before. A strict reading of 01-03 criterion 4 would degrade a server over an
    unused key; kept as today's eip behaviour.
14. **Operator key plus key file.** With an operator key set, the key file is not read. Adding a
    superseded key file as a previous key is for 05-01.
15. **02-01 touched two files outside its plan.** `instance_admin.rs` makes
    `require_instance_admin` `pub(crate)` (no behaviour change) so the instance page reuses
    the uniform 404. `integrations/mod.rs` adds read-only `key_mode()` and
    `holds_direct_seals()` so the page can tell I-3 from I-2e0 and I-2e from I-2e0.
16. **02-02 touched files outside its plan.** `foundry-store/src/integrations/instance.rs` and
    `mod.rs` add `instance_activity(limit)` (SQL on 0033 tables stays in that module);
    `handlers/mod.rs` exports the POST; `foundry-app/tests/eip_phase5_doors.rs` holds the
    Tomás handler test; `foundry-acceptance/tests/eip_slack_live.rs` follows the new
    `slack::manifest(&Integrations)`. `no_connection.rs` needed no change.
17. **Reload is `Integrations::reboot`, not `boot`.** It shares boot's probe steps but not the
    function, because `IntegrationSettings` cannot be cloned (`SecretBox`). 02-04 should make
    the poll reload use the same path, or fold `reboot` into `boot`.
18. **Activity copy.** Only "generated the secrets key" has copy; other actions render their
    stored name with spaces until their steps add copy (02-03 adds the Slack save line).
19. **02-03 touched files outside its plan.** `handlers/mod.rs` re-exports the save handler;
    `foundry-store/src/lib.rs` adds `Store::user_display_name` for I-6's "saved by";
    `Cargo.lock` gains `base64`/`hex` for foundry-acceptance (earlier lockfile drift).
20. **Left open by 02-03.** No field validation on the Slack save (03-02 owns I-11/I-12). A saved
    app uses Slack's default API/authorize URLs; the `SLACK_API_BASE_URL`/`SLACK_AUTHORIZE_URL`
    seams apply only to the env trio (03-03 must apply them to the saved app). The Slack form
    renders only in Sealed mode, not Direct, although the POST accepts Direct.
21. **02-04 resolved items 12 and 17.** The replica harness boots through `Integrations::boot`;
    boot, the serving-replica apply and the reload share `Integrations::rebuilt`. Out-of-plan
    edits: `multi_replica_harness.rs`, `external_integration_plugins.rs` (await), `keys.rs`
    (`resolve_holding` keeps a held key whose id is unchanged). Item 10 stays open; the
    `key_mode` wire names are now `no_instance_key`, `direct`, `awaiting_secrets_key`, `sealed`, `unopenable`.
22. **For DISTILL: the race scenario does not exercise the reload.** "Two generate requests at the
    same moment" passes with the reload deleted, since each replica rebuilds after its own POST.
    Only the walking skeleton and `foundry-app/tests/iis_reload.rs` depend on the loop.
23. **Roadmap lane tag.** `us-02-multi-replica` matches nothing; the tag is `@multi-replica`.
24. **For DEVOPS: metrics not wired.** D5 / task 8's `foundry_integration_secrets_degraded` and
    `probe_failures_total` updates from the reload are in no step's criteria; pick up in 06-0x.
25. **"Reload never touches the key file" holds by construction** (no `InstanceKeySource` in
    `Reload`/`rebuilt`), not by a test.
26. **03-01 out-of-plan edit and choices.** `integrations/mod.rs`: `holds_direct_seals()` became
    `Integrations::direct_seals(&InstanceSnapshot)`, with a shared `sealed_values` helper. The
    page now takes one REPEATABLE READ snapshot per view so the direct-seal count is live.
    Item 20 resolved: the Slack form shows under I-7 and I-5 wherever credentials can be sealed
    (Direct or Sealed). Two of the three scenarios passed before the change (restart-based).
27. **Workspace connections do not bump the config version.** A Matrix/Slack connection made
    after boot is not seen by a replica's mode decision until a restart or another bump. The
    page works around it with a per-view snapshot; check that Direct-vs-Awaiting on other
    replicas (and 05-0x Move gating) does not depend on that stale decision.
28. **For DESIGN/DISTILL: I-7 "below" points at no form** in No-instance-key, Awaiting and
    Unopenable modes, where I-7 still shows because the environment's app is in effect.
29. **For DISTILL: outline placeholders with spaces never substitute.** cucumber-rs 0.21.1 only
    fills `<([^>\s]+)>`. 03-02 renamed `<client id>`, `<client secret>`, `<signing secret>` to
    snake case (values, assertions and copy unchanged). No other outline has the problem.
30. **03-02 log has an extra GREEN pair.** The trace reads RED PASS, GREEN PASS, GREEN FAIL,
    GREEN PASS, COMMIT PASS: GREEN PASS was logged before the first lane result, which failed
    on item 29. Expect des-verify-integrity to show it; the final GREEN is genuine.
31. **03-03 out-of-plan edit and a changed unit-test row.** `integrations/config.rs` gains
    `IntegrationSettings::slack_urls`, read on its own, so `SLACK_API_BASE_URL` /
    `SLACK_AUTHORIZE_URL` apply to whichever app is in effect (closes item 20's seam note).
    03-01's crafter-written row `(Direct, false, None, false)` in
    `the_environments_app_is_named_in_every_mode` became `true` (form shows in Direct with no
    app), matching item 26 and the I-13 scenario. Adversarial review to confirm.
32. **Uncovered combination.** I-13 also shows when the environment's app wins and the saved
    app does not open; no scenario covers it.
33. **03-04 out-of-plan edits.** `foundry-services/src/lib.rs` passes `sealed_under` through;
    `sealed_under: None` added to `NewConnection` literals in `foundry-store/tests/eip_store.rs`,
    `foundry-app/tests/eip_contract_shapes.rs`, `eip_phase5_doors.rs` and
    `foundry-services/tests/eip_integrations_use_cases.rs` (no assertion changed).
34. **Doctor rotation does not take the exclusive instance lock.** `keys.rs::reseal_credentials`
    is on the seal-fence allow-list, but ADR-IIS-004 rule 1 wants the lock. Owner: the step
    doing doctor rotation (05-0x).
35. **Gaps left by 03-04.** The handler's retry-once after a reload has no test of its own; the
    seal-fence rule reads string literals only, so a `format!`-split write could slip past;
    an absent seam file passes the marker check (an absent seam cannot ship).
36. **04-01 split the store's count: values vs credentials.** I-22 counts credentials (each
    connection 1, the saved app 1); the 01-02 store test `move_reseals_every_sealed_value_in_
    the_same_transaction` asserts sealed values (app = 2). 04-01 added `move_to_secrets_key`
    in `foundry-store/src/integrations/instance.rs` (out of plan) sharing the transaction
    body; `store_secrets_key` still counts values. For the L1-L6 pass / acceptance designer:
    change the store test to count credentials and fold the two back into one. Also out of
    plan: `handlers/mod.rs` re-export. Items 27 and 34 handled for Move (fresh snapshot gate;
    exclusive lock through the shared body).
37. **05-01 (4d9f5ef) deviations.** Out of plan: `integrations/config.rs` now re-exports
    `DATA_DIR`/`DEFAULT_DATA_DIR` from `key_file.rs` so the literal lives only in the adapter.
    Item 7 closed (temp `FOUNDRY_DATA_DIR` per eip/wayfinding scenario). Item 14 NOT done:
    holding the superseded file key as a previous key needs a foundry-auth `SecretBox` API to
    add a previous key after build; belongs with ADR-IIS-002 r5 (re-wrap, I-18, I-30, Deliverable
    B). Left for 05-02: ADR r3 (no file minted when the DB holds a secrets key or direct seals),
    a link EPERM gold test, the owner check on a pre-existing file (no libc in foundry-app).
    `multi_replica_harness.rs` still boots replicas with `NoKeyFile`; shared-directory scenarios
    must change it. eip/wayfinding lanes not rerun after the ENOTDIR fix (temp dir is writable).
38. **05-02 (5c82d08) RED did not fail.** The four target scenarios already passed on 05-01's
    code; RED logged EXECUTED/FAIL. New behaviour (r3: no file minted over a stored secrets key
    or direct seals; link EPERM/ENOTSUP -> I-27) is pinned by gold tests written alongside the
    code, not before. Expect des-verify-integrity to flag it. Out of plan: `keys.rs`
    (`key_file(may_make)`, `KeyFileRead::Withheld`, `KeyFile::Withheld`; decide maps Withheld
    to Unopenable / AwaitingSecretsKey+I-4, never I-27). Owner check on a pre-existing file
    compares file uid with data-dir uid and only warns (`integrations.key_file.other_owner`);
    ADR is silent on refuse vs warn. Item 37 sub-points closed.
39. **Withheld still shows I-25.** With no file made, the page's key line still reads "made by
    Foundry in {path}". Needs a DESIGN/DISTILL copy decision and a `handlers/instance.rs` edit.
40. **Boot reads the store snapshot twice** (before the key file, then in `reboot`). Share it in
    the refactor pass.
41. **key_file.rs mutation: 59% so far (13 caught / 9 missed / 7 unviable, 29 of 35 run at the
    3600 s bound).** Misses: warning-only branches (lines 119, 126, 148; no tracing capture),
    line 78 (non-NotFound stat error -> Unreadable untested), line 166 (EEXIST lost race never
    reached; needs a link seam returning AlreadyExists), line 192 (fresh-file check cannot fail
    on a real fs). Mutation stage must close these.
42. **05-03 (c947ba9) recording integrity.** RED EXECUTED FAIL was logged (13:15:03Z) before the
    RED run; the run that followed did fail 1 of 15 (the un-pended scenario, cause not kept in
    the truncated log). Store CAS and admin_cli tests were written alongside the code, never
    seen failing. Expect des-verify-integrity to flag both.
43. **05-03 item closures.** Item 14 done via the out-of-plan foundry-auth builder
    `SecretBox::with_previous(self, key)` (boot holds only the built box, not the raw key).
    Item 34 done: rotation runs in `Store::rotate_instance_key` under the exclusive lock, all or
    nothing. The old `foundry-store/src/integrations/keys.rs::reseal_credentials` is now
    test-only (two eip_store tests) and still on the seal-fence allow-list: retire it with its
    tests and allow-list entry (refactor or 06-0x). Item 40 partly done (`reboot` still re-reads).
    Item 39 untouched. Out of plan: foundry-auth `secret_box.rs`, store `integrations/mod.rs`,
    `store_capabilities.rs` (`KeyProbe::rewrap_secrets_key`), `config.rs` (`secrets()` now
    pub(crate)), `boot(..., now)` signature through main.rs, eip support, multi_replica_harness,
    iis_reload, key_file test probe; check-arch `SEAL_FENCED_WRITERS` +`update_wrapped_key`;
    new `foundry-app/tests/iis_admin_cli_rotate.rs`. Rotation writes no activity line (no copy).
    RELEASING R-5 (06-03) must quote the new rotate-secrets output lines.
44. **06-01 (b4e4fbc).** Item 24 closed: `reload.rs::raise_alert_inputs` is the one path setting
    the degraded gauge, `probe_failures_total{probe_name="secrets"}` (+1 per degraded applied
    reload) and the `health.degraded probe=secrets` line, called from boot and `Reload::apply`.
    Degraded = Unopenable (I-26) or I-4; before, I-26 alone never raised it. The serving server's
    own post-write rebuild no longer logs health.degraded (no test checked it). The restart
    scenario passed before any change. Out of plan, DISTILL bug: the sweep's
    `process_output_present` looked for `foundry listening on`; main.rs logs
    `foundry listening addr=` — fixed in eip support; leak checks untouched. Confirm in review
    that +1 per degraded reload matches the DEVOPS task 8 alert rule.
45. **06-02 (efe406b).** Image: builder `mkdir /foundry-data`, runtime `COPY --chown=65532:65532
    --chmod=0700` to /var/lib/foundry + `VOLUME`. Compose mounts named `foundry-data`. New
    scenario tagged `@real-io` (inherits `@docker-compose`), deliberately NOT
    `@instance-integration-setup` (that lane would run it without the shared image). Out of plan:
    `compose_harness.rs` gains `image_directory` / `image_volumes` (docker create + cp -a + rm -v;
    image inspect). Caveat: running the @docker-compose lane straight through test-in-lima.sh
    needs `cp .env.example .env` in the same command (ignored files are not synced).
46. **06-03 (a871310).** Docs only, four planned files. Beyond the wording: RELEASING "Cutting a
    release" step 5 no longer names the homelab/zot (deployment-independence criterion); R-5
    example `deploy/foundry` corrected to `deploy/foundry-app`; R-2 "Dev and prod" now "Each
    instance". Release time: CHANGELOG `### Migration notes` for 0033 (task 7), and CHANGELOG
    line ~327 links the old section title "Integrations: secrets key and Slack".
47. **Adversarial review (2026-11-07): NEEDS_REVISION.** Orchestrator triage:
    - B1 (03-03 Direct-mode form row) REFUTED: I-2e0 reads "Credentials saved here are sealed
      directly with the instance key", so Direct shows the form; AC1's "no Slack form" is the
      no-secrets-key/AwaitingSecretsKey (I-3) state. Item 31 stands.
    - B2 (RED evidence for 05-02/05-03 guards) -> mutation phase must mutation-prove the r3
      guard, the `wrapped_by_key_id` CAS predicate and the EPERM mapping.
    - H1 CONFIRMED: `probe` removes every `instance.key.tmp.*`, including a sibling replica's
      in-flight temp file on a shared volume; that replica's `link` then fails NotFound and
      reads Unwritable (I-27). Fix: remove only stale temp files; NotFound on link re-reads.
    - H2: delete the unlocked `reseal_credentials` with its two eip_store tests and allow-list
      entry (item 43).
    - H3 CONFIRMED (item 39): Withheld/no-file states show I-25 "made by Foundry in {path}".
    - H4: post-write `install` raises no alert inputs and is not version-CAS; route every
      install through one function that raises alerts and refuses a lower applied version.
    - M1 retry-twice-StaleKey returns no message; M2 one Credentials tally (item 36);
      M3 check `update_wrapped_key`'s bool; M4 key_file branch tests (item 41);
      M5 Canzan fixture hosts in new test lines (repo-wide convention, still replace in range).
48. **Post-merge gate (2026-11-07): BLOCKED.** Lanes green (IIS 59/597, eip 181/1942,
    wayfinding 54/400, docker-compose 4/23); all five elevator pitches observed on a real
    process (deliver/wave-decisions.md). `cargo xtask ci` FAILED: foundry-app lib
    `key_file::tests::two_servers_racing_on_one_directory_hold_one_key` "expected a held key,
    got Unwritable", 11/30 alone — the H1 race (item 47). Also: `FOUNDRY_PUBLIC_URL=` (empty)
    makes I-29 read "use , which Slack cannot reach" and relative manifest URLs; main.rs:122
    uses the raw variable while IntegrationSettings treats empty as unset.
49. **Review revision `9dff994` (2026-11-07): gate GREEN.** H1 (temp file stale only after
    10 min `STALE_TEMP`; link NotFound reads sibling file or retries once; race test 100/100
    alone), H4 (one `swap`: version-CAS + `raise_alert_inputs`; a degraded post-write install
    now counts +1 probe failure), H3 (no key-file line when Withheld — copy choice for the
    user), H2 (`reseal_credentials` gone; `SEAL_FENCED_WRITERS` 6 -> 5), M2 (one
    `store_secrets_key` tally), M3 (`RotateOutcome::KeyChanged`, exit 3), M1 (retried save
    answers NOT_CONNECTED as placeholder — needs a DISCUSS copy line), blank
    `FOUNDRY_PUBLIC_URL` = unset. Lanes IIS 59/597, eip 181/1942, wayfinding 54/400;
    `xtask smoke` and `xtask ci` (Docker, 2132 scenarios) all gates green.
    Open: ADR-IIS-004 line 35 and brief.md line 1258 still name `reseal_credentials`;
    key_file mutation not rerun (mutation phase).
50. **Refactor pass `bc9af98` (2026-11-07): L1-L3, gate GREEN.** Key text in a
    `Zeroizing<String>` sized past `MAX_FILE_BYTES`; `may_make` TOCTOU documented (mod.rs
    ~257); item 40 closed (boot rebuilds over its snapshot via `with_key_check`, re-reads only
    after a re-wrap or a failed first read; a sibling write during boot now lands on the next
    polled reload); store `commit_recorded`; one `CONNECT_ATTEMPTS`, `reinstall`,
    `sealed_under`, shared Generate/Move `key_door`; ADR-IIS-004:35 and brief.md:1258 renamed
    to `reseal_sealed_values`. L4-L6 no change. Skipped: skipping the probe under an operator
    key (it reads the previous key for I-30 and sweeps stale temps). Open copy issue: with an
    operator key and no file, `integrations.key_file.withheld` logs a misleading reason.
    Unit 649/649; lanes IIS 59/597, eip 181/1942, wayfinding 54/400; smoke and ci (Docker,
    2132 scenarios) all gates green.
51. **Mutation gate (2026-11-07): PASS 90.8%** (315 caught / 32 missed / 114 unviable,
    cargo-mutants 27.1.0 `--in-diff`, commits `3aecfd1` pass 1 at 76.1%, `ae73c5e` pass 2).
    Item 41 closed (key_file 89.5%); B2 proofs a/b/c each killed by named tests
    (deliver/mutation/mutation-report.md). Survivors: handlers/instance.rs axum bodies (12,
    acceptance-only), secrets_key_seam.rs (4, acceptance seam), `with_saved_slack` (5),
    `log_secrets_verdict` (4, log only), key_file (4: root-only / crash-only / equivalent),
    store instance.rs (3).
52. **des-verify-integrity (2026-11-07):** "All 18 steps have complete DES traces". It does
    not detect the RED-before-run ordering in items 38 and 42; those stay recorded here.
