# Token-Management API — JSON Contract (DESIGN)

> **Amendment, 2026-10-18 (api-openapi-document DESIGN, OQ-11): the OpenAPI document is now the
> contract of record.** It is committed at `crates/foundry-api/openapi.json` and served at
> `GET /api/v1/openapi.json` with no token. `cargo xtask ci` fails when it drifts from the code.
>
> **What the document records for these two routes:**
>
> | Route | Answers |
> |---|---|
> | List tokens | 200, a `Token` array, newest first |
> | Revoke a token | 204, no body; also 400 `text/plain` for a `{jti}` that is not a UUID, and 429 `rate_limited` |
> | Both | 401, 403 `forbidden` (decided before the address), 404 `not_found`, 500 `internal` |
>
> The `Token` schema has exactly the 8 `TokenJson` keys and no value-bearing key. No create-token
> operation exists; a `POST` on the tokens path answers 405. The `machineToken` scheme says a token
> is minted by a signed-in workspace admin at `/admin/tokens`.
>
> **What this file still holds.** The design rationale below stays authoritative: placement, the
> address resolution order (2026-10-15), the guardrail and the no-mint boundary. Where this file and
> the document disagree, the document is right.
>
> **Findings across the whole API (C1-C13).** They are listed in the web-tier-extraction
> api-contract.md amendment and in `docs/feature/api-openapi-document/feature-delta.md`. The ones
> touching this file:
> - **C6** — 204, 405, 429 and 500 are part of the status set.
> - **C9** — the envelope messages are the code's own (`forbidden`, `not found`).
> - **C13** — 401 is also answered when the token registry cannot be read.
> - **"400/422 by axum"** below — for a bad `{jti}` the answer is 400 `text/plain`.

> Two routes under the EXISTING `/api/v1`, bearer-authenticated by the SHIPPED `MachinePrincipal`
> extractor, using the SHIPPED `status_for` / `ErrorBody` envelope. No new error shape. No mint.

## 1. Path placement & versioning

Both routes sit under the existing versioned prefix and mirror the issue/comment path shape
(`/api/v1/teams/{team_slug}/projects/{project_slug}/…`). Versioning is inherited: a future breaking
change would land under a new `/api/v2`, never by mutating these. No new version is introduced.

```
GET    /api/v1/teams/{team_slug}/projects/{project_slug}/tokens
DELETE /api/v1/teams/{team_slug}/projects/{project_slug}/tokens/{jti}
```

**The address is resolved (amended 2026-10-15, user decision).** The `{team_slug}`/`{project_slug}`
segments are no longer ignored. Order, all in `foundry_services::tokens` (`list_project_tokens`,
`revoke_project_token`; authz never in the adapter, `check_api_no_adhoc_authz`):
1. the workspace-admin gate — a non-admin is `403 forbidden` whatever the address, so the 403 is
   decided before the address is read and says nothing about it;
2. the team by slug in the bound principal's workspace — unknown → `404 not_found`;
3. a team-scoped credential whose scope is another team → `404 not_found`;
4. the live project by slug under that team — unknown or deleted → `404 not_found`.
Those 404s are the API's one not-found envelope, byte-identical to every other `/api/v1` door. The
list and the revoke behind a live address stay **workspace-wide** (the token registry is per
workspace, not per project); no migration.

`{jti}` is a UUID (the token id / row PK; safe to display; the revoke key). Axum path type
`Path<(String, String, uuid::Uuid)>` — a non-UUID `{jti}` fails extraction → 400/422 by axum's
default, BEFORE the handler; that is acceptable and does not leak existence (it is a parse failure,
identical for any malformed id).

## 2. `GET .../tokens` — list (US-TMA01)

Returns a JSON **array** of token metadata for the caller's workspace, newest-first (the use-case
ORDERs by `created_at DESC`). **Never a token value.** Mirrors `TokenView` exactly.

### Response shape — `TokenJson` (the CREATE-NEW serde struct)

```json
[
  {
    "jti": "0190a5c1-2b3d-7e4f-8a9b-1c2d3e4f5a6b",
    "label": "ci-issue-filer",
    "scope_team_id": null,
    "scope_team_name": null,
    "expires_at": "2026-09-05T00:00:00Z",
    "revoked": false,
    "last_used_at": "2026-06-07T03:11:00Z",
    "minted_by": "priya@acme.dev"
  }
]
```

| JSON field | Type | Source (`TokenView`) | Notes |
|---|---|---|---|
| `jti` | string (UUID) | `jti` | the token id / revoke key |
| `label` | string | `label` | admin-chosen label |
| `scope_team_id` | string\|null | `scope_team_id` | `null` = whole-workspace grant |
| `scope_team_name` | string\|null | `scope_team_name` | resolved team name; `null` for workspace grant or unresolved team |
| `expires_at` | string (RFC3339) | `expires_at` | |
| `revoked` | bool | `revoked` | derived from `revoked_at.is_some()` |
| `last_used_at` | string (RFC3339)\|null | `last_used_at` | `null` = never used |
| `minted_by` | string\|null | `minted_by` | resolved issuer email; `null` = deleted admin / legacy row |

