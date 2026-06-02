# Slice 1 — Walking Skeleton: "The board is a styled, templated product surface"

> Feature B (htmx-web-tier). Entry slice. Reuses Feature A's shipped `foundry_services` seam.

## Stories
- **US-B01** — Render the issue board from a template (`job_id: htmx-web-1`)
- **US-B02** — Render the board from vendored assets, no CDN (`job_id: htmx-web-2`)
- **US-B06** — Stand up the template + static-asset pipeline (`@infrastructure`, folded in)

## Learning hypothesis
A real template engine + a vendored static-asset pipeline can render the highest-traffic
surface (the board) inside the ≤200 ms budget, fully offline, while keeping the
substring-asserting acceptance tests green. (Riskiest assumption: engine in budget + assets
without a runtime service.)

## End-to-end demonstrable value
Mei opens the Auth v2 board on an air-gapped VM and sees a styled, interactive board —
template-rendered from data fetched through the existing `foundry_services` seam, with
htmx/Alpine/CSS loaded from the binary's own `static/` path and 0 external origins — while the
full existing acceptance suite stays green.

## IN scope
- Template engine wired into `foundry-app`; `templates/` loads (US-B06).
- Static-asset route serving `static/` only, no traversal, no CDN (US-B06).
- Base layout template skeleton (US-B06).
- Board route renders from `templates/board.html` + a shared issue-card partial (US-B01).
- Issue-create fragment + state-change fragment use the same card partial (US-B01).
- Vendored htmx + Alpine + a Foundry stylesheet under `static/`, served by the binary (US-B02).
- Styled board (columns/cards/header) + keyboard operability + visible focus (US-B02).
- Asset-resolution check (missing asset fails the build) (US-B02).
- Empty-board inviting empty state (US-B01).

## OUT of scope (this slice)
- Issue-detail/comment templating (Slice 2), sign-in (Slice 3).
- htmx version bump or directive rename — existing `hx-swap-oob` MOVES into the template
  as-is; the bump is Slice 4 (US-B05).
- Template engine choice / CSS strategy / build-time-vs-vendored asset decision (DESIGN).

## Key constraints (carried)
- Web tier gains NO DB pool; data comes via `foundry_services` (NFR-WEBB-BND-01).
- Render contract byte-stable: "Backlog/Todo/In-Progress/Done", issue key, `data-column`,
  `data-issue-key` (NFR-WEBB-COMPAT-02).
- ≤200 ms P95 render (NFR-WEBB-PERF-01); 0 external origins (NFR-WEBB-PERF-03); no new runtime
  service (NFR-WEBB-INFRA-01).
- Existing board acceptance scenarios stay green (NFR-WEBB-COMPAT-01).

## Demo script
1. Run Foundry on a no-egress host; open the Auth v2 board.
2. Show it is styled (DevTools: all assets from `/static/`, 0 external origins).
3. Press `c`, file an issue; card appears in Backlog via the shared partial.
4. Open the empty Sandbox board; show the inviting empty state.
5. Change the empty-state wording in `templates/board.html` only; suite stays green.
6. Run `cargo test -p foundry-acceptance --release`; passing count unchanged.

## Definition of Done (slice)
- US-B01 + US-B02 + US-B06 ACs all green.
- Acceptance suite passing count unchanged; render bench ≤200 ms P95.
- Boundary guard green (no new DB pool in the web tier).
- 0 external-origin requests on the board (no-egress host).

## Size / sequence
~4-6 days total (B06 ~1d, B01 ~2-3d, B02 ~2-3d, with overlap). P1.
