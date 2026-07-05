# DISTILL Test Scenarios — dashboard-enhancements

> Acceptance design (DISTILL wave). Source Gherkin SSOT:
> `crates/foundry-acceptance/tests/features/dashboard-enhancements.feature`.
> Every scenario ships `@pending` and is excluded from all lanes (`acceptance.rs` `filter_run`); DELIVER
> removes the tag per-scenario as it authors the step glue and turns it GREEN (Outside-In).

## Configuration

- **test_type**: core feature (web adapter) + store integration (slice 04).
- **framework**: cucumber-rs (`tests/features/*.feature`; harness `acceptance.rs`, `harness=false`; step
  glue authored in DELIVER under `crates/foundry-acceptance/src/steps/feature_dashboard_enhancements.rs` +
  `world.rs`, registered in `lib.rs`).
- **integration approach**: real services — real Postgres (testcontainers, per-scenario schema) + the HTTP
  surface driven with a session cookie, mirroring `us-06-signin` / `us-07-project-create`. Tag `@real-io`.
- **driving port**: HTTP `GET /`, `POST /sign-out`; store integration for `list_projects_for_workspace`
  and the new greeting query (slice 04 unit-level, in `foundry-store` tests).
- **layer**: LAYER-3 (real adapter). Example-based; no PBT.
- **lang-mode**: rust. **policy-mode**: inherit (`docs/architecture/atdd-infrastructure-policy.md`).

## Wave-Decision Reconciliation — PASS

0 contradictions with DISCUSS `wave-decisions.md`. D2 (sign-out ripples `dashboard_root` to `(headers,
Html)`) and D1 (greeting degrades to 200) are reflected directly in the sign-out and greeting-fallback
scenarios. No DESIGN wave was run (architecture is all reuse — captured in `requirements.md` seam table);
the seam table stands in for a design doc, and no scenario depends on an unbuilt seam.

## Scenario catalog

| # | Scenario | Slice / Story | AC | Port | RED state |
|---|----------|---------------|----|------|-----------|
| 1 | Greets by name + names workspace | 01 / US-01 | AC-01.1/.2/.5 | GET / | @pending — greeting not yet rendered |
| 2 | Markup in display name rendered inert | 01 / US-01 | AC-01.3 | GET / | @pending |
| 3 | Greeting degrades to 200 on load failure | 01 / US-01 | AC-01.4 (D1) | GET / | @pending |
| 4 | Super-admin sees instance-admin link | 02 / US-03 | AC-03.1/.3 | GET / | @pending |
| 5 | Non-super-admin never sees the link | 02 / US-03 | AC-03.2/.4 | GET / | @pending |
| 6 | Sign out → redirect to /sign-in | 03 / US-02 | AC-02.1/.2/.3/.5 | GET / + POST /sign-out | @pending |
| 7 | Forged CSRF refused | 03 / US-02 | AC-02.4 | POST /sign-out | @pending |
| 8 | Lists projects + links to board | 04 / US-05 | AC-05.3 | GET / | @pending (base-coverage backfill) |
| 9 | Styles served from stylesheet, not inline | 04 / US-04 | AC-04.1/.2/.4 | GET / + /static | @pending @refactor |

### Store-integration scenarios (foundry-store unit/integration tests, authored in slice 01 & 04)

| Scenario | Slice / AC | Assertion |
|----------|-----------|-----------|
| `list_projects_for_workspace` isolation + ordering | 04 / AC-05.1 | workspace A's projects only, name-ordered; B's excluded |
| `list_projects_for_workspace` empty case | 04 / AC-05.2 | project-less workspace → empty vec |
| greeting query scoping + fallback | 01 / AC-05.4 | (display_name, workspace_name) for session ids; error → None |

## Graceful degradation log

- **DESIGN absent**: WARN not block. The reuse-only seam table (`requirements.md`) + D2 substitute for an
  architecture doc; every scenario's port maps to a verified-present seam.
- **DEVOPS absent**: not applicable — the only environment is the shipped per-scenario testcontainers PG16.
- **State-delta port**: none in the Rust suite (matches all prior features); LAYER-3 assertions are
  traditional assertions over port-observables (response body, status, cookie, DB rows).

## Driving-adapter coverage

Every behavioural scenario drives a real inbound HTTP surface (`GET /`, `POST /sign-out`) — no scenario
asserts only on internal state. Slice-04 store scenarios drive the store port directly (integration tests),
covering the query that `GET /` depends on.
