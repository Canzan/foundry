# Programmatic Foundry (Feature A) — JSON API Contract

Owner: solution-architect (Morgan). Scope: the JSON contract surface for US-W05a (read) and
US-W05c (writes). Propose mode: the contract-surface question (Open Question 2) is presented with
options + a recommendation; the recommendation is also listed in `wave-decisions.md` for user
ratification. Constraints honored: stable versioned contract (NFR-WEB-API-CON-01), no HTML from
JSON handlers (NFR-WEB-API-CON-03), write rule-parity (NFR-WEB-API-CON-02).

## Open Question 2 — contract surface: how does a request select JSON?

| Option | Mechanism | Routing clarity | Boundary-guard enforceability | Caching/versioning | Separation from htmx fragments |
|---|---|---|---|---|---|
| **A — `/api/v1/…` path prefix** (RECOMMENDED) | JSON lives under a distinct path tree; HTML keeps `/team/…` | High — one glance at the route table tells you which tier owns a path | High — the guard checks "no `Html(...)` constructed under handlers mounted on `/api`" by module path; trivially mechanical | Versioning is in the URL (`/v1`); caches key on path naturally | Total — `/api/v1` and `/team/…` never overlap; no `Accept`-sniffing in shared handlers |
| B — content negotiation on existing paths (`Accept: application/json`) | One handler per resource branches on `Accept` | Low — the same handler emits HTML *and* JSON, re-coupling the two presentations in one function | Low — defeats the boundary guard's whole premise (a single handler can emit both; the guard would have to reason about runtime branches) | Versioning needs a media-type param (`application/vnd.foundry.v1+json`) — more complex; `Vary: Accept` caching | Poor — re-introduces the mixed-handler shape this feature set out to kill |
| C — subdomain (`api.host/…`) | Separate host | High | High | Standard | Total, but needs DNS/TLS/host config = new ops surface |

**Recommendation: Option A (`/api/v1/…` path prefix).** It is the only option that makes the
api≠HTML boundary *structurally* checkable by module path (the whole point of US-W06), keeps
versioning in the URL (simplest stable-contract mechanism, NFR-WEB-API-CON-01), and never
re-couples HTML and JSON in one handler. Option B is explicitly rejected because it reverses the
mixed-handler separation; Option C adds an ops surface that violates the one-binary/no-new-infra
ethos.

## Route surface (Feature A)

Defined in the `foundry-api` crate (ADR-W01) and `.merge()`-d into `foundry_app::build_router` via
`foundry_api::routes(state)`. All under `/api/v1`. All require a valid machine token — a bearer JWT
(US-W05b) — except where US-W05a's slice-1 transitional note applies.

```
GET   /api/v1/teams/{team_slug}/projects/{project_slug}/issues          # US-W05a: board issues as JSON
POST  /api/v1/teams/{team_slug}/projects/{project_slug}/issues          # US-W05c: create issue
PATCH /api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number} # US-W05c: update issue state
POST  /api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments          # US-W05c: create comment
PATCH /api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}/comments/{comment_id} # US-W05c: edit comment
```

Path shape mirrors the existing HTML routes (`/team/{team}/project/{project}/…`) but pluralized and
prefixed (`/api/v1/teams/…/projects/…`) — REST-conventional and visually distinct from the htmx
tree. Out of Feature A scope (deferred per `out-of-scope.md`): projects CRUD, attachments, search,
listing filters, pagination, comment delete via API.

**Auth header (US-W05b).** Every authenticated request carries the machine token as a bearer JWT
(ratified mechanism, ADR-W02 / `auth.md`):

```http
GET /api/v1/teams/acme/projects/auth/issues HTTP/1.1
Host: foundry.example.com
Authorization: Bearer eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI…<jti,scope,exp,iat>…
Accept: application/json
```

The `alg` header is always `EdDSA`; the server pins verification to `[EdDSA]` and rejects any other
algorithm (including `none`). No cookie, no CSRF token. The JWT is opaque to the contract beyond
"a bearer credential"; its claims are an internal detail (`auth.md`), not part of the wire contract
clients depend on.

> Slice-1 transitional note (US-W05a): the read endpoint MAY accept the existing browser session
> for auth in slice 1 to prove core neutrality *before* the token surface lands in slice 2. The
> route still lives under `/api/v1` and emits JSON only; slice 2 makes the token the required
> credential. This matches the slice brief (`slices/slice-01-json-read.md`).

## Resource shapes (serde)

Serialization happens in the `foundry-api` crate (the adapter), never in core/store/services. Shapes
are derived from the neutral `foundry-services` results (`architecture.md` §service seam), not from
store row structs directly, so the wire contract is decoupled from the DB schema (NFR-WEB-API-CON-01).

### Issue (read + write response)

```json
{
  "key": "AUTH-2",
  "number": 2,
  "title": "Refresh token rotation",
  "state": "in_progress",
  "priority": "none"
}
```

- `state` is the canonical lower_snake enum the store persists (`backlog`, `todo`, `in_progress`,
  `done`, `cancelled`) — the API exposes the canonical value, NOT the human label. Input is
  normalized through the SAME `normalize_state` logic the UI uses (`issues.rs:214`), lifted into the
  `foundry-services` crate so `"in-progress"` and `"in_progress"` both map to the canonical value
  (rule-parity).
