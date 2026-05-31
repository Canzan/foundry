# Slice 1 — Walking Skeleton: "Foundry data is readable as JSON from a neutral core"

> DRIVER-CORRECTED (2026-05-30). The thinnest end-to-end slice that proves the now-PRIMARY job:
> a real machine consumer can read Foundry data as JSON from a presentation-neutral core.

## Stories
- **US-W05a** — Read the board's issues as JSON over a presentation-neutral core seam.

## Learning Hypothesis
*Core can feed a non-HTML JSON consumer through the SAME call path the UI uses — i.e. core is
genuinely presentation-neutral, not HTML-shaped.* This is the riskiest assumption in the whole
feature: if core is secretly HTML-shaped, the first-class-API premise (the headline) collapses.

## End-to-End Demonstrable Value
Devansh runs `curl -H "Accept: application/json" .../team/backend/project/auth-v2/issues`
(exact path/negotiation TBD in DESIGN) and gets a JSON array of the Auth v2 board's issues
(keys, titles, states) — served by `foundry-api` from the same core call the UI board uses,
with zero HTML — while the full existing acceptance suite stays green.

## IN scope
- New `foundry-api` module serving ≥1 read endpoint (the board's issues) as JSON.
- The endpoint reuses the same core/store call the web board will use (presentation-neutral seam).
- JSON-only responses (no HTML), including the empty case (`[]`, HTTP 200).
- Authorization equivalent to the web tier (membership). For this slice the existing browser
  session MAY authenticate the call; the machine-token surface arrives in Slice 2.

## OUT of scope (this slice)
- Machine-token auth (Slice 2 / US-W05b), JSON writes (Slice 2 / US-W05c).
- The boundary CI guard (Slice 2 / US-W06).
- Any web-tier template extraction (Slices 3-5).
- Route prefix, content negotiation, serialization shape, API versioning mechanism (DESIGN).

## Boundary invariants asserted by this slice
- `foundry-api` emits JSON only, never HTML (NFR-WEB-API-CON-03, NFR-WEB-BND-02).
- Core is presentation-neutral: the JSON endpoint and (later) the HTML board use one core call
  (NFR-WEB-BND-05).
- In-process only — `foundry-api` is a peer module, no second process (NFR-WEB-BND-04).

## Shared artifacts (see journey.md)
`$BOARD_ISSUES` (one core call, two future consumers: JSON + HTML), `$ISSUE_KEY` (Postgres
sequence), `$CORE_BOARD_QUERY` (the presentation-neutral seam).

## Acceptance anchors
- `The board's issues are available as JSON` (US-W05a)
- `An empty project returns an empty JSON array` (US-W05a)
- `The JSON tier reuses the same core data path as the web tier` (US-W05a)
- `Unauthorized JSON access is refused` (US-W05a)

## Definition of Done (slice)
- All US-W05a ACs met; UAT scenarios green.
- `cargo test -p foundry-acceptance` passing count does not drop (NFR-WEB-COMPAT-01).
- Response parses as JSON with 0 HTML bytes; data provably via the same core call as the UI.
- Demoable: a `curl` returning the board's issues as JSON in a single session.

## Estimate
~2-3 days (US-W05a M), one developer.

## Dependencies
None (entry slice). Establishes the presentation-neutral core seam reused by every later slice.
