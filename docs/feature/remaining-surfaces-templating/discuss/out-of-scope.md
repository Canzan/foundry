# Remaining-Surfaces Templating — Out of Scope

> The deferred follow-up to Feature B (htmx-web-tier). This feature finishes the
> `format!()`→template MOVE for the surfaces Feature B's `out-of-scope.md`
> enumerated under "Deferred: remaining inline-`format!` surfaces". The framing is
> identical to Feature B: **this is a RENDERING-LAYER move inside one binary — NOT
> new behavior, NOT a new API, NOT a service split, NOT a frontend rewrite, NOT an
> auth change.** Everything Feature B put out of scope stays out of scope here.

## Hard non-goals (same shape as Feature B)

### NOT new behavior or a visual redesign
- Move-only. Templates reproduce the selector-and-substring-identical render
  contract. No new affordances, no copy redesign beyond literal-preserving moves,
  no layout rework. Visual improvement comes only from the surfaces now linking the
  EXISTING `/static` stylesheet via `base.html` — same as Feature B's surfaces got.

### NOT a new template engine, asset pipeline, or infrastructure
- Reuse Feature B's shipped Askama engine, `base.html`, `views.rs` view-model layer,
  and the already-vendored htmx2/Alpine/CSS in `static/`. NO new engine, NO new
  vendored asset, NO new dependency, NO new runtime service, NO CDN, NO Node
  toolchain. Still one `foundry` binary + Postgres.

### NOT an auth / CSRF / session change
- The browser auth path is unchanged: `_csrf` field, `/bootstrap` CSRF exemption,
  tower-sessions, redirect-when-signed-out, all status codes (200/400/401/403/413/
  SEE_OTHER). Only the MARKUP of these surfaces moves; handlers, contracts, and
  secrets are untouched (NFR-WEBB-COMPAT-03/04).

### NOT a re-templating of the surfaces Feature B already shipped
- The board (`render_board`), the sign-in form (`render_signin_form`), and the
  issue page + comment cards are **already templated by Feature B** (confirmed in
  code: `render_board` builds a view-model and calls Askama; `render_signin_form`
  returns `views::SigninPage`). They are OUT of this feature.

### NOT the htmx 1→2 normalization/upgrade
- That was Feature B's dedicated slice (US-B05, job htmx-web-3) and is done there.
  The remaining surfaces carry only the attachment OOB swap and the state-change
  fragment as active `hx-*`; they move AS-IS on the already-pinned htmx version.

### NOT mobile-responsive polish, theming, dark mode, or a design system
- Inherited from Feature B / backend-mvp: desktop-first; "looks intentional,
  accessible, consistent" via the shared stylesheet, not a token-based design
  system. Theming/dark mode/mobile remain post-extraction enhancements.

## Surfaces explicitly IN scope (the deferred list, now verified in code 2026-06)

These were enumerated as deferred in Feature B's `out-of-scope.md`; this feature
templatizes them. Verified still inline `format!()` with bare `<head>` on full pages:

| Surface | Location | Story |
|---------|----------|-------|
| Project-create form | `projects.rs::render_create_form` | US-R01 |
| Project-create error fragment | `projects.rs::render_error_fragment` | US-R01 |
| New-issue modal (fragment + full-page fallback) | `keyboard.rs::render_modal_fragment` / `render_modal_full_page` | US-R02 |
| Issue-create error fragment | `issues.rs::bad_request_fragment` | US-R03 |
| Issue state-change `<span>` fragment | `issues.rs` (state-change response) | US-R03 |
| Dashboard landing `/` | `signin.rs::dashboard_root` | US-R04 |
| Events sign-in-required page | `events.rs::unauthorized_response` | US-R04 |
| Attachment upload-error fragment | `attachments.rs::bad_request_fragment` | US-R05 |
| Attachment-row OOB swap | `attachments.rs::render_attachment_row_oob` | US-R05 |
| Upload-too-large (413) page | `attachments.rs::payload_too_large` | US-R05 |
| Attachment not-found page | `attachments.rs::not_found_page` | US-R05 |
| Bootstrap dashboard | `bootstrap.rs::dashboard` | US-R06 |
| Workspace-claim form | `bootstrap.rs::render_claim_form` | US-R06 |
| Invite-link page | `bootstrap.rs::create_invite` | US-R06 |
| Shared not-found/error page | `signin.rs::invalid_page` (shared helper) | US-R06 |

### Optional tail (tracked, not a blocking slice)
- `keyboard.rs::render_search_fragment` and `keyboard.rs::show_keyboard_help`
  overlay are also inline `format!()`. Same pattern, lowest risk/visibility. Fold
  into US-R02 if cheap, else a trivial follow-up. Listed for audit completeness so
  "did we get ALL the inline HTML?" has a clean answer.

## Re-evaluation Triggers

| Item | Trigger | Likely target |
|------|---------|---------------|
| keyboard search/help overlay | US-R02 lands cheaply | fold into US-R02 or a one-line follow-up |
| Mobile polish | ≥3 reports of unusable mobile | v0.4 (per backend-mvp) |
| Design system / theming | sustained demand | post-extraction |
| Any NEW behavior on these surfaces | a product request | a separate behavior feature, not this move |
