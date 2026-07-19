# DISCUSS Decisions — pwa-mobile-rendering

## Key Decisions

- **[D-TOOL] Mobile-render tests use the EXISTING fantoccini + chromedriver `@needs-browser` lane, NOT
  Playwright** (user-ratified 2026-07-19: original "use Playwright" wording explicitly overridden with "Stick
  with fantoccini"). No `package.json`, no Node, no second browser stack — the repo stays Rust-only. Mobile
  viewport is driven via chromedriver window sizing / CDP device metrics in the shipped `BrowserHarness`
  (ADR-007, keyboard-shortcut-bindings). Load-bearing: it defines how "renders correctly on mobile" is proven.
- **[D1] Layered baseline, sliced by dependency**: (1) viewport meta + no-overflow is the precondition and the
  walking skeleton — nothing renders "correctly" without it; (2) responsive CSS makes surfaces usable; (3)
  manifest + icons + theme-color make it installable. Each is an independently shippable slice.
- **[D2] Static assets only — no new content routes, no migration** (latest stays `0014`). The manifest,
  icons, and any service worker are served by the existing `/static` mechanism.
- **[D3] Responsive CSS is additive; desktop is unchanged.** `@media` breakpoints add mobile behaviour without
  altering the desktop layout; shipped desktop `@needs-browser` scenarios stay green.
- **[D4] Offline / full service-worker caching is OUT of v1.** "Modern PWA" here = **installable + responsive**.
  htmx + SW caching has real correctness gotchas (stale fragments, CSRF, the hashed-asset cache story). A
  *minimal* SW ships only if DESIGN finds it required for the install prompt (ODD-3). Full offline is a
  follow-up feature.
- **[D5] CSS hash-rotation discipline**: every CSS change rotates the hash in BOTH `base.html` and
  `lib.rs:297` (precedent: navigation-bar-linear-ui). A per-slice checklist item.
- **[D6] JTBD framed lightly, stories tied to outcome KPIs** (navigation-bar-linear-ui precedent; no
  `docs/product` SSOT in this repo). One primary mobile job; no `jobs.yaml` traceability.
- **[D7] Repo convention**: legacy multi-file nWave layout; no SSOT/feature-delta; no PR; trunk-based.

## Open Design Decisions (for DESIGN)

| # | Question | Proposal |
|---|----------|----------|
| ODD-1 | Board columns at mobile width: horizontal-scroll the column strip, or stack columns vertically? | Propose **horizontal-scroll the column strip within its own container** (keeps the kanban mental model; the *page* never overflows). DESIGN + a mobile dogfood decide. |
| ODD-2 | Nav/sidebar mobile affordance: top bar, hamburger drawer, or bottom bar? | Propose a **collapsed top bar with a toggle** (least new JS; reuses the existing rail markup). |
| ODD-3 | Does the install prompt require a service worker? | Verify against current Chrome criteria. Propose: manifest + HTTPS may suffice on Android Chrome; ship a **minimal no-op-fetch SW** only if the prompt needs it, and keep it from caching dynamic HTML. iOS "Add to Home Screen" needs the apple meta regardless. |
| ODD-4 | Icon source + sizes | Need real 192/512 + maskable + apple-touch-icon. Propose generating from a simple Foundry mark; DESIGN/DELIVER produce the assets. |
| ODD-5 | Modal at mobile width: full-screen sheet vs centered card | Propose **full-width bottom/emerging sheet** with a scrollable body (fits small screens, matches app conventions). |

## Requirements Summary

- **Primary need**: use Foundry on a phone (read/triage the board, open issues) and install it to the home
  screen — today impossible (no viewport meta, no responsive CSS, no manifest).
- **Walking skeleton**: US-01 — viewport meta + no horizontal overflow at 390px on the primary authed pages,
  proven in the fantoccini lane at a mobile window.
- **Feature type**: cross-cutting (user-facing mobile UX + test-tooling/infra), brownfield.

## Constraints Established

- fantoccini lane only (D-TOOL); Rust-only, no Node.
- CSS hash-rotation in two places per CSS change (D5).
- Static assets only; no new content route; no migration.
- PWA install needs HTTPS (or localhost) + a valid manifest.
- Progressive enhancement / no-JS + every shipped oracle preserved.

## Scope Assessment: PASS (with a split)

Right-sized as **3 thin slices** (viewport/no-overflow → responsive surfaces → installable), each independently
shippable and dogfoodable on a phone. Oversized signals checked: >10 stories ✗ | >3 bounded contexts ✗ (web
tier only) | effort >2 weeks ✗ (~2-3 days) | independent outcomes that could ship separately — yes, which is
exactly why they are 3 slices. Offline explicitly deferred to keep scope honest.

## Upstream Changes

- None from a prior wave (escalated via `/nw:new`; requirements clear-in-head). The original "use Playwright"
  request was overridden at intake (D-TOOL) — recorded here, not a DISCOVER doc.
