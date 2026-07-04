# Slice 03 — Sign out

**Goal**: a signed-in user can sign out from the dashboard (CSRF-protected).

**Stories**: US-02 (+ tests).

**IN scope**
- `dashboard_root` mints a CSRF token via `ensure_csrf_cookie` and returns `(SET_COOKIE, Html)`
  (response-type change — D2).
- `DashboardRoot` gains `csrf: String`; template renders a `<form method="post" action="/sign-out">` with
  the hidden `_csrf` field + a Sign out button.

**OUT of scope**
- Any other POST affordance on the dashboard. "Sign out everywhere"/session listing.

**Learning hypothesis**: disproves "CSRF plumbing on `/` is a straight copy of `admin_tokens::show_index`"
if the `Html` → `(headers, Html)` change ripples beyond the handler (e.g. other callers of the view).
(Confidence medium-high: `ensure_csrf_cookie` already exists in `signin.rs:305`; the ripple is local.)

**Acceptance**: `acceptance-criteria.md` US-02 (sign out → redirect to `/sign-in`; forged `_csrf` refused).

**Seams**: `signin.rs:190 submit_signout`; route `/sign-out` (`lib.rs:291`); `signin.rs:305 ensure_csrf_cookie`;
`csrf.rs generate_token/build_csrf_cookie`; mirror `admin_tokens.rs:61 show_index`.

**Dependencies**: sequenced after slices 01–02 to isolate the response-type change. **Effort**: ~0.5–1 day.
