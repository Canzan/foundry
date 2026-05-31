# Slice 5 — "First impression: sign-in looks trustworthy"

> DRIVER-CORRECTED (2026-05-30). Was Slice 3 under the old order; now Slice 5 — the last slice in
> the web-tier track. Moves the full-page auth screens to templates extending the shared base
> layout. Lowest fragment-extraction risk (no htmx fragment swaps), so it follows the
> fragment-heavy surfaces.

## Stories
- **US-W04** — Move sign-in and forgot-password to templates.

## Learning Hypothesis
*Full-page surfaces (sign-in, forgot-password) can share ONE base layout with the fragment
surfaces (board, issue), so CSS/asset consistency is automatic (Nielsen #4) with no duplicated
head/asset boilerplate — while the session-cookie and non-enumerable-error contracts survive
the move untouched.*

## End-to-End Demonstrable Value
Mei (returning, or a first-time evaluator) visits `/sign-in`, sees a styled full-page form
(labels above inputs, clear primary button, "Forgot your password?" link) rendered from the
same base layout as the board, signs in, and lands on the dashboard with the same 30-day session
cookie as before. A wrong password shows the unchanged non-enumerable error in the styled form.

## IN scope
- Sign-in and forgot-password templates extending the shared base layout (head, vendored assets,
  header) established in Slice 3.
- Styled, accessible form: labels above inputs, one-column, visible focus, ≥4.5:1 contrast.
- Preserve unchanged: session-cookie attributes (HttpOnly, Secure, SameSite=Lax, 30-day);
  the "Invalid email or password" non-enumerable copy; the CSRF contract (cookie set on GET,
  hidden `_csrf` field, 403 on missing/invalid).

## OUT of scope (this slice)
- Bootstrap/admin-claim page and dashboard-root render extraction (follow-on; same pattern).
- Board (Slice 3), issue/comments (Slice 4), JSON API track (Slices 1-2).
- Any change to password auth, brute-force delay, or session storage.
- The machine-token auth surface (that is the API track, US-W05b — a different credential path).

## Boundary invariants asserted by this slice
- Auth screens render from templates via foundry-web; no DB access from the web tier
  (NFR-WEB-BND-01); the POST handlers' auth logic stays in core/auth (NFR-WEB-BND-04).
- One shared base layout across all full pages (NFR-WEB-MAINT-01).

## Shared artifacts (see journey.md Journey 3)
`$CSRF_TOKEN` (unchanged), `$SESSION_COOKIE` (unchanged attrs), `$GENERIC_SIGNIN_ERROR`
("Invalid email or password" — same for both failure cases), `$LAYOUT_TEMPLATE` (shared base).

## Acceptance anchors
- `Sign-in renders from the shared layout and signs the user in` (US-W04)
- `Invalid credentials show the unchanged non-enumerable error in the styled form` (US-W04)
- `Forgot-password page renders from the shared layout` (US-W04)
- `CSRF token contract is preserved on the templated form` (US-W04)

## Definition of Done (slice)
- All US-W04 ACs met; UAT scenarios green.
- Existing sign-in/forgot acceptance scenarios green (NFR-WEB-COMPAT-01/03/04/05).
- Auth screens extend the one base layout; 0 duplicated head/asset boilerplate.
- Demoable: styled sign-in + same cookie; wrong-password non-enumerable error.

## Estimate
~2 days (US-W04 S-M), one developer.

## Dependencies
Slice 3 (US-W01) — base layout + static pipeline must exist.
