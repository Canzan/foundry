# DEVOPS Decisions — integrations-wayfinding

Read before writing:
- ✓ docs/product/architecture/brief.md (Wayfinding subsection, Crate graph)
- ✓ docs/product/kpi-contracts.yaml (format and instrument kinds; external-integration-plugins entry)
- ✓ discuss/outcome-kpis.md
- ✓ design/wave-decisions.md

None of these decisions contradicts DESIGN.

Decisions 1–9 are fixed by the existing platform, so they were not asked again
(they are recorded in AGENTS.md, CODING_STANDARDS.md, DEVELOPER.md and the
cut-release skill):

| # | Decision | Value (existing) |
|---|----------|------------------|
| 1 | Deployment target | On-premise homelab: Canzan Core Dev (Lima k3s, `foundry.unintelligent-design.us`) and Canzan Core Prod (Pi cluster, `foundry.jeffbailey.us`) |
| 2 | Orchestration | Kubernetes (k3s), Argo CD |
| 3 | CI/CD | GitHub Actions (`ci.yml`, `release.yml`) plus the Forgejo zot image build; the local gate is `cargo xtask ci` in Lima VM `foundry-test` |
| 4 | Existing infra | Both exist; this feature changes neither |
| 5 | Observability | Prometheus metrics and alerts (homelab `alerts.tf`); no log store, no product analytics |
| 6 | Deployment strategy | Rolling. Dev auto-syncs via Argo CD Image Updater; prod waits for `gitops-promote.sh` approval |
| 7 | Continuous learning | None (single-user instance) |
| 8 | Branching | Trunk-based on `main`, no PRs; commit after `xtask smoke`, push after `xtask ci` |
| 9 | Mutation testing | Per feature (CODING_STANDARDS.md): scoped to the files the delivery modified, kill rate ≥ 80% |

## Key Decisions
- [D1] No infrastructure change. There is no migration, env var, secret,
  manifest, alert or metric (see: design/wave-decisions.md, "Constraints
  Established").
- [D2] Ship as foundry v0.18.2 (patch: links and wording, no new capability)
  through the existing release path. Commit and tag, then GitHub `release.yml`
  and the zot image. Dev rolls out by itself. Prod is promoted only on the
  user's go. The tag and every push also wait for the user's explicit go.
- [D2a] Rollback: v0.18.2 has no migration, setting or stored-data change, so
  rolling back is redeploying v0.18.1 (`e4f7e3f`) through the same gitops
  path (dev: pin the image tag; prod: do not promote, or promote v0.18.1).
  No data rollback is needed.
- [D3] KPIs are measured with the existing instrument kinds (kpi-contracts.yaml):
  - `acceptance` for KPI-IW-1, -2, -3 and -G1;
  - `acceptance` for KPI-IW-G2 too: the eip features 01–06 (tag
    `@external-integration-plugins`) as run by `cargo xtask ci`;
  - `dogfood` for KPI-IW-M, a walk on dev after the release.
  No runtime metric is added: a page render that shows a link is not an event
  worth counting on a single-user instance.
- [D4] The scenarios run in the default acceptance lane (`InProcHarness`,
  `@real-io`), like eip-06, and need no Docker image. The "no Slack app" and
  "no secrets key" states are arranged through the harness's existing
  integration settings, which eip-01 already uses for K-2 and K-20. The
  lane excludes anything needing the Synapse container (`@needs-matrix`), so
  a connected or unreadable Matrix connection is not an acceptance scenario
  here; DISTILL D5 assigns those cells to unit tests.
- [D5] Mutation testing (per feature) covers `no_connection.rs`, the
  `nav_identity` mapping and the `managed_project` change. The pure cause
  table is where surviving mutants would matter.

## Infrastructure Summary
- Deployment: homelab k3s, rolling; dev automatic, prod by approval.
- CI/CD: GitHub Actions and Forgejo, trunk-based; the local gates are
  `xtask smoke` before commit and `xtask ci` before push.
- Observability: unchanged (Prometheus). This feature adds nothing.
- Mutation testing: per feature, ≥ 80% kill rate on the modified files.

## Constraints Established
- The scenarios must pass in the default lane, without Docker, so `xtask smoke`
  covers them.
- The post-release check (KPI-IW-M) uses dev's real state: workspace
  "Default", project ABC (`/team/team-2/project/abc`), no Slack app.

## Upstream Changes
- None.
