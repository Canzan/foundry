# ADR-002 — One `DESCRIPTION_MAX_LEN`, applied to create AND edit, matching the DB CHECK

**Status**: Accepted (2026-07-17) · **Resolves**: ODD-1 · **Story**: US-03 (+ AC-02.4)

## Context

DISCUSS asserted that `description_md` was "unbounded on the shipped edit path." **DESIGN verification corrected
this**: `crates/foundry-store/migrations/0001_init.sql:70` defines

```sql
description_md TEXT NOT NULL DEFAULT '' CHECK (length(description_md) <= 262144)
```

So there IS a bound — at the **database**, 262144 characters (Postgres `length()` counts characters). What is
missing is an **application-level** bound: `TITLE_MAX_LEN = 256` guards the title in the service (`issues.rs:60`,
`:188`), but there is no corresponding description guard. The real defect is therefore not "unbounded storage"
but a **bad error surface**: an over-long edit description is passed verbatim to the DB, the CHECK rejects it,
and that surfaces as `IssueInsertError::Store(_)` → `ServiceError::Internal` → **HTTP 500**, instead of a clean
"Description is too long" validation error.

The dev database was checked for a data-driven value (7 issues, max description 39 chars, avg 7) — far too
small to justify a product limit. The DB CHECK is the only authoritative existing bound.

## Decision

Introduce **`const DESCRIPTION_MAX_LEN: usize = 262144;`** in `foundry-services/src/issues.rs`, beside
`TITLE_MAX_LEN`, and enforce it in the shared service validation on **both** `create_issue` and
`edit_issue_details`.

- **Value = 262144**, matching the DB CHECK exactly (user decision, 2026-07-17). This makes the change a pure
  **"same rule, better error"**: the app accepts exactly what the DB accepts and refuses exactly what the DB
  refuses — no shipped-behavior tightening, no content that saves today becomes refused.
- **Counted with `chars().count()`**, mirroring the title rule, so multi-byte text is measured in characters
  (consistent with the DB's `length()` which also counts characters — the two bounds are directly
  comparable).
- **Error**: `ServiceError::Validation { code: "description_too_long", message: "Description is too long" }`
  (ODD-3). On web → the existing `bad_request_fragment` arm (400). On API → **422** with the same message
  (AC-02.4), mirroring how `title_required` maps today.

### What changes, concretely

| Path | Before | After |
|------|--------|-------|
| Edit, over-long description | DB CHECK → 500 Internal | Service → 422 / `description_too_long`, nothing written |
| Create, over-long description | (create can't take a description today) | Service → 422 / `description_too_long`, no issue created |
| Edit/create, ≤ 262144 | Accepted (edit); n/a (create) | Accepted — identical threshold, unchanged |

The edit path's refusal reads old values then UPDATEs; the guard runs **before** the store call, so on refusal
the issue's title and description are both untouched (AC-03.3 — no partial write).

## Alternatives considered

- **`DESCRIPTION_MAX_LEN = 65536`** (the user's first choice, before the DB CHECK was known) — reconsidered
  and rejected once 262144 surfaced: 65536 is *below* the DB bound, so it would newly **refuse** descriptions
  of 65536–262144 chars that the edit path can store today. That is a product decision to make descriptions
  shorter, not an error-message fix — and no evidence supports it.
- **No app bound; catch the DB error and map 500→422** — rejected: relies on parsing a driver/constraint
  error string to distinguish "too long" from other store failures; brittle and couples the service to the
  Postgres error format. An explicit length check is deterministic and testable at the boundary.
- **Raise the DB CHECK and pick a smaller app limit** — rejected: adds a migration (breaks the
  no-migration property) for no benefit.

## Consequences

- **Changes shipped edit behavior** in one respect only: an over-long edit description now returns 422 instead
  of 500. That is strictly better; the accept/reject *threshold* is unchanged. Recorded in `upstream-changes.md`.
- Defense in depth: the DB CHECK remains as the last line; the app bound keeps it from ever firing on the app
  path. If they ever diverge, the app (tighter-or-equal) wins first — a property worth an assertion.
- Boundary tests required: at 262144 accepted, at 262145 refused, multi-byte counted as chars (AC-03.4/.5).
- On the **browser** path the new 422/400 message is not shown (htmx does not swap 4xx — architecture §Error
  behavior); the API path shows it. Making it visible in-browser is the deferred app-wide defect, not this ADR.
