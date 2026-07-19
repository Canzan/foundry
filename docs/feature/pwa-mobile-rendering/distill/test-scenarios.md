# DISTILL Test Scenarios — pwa-mobile-rendering

> SSOT: `crates/foundry-acceptance/tests/features/pwa-mobile-rendering.feature`. All `@pending`; DELIVER
> un-@pends per slice. `@needs-browser` **mobile-emulation** lane (ADR-003).

## Configuration
- **test_type**: core feature — mobile rendering + installability. Cross-cutting (UI/CSS/static + test tooling).
- **framework**: cucumber-rs; DELIVER glue at `steps/feature_pwa_mobile.rs` (registered + force-linked). NEW
  `open_mobile_session()` in `browser_harness.rs` (chromedriver `mobileEmulation`, deviceMetrics 390×844
  mobile:true). Reuse sign-in/board/dialog helpers.
- **integration**: real Postgres (testcontainers) + fantoccini + chromedriver. `@needs-browser`.
- **HARNESS BOUNDARY (ADR-003, load-bearing)**: mobile emulation, NOT a narrow desktop window — headless
  desktop Chrome ignores the viewport meta at a narrow window, so a resize-only test is green-over-nothing.
  Assertions are LAYOUT FACTS (`scrollWidth<=innerWidth`, element rects, manifest fetch/parse), never
  screenshots. Legibility, the OS install prompt, and thumb-reach are human phone-dogfood items. NOT Playwright.

## Scenario catalog

### Slice 01 — viewport + no overflow (walking skeleton)
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S1 | Mobile session fits the board | mobile oracle works; viewport meta present; no overflow | `@lane-probe @walking_skeleton` |
| S2 | Primary surfaces fit (outline ×4) | dashboard/board/issue/open-modal: `scrollWidth<=innerWidth` at 390 | `@us-01` |

### Slice 02 — responsive surfaces
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S3 | Dialog fits + body scrolls | dialog ≤ viewport, page no-overflow, body scrolls | `@us-02` |
| S4 | Columns scroll, page doesn't | page no-overflow; `.board` container scrollable | `@us-02` |
| S5 | Nav collapses | full desktop rail not shown; mobile affordance present | `@us-02` |
| S6 | Tap targets | New-issue control ≥ ~44px min dimension | `@us-02` |
| S7 | Desktop unchanged | DESKTOP session: rail shown, layout matches shipped | `@us-02 @desktop @scoped` |

### Slice 03 — installable PWA (no SW)
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S8 | Manifest linked/served/valid | linked, 200, valid JSON, required fields, 192/512/maskable icons | `@us-03` |
| S9 | Icons served | each icon URL 200 + image content-type | `@us-03` |
| S10 | Standalone + theme + apple | theme-color meta, apple-mobile-web-app-capable + apple-touch-icon, display standalone | `@us-03` |

### Cross-cutting
| # | Scenario | Asserts | Tag |
|---|----------|---------|-----|
| S11 | CSS hash consistent | base.html & lib.rs reference the same `foundry.<hash>.css` (source-level guard) | `@slice1 @cross-feature` |

## Port-to-port coverage
- **Driving port**: the real mobile browser DOM (the user's port) — the only layer that exercises the viewport
  meta + `@media` + manifest as a browser sees them. Plus the manifest/icon HTTP surface (`/static`).
- **Driven port**: none new (no store change). S7 uses the shipped DESKTOP session as the non-regression guard.

## Falsification (a passing scenario must be able to fail)
- **S1/S2 RED against the current tree** (no viewport meta) under `open_mobile_session()` — mobile Chrome uses
  the 980px fallback → overflow. The faithful defect reproduction; proves the oracle measures the true viewport.
- **S3/S4 RED** before their `@media` rules (dialog overflows / columns overflow the page).
- **S7 RED against an unbounded `@media`** that leaks mobile rules into desktop (the blast-radius guard).
- **S8/S9 RED** before the manifest/icons exist (no link / invalid JSON / missing icon files).
- **S1 must ALSO be shown to distinguish window-resize from emulation**: under a plain narrow-window session
  (no mobileEmulation) the no-viewport tree would falsely pass — demonstrating why ADR-003's emulation is
  required (a throwaway check during slice 01).

## Graceful degradation
DESIGN present (ADR-001/002/003) → every scenario maps to a designed seam (viewport meta, `@media` rules,
static manifest/icons, `open_mobile_session()`). Wave-decision reconciliation **PASS**: ODD-1..5 ratified; no
service worker (ADR-002); no new route/migration. The HTTPS-for-install and `.webmanifest`-content-type items
are DELIVER verification notes.