**Field-name decision (Q-LIST-SHAPE — RECOMMENDED, confirm at ratification):** field names are the
verbatim `TokenView` field names in snake_case. Rationale: (a) zero translation risk / one obvious
mapping; (b) `minted_by` (not `created_by`) matches the use-case's resolved-issuer semantics and the
web `TokenRow`; (c) timestamps are RFC3339 strings (the same `format_ts` the web UI uses), the
conventional JSON timestamp encoding. **NO `value` / `token` / `secret` / `hash` key exists in the
struct** — enforceable by an API contract assertion over the response keys (NFR-TMA-SEC-02).

Status: empty registry → `[]` with **200** (not 404) — US-TMA01 AC. Refusals below.

## 3. `DELETE .../tokens/{jti}` — revoke (US-TMA02 / US-TMA03 revoke-self)

Calls `Services::revoke_token(&principal, jti)` after the rate guardrail. Idempotent. Revoke-self
allowed (the caller's own `jti` is in its own workspace — a subset of `revoke_token`; the in-flight
request succeeds because auth already passed; the denylist bites the NEXT call).

**Success response (Q-DELETE-RESPONSE — RECOMMENDED `204 No Content`):**

```
HTTP/1.1 204 No Content
(no body)
```

Rationale for **204 over 200+body**: (a) `revoke_token` returns `()` — there is no resource
representation to return, and the LIST is the canonical read-after-write source (US-TMA04); (b) 204
is the idiomatic REST result for a successful DELETE with nothing to say; (c) the contract envelope
is for *errors* — a success that fabricates a `{"revoked": true}` body would invent a new success
shape the rest of the API does not use (the issue/comment writes return the resource, but revoke has
none). 204 keeps the contract minimal and the read-after-write story honest (re-LIST shows
`"revoked": true`). Idempotent re-DELETE also returns 204 (NFR-TMA-REL-01).

> If the user prefers an explicit confirmation for SDK ergonomics, the fallback is
> **200 + `{"jti":"…","revoked":true}`** — a small, value-free body. Recommended default remains 204.

## 4. Status-code table (all via the SHIPPED `status_for` envelope)

| Outcome | HTTP | Body | Source |
|---|---|---|---|
| List OK (incl. empty) | 200 | `[TokenJson]` (or `[]`) | handler |
| Revoke OK / re-revoke (idempotent) | **204** | none | handler |
| No / malformed / expired / forged / revoked / wrong-alg / `alg:none` bearer | 401 | `{"error":{"code":"unauthorized","message":"unauthorized"}}` | `MachinePrincipal` extractor (reused, non-enumerable, identical) |
| Non-management caller (bound user not workspace-admin) | 403 | `{"error":{"code":"forbidden","message":"forbidden"}}` | `ServiceError::Forbidden` → `status_for` |
| Unknown or deleted team or project in the path, or a team outside a scoped credential (after the admin gate) | 404 | `{"error":{"code":"not_found","message":"not found"}}` | `ServiceError::NotFound` → `status_for` (2026-10-15) |
| Revoke unknown OR cross-workspace `jti` | 404 | `{"error":{"code":"not_found","message":"not found"}}` | `ServiceError::NotFound` (non-enumerable) → `status_for` |
| Revoke storm beyond the guardrail | **429** | `{"error":{"code":"rate_limited","message":"…"}}` | guardrail — see `rate-guardrail.md` |
| Internal failure | 500 | `{"error":{"code":"internal","message":"internal error"}}` | `ServiceError::Internal` → `status_for` |

**Non-enumerability (NFR-TMA-SEC-03):** 403 (non-management) leaks no registry content; unknown and
cross-workspace `jti` are the byte-identical 404; every bearer-auth failure class is the byte-identical
401. All three properties are inherited unchanged from the use-cases + extractor.

## 5. Error envelope (REUSE — no new shape)

The SHIPPED `ErrorBody { error: { code, message } }` (`foundry-api/src/lib.rs:44`). `status_for`
already maps `Unauthorized/Forbidden/NotFound/Validation/Internal`. The 429 `rate_limited` code is
the only addition; it is emitted by the guardrail layer (a 429 is outside `ServiceError`'s current
variants — see `rate-guardrail.md` for whether it rides as a guardrail-layer response or a new
`ServiceError::TooManyRequests`). **No HTML, SQL, stack trace, or credential material in any body**
(boundary guard LAYER 1a keeps foundry-api HTML-free).

## 6. Read-after-write consistency (NFR-TMA-CON-02 / US-TMA04)

A LIST after a successful revoke shows the same row with `"revoked": true`; every other field
byte-identical to the prior read (modulo `last_used_at` advancing). The 204 carries no
representation, so the LIST is the single canonical read — no two success shapes to keep in sync.
</content>
