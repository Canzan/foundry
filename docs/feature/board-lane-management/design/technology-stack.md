# Technology Stack — board-lane-management

**Verdict: no new technology.** Every requirement is served by the shipped stack; adding
anything would violate the simplest-solution principle and the homelab operational envelope.

## Justification per requirement

| Requirement | Served by (existing) | Why nothing new |
|---|---|---|
| Per-project lane storage + referential invariant | PostgreSQL (existing store), composite FK | The no-stranded-card invariant is a relational-integrity problem; Postgres FKs solve it natively. No config store, no JSONB lane blob (a blob cannot be an FK referent — it would surrender the invariant). |
| Atomic two-fate delete | sqlx transactions (existing), `FOR UPDATE` + bounded retry | Single-node Postgres; no distributed coordination exists to need a saga/queue. |
| Dialog swap, fate form, error routing | htmx 2.0.4 + askama + `form-errors.js` (existing) | The fate dialog is the same `#modal-root` fragment pattern as the edit dialog; two submit buttons carrying `name=fate` is plain HTML forms. |
| Dialog close | `data-action="close-modal"` (ADR-modal-close-001) | Template-only; BR-4 forbids new listeners by construction. |
| Board refresh without reload | htmx OOB swap (existing house idiom) | Already used for the edit-dialog card replace. |
| dnd / keyboard nav over dynamic lanes | `board-dnd.js`, `keyboard.js` (existing) | Both already walk `[data-column]` generically; lane count is irrelevant to them. |
| Migration 0015 | sqlx migrator (existing, forward-only) | Same mechanism as 0001–0014. `gen_random_uuid()` is PG13+ built-in — no extension install. |
| Testing | HTTP acceptance lane + fantoccini `@needs-browser` + cargo-mutants ≥80% (existing) | The concurrency gold test (FK strand-guard) runs in the existing HTTP lane against real Postgres. |
| Architecture enforcement | `cargo xtask check-arch` + `deny.toml` (existing) | New rule (no static lane lists in adapters) extends the existing AST-walk idiom — no new tool. |

## Licenses

All existing: Rust/axum/sqlx/askama (MIT/Apache-2.0), htmx (BSD-2/0BSD), PostgreSQL (PostgreSQL
License), fantoccini (MIT/Apache-2.0). Nothing proprietary; nothing added.

## Explicitly rejected additions

- **Redis/cache for lane lists** — homelab scale; one indexed query per render is nothing.
- **A JS framework for the fate picker** — a `<select>` and two submit buttons.
- **pgcrypto extension** — not needed; `gen_random_uuid()` is core since PG13.
- **Soft-delete/tombstone library or column** — D7/D9 pin hard delete; the comments tombstone
  precedent is comment-scoped and does not extend here.
