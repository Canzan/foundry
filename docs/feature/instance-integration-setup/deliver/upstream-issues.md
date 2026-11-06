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
