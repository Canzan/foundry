# CONTEXT

## Current Task

**`web-provisioning-flow` shipped + finalized** (`main`, not pushed). The deferred `/admin/instance/…` WEB provisioning surface of the CLI-first `multi-workspace-provisioning` feature: a thin htmx driving adapter over the SHIPPED `provision_workspace` use-case + `is_instance_admin` authz. GET dashboard + provision/grant POSTs under session+CSRF, fail-closed `require_instance_admin` gate with uniform non-enumerable 404; legacy `POST /workspaces` 409 route retired; new tenant proven isolated via `resolve_active_workspace`. 11 DES-monitored TDD steps (3 phases, `02029e7`→`0c32abd`) + regression fix `9efd8e9` + remediation `ea09033`. `@all` 285/285 scenarios, adversarial review APPROVED. Zero new crate, zero migration. Evolution: `docs/evolution/2026-06-13-web-provisioning-flow.md`.

## Key Decisions

- **D1–D6 all IMPLEMENTED** (wave-decisions.md + adr-001..005 marked SHIPPED): D1 one-page routes; D2 inline require_instance_admin + uniform-404 non-enumerability; D3 legacy 409 route RETIRED (deleted, per AGENTS.md dead-code policy); D4 thin adapter + `list_workspaces`; D5 invite-accept OUT of v1; D6 +1 LAYER-1e allow-list line.
- **Nine features through the full nWave pipeline, all on trunk** (each in `docs/evolution/`): …multi-workspace-tenancy, multi-workspace-provisioning `e1dfa97`, **web-provisioning-flow**. All feature workspaces PRESERVED.
- **Trunk-based**: commit to `main`, no PRs; verify full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings`; DES hook requires the 5-phase contract.

## Next Steps

- Optional: `git push` (orchestrator confirms push separately — this finalize did NOT push).
- **Highest-value follow-up**: real `/invites/accept` route (token verify + password-set + consume-invite tx) — fixes the dead invite link on BOTH CLI and web.
- **Deferred**: nightly mutation pass on `instance_admin.rs`/`list_workspaces`/api catch-all; Prometheus `foundry_token_mutations_total` exporter; per-workspace backup (OD-5); key-rotation UX.
