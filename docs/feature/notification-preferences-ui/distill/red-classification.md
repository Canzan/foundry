# RED Classification — notification-preferences-ui

Pre-DELIVER fail-for-the-right-reason gate. DELIVER reads this at PREPARE/RED phase to
confirm RED is genuine (implementation missing) rather than BROKEN (compile / import /
fixture / harness fault).

## Compile gate (BROKEN check)

```
cargo test -p foundry-acceptance --test acceptance --no-run
# → Finished `test` profile ... (exit 0)
```

The acceptance test binary — including the new `feature_notification_preferences_ui`
step module + the `world.rs` `npui_*` fields + the `acceptance.rs` force-link — COMPILES
and LINKS cleanly. No `ImportError`/missing-symbol/duplicate-step-registration fault.
=> **not BROKEN.**

## RED demonstration (walking skeleton)

`@pending` is excluded from every lane, so the committed feature file runs 0 scenarios.
To demonstrate a genuine RED, the walking skeleton (`@walking_skeleton`, scenario 1) was
temporarily un-pended and run through the real harness; then `@pending` was restored (the
committed state keeps the default + `@all` lanes green).

```
FOUNDRY_ACCEPTANCE_TAGS=notification-preferences-ui \
  cargo test -p foundry-acceptance --test acceptance
```

Result:

```
[Summary]
1 feature
1 scenario (1 failed)
2 steps (1 passed, 1 failed)
acceptance run failed: 1 step(s) failed, 0 parsing error(s), 0 hook error(s)
```

- `Given Nadia is signed in and belongs to "Northwind", "Contoso", and "Initech"` — **PASSED**: the REAL in-process axum app + testcontainers Postgres spawned, Nadia was seeded + signed in, and `GET /` rendered the dashboard + shared sidebar through the production composition root.
- `When Nadia opens an authenticated page and follows the settings link in the sidebar` — **FAILED (RED)**: the step reached its assertion and panicked because the rendered `.sidebar__user` footer offers only `Keyboard shortcuts` + `Sign out` — there is no `href="/account/settings"` link yet. The feature is unimplemented; the harness/wiring is sound.

## Classification

| Scenario | Failure mode | Class |
|----------|--------------|-------|
| 1 — walking skeleton (sidebar → settings surface → mute) | Assertion: no `/account/settings` link in the rendered sidebar footer (feature not built) | **MISSING_FUNCTIONALITY (RED)** |
| 2–12 (still `@pending`, not run) | Would 404 on `/account/settings` / `/account/settings/mute`, or find no `data-status` rows / no settings link — all assertion-class, same missing seams | **MISSING_FUNCTIONALITY (RED)** by construction |

No scenario fails for `IMPORT_ERROR` / `FIXTURE_BROKEN` / `SETUP_FAILURE` (the `Given` reaches through real DI and passes) or `WRONG_ASSERTION` / `OBSERVABLE_NOT_AT_PORT` (assertions target port-exposed observables — rendered `data-status` markers, HTTP status, and `Store::is_unsubscribed` reads, never internal struct fields).

**Verdict: RED (genuine). Cleared for DELIVER.** DELIVER unskips slice-by-slice (WS →
sidebar link → settings surface → mute action → resubscribe regression), mounting the two
new routes under `session_layer` + `csrf_middleware`, rendering the settings shell + the
`sidebar__user` `<a href="/account/settings">`, and turning each scenario GREEN.
