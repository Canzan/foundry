# Upstream issues found in DISTILL — instance-integration-setup (Deliverable A)

None blocks DELIVER; reconciliation found no contradiction between DISCUSS, DESIGN and DEVOPS.
Each item names the document it refers to.

## For the product owner (discuss/user-stories.md)

1. **Resolved (product owner, 2026-11-06): removed from both ACs.** **`/api/v1/integrations` does not exist.** US-IIS-02 AC 4 and US-IIS-03 AC 3 list it as a
   shipped door to scan for the planted key and secrets. No such route is in the code or in
   `crates/foundry-api/openapi.json`. The KPI-IIS-G1 sweep scans pages, page source, activity,
   process output and the database. Either drop "and `/api/v1/integrations` (shipped)" from both
   ACs or name the API answers meant.
2. **Resolved (product owner, 2026-11-06): the singular form is in the Copy table and used by the replica Move scenario.** **I-22 had no one-credential form.** `Moved. {n} saved credentials are sealed …` reads
   "1 saved credentials" when n = 1, the case I-14 already fixed. The scenario uses n = 2 (a
   Slack connection and the saved Slack app). Proposed: `Moved. 1 saved credential is sealed
   under a new secrets key. Nobody needs to reconnect.`
3. **"Canzan Labs connected Matrix" is a Slack connection in the scenarios.** US-IIS-02 examples
   2–3 and scenario 3, and US-IIS-08 scenarios 3 and 6, use a Matrix connection as "a saved
   credential". Saving one needs the Synapse container (`@needs-matrix`), which DEVOPS D8 keeps
   out of this feature's lanes. The scenarios use a Slack connection made through the Slack double
   instead. It is sealed the same way and proves the same thing. "Matrix is ready" is observed
   as the Connect Matrix form on the workspace page. No AC changes. Recorded so a reader of the
   story finds the substitution.
4. **`@property` on layer-3 scenarios.** US-IIS-02 "A database dump alone opens nothing" and
   "The secrets key is never shown", and US-IIS-03 "The saved Slack secrets never leave Foundry",
   are tagged `@property`. They run against real Postgres, real HTTP and a real process, so they
   are example-based with planted values (test-design Mandates 9 and 11). Property tests of the
   pure decisions (key mode, offers, shapes) are DELIVER's unit tests. No AC changes.
5. **US-IIS-02 AC 6 is split.** The activity line is in-process ("Generating is recorded"). The
   restart and `probe_failures_total{probe_name="secrets"} == 0` run on the operator's process
   (`@iis-subprocess`), because metrics are per process.

## For DESIGN (brief.md)

6. **Stale line about the pending Slack app.** brief.md "Slack app (ADR-IIS-003)" says a switch
   to a different client ID "is held sealed in the session until confirmed". ADR-IIS-003 r4 and
   design D4 hold it in the fenced `instance_slack_app_pending` table, never the session. This is
   Deliverable B, so no scenario here depends on it. Reword the brief line.
7. **The alert-input scenario runs on two operator processes, not `MultiReplicaHarness`** (tagged `@iis-subprocess` only, with a comment). DEVOPS
   D8 places the reload-alerting scenario on `@iis-replica`. Metrics live in one process-wide
   registry, so in-process replicas cannot show a per-replica `/metrics`. The scenario starts two
   `foundry` processes on one database, one with a different operator key, and carries
   `@iis-subprocess` only.

## Product owner resolution (2026-11-06)

- **1, resolved.** `/api/v1/integrations` is removed from US-IIS-02 AC 4,
  US-IIS-03 AC 3, their scenarios and KPI-IIS-G1. The page, log, activity and
  database checks stay.
- **2, resolved.** I-22 has an n = 1 form:
  `Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect.`
- **6, resolved by note.** This is Deliverable B, already superseded by
  ADR-IIS-003 r4. brief.md is left alone for now. The note is recorded in
  discuss/wave-decisions.md.

## Coverage left to existing suites or to DELIVER

8. US-IIS-06 AC 5's "a partial SLACK_* trio still refuses start", and AC 1's
   `FOUNDRY_SECRETS_KEY_PREVIOUS`, are covered unchanged by external-integration-plugins (KPI-IIS-G3).
   They are not duplicated here.
9. **Closed at the DISTILL review:** an `@iis-replica` Move scenario now polls each replica's route history and channel picker. Original note: US-IIS-06 AC 3, "every new-release replica's route history shows deliveries within 60 seconds"
   during a Move, has no two-replica scenario. Move is covered on one replica: deliveries continue,
   nothing is lost and no email is sent. The two-replica live path is covered for Generate and for
   the Slack save (walking skeleton). Gap accepted for Deliverable A; add a replica Move scenario
   if DELIVER finds Move's reload path differs from Generate's.
10. DEVOPS tasks 2 (the `@docker-compose` image-shape scenario) and 3 (amending us-01
    `@nfr-port-01` to two named volumes) remain DELIVER tasks, as DEVOPS assigned them. They test
    the image and compose file, not a feature door.
11. **US-IIS-06 AC 2's "a failure changes nothing" has no acceptance scenario.** No door can make
    one re-seal fail mid-transaction. It belongs to DELIVER's store test with an injected failure:
    the closure-shaped re-seal must leave `integration_secrets_key`, every sealed column and the
    activity unchanged.
