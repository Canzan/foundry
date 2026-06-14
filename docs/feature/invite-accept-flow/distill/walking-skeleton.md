# DISTILL — Walking Skeleton: invite-accept-flow

> Quinn (nw-acceptance-designer), DISTILL wave.

## The single walking skeleton

**Scenario #1 — "A first-admin sets her password and lands on her workspace signed in"**
(`@walking_skeleton @wiring_e2e @us-01`), in
`crates/foundry-acceptance/tests/features/us-invite-accept.feature`.

```gherkin
Background:
  Given a super-admin provisioned the "Northwind" workspace
  And Priya Nair was seeded as its first-admin with a live invite link valid for 7 days

@walking_skeleton @wiring_e2e @us-01
Scenario: A first-admin sets her password and lands on her workspace signed in
  Given Priya has opened her live invite for "Northwind" and seen the set-password form
  When she sets a password meeting the strength policy and confirms it
  Then she is signed in without a separate login step
  And she lands on the "Northwind" workspace dashboard
  And she sees no data from any other workspace
  And her invite is recorded as used exactly once
```

## Litmus test (Dimension 5 — user-centricity)

1. **Title describes a user goal?** YES — "sets her password and lands on her workspace signed
   in" is Priya's job-to-be-done (`claim-my-account-and-sign-in`), not a technical flow.
2. **Given/When are user actions/context?** YES — "opened her live invite … seen the
   set-password form", "sets a password … and confirms it". No DB/route/handler language.
3. **Then are user observations?** YES — "signed in without a separate login step", "lands on the
   'Northwind' workspace dashboard", "sees no data from any other workspace". The one
   state-shaped clause ("her invite is recorded as used exactly once") is the SINGLE
   load-bearing single-use observable the journey itself foregrounds (NFR-2) — phrased as an
   observable fact, not an internal-field assertion (DELIVER reads it via the read-only
   `db_introspect.rs` SELECT helper the policy already sanctions, not a private struct field).
4. **Non-technical stakeholder confirms "yes, that is what users need"?** YES — this is the
   elevator pitch of US-01 verbatim ("dropped straight onto her Northwind dashboard, signed in").

## What it proves end-to-end (the thinnest complete vertical)

Real HTTP `POST /invites/accept` through the in-process `spawn_app` router → SHIPPED
double-submit CSRF check passes → SHIPPED `InviteToken::verify` re-check → NEW
`check_password_policy` passes → SHIPPED `hash_password` → NEW one-TX
`set_first_admin_password_and_consume` (guarded-UPDATE consumes the REAL `invites` row +
writes the hash) → SHIPPED session insert (auto sign-in) → SHIPPED `resolve_active_workspace`
→ 303 landing on `invites.workspace_id`. It closes the dead-URL loop the whole feature exists
to fix, touching every new component (route, handler, consume TX, policy) and the reused seams
as a CONSEQUENCE of the journey — not as a design goal.

## WS strategy (Architecture of Reference + Project Infrastructure Policy)

Per the retired-strategy note, the WS uses the project's structural defaults, NOT a per-feature
A/B/C/D pick:
- **Driving port** (the public `/invites/accept` route pair) → REAL adapter: the in-process
  `foundry_app::test_support::spawn_app()` axum router over real HTTP (`reqwest::Client`) — the
  driving mechanism the policy already records for every web surface.
- **Driven internal** (the `invites` row, `users`, `tower_sessions`, `workspace_memberships`) →
  REAL adapter: shared testcontainers PG16 + per-scenario `CREATE SCHEMA test_<uuid>` (the
  policy's Slice-1 speed-vs-isolation pivot).
- **Driven internal** (CSRF middleware, session layer, `InviteToken::verify`, `hash_password`,
  `resolve_active_workspace`) → REAL, never mocked (SHIPPED seams reused verbatim).
- **Driven external / non-deterministic**: the only candidate is the **clock** for `expires_at`
  (the just-inside / just-past expiry boundaries, #4/#6, and TOCTOU #14). The policy already
  records a `FakeClock`/`MockClock` injection seam for `expires_at` (HMAC tokens) — DELIVER
  reuses it so the boundary scenarios advance time without sleeping. The WS itself (#1) uses a
  genuinely-live 7-day invite (no clock manipulation needed).

Tag is `@real-io` (driving + driven-internal real per the Architecture of Reference). No
`@in-memory` anywhere in this feature (no in-memory doubles; no Tier B). No `@requires_external`
(no costly external dependency).

## Background seeds a REAL token (no synthesis)

The `Background` runs the SHIPPED provisioning path (the emit site that mints the live signed
`InviteToken` + inserts the `invites` row) so the `id`+`sig` under test is a genuine
HMAC-bound token, not a hand-built fixture. This is the integration-checkpoint #1/#2 guarantee
from the journey: the `sig` POST re-verifies is the SAME value GET rendered, and the consumed
`invite_id` is the row provisioning created.

## RED-state (Mandate 7 / ADR-025)

The crate COMPILES (Gherkin text; no undefined-symbol reference; `acceptance.rs` untouched →
`inventory` force-linking intact) → NOT BROKEN. At runtime against real PG16 the WS is genuine
MISSING_FUNCTIONALITY RED: the `/invites/accept` POST route is unknown, `submit_accept` and
`set_first_admin_password_and_consume` do not exist. DELIVER's RED phase unskips this one
scenario first, drives it GREEN through the new route+handler+consume-TX, commits, then unskips
the next `@pending` scenario one at a time.
