# Technology Stack — instance-admin-project-rename

## Verdict: no new technology

Every capability this feature needs is already shipped, licensed, and exercised
in CI. Adding anything would violate simplest-solution-first for a
2.5-day, three-slice delta to an existing surface.

| Concern | Existing choice (reused) | License | Why sufficient |
|---|---|---|---|
| HTTP routing/handlers | axum (workspace pin) | MIT | One new route in the existing HTML mount |
| Templates | askama | MIT/Apache-2.0 | One new row partial + one page extension |
| Interactivity | htmx 2.0.4 (vendored) | 0BSD | Row swap (`hx-target`/`hx-swap="outerHTML"`) is core htmx |
| 4xx error display | `static/js/form-errors.js` (app-owned) | project license | Per-row `[data-error-slot]` resolution already works via `closest('form')` — zero changes |
| Persistence | sqlx + PostgreSQL | MIT/Apache-2.0 + PostgreSQL License | Three reads + one UPDATE on existing tables; no migration |
| Sessions/CSRF | tower-sessions + double-submit `_csrf` middleware | MIT | Rename form mounts under the shipped middleware stack |
| AuthZ | `instance_admins` table + `require_instance_admin` / `is_instance_admin` | project | Reused verbatim, twice (gate + use-case) |
| Testing | HTTP acceptance lane + fantoccini `@needs-browser` lane; cargo-mutants gate ≥80% | MIT/Apache-2.0 | Both lanes already run in CI; slice-03 explicitly needs the browser lane |
| Arch enforcement | `cargo xtask check-arch` + `deny.toml` | project | Extended with one new AST rule (no `fn slugify(` in foundry-app), no new tool |

Alternatives considered and rejected:

- **A JS framework or custom fetch wrapper for inline editing** — rejected: htmx
  + the shipped form-errors contract already implement exactly this interaction
  (issue edit dialog precedent); anything else forks the error-display contract
  the repo built after an RCA.
- **A DB constraint/trigger for D4 uniqueness** — rejected in
  `data-models.md` §2 (rule not expressible in SQL without moving `slugify` out
  of domain code; pre-existing-row hazard).
- **A dedicated admin API endpoint under `/api/v1`** — rejected: that mount is
  CSRF-exempt by design for machine tokens; a browser-facing mutating form there
  would bypass the exact rail D5 mandates.

All licenses are permissive OSS already vetted by the workspace's `deny.toml`;
no proprietary component is introduced or needed.
