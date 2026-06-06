# DISTILL Driver — Remaining-Surfaces Templating

Owner: acceptance-designer (Sentinel). How the acceptance scenarios for this
move-only feature are wired into the EXISTING cucumber-rs harness. Mirrors
`docs/feature/htmx-web-tier/distill/driver.md`.

## 1. Harness — reused wholesale, zero new infrastructure

Per the Architecture of Reference + the Project Infrastructure Policy
(`docs/architecture/atdd-infrastructure-policy.md`, applied `--policy=inherit`),
this feature adds **no new port and no new mechanism**. Every scenario drives the
SAME in-process axum harness Feature B uses:

| Port | Mechanism (from the project policy) | This feature |
|---|---|---|
| Driving — HTTP (axum routes) | `reqwest::Client` (redirects disabled, no cookie jar) against `InProcHarness::spawn` (real `build_router`) | reused as-is |
| Driven internal — Postgres | shared testcontainers PG 16 + per-scenario schema | reused as-is |
| Driven internal — static assets | `tower_http::ServeDir` over real `crates/foundry-app/static/` | reused as-is (the full-page scenarios link the content-hashed CSS) |
| Driven internal — multipart upload | real `axum::extract::Multipart` | reused as-is (US-R05 error scenarios) |
| Driven external — clock / email / signing | `FakeClock` / `FakeEmailSender` / fixed test keypair | not touched (no auth/email surface moves) |

The single new step module is `crates/foundry-acceptance/src/steps/feature_remaining_surfaces.rs`,
registered in `src/lib.rs` (`pub mod feature_remaining_surfaces;`) and force-linked
in `tests/acceptance.rs` (`use ... as _feature_remaining;`), exactly as Feature B's
module is wired.

## 2. Per-scenario state

New world fields (a small `r_*` namespace, distinct from Feature B's `b_*` so the
two feature modules cannot collide within a scenario):
- `r_signed_in_email: Option<String>` — the persona the reused Feature-B
  `is signed in as a Backend member` Given recorded (read via `signed_in_email`,
  which falls back to `b_signed_in_email`).
- `r_last_body / r_last_status / r_last_headers` — the most recent surface
  GET/POST captured by a When step, consumed by the Then assertions.

## 3. Reused step text (cucumber-rs requires globally-unique phrases)

Givens are reused, NOT redeclared:
- `a workspace "..." exists with admin "..."` (us_06_signin)
- `a member "..." belongs to the team "..."` (us_07_project_create)
- `a project "..." with key prefix "..." exists in the "..." team` (us_08_file_issue)
- `the "..." project has issue ... titled "..." in the backlog` (feature_a)
- `(\w+) is signed in as a Backend member` / `(\w+) has no current browser session`
  (feature_b_web_tier) — these set `b_signed_in_email`, which this module reads.

The two Feature-B signed-in/out Givens are the seam between the modules: they
record the persona; this module's When steps re-authenticate per request (no
cookie jar, mirroring the whole browser suite) and capture into the `r_*` slots.

### Phrase-collision fixes made during RED classification
- `changes the state of "X" to "Y"` collided with `us_09_realtime_sse`. Renamed
  this feature's step to `moves "X" to the "Y" state from the board`.

## 4. Assertion style

Structural assertions go through `support::html_assertions` (scraper CSS
selectors); copy assertions through `body.contains`. Two small helpers are
replicated from `feature_b_web_tier` (they are private there, and the step modules
link independently): `assert_links_local_stylesheet` (asserts a content-hashed
`/static/css/foundry.<hex>.css` link per ADR-B03) and `assert_no_external_origin`.
This is the **selector-and-substring-identical** render contract — the same
ground truth Feature B established.

These are **layer-3 (subprocess/real-IO) acceptance tests**. Per Mandate 9/11 they
are example-based (no PBT generation); per Mandate 8 the state-delta universe-guard
is a layer-1-3 *Python-pilot* construct — the Rust acceptance harness uses the
established scraper/`contains` assertion idiom at this layer, which is the correct
"traditional assertions OK" posture for layers 3+.

## 5. Lanes

All scenarios are `@real-io @driving_adapter` (except US-R07 `@source-tree`, which
makes no HTTP call). They run in the DEFAULT lane and the `@all` lane (no `@slow`,
no `@docker-compose`, no `@manual`). No new lane is introduced.

## 6. RED → GREEN handoff

RED is genuine MISSING_FUNCTIONALITY (see `red-classification.md`). DELIVER unskips
nothing (there are no skip markers — the harness convention is live scenarios);
DELIVER moves each surface's markup into a template and the assertions flip GREEN.
The exact template + view-model wiring per surface is in `step-skeletons.md`.
