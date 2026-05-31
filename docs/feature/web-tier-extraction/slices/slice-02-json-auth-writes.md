# Slice 2 — "Agents can authenticate and DRIVE Foundry over JSON"

> DRIVER-CORRECTED (2026-05-30). Completes the headline outcome: a machine client authenticates
> with a token and creates+updates issues/comments via JSON, with the boundary guard folded in.

## Stories
- **US-W05b** — Authenticate programmatic clients with a machine token.
- **US-W05c** — Create and update issues and comments through the JSON API.
- **US-W06** — Lock the web/api boundary with a structural guard (`@infrastructure`, folded in).

## Learning Hypothesis
*A machine-token auth surface can be added ADDITIVELY (without perturbing the browser session/
CSRF model) AND API writes can reuse core's write+outbox path with full rule-parity to the UI
(same authz/validation/sanitization).* Validates that the API is genuinely first-class — not a
privileged or divergent back door.

## End-to-End Demonstrable Value
A workspace admin issues Devansh's agent a machine token. The agent
`POST`s `{"title":"Refresh token rotation broken on Safari"}` with `Authorization: Bearer …`
and gets `201` with the created issue as JSON; it `PATCH`es the issue to `in_progress` and posts
a comment — all honoring the same rules the UI enforces, with the new issue appearing on a
watching teammate's board in real time. Revoking the token refuses the next call. And CI now
fails if `foundry-api` ever emits HTML or `foundry-web` ever reaches the DB.

## IN scope
- Machine-token authentication accepted by the API as a first-class, additive credential.
- Admin issuance + revocation of tokens; revoked token refused; scope-bounded authorization.
- JSON write endpoints: create issue, update issue state, create comment, update comment.
- Writes reuse the SAME core write functions + outbox + sanitization as the browser handlers.
- API writes/reads return/emit JSON only (errors as JSON, never HTML).
- Boundary guard (US-W06): CI fails on web→DB pool dependency or api→HTML body.

## OUT of scope (this slice)
- Token MECHANISM (format/storage/hashing/rotation/issuance UX/scoping granularity) — DESIGN.
- JSON request/response shapes, status conventions, PATCH-vs-PUT/idempotency — DESIGN.
- Broader resource coverage (projects CRUD, attachments, search) — follow-on.
- Rate-limiting/quotas, OpenAPI/SDK gen, outbound webhooks — out (see out-of-scope.md).
- Web-tier template extraction (Slices 3-5).

## Boundary invariants asserted by this slice
- Browser session/CSRF model UNCHANGED by the additive token surface (NFR-WEB-API-SEC-01).
- Tokens revocable + scope-bounded; no privilege escalation; authz decisions in core
  (NFR-WEB-API-SEC-02).
- Tokens handled as secrets — not plaintext at rest, not logged (NFR-WEB-API-SEC-03).
- API writes have rule-parity with the UI via the same core write+outbox (NFR-WEB-API-CON-02).
- api≠HTML and web≠DB enforced structurally by US-W06 (NFR-WEB-BND-01/02).

## Shared artifacts (see journey.md)
`$MACHINE_TOKEN` (issued by admin, presented by client), `$CORE_WRITE_PATH`
(`insert_issue_with_outbox` etc. — one path for UI + API), `$OUTBOX` (one event path so API and
UI writes are indistinguishable to realtime), `$AUTHZ_DECISION` (core-decided, reused by both).

## Acceptance anchors
- `A valid machine token authenticates an API request` (US-W05b)
- `A machine-token request needs no browser session or CSRF token` (US-W05b)
- `A revoked machine token is refused` (US-W05b)
- `Create an issue through the JSON API` (US-W05c)
- `An API-created issue is visible to the UI in real time` (US-W05c)
- `An invalid write is rejected with the same rule as the UI, as JSON` (US-W05c)
- `An HTML response from the API tier fails the guard` (US-W06)

## Definition of Done (slice)
- All US-W05b + US-W05c + US-W06 ACs met; UAT scenarios green.
- Browser session/CSRF acceptance scenarios still green (additive surface; NFR-WEB-COMPAT-01).
- Paired API-vs-UI write scenarios prove rule-parity (NFR-WEB-API-CON-02).
- Boundary guard bites on an injected violation (api→HTML, web→DB).
- Demoable: token-issue → create/update issue+comment via curl → change shows on the UI board.

## Estimate
~7-8 days (US-W05b M≈3d + US-W05c M-L≈3-4d + US-W06 S≈1d), one developer.

## Dependencies
Slice 1 (US-W05a — the JSON read surface + presentation-neutral core seam). US-W06's web≠DB
half begins biting once US-W01 lands in Slice 3; its api≠HTML half bites from Slice 1 onward.
