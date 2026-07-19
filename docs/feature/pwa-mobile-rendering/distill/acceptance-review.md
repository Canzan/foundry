# DISTILL Acceptance Review — pwa-mobile-rendering

## Coverage vs the design

| Design element | Scenario(s) | Covered |
|----------------|-------------|---------|
| ADR-001 viewport meta + no overflow | S1, S2 | ✓ |
| ADR-001 dialog sheet (ODD-5) | S3 | ✓ |
| ADR-001 columns scroll (ODD-1) | S4 | ✓ |
| ADR-001 nav reflow (ODD-2) | S5 | ✓ |
| ADR-001 tap targets | S6 | ✓ |
| ADR-001 desktop non-regression | S7 (desktop session) | ✓ |
| ADR-002 manifest + icons + meta (no SW) | S8, S9, S10 | ✓ |
| ADR-003 mobile-emulation oracle | ALL `@mobile` scenarios use `open_mobile_session()` | ✓ |
| D5 CSS hash two-place | S11 (source guard) | ✓ |

Every US-01/02/03 acceptance criterion maps to a scenario. No SW scenarios (ADR-002: none in v1).

## The load-bearing review point (ADR-003)

**No scenario may use a narrow desktop window as a stand-in for mobile.** Every `@mobile` scenario drives
`open_mobile_session()` (chromedriver `mobileEmulation`), because headless desktop Chrome ignores the viewport
meta at a narrow window — a resize-only test would be green whether or not the fix exists. The slice-01
throwaway explicitly demonstrates the difference (narrow-window passes the no-viewport tree; emulation reds
it). This is the DISTILL application of "a green can be an artefact of the instrument."

## Honesty about what's asserted vs dogfooded
- Asserted (headless, deterministic): layout facts (`scrollWidth<=innerWidth`, element rects, tap-target
  boxes) and manifest facts (linked, 200, valid JSON, icons served, standalone, theme-color).
- Dogfooded (human phone): legibility at 1×, the real OS install prompt (needs HTTPS), thumb-reach, touch DnD.
  Called out so a green lane is never mistaken for "feels right on a phone."

## Wave-decision reconciliation
| DESIGN | Reflected |
|--------|-----------|
| D1 responsive CSS | S2–S6 assert the layout invariants (no overflow, dialog ≤ viewport, nav affordance, 44px) |
| D2 no SW | zero SW scenarios; S8–S10 prove install via manifest alone |
| D3 mobile emulation | every `@mobile` scenario; the throwaway resize-vs-emulation proof |
| D4 no server change | no scenario asserts a new route; manifest/icons are `/static` |
| D5 hash two-place | S11 |
| desktop unchanged | S7 (shipped desktop session) |

## Port-to-port + falsification
- Driving port = mobile browser DOM + the `/static` manifest HTTP surface. No internal-component reach.
- Falsifications encoded per scenario (S1/S2 red on no-viewport; S3/S4 red pre-`@media`; S7 red on unbounded
  `@media`; S8/S9 red pre-manifest). The oracle can see the defect.

## fail_on_skipped + residuals
- All `@pending`; `fail_on_skipped()` on. Browser-lane traps carried (`[[foundry-browser-lane-fantoccini]]`):
  version match, chromedriver leak (clean between runs), timing flake → bounded waits.
- DELIVER notes: verify `.webmanifest` content-type (fallback `manifest.json`); generate real icon assets;
  HTTPS-install is a dogfood; keep `mobileEmulation` from calling `set_window_size(desktop)`.
- **Verdict: READY for DELIVER.** 11 scenarios (10 browser + 1 source guard) across 3 slices; the mobile
  oracle is faithful, not a narrow-window stand-in.
