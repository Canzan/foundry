# Story Map — pwa-mobile-rendering

## Backbone (user activities)

```
Open on phone   →   Read/triage the board   →   Keep it one tap away
(it fits the        (columns, dialogs, nav       (install to home
 screen)             usable with a thumb)         screen, standalone)
```

## Walking skeleton

**US-01 — viewport meta + no horizontal overflow at 390px.** The precondition: no responsive refinement or
manifest matters if the page renders at desktop width and pinch-zooms. It's also the thinnest end-to-end proof
that the fantoccini lane can drive a *mobile* window and assert the DOM — the instrument this feature depends
on. Ships in hours.

## Slices

| # | Slice | Story | Value shipped | Effort |
|---|-------|-------|---------------|--------|
| 01 | Viewport + no-overflow (walking skeleton) | US-01 | The app fits a phone screen; the fantoccini mobile-window oracle exists | ~0.5 day |
| 02 | Responsive surfaces | US-02 | Board / dialogs / nav usable with a thumb at phone width | ~1–1.5 day |
| 03 | Installable PWA | US-03 | Manifest + icons + theme-color; install to home screen, standalone launch | ~1 day |

Briefs: `docs/feature/pwa-mobile-rendering/slices/slice-0{1,2,3}-*.md`.

## Carpaccio taste tests

| Test | Verdict |
|------|---------|
| Any slice ships 4+ new components? | **Pass** — 01 is a meta tag + minimal CSS + a browser helper; 02 is `@media` rules; 03 is a static manifest + icons + head tags. |
| Every slice depends on a new abstraction? | **Pass** — the only new machinery is the fantoccini *mobile-window* helper, built in slice 01 (the skeleton) and reused. |
| Does any slice disprove a pre-commitment? | **Pass** — 01 disproves "the app already fits a phone / the lane can drive mobile"; 03 disproves "the app is installable" and tests the ODD-3 no-SW-needed assumption. |
| Synthetic-data-only? | **Pass** — every slice is dogfooded on a real phone (or a real chromedriver mobile window) against the real app. |
| 2+ slices identical except scale? | **Pass** — three distinct concerns (viewport, responsive layout, installability). |

## Slice composition gate

Every slice has a user-visible value story (fits the screen / usable with a thumb / installable). No
`@infrastructure`-only slice. The fantoccini mobile helper (slice 01) is precursor tooling *inside* a
value-bearing slice, not a standalone infra slice.

## Prioritization

**01 → 02 → 03.**

- **01 first — highest leverage + the instrument.** It's the precondition for "renders correctly on mobile"
  and builds the mobile-window fantoccini oracle every later slice asserts through. If the lane can't drive a
  mobile viewport (chromedriver window sizing / CDP), it fails here, cheaply.
- **02 second — the bulk of the value**, and it depends on 01's viewport meta to mean anything. Highest
  uncertainty is the board-columns and modal behaviour (ODD-1/ODD-5) — dogfood on a real phone.
- **03 last — installability**, independent of 02 but naturally last (you install something that already works
  on mobile). It tests ODD-3 (does the prompt need a SW).

**Dogfood cadence**: each slice dogfooded on a real phone the same day (and asserted in the fantoccini mobile
lane). The fantoccini oracle proves *layout facts* (no overflow, sizes, manifest served); the true OS install
prompt and "feels right" are a human phone check — the standing lesson that a green lane can be an artefact of
the instrument.
