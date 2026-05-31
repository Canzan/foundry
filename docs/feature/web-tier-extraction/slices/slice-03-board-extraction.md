# Slice 3 — "The board is a real, styled web peer of the same core"

> DRIVER-CORRECTED (2026-05-30). Was Slice 1 (walking skeleton) under the old web-first order;
> now Slice 3 — the entry of the SECONDARY web-tier track, reusing the presentation-neutral core
> seam the JSON-API track (Slices 1-2) already proved.

## Stories
- **US-W01** — Extract the issue board into a web tier over a core seam.
- **US-W02** — Render the board from vendored assets so it looks like a product.

(Two stories, shipped together as one release, so the slice is not fat but is clearly user-visible.)

## Learning Hypothesis
*The SAME presentation-neutral core seam that feeds JSON (proven in Slice 1) can feed a
template-rendered HTML board inside the ≤200 ms server-render budget AND keep the substring-
asserting acceptance suite green — i.e. the web tier is a clean peer consumer at no runtime cost.*

## End-to-End Demonstrable Value
Mei opens `/team/backend/project/auth-v2`, sees a **styled** board (columns, cards, header)
served by `foundry-web` from a template using vendored htmx/Alpine/CSS, files an issue with
`c` (same fragment contract), and the full existing board acceptance suite is green — while the
web tier provably never touches Postgres directly (it uses the same core seam as the JSON API).

## IN scope
- New `foundry-web` module that owns the board render (board page + issue-create fragment +
  state-change fragment) by calling the existing core/store seam (the one Slice 1 proved neutral).
- Board template + a single **issue-card partial** used by full-page, htmx-swap, and SSE paths.
- Static-asset pipeline: vendored htmx, Alpine, and a Foundry stylesheet under the web tier's
  static path, served by the binary (no CDN).
- Inviting empty-board state.
- Preserve the render contract: "Backlog"/"Todo"/"In-Progress"/"Done" labels, issue-key format,
  `hx-swap-oob` target, `data-column`/`data-hx-fragment` markers.

## OUT of scope (this slice)
- Issue-detail/comments templates (Slice 4), sign-in templates (Slice 5).
- JSON API track (Slices 1-2, already delivered).
- htmx version choice / `hx-`-vs-`data-hx-` normalization (DESIGN; preserved as-is here).
- Template-engine choice and asset-build details (DESIGN).
- Restyling beyond "looks intentional + accessible".

## Boundary invariants asserted by this slice
- `foundry-web` has **no** DB pool dependency; board data flows via the core seam
  (NFR-WEB-BND-01). US-W06's web≠DB guard begins biting here.
- Web and JSON board use one core call (NFR-WEB-BND-05).
- In-process only — no network hop between web and core (NFR-WEB-BND-04).
- One binary, one Postgres, no new service, no CDN (NFR-WEB-INFRA-01).

## Shared artifacts (see journey.md Journey 1)
`$BOARD_COLUMNS` (core constant), `$ISSUE_CARD_MARKUP` (one partial, three call sites),
`$ISSUE_KEY` (Postgres sequence), `$CSRF_TOKEN` (unchanged browser contract),
`$STATIC_ASSET_PATHS` (vendored, no CDN), `$CORE_BOARD_QUERY` (shared with the JSON endpoint).

## Acceptance anchors
- `Board renders through the web tier with the same content` (US-W01)
- `The existing board acceptance scenarios remain green` (US-W01)
- `The web tier holds no direct database access` (US-W01)
- `The board loads with vendored styles and scripts, no external CDN` (US-W02)
- `The board is keyboard operable with visible focus` (US-W02)

## Definition of Done (slice)
- All US-W01 + US-W02 ACs met; their UAT scenarios green.
- `cargo test -p foundry-acceptance` passing count does not drop (NFR-WEB-COMPAT-01).
- Render-path benchmark within the ≤200 ms P95 budget (NFR-WEB-PERF-01).
- Demoable: styled board + `c`-to-create in a single session.

## Estimate
~5-6 days (US-W01 M≈3d + US-W02 M≈2-3d), one developer.

## Dependencies
Slice 1 (US-W05a) — the presentation-neutral core seam already exists and is proven.
If the feature is split (D8), this is the entry slice of Feature B.
