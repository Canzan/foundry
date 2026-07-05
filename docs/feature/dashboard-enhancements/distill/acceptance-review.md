# Acceptance Review — dashboard-enhancements (DISTILL self-review)

Self-review against the DISTILL quality bar. A consolidated peer review (`/nw-review`) can run before
DELIVER if desired; given the reuse-only scope it is optional.

## Checklist

| Criterion | Verdict | Note |
|-----------|---------|------|
| Every AC has ≥1 scenario | ✅ | 9 acceptance scenarios + 3 store-integration scenarios cover AC-01..05 (`test-scenarios.md` map). |
| Scenarios are port-driven (no internal-state-only asserts) | ✅ | All drive `GET /` / `POST /sign-out` / the store port. |
| Business-readable (Given/When/Then, named personas) | ✅ | Matches `us-06-signin` / `us-07-project-create` vocabulary. |
| Security/negative paths covered | ✅ | Markup-escaping (AC-01.3), non-super-admin absence (AC-03.2), forged CSRF (AC-02.4). |
| Tenancy asserted | ✅ | Store isolation scenario (AC-05.1) — workspace B's projects excluded. |
| Lane safety | ✅ | All `@pending`; excluded by `acceptance.rs filter_run`; `@all` stays green until DELIVER. |
| Wave-decision reconciliation | ✅ | D1 (200 fallback) + D2 (CSRF response-type) reflected in scenarios; 0 contradictions. |
| No dependency on unbuilt seams | ✅ | Every port maps to a verified-present seam (`requirements.md` table). |

## Risks / watch-items for DELIVER

- **R1 — greeting fault injection**: scenario 3 needs a way to force the identity query to fail. If no fault
  seam exists, assert the fallback via a unit test on the handler's `None` branch instead of an acceptance
  fault-inject (downgrade acceptance → unit; note in the step glue).
- **R2 — CSS hash assertion**: scenario 9 matches `/static/css/foundry.*.css` by glob, not a fixed hash, so
  the slice-04 hash bump won't require editing the scenario. Good.
- **R3 — response-type ripple (D2)**: slice 03 changes `dashboard_root`'s return type; confirm no other
  caller of `DashboardRoot`/`dashboard_root` breaks (only `lib.rs` route wiring should reference it).

## Verdict

**READY for DELIVER.** Recommend executing slices in order 01→02→03→04, un-`@pending`-ing one scenario
group per slice.
