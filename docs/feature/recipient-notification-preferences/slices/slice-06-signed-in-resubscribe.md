# Slice 06 — Signed-in resubscribe (undo a mute)

**Goal**: let an account holder resubscribe a previously-muted workspace from the settings page → Maria can
turn Northwind's notifications back on herself, any time, without hunting for an old link.
**Story**: US-06.

**IN scope**
- A **CSRF-protected** `POST /account/notifications/resubscribe` under the shipped CSRF middleware
  (`crates/foundry-app/src/csrf.rs:137`, layer `lib.rs:536-539`), rendered from the US-05 page.
- Scoped to the **session** member's own `(email_lower, workspace_id)` (NFR-6): clears the unsubscribe row via a
  `Store.delete_unsubscribe` method (pattern-sibling of `insert_unsubscribe`, slice 01) and re-renders the
  workspace as **Subscribed** (returning to the default, BR-7).
- Idempotent (BR-8): resubscribing an already-subscribed workspace is a no-op success.
- Acceptance: resubscribe restores delivery for the pair; already-subscribed → no-op; a forged cross-site POST
  without a valid `_csrf` → `403`, no state change; a member can only resubscribe their own pairs.

**OUT of scope**: **account-less** resubscribe (recipients who can't sign in — DESIGN ODD-6, a token-based
resubscribe / undo-on-confirmation-page); the status view itself (US-05); the operator metric (US-07).

**Learning hypothesis**: disproves "clearing the `0014` row via a CSRF-protected, session-scoped POST cleanly
reverses a mute — the same single-source state drives suppression, status, and resubscribe with no divergence"
if a resubscribe can be forged cross-site, if it can touch a pair outside the member's scope, or if clearing the
row fails to resume delivery.

**Seams**: CSRF middleware (`crates/foundry-app/src/csrf.rs:137`, `ensure_csrf_cookie` `:54`, layer
`lib.rs:536-539`); session identity (`session.rs`); the US-05 page + `delete_unsubscribe` store method
(sibling of `insert_unsubscribe`, slice 01); the suppression filter (slice 01) that now delivers again once the
row is cleared.
**Dependencies**: slice 05 (US-05, the page it acts on) + slice 01 (US-01, the state it clears). DESIGN ODD-6
(account-less resubscribe). No new migration.
**Effort**: ~0.5–1 day (one CSRF-protected POST + re-render; idempotent, least-privilege).
</content>
