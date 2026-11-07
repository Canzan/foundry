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
