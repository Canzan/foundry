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
