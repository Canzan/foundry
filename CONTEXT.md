# CONTEXT

## Current Task

**`multi-workspace-provisioning` (slices 5-6) shipped + finalized** (`main`, not pushed). The deferred slices of the isolation core, delivered as their own feature: instance super-admin role (`0011 instance_admins` + `is_instance_admin`), CLI-first `foundry doctor provision-workspace` + `grant-super-admin` (D1/D2/D3), migration-as-guarantee no-backfill proof (D4), and idle+LRU rate-bucket eviction closing residual F2 (D5). 16 DES-monitored TDD steps (5 phases, commits `299a8c1`→`3cf4ec1`). `@all` acceptance 275/275, scoped mutation 100% (28/28), adversarial review APPROVED. Zero new crates. Evolution doc: `docs/evolution/2026-06-12-multi-workspace-provisioning.md`.

## Key Decisions

- **D1–D7 all IMPLEMENTED** (wave-decisions.md + adr-001..005 marked shipped): D1 bootstrap+grant super-admin, D2 CLI-first (web flow DEFERRED), D3 instance_admins/is_instance_admin off the LAYER-1e guard, D4 no-backfill real-snapshot proof, D5 idle+LRU eviction off the shipped clock, D6 single additive 0011, D7 no new check-arch rule.
- **Eight features through the full nWave pipeline, all on trunk** (each in `docs/evolution/`): …token-management-api, multi-workspace-tenancy (slices 1-4) `384fa5f`, **multi-workspace-provisioning (slices 5-6)**. Both feature workspaces PRESERVED.
- **Trunk-based**: commit to `main`, no PRs, no CI commit-gate; verify full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings`; DES hook requires the 5-phase legacy contract.

## Next Steps

- Optional: `git push` (origin behind; the whole arc incl. provisioning is local — orchestrator confirms push separately).
- **Deferred follow-ups**: web provisioning flow `/admin/instance/…`; a real invite-accept/password-set flow; review nits (D1 refusal-identity test variant, D2 vacuous invites snapshot, D3 refactor `run_provision_workspace`, D4 add `instance_admins` to backup-verify).
- **Still-open from parent**: Prometheus exporter for `foundry_token_mutations_total`; per-workspace backup (OD-5); key-rotation UX.
