# Walking Skeleton — dashboard-enhancements

The walking skeleton already exists: the base dashboard (project list + quick actions) shipped in
`51ba981`. This feature has no new load-bearing abstraction to stand up first — every slice is an additive
thin cut on the shipped `dashboard_root` handler + `DashboardRoot` view + `dashboard_root.html` template.

## First failing test (DELIVER entry)

**Scenario 1 — "The dashboard greets the user by name and names the workspace"** (`@us-01
@walking_skeleton`), slice 01.

RED → GREEN path:
1. **RED (acceptance)**: un-`@pending` scenario 1; author the step glue
   (`feature_dashboard_enhancements.rs`: "Given a workspace … with admin … display name …", "When Ada
   visits /", "Then the response body contains …"). It fails — the template renders no name/workspace.
2. **RED (unit)**: add a `foundry-store` test for the new greeting query (scoping + fallback) — fails (no
   query yet).
3. **GREEN**: add `Store::dashboard_greeting(user_id, workspace_id) -> Option<(String, String)>` (runtime
   `query_as`, single tenant-scoped read); extend `DashboardRoot` with `display_name` + `workspace_name`;
   load in `dashboard_root` (degrade to fallback on `None`/error, D1); render in the template.
4. **REFACTOR / COMMIT**: fmt + full-workspace release clippy; commit slice 01.

## Slice sequence (from story-map.md)

01 greeting → 02 instance-admin link → 03 sign-out (response-type ripple, D2) → 04 coverage + style
promotion (acceptance scenario over the fully-assembled dashboard + visual-equivalence refactor).

## Lane safety

All 9 scenarios ship `@pending`; `acceptance.rs` `filter_run` excludes `@pending` from every lane, so the
`@all` lane stays green until DELIVER turns each scenario on. Verified pattern: prior features (e.g.
`us-06-signin`) landed `@pending` scenarios the same way. Full `@all` verification runs in DELIVER (needs
Docker), per the repo's standard loop.
