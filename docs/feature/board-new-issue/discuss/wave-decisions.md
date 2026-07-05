# DISCUSS Decisions — board-new-issue

## Key Decisions

- **[D1] Modal container over button-replace**: the "New issue" button `hx-get`s the modal into a dedicated
  container element in `board.html` (e.g. `<div id="modal-root"></div>`) via `hx-target="#modal-root"
  hx-swap="innerHTML"`, rather than replacing the button itself. Keeps the button available and gives the
  modal a stable close target. (OD-1)
- **[D2] Close-on-success via the shipped OOB card + an empty modal target**: the create response is already
  an OOB swap into `[data-column='backlog']` (`issues.rs:293`), so the card lands in Backlog regardless of the
  hx-post target. Make the modal form's `hx-target` the modal container with `hx-swap="innerHTML"` so a
  successful create (whose primary body is the OOB card) leaves the container empty → the modal closes. No new
  JS. (OD-2)
- **[D3] Error renders inside the open modal**: `submit_create` returns `bad_request_fragment("Title is
  required")` (a bare fragment, 400) on empty title. With the modal form targeting the modal container, that
  error swaps INTO the modal (replacing its body) — visible, board untouched, no card created. (OD-3)
- **[D4] No-JS fallback preserved**: the modal form keeps `method="post" action="{{ action }}"` alongside the
  new `hx-post`; with htmx absent the plain POST hits the shipped non-htmx branch (`redirect_to(board)`), so
  the issue is still created and shown. Wiring is additive, never replacing the plain-form contract.
- **[D5] Near-zero backend change (REVISED at DELIVER)**: no handler, service, route, or migration change —
  the modal endpoint, create POST, OOB card, and CSRF token all ship. **Correction**: the button's `hx-get`
  needs the team + project **slugs**, which `BoardPage` (`views.rs:198`) does NOT expose (only `team_name`,
  `project_name`, `key_prefix`, `columns`, `kb_items`). Template-only workarounds are fragile (relative
  `issues/new` resolves wrong on a no-trailing-slash board URL; `project_name|lower` breaks on multi-word
  names like "Auth v2"). So the slice adds **two `String` fields (`team_slug`, `project_slug`) to `BoardPage`**,
  populated in `build_board_page` from the already-present `ProjectRow.slug` + the existing `slugify(team_name)`
  (plus one unit-test call-site fix). No new logic, no migration — surfacing existing data to the template.
  See `## Changed Assumptions` in `requirements.md`.
- **[D6] Repo convention**: legacy multi-file nWave layout (per `[[foundry-nwave-old-multifile-convention]]`);
  no SSOT/feature-delta, no migration. DES step-monitoring exempt (lean mode).

## Requirements Summary

- **Primary need**: make the board's "New issue" button actually file an issue (open modal → title → card in
  Backlog → modal closes), reusing the fully-shipped backend.
- **Walking skeleton**: n/a — the backend skeleton ships; this is client wiring.
- **Feature type**: user-facing (front-end), brownfield.

## Constraints Established

- Template/markup only; no backend change; CSRF + tenancy + no-JS fallback all preserved.
- US-R07 (templates extend base.html).

## Scope Assessment: PASS

Right-sized: 1 story → **1 slice**, one bounded surface (the board template + the modal partial), no new
integration points, well under a day. No split needed.

## Upstream Changes

- None. Brownfield increment; requirements grounded in the shipped board/issue code + the live-verified inert
  button.
