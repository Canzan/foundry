# Slice 4 — "Issue & comments read like a product"

> DRIVER-CORRECTED (2026-05-30). Was Slice 2 under the old order; now Slice 4 in the web-tier
> track. Extracts the most tangled surface — the issue page and comment thread — into templates,
> collapsing the multiple `format!` comment-render sites into one partial.

## Stories
- **US-W03** — Move the issue detail and comment thread to templates.

## Learning Hypothesis
*The four comment-render paths today (`render_comment_card`, `render_comment_card_oob`, the
inline edit-form fragment, and the full-page render) can collapse into ONE comment-card partial
without the live (htmx-appended) card diverging from the reloaded card — while keeping
authorization affordances and markdown sanitization in core.*

## End-to-End Demonstrable Value
Mei opens AUTH-3, reads a styled comment thread, posts a markdown comment (Hiroshi sees the
appended card match a reloaded one), edits her own comment (the "(edited)" marker appears),
and a non-author's edit attempt is refused — all rendered from one comment-card partial, with
Edit/Delete affordances decided in core and markdown still sanitized by core.

## IN scope
- Issue-page template + a single **comment-card partial** used by: full-page render,
  htmx post-append, inline edit re-render, and cancel single-card render.
- Inline edit-form fragment moved to a template.
- Affordance flags (can_edit = author; can_delete = author||admin) computed in core/store and
  passed to the partial as booleans; the partial renders, never decides.
- Preserve exact error copy: 400 ("Comment cannot be empty" / "too long"), 403 ("You may only
  edit your own comments."), 410 ("This comment has been deleted. Refresh to see the latest
  state."), and the `data-comment-list` / `data-hx-fragment` markers.
- Resolve the OOB-card-omits-buttons quirk so the live card and reloaded card match.

## OUT of scope (this slice)
- Attachments-section extraction beyond what the issue page already renders (follow-on).
- Board (Slice 3), sign-in (Slice 5), JSON API track (Slices 1-2).
- Threaded comments, comment realtime changes (unchanged from backend-mvp).

## Boundary invariants asserted by this slice
- Markdown sanitization stays in `foundry_core::render_comment_markdown`; web tier never
  sanitizes (NFR-WEB-BND-03). (Same sanitization the JSON comment-write path reuses, US-W05c.)
- Authorization (`is_workspace_admin`, authorship) stays in core/store; 0 authz call sites
  under the web tier (NFR-WEB-BND-03).
- One comment-card partial; live == reloaded (NFR-WEB-MAINT-02).

## Shared artifacts (see journey.md Journey 2)
`$COMMENT_CARD_MARKUP` (one partial, four call sites), `$SANITIZED_HTML` (core/ammonia — shared
with the JSON comment-write path), `$AUTHZ_AFFORDANCES` (core-decided flags),
`$CSRF_TOKEN` (`_csrf` for PATCH, `HX-CSRF` for DELETE — unchanged browser contract).

## Acceptance anchors
- `Issue page and comment thread render from templates` (US-W03)
- `A live-posted comment card matches a reloaded one` (US-W03)
- `Edit and delete affordances are gated in core, rendered in the template` (US-W03)
- `Non-author edit is refused with the unchanged message` (US-W03)
- `Markdown sanitization remains in core` (US-W03)

## Definition of Done (slice)
- All US-W03 ACs met; UAT scenarios green.
- Existing comment/issue acceptance scenarios green (NFR-WEB-COMPAT-01/02).
- Comment-render `format!` sites reduced from ≥3 to 1 partial; 0 authz logic in web tier.
- Demoable: post/edit/delete a comment; live card matches reload.

## Estimate
~3 days (US-W03 M), one developer.

## Dependencies
Slice 3 (US-W01) — the web seam, base layout, and card-partial pattern must exist.
