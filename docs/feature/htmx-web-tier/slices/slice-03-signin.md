# Slice 3 — "First impression: sign-in looks trustworthy"

> Feature B (htmx-web-tier). Depends on Slice 1 (base layout + static pipeline).

## Stories
- **US-B04** — Move sign-in and forgot-password to templates (`job_id: htmx-web-2`)

## Learning hypothesis
Full-page templates share one base layout with the fragment surfaces, so CSS/asset
consistency is automatic (Nielsen #4) without duplication — and the security-critical sign-in
contracts (cookie attrs, non-enumerable error, CSRF) survive the move untouched.

## End-to-end demonstrable value
Mei (returning) and a first-time evaluator land on a styled, full-page `/sign-in` rendered from
the shared base layout — labels above inputs, clear primary button, "Forgot your password?"
link — that posts to the same endpoint, sets the same 30-day cookie, and shows the same
non-enumerable error. The auth screens now look as trustworthy as the board.

## IN scope
- Sign-in template + forgot-password template, both extending the shared base layout.
- Styled form (centered card, labels above inputs, one-column layout).
- Inline error rendering in the styled form.
- (Optional) `dashboard_root` landing extends the base layout too, for consistency.

## OUT of scope (this slice)
- Board (Slice 1), issue/comments (Slice 2), htmx bump (Slice 4).
- ANY change to the sign-in/forgot/sign-out HANDLERS, CSRF logic, session logic, password
  verification, brute-force delay, or the `GENERIC_SIGNIN_ERROR` constant — markup only.
- Password reset page (`/reset-password`) restyle — follow-on (not one of the three named surfaces).

## Key constraints (carried)
- Session cookie attrs unchanged: HttpOnly, Secure, SameSite=Lax, 30-day (NFR-WEBB-COMPAT-04).
- Non-enumerable "Invalid email or password" for both unknown-email and wrong-password
  (NFR-WEBB-COMPAT-05); brute-force delay (`BRUTE_FORCE_*`) unchanged, server-side.
- CSRF contract unchanged: cookie set on GET via `ensure_csrf_cookie`, hidden `_csrf` field,
  403 on missing/invalid (NFR-WEBB-COMPAT-03).
- Full pages extend ONE base layout; 0 duplicated `<head>`/asset boilerplate
  (NFR-WEBB-MAINT-01).
- WCAG 2.2 AA: labels associated with inputs, contrast, focus (NFR-WEBB-A11Y-02).
- Existing sign-in/forgot acceptance scenarios stay green (NFR-WEBB-COMPAT-01).

## Demo script
1. Open `/sign-in` on a fresh browser; show the styled card from the base layout; CSRF cookie
   set + matching hidden `_csrf` field.
2. Submit valid creds; land on dashboard; inspect Set-Cookie (HttpOnly/Secure/SameSite=Lax/30d).
3. Submit an unregistered email and a registered-email-wrong-password; both show
   "Invalid email or password" inline in the styled form.
4. Open `/forgot-password`; show it renders from the base layout; submit -> "if on file" copy.
5. POST without a valid CSRF token -> 403.
6. Suite green.

## Definition of Done (slice)
- US-B04 ACs all green.
- Auth templates extend the base layout; 0 duplicated `<head>`.
- Cookie/CSRF/error contracts inspected unchanged; acceptance suite passing count unchanged.

## Size / sequence
~2 days, 4 scenarios. P3 (lowest extraction risk — full-page, no fragment swaps).
