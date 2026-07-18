# Slice 03 — Bound the description on create AND edit

**Goal**: an over-long description is refused with a clear validation error on **both** paths, and nothing is
persisted — replacing today's DB-CHECK **500** on the edit path with a clean 422 at the same threshold.

**DESIGN-corrected premise**: description is NOT unbounded — the DB enforces `CHECK (length ≤ 262144)`
(`0001_init.sql:70`). The bound value is therefore **262144** (ADR-002), matching the DB so nothing that saves
today becomes refused. This slice adds the *application* guard the service is missing.

**Story**: US-03. **Depends on**: slice 01 (create must accept a description before it can bound one).

**IN scope**
- `DESCRIPTION_MAX_LEN = 262144` const beside `TITLE_MAX_LEN` (`services/src/issues.rs:30`), matching the DB
  CHECK; counted with `chars().count()` to match the title rule (and comparable to the DB's `length()`).
- One shared description validation applied in **both** `create_issue` (`:49`) and `edit_issue_details`
  (`:188`) — mirroring how `TITLE_MAX_LEN` is enforced at `:60` and `:188` today.
- `ServiceError::Validation { code: "description_too_long", … }` (ODD-3), surfaced as the in-modal fragment on
  web (mirror `bad_request_fragment("Title is required")`) and **422** on the API (mirror `title_required`).
- Tests: over-long refused on create (nothing created) and on edit (title AND description both unchanged —
  no partial write); at-bound accepted; MAX multi-byte chars accepted.

**OUT of scope**
- A client-side `maxlength` attribute or live counter (server-side truth first; a UI affordance is a separate
  concern and must never be the only guard).
- Any bound on existing rows — no backfill, no migration.
- Revisiting `TITLE_MAX_LEN`.

**Learning hypothesis**: disproves **"one rule, both paths"** if create and edit turn out to need different
description bounds or different error surfaces — which would mean the field isn't actually one concept and
D2 was wrong. Confirms it if a single validation serves both call-sites unchanged.

**Acceptance**: `discuss/acceptance-criteria.md` US-03 + AC-02.4 (the API's 422).

**Seams**: `TITLE_MAX_LEN` + its two enforcement sites (`services/src/issues.rs:30`, `:60`, `:188`);
`bad_request_fragment`; `ServiceError::Validation`; `update_issue_details_with_outbox` (`store/src/lib.rs:1816`)
— the read-old → UPDATE tx that must not partially write.

**Watch items**
- **Behavior change is only the error surface, not the threshold.** Because the bound equals the DB CHECK,
  the same inputs are accepted/refused as today; an over-long edit description that returns **500** today
  returns a clean **422** after this slice. Nothing that saves today becomes refused.
- The edit path's refusal must leave title *and* description untouched (AC-03.3) — the update tx reads old
  values first, so validate before it opens.
- Assert the defense-in-depth ordering: the app guard (≤ 262144) fires before the DB CHECK, so the CHECK is
  never reached on the app path. If the two ever diverge, the app (tighter-or-equal) must win first.

**Effort**: ~3 h. **Reference class**: the shipped `title_required` path end-to-end (service → fragment → 422).
