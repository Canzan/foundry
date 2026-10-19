# Slice 03: A self-hosted page to read the API and try a call safely

Stories: US-AOD-05, US-AOD-06 | Estimate: 2 days | job_id: `job-api-machine-readable-contract`

## Goal

Each instance serves a Swagger UI page (proposed `/docs/api`, OQ-3) built from
vendored assets. It renders the instance's own `/api/v1/openapi.json`, and
"Try it out" sends calls with a pasted machine token to the same origin only.
The token lives in memory only, the page ignores any spec or config URL in its
query string, and the browser makes no request to another origin.

## IN

- The page, served by the web tier outside `/api` (OQ-3), public (OQ-2), with the D14 title, heading and `<noscript>` copy.
- swagger-ui release files vendored upstream-verbatim, plus an authored-in-tree initializer; every file a `VENDOR.md` row with sha256, the advisory-check date and the total size (D11, NFR-AOD-1, NFR-AOD-3).
- The page's content policy (OQ-4): another origin's script, style, image, font, connection or frame is refused, and the page cannot be framed.
- A visible failure state with the document link when the document cannot be loaded.
- Refused calls from the page (revoked token 401, member 403, guardrail 429) shown as documented.
- D10: spec URL fixed, `url` and `configUrl` ignored, validator off, no persisted authorization, same-origin requests only.
- Browser-lane assertions: zero cross-origin requests; no token in storage, cookies or URL after a call and after a reload; other pages' request sets unchanged (D12).

## OUT

- A link to the page from Foundry's navigation.
- A site-wide content policy (follow-up).
- Any change to the API itself.

## Learning Hypothesis

- **Disproves if it fails**: that swagger-ui can run on Foundry's origin under a restrictive page policy with no cross-origin request at all. If it needs a remote resource or an inline script that cannot be moved to the initializer, the page needs a weaker policy or a different viewer, and the user decides.
- **Confirms if it succeeds**: that vendoring a large third-party bundle fits the shipped provenance model (ADR-CANZAN-THEME-002/003) with no new row shape.

## Acceptance Criteria

- [ ] On dev, the page lists every operation and shows "Foundry API v0.17.0. The OpenAPI document is at /api/v1/openapi.json."
- [ ] A pasted `ci-issue-filer` token lists Backend's `auth` issues (200, AUTH-7); without a token, the 401 envelope.
- [ ] After a reload, no token anywhere in the browser's storage, cookies or address.
- [ ] `/docs/api?url=https://attacker.example/spec.json` shows the instance's own document and requests nothing from `attacker.example`.
- [ ] With scripting disabled, the `<noscript>` copy and link.
- [ ] check-arch R1-R3 green with the new rows; `check_api_no_html` unchanged and green.
- [ ] Dogfood on dev: mint a scratch token, try list issues and revoke the scratch token from the page, reload, inspect the network log.

## Dependencies

- Slice 02 (a full document to render).
- OQ-2, OQ-3 and OQ-4 answered.
- `FOUNDRY_XTASK_INCLUDE_DOCKER=1` for the browser lane.
- Reference class: htmx-web-tier's vendoring of htmx (one upstream-verbatim row plus acceptance), about 1 day, plus the token-handling assertions.
