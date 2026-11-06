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
