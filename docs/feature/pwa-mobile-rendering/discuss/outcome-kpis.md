# Outcome KPIs — pwa-mobile-rendering

| KPI | Target | Measurement |
|-----|--------|-------------|
| Viewport declared | `<meta name="viewport" width=device-width>` on every authed page | fantoccini DOM assertion + HTTP body check |
| No horizontal overflow | `scrollWidth <= innerWidth` at 390px on dashboard, board, issue page, open modal | fantoccini @needs-browser at a 390×844 window |
| Board usable on mobile | columns scroll within their container; the page never overflows | fantoccini (AC-02.1) + phone dogfood |
| Dialog fits | open modal ≤ viewport width, body scrolls when tall | fantoccini (AC-02.2) + phone dogfood |
| Nav collapses | mobile affordance (not the full desktop rail) at ≤ breakpoint | fantoccini (AC-02.3) + phone dogfood |
| Tap targets | primary controls ≥ ~44px min dimension at mobile width | fantoccini bounding-box measure (AC-02.4) |
| Manifest valid + served | linked, 200, valid JSON, all required fields incl. 192/512 + maskable icons | fantoccini fetch + parse (AC-03.1/.2) |
| Installable | `display: standalone` + theme-color + apple meta present; browser offers install | fantoccini (layout facts) + real-phone install dogfood |
| Desktop unchanged | desktop layout + every shipped @needs-browser scenario stays green | full `@needs-browser` lane |
| No regressions | default lane + `@needs-browser` lane + `cargo xtask ci` green | full CI |
| Two-place hash intact | CSS hash matches in base.html + lib.rs:297; shipped hashed-URL check green | smoke / unit |

**North-star**: a member pulls Foundry out of their pocket, the board fits and reads cleanly, they triage an
issue with a thumb, and the app sits on their home screen launching like a native app — none of which is
possible today.

## Counter-metric (guard against a green-over-nothing outcome)

Per the standing "a green can be an artefact of the instrument" lesson:
- The no-overflow assertion MUST be shown **red against the current tree** (no viewport meta → the mobile
  window renders desktop-width → overflow) before it's accepted green. That's the direct reproduction of the
  defect and proof the mobile-window oracle actually measures the viewport.
- "Renders correctly" is asserted as **layout facts** (overflow, sizes, manifest served), NOT screenshots or
  "looks fine" — plus a human phone dogfood for the parts a headless assertion can't judge (legibility, the
  real install prompt, thumb-reach).
