# DESIGN Decisions — pwa-mobile-rendering

## ODD resolutions

| # | Question | Resolution |
|---|----------|-----------|
| ODD-1 | Board columns at mobile | Horizontal-scroll the `.board` strip (`overflow-x:auto`) with a `.column` min-width; the page never overflows. Keeps kanban. (ADR-001) |
| ODD-2 | Nav mobile affordance | CSS-only reflow: `.app-shell` column-direction, sidebar → compact top bar. No new JS in v1; drawer is a follow-up. (ADR-001) |
| ODD-3 | Service worker for install? | **No SW in v1.** Modern Chrome doesn't require one; iOS uses apple meta. Avoids htmx-caching hazards; no offline. (ADR-002) |
| ODD-4 | Icon assets | 192 + 512 + maskable + apple-touch-icon under `/static/icons`; DELIVER generates from a Foundry mark. (ADR-002) |
| ODD-5 | Modal at mobile | Full-width scrollable sheet (`width:100%`, `max-height:100vh`, body scrolls). (ADR-001) |

## Key decisions

- **[D1] Viewport meta + `@media`-bounded CSS on existing markup** — no mobile SPA, no new route. Desktop
  untouched. (ADR-001)
- **[D2] Installable via static manifest + icons + head meta; NO service worker, NO offline in v1.** (ADR-002)
- **[D3] The mobile test oracle uses chromedriver `mobileEmulation` (deviceMetrics + mobile), not a narrow
  desktop window** — a narrow window on headless *desktop* Chrome doesn't apply mobile viewport semantics and
  would be green whether or not the viewport meta exists. This is the load-bearing test decision. (ADR-003)
- **[D4] All changes are head tags / CSS / static assets** — no new route/endpoint/migration (latest stays
  `0014`); no Node. Manifest + icons served by the existing `ServeDir` `/static`.
- **[D5] CSS hash rotates in two places** (`base.html` + `lib.rs:297`) on every CSS change (D5 from DISCUSS).
- **[D6] Paradigm unchanged**: `@nw-software-crafter`; no `CLAUDE.md` change.

## Existing-system analysis (performed before design)

- `base.html` head read — no viewport/manifest/theme tags; scripts loaded `<script defer>` (form-errors.js
  precedent for adding assets).
- Layout hierarchy read: `base.html → app_shell.html (.app-shell flex + partials/sidebar.html) → board.html
  (.board > .column, #modal-root)`. Responsive rules hook these existing classes.
- Static serving read: `ServeDir` from `static/` (`lib.rs:230-245`) + path-aware `static_cache_control` — the
  manifest/icons are plain static files; no new route.
- Stylesheet read: one hand-hashed file, zero `@media` breakpoints; hash asserted at `lib.rs:297`.
- Harness read: `open_session` builds `goog:chromeOptions` JSON + fixed desktop window — confirmed a
  `mobileEmulation` variant is a small additive change, and that a narrow window alone would be an unfaithful
  oracle (ADR-003).

## Reuse vs new

- **NEW**: the head tags, the `@media` rules, `static/manifest.webmanifest` + icons, `open_mobile_session()` +
  `feature_pwa_mobile.rs` step glue. **No SW, no route, no migration, no Node.**
- **REUSE**: `ServeDir` `/static`, the hashed-CSS mechanism, the fantoccini `BrowserHarness` +
  chromedriver + sign-in/board/dialog step helpers, the navigation-bar-linear-ui hash-rotation precedent.
- **UNCHANGED**: every server handler/route; the desktop layout; the desktop `@needs-browser` scenarios.

## Handoff

**To**: DISTILL (mobile-window `@needs-browser` scenarios via `open_mobile_session()`, asserting layout +
manifest facts) then DELIVER. Slice plan: 01 viewport + oracle, 02 responsive surfaces, 03 installable
manifest. DELIVER generates the icon assets and verifies the `.webmanifest` content-type + the two-place hash.