- Board read (`GET …/issues`) returns a **JSON array** of these objects. Empty project → `[]` with
  `200` (US-W05a AC, scenario 2) — never an HTML empty state, never `204`.

### Create-issue request

```json
{ "title": "Refresh token rotation broken on Safari" }
```

Validation is the service's, identical to the UI: title trimmed, non-empty, ≤256 chars
(`issues.rs:35`,`:84`). On success → `201 Created` with the Issue object (the freshly-allocated
`key`/`number`/`state=backlog`), and a `Location: /api/v1/…/issues/{number}` header.

### Comment (response)

```json
{
  "id": "0192f…",
  "author_email": "mei@acme.com",
  "body_html": "<p>Picked up — investigating <code>SameSite</code>.</p>",
  "edited": false,
  "created_at": "2026-05-31T12:00:00Z"
}
```

`body_html` is the **core-sanitized** HTML (`render_comment_markdown`) — the SAME bytes the UI
stores and renders (NFR-WEB-BND-03; US-W05c scenario 4). The API does not sanitize; it serializes
what the service produced. (Emitting sanitized HTML inside a JSON *string field* does not violate
api≠HTML: the boundary guard forbids constructing an `axum` HTML *response body*, not a JSON string
that happens to contain markup — see `boundary-guard.md` for the precise rule.)

### Create-comment request

```json
{ "body": "Picked up — investigating SameSite." }
```

## Status code conventions

| Situation | Status | Body |
|---|---|---|
| Read success | `200` | JSON (array or object) |
| Create success | `201` | created resource + `Location` header |
| Update success | `200` | updated resource |
| Empty collection | `200` | `[]` |
| Validation failure (e.g. empty title) | `422` | error envelope (`title_required`) — same rule as UI's "Title is required" |
| Malformed JSON body | `400` | error envelope (`invalid_json`) |
| Missing/invalid/bad-signature/wrong-alg/expired/revoked JWT | `401` | error envelope (`unauthorized`) |
| Token lacks scope / not a member / not author | `403` | error envelope (`forbidden`) |
| Team/project/issue/comment not found | `404` | error envelope (`not_found`) |
| Comment soft-deleted (gone) | `410` | error envelope (`gone`) — parity with UI's 410 |

Mapping is centralized in `api`'s error type (`error-and-observability.md`) so every service
`ServiceError` variant maps to exactly one status + envelope code. US-W05c scenario 5 (empty
title) and scenario 6 (non-author edit → 403) are direct consequences of reusing the service's
authz/validation.

## Error envelope

Every non-2xx JSON response uses one stable shape (NFR-WEB-API-CON-03 — errors are JSON, never
HTML):

```json
{
  "error": {
    "code": "title_required",
    "message": "Title is required"
  }
}
```

- `code` is a stable machine-readable token (snake_case, part of the v1 contract — adding codes is
  non-breaking, renaming is breaking).
- `message` is human-readable and, where a UI equivalent exists, carries the **same copy** the UI
  shows ("Title is required", "You may only edit your own comments.") so API and UI errors are
  recognizably the same rule (NFR-WEB-API-CON-02). The copy lives on the `ServiceError` variant
  (one source), not duplicated in the adapter.
- No stack traces, no SQL, no token material ever appears in an envelope (NFR-WEB-API-SEC-03).

## PATCH vs PUT and idempotency

- **PATCH** for partial updates (issue state, comment body) — the request carries only the changed
  field(s); the service applies a partial mutation via the existing `update_*_with_outbox` methods.
  PUT (full-resource replace) is not offered in Feature A; the smallest real surface is partial
  state/body changes (US-W05c).
- **Idempotency**: `PATCH` state to the same value is naturally idempotent (the store `UPDATE` is a
  no-op-equivalent and re-emits an outbox event; that matches UI behavior). `POST` create is **not**
  idempotent (each call allocates a new sequential key via the row-locked counter,
  `insert_issue_with_outbox:651`) — same semantics as the UI's create. Client-supplied idempotency
  keys are out of scope for Feature A (deferred; `out-of-scope.md` rate-limiting/abuse class).

## Versioning strategy (NFR-WEB-API-CON-01)

- Version lives in the path: `/api/v1`. A breaking change (removing/renaming a field, changing a
  `state` enum value, changing an error `code`) requires `/api/v2`; v1 keeps responding with its
  frozen shape. Additive changes (new optional field, new error `code`, new endpoint) stay in v1.
- **Contract snapshot test** (NFR-WEB-API-CON-01 test): a recorded v1 response for the board
  endpoint is asserted structurally in the acceptance suite; a breaking field change fails the
  snapshot before merge. This is the regression net that lets the UI/markup evolve (Feature B)
  without breaking integrators.
- No OpenAPI/SDK generation in Feature A (deferred, `out-of-scope.md`); the snapshot test + this
  document ARE the contract.

## Content negotiation guarantee (NFR-WEB-API-CON-03)

Because the surface is path-prefixed (Option A), negotiation is unambiguous by construction: a
request to `/api/v1/…` is JSON; a request to `/team/…` is HTML. No handler inspects `Accept` to
decide format. The boundary guard enforces that no `/api/v1` handler can construct an HTML response
body, and (Feature B) no `/team` handler constructs a JSON body.
