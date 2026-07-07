# Slice 03 — Program JSON change feed

**Goal**: a program can GET an issue's change events as stable JSON over the `/api/v1` surface.
**Story**: US-03.

**IN scope**
- `GET /api/v1/.../issues/{n}/history` — returns the issue's change events as a JSON array of
  `{actor, field, old, new, at}` (ISO-8601 UTC), ordered oldest→newest (stable audit order), from the SAME stored
  events the human timeline reads (no second source of truth).
- Auth/authz + uniform non-enumerable refusal exactly like the other `/api/v1` issue routes (foreign/absent →
  refusal, never 500).
- A pagination-ready envelope (documented shape; v1 may return all with a stable order + a reserved cursor field)
  per the api-contract conventions (ODD-6).
- Acceptance: the JSON contract + auth/non-enumerability + same-events-as-timeline.

**OUT of scope**: the report/CSV (04); write access to history (append-only, no API mutation); webhooks/push.

**Learning hypothesis**: disproves "the stored change-event serializes cleanly for program consumption over the
`/api/v1` contract" if the envelope / auth / non-enumerability needs a second model or path.

**Seams**: `foundry-api` `routes()` (`/api/v1/.../issues`); the slice-01 stored events; the shipped API
auth/non-enumerability pattern (mirror the comments/PATCH routes).
**Dependencies**: slice 01 (stored events). **Effort**: ~0.5 day (a read endpoint over proven data).
