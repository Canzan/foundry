# DELIVER upstream issues — project-lifecycle

Deviations from DESIGN/DISTILL found during DELIVER, with the orchestrator's decision.

## UI-1: rule-2 allow-list has four names, not two (step 02-02)

- **DESIGN (DDD-22):** the write-guard rule allow-lists `gc_tombstoned_comments` and `insert_project` by name only.
- **Found:** `create_initial_workspace` and `claim_bootstrap_and_create_workspace` (store `lib.rs`) call `seed_initial_workspace`, which inserts the bootstrap project and then its lanes in the same transaction. No guard can run before those writes because the project does not exist yet — the same reason `insert_project` is exempt.
- **Decision:** accepted. The allow-list is four names, still matched by name only, with the reason at the constant (`f10c592`).

## UI-2: test gaps found by hand-pinned mutations (for Phase 4/5)

- **0025 probe (01-01):** a probe that ignores one `project_lifecycle_events` column still passes every test.
- **Guard on a separate pooled transaction (02-02):** only the static write-guard rule catches it; the store race test cannot tell it from a correct guard.
- **Lane fixtures (02-02):** `run_writer` renames/inserts lane slug `"todo"`, which the seeded lanes lack, so those rows' not-found answer holds even without a guard; only the "waited" check kills an unguarded lane writer.

## UI-3: Phase 4 review (2026-10-15) — dispositions not fixed in code

- **F-01 Mandate-9 `CONTRACT_SHAPE:` docstrings:** not applicable. This repo declares contract shape with `@contract-shape:<value>` scenario tags (279 uses across the acceptance features); no test in the repo uses the docstring form. The tag form is the project's approved equivalent.
- **F-02 / F-03 RED evidence:** the execution log has no failure-text field. Evidence is in the step commits and crafter reports: 02-07's RED was a compile failure of the copy test (the POST route did not exist, so its scenarios could not pass); 02-08 and 03-02's guard scenarios and 04-03's scenarios 53–59 were planned as verification/guard rows (roadmap "EXPECTATION: every row passes at once"; DISTILL recorded all of them RED before the store steps), with their mechanism driven RED→GREEN at store level in 02-01..02-05 and 04-02. Recorded, not re-run.
- **F-08 rename NoOp on a pre-lock read:** kept. `classify_rename` compares the submitted name with the name read at the start of the same request, before the row lock; a rename by someone else committing inside that window makes the submit a no-op instead of overwriting their rename. That is the safer outcome; the locked byte-equal check still decides every write.
- **F-14 post-write authority re-read:** follow-up. After a successful delete or rename the members page is re-read with the actor's current authority; a demotion landing in that window answers the uniform 404 although the write committed. Rare, and the operator list shows the truth.
- **F-22:** the execution log is committed at finalize.
