# DISTILL Driver — Feature B "htmx Web Tier" (`htmx-web-tier`)

Mirrors `docs/feature/web-tier-extraction/distill/driver.md` (Feature A).
Records the harness, the walking-skeleton driver, and the per-scenario state.

## Harness (reused, zero new infrastructure)

Feature B drives the SAME in-process cucumber-rs harness the whole browser
suite uses — no new harness, matching DESIGN (build_router/spawn_app unchanged,
the `/static` route is a `.nest_service` addition):

- `InProcHarness::spawn(now)` (`support/harness.rs`) — binds the real axum
  `build_router` on `127.0.0.1:0` against a per-scenario Postgres schema
  (shared testcontainers Postgres 16-alpine container + `CREATE SCHEMA test_*`).
  This is the production composition root (Pillar 3): the SUT is the real
  router with real session/CSRF middleware and the real `foundry_services` seam;
  only the clock and email are faked (FakeClock / FakeEmailSender), per the
  Architecture of Reference.
- `reqwest::Client` (redirect=none, no cookie jar) issues real HTTP — board
  GETs, sign-in/forgot GETs, comment POST/PATCH (htmx path), issue create POST,
  and `/static/...` asset GETs.
- Structural assertions go through `support/html_assertions.rs` (`scraper` CSS
  selectors + trimmed text), copy assertions through `body.contains` — the
  render contract is selector-and-substring-identical (ADR-B02), NOT bytes.

## Project Infrastructure Policy (applied)

`docs/architecture/atdd-infrastructure-policy.md` already classifies the HTTP
driving port (in-process `spawn_app` + reqwest) and Postgres (driven-internal,
real, per-scenario schema). Feature B adds ONE new driving-adapter mechanism —
the `/static` ServeDir route — and one new driven adapter — the real filesystem
read of the vendored `static/` blobs. These are appended to the policy
(`feature-b-additions` block); the policy file should gain:

```markdown
## Driving
| Static-asset route (`/static` via tower_http::ServeDir) — Feature B (US-B02) | reqwest GET against the SAME in-process spawn_app router; ServeDir reads crates/foundry-app/static/ from the real filesystem | Mounted .nest_service in build_router; GET-only, CSRF-exempt (safe method); path-traversal-safe by construction. |

## Driven internal (real)
| Vendored static blobs (htmx/Alpine/CSS under static/) — Feature B (US-B02/B05) | Real filesystem read by ServeDir from the committed static/ dir | The blobs ship in the repo + the image (Dockerfile COPY); the @real-io @adapter-integration scenario fails if the blob is absent. |
```

(Append-only; git history is the audit trail. The Architecture of Reference
treatment is unchanged — driving = real adapter, driven-internal = real.)

## Walking-skeleton driver (Slice 1, US-B01)

> **Mei opens the Auth v2 board on a no-egress host and sees a styled,
> interactive board — rendered by a template from data fetched through the
> existing `foundry_services` seam, with htmx/Alpine/CSS loaded from the
> binary's own `/static` path and zero external origins — while the full
> existing acceptance suite stays green.**

Driver path: `Given` (workspace/member/project/issues, reusing the existing
seed steps) → `When Mei opens the "Auth v2" board in her browser` (authenticated
GET of `/team/backend/project/auth-v2`) → `Then` the board shows the same
columns + cards (selector-identical), AND links the vendored stylesheet + loads
htmx/Alpine from `/static`, AND references no external origin. The asset
references are the genuine delta — absent today, present once DELIVER moves the
board onto `templates/board.html` + `base.html` and vendors the blobs.

Litmus (Dimension 5): a non-technical stakeholder confirms "yes — the board
should look styled and load instantly offline." Title is a user goal, the Then
steps are user observations (styled board, same issues), not internal side
effects.

## Per-scenario world state (`world.rs`, `b_*` fields)

| Field | Holds |
|---|---|
| `b_signed_in_email` / `b_signed_in_password` | the persona for the authenticated GET |
| `b_last_body` / `b_last_status` / `b_last_headers` | the most recent board/issue/sign-in response |
| `b_live_fragment` | the htmx OOB fragment (the live comment/issue card) |
| `b_reloaded_page` | the issue page after a full reload (compared to `b_live_fragment`) |
| `b_force_template_failure` | whether the clean-500 render-failure path is requested |
| `b_asset_body` / `b_asset_content_type` / `b_asset_cache_control` / `b_asset_status` | the most recent `/static/...` asset GET |

## Step vocabulary (Pillar 1 — domain language)

Background phrases are REUSED unchanged (cucumber-rs requires globally-unique
step text): `a workspace … exists with admin …`, `a member … belongs to the
team …`, `a project … with key prefix … exists in the … team`, `a member … is
registered with password …`, `the … project has issue … (in progress|in the
backlog)`, `the … project has no issues`. Only Feature-B-specific phrases are
declared in `steps/feature_b_web_tier.rs` — all in domain language (`opens the
board in her browser`, `posts the comment … on AUTH-3`, `the live-appended
comment card and the reloaded comment card are structurally identical`). No
technical jargon (HTTP, JSON, route, selector) appears in any scenario title or
step text; technical detail lives only in step bodies.
