# Slice 2 — "Issue & comments read like a product, from one card partial"

> Feature B (htmx-web-tier). Depends on Slice 1 (template pipeline + base layout + card-partial pattern).

## Stories
- **US-B03** — Move the issue detail and comment thread to templates (`job_id: htmx-web-1`)

## Learning hypothesis
The four comment-render `format!` sites in `comments.rs` (`render_issue_page`,
`render_comment_card`, `render_comment_card_oob`, the inline edit-form) can collapse into ONE
comment-card partial without diverging the live-updated card from the reloaded card — while
sanitization and authz stay in core/handler.

## End-to-end demonstrable value
Mei opens AUTH-3, reads the thread, posts/edits/deletes comments — all unchanged in feel — and
a live-appended comment card is now structurally identical to a reloaded one (fixing today's
`render_comment_card_oob` quirk that omits Edit/Delete). A contributor can restyle the thread
by editing one partial.

## IN scope
- Issue-page template + ONE comment-card partial used by: full-page render, POST-comment OOB
  append, PATCH-edit re-render, GET single-card (cancel).
- Edit-form fragment as a template.
- Affordance flags (`can_edit` = author; `can_delete` = author or admin) computed in the
  handler/core and passed to the partial as booleans.
- The OOB (live) card now uses the same partial -> shows the same affordances as a reloaded card.
- 400/403/410 error fragments keep exact copy.

## OUT of scope (this slice)
- Board (done, Slice 1), sign-in (Slice 3), htmx bump (Slice 4).
- Moving sanitization or authz into the template (they STAY in core/handler).
- Attachments-section restyle beyond preserving the `.attachments-empty` contract marker.

## Key constraints (carried)
- Sanitization stays in `foundry_core::render_comment_markdown`; template never sanitizes
  (NFR-WEBB-BND-03).
- One partial per repeated component (NFR-WEBB-MAINT-02).
- Render contract preserved: `data-comment-list`, `data-comment-id`, `data-author`,
  comment error copy ("You may only edit your own comments.", the 410 gone copy), `(edited)`
  marker (NFR-WEBB-COMPAT-02).
- CSRF contract unchanged: `_csrf` field for PATCH, `HX-CSRF` header for DELETE
  (NFR-WEBB-COMPAT-03).
- Existing comment/issue acceptance scenarios stay green (NFR-WEBB-COMPAT-01).
- Existing htmx directives (`hx-patch`/`hx-get`/`hx-target`/`hx-swap`/`hx-delete`,
  `hx-swap-oob`) MOVE into the partial as-is; no version bump here.

## Demo script
1. Open AUTH-3; show the page + both comment cards render from templates.
2. As Hiroshi (viewing), have Mei post a comment; show the appended card is identical to a
   reloaded one (same affordances) — the divergence is gone.
3. As Mei, edit her comment; "(edited)" marker appears.
4. As Devansh (admin), show Delete-but-not-Edit on Mei's comment; as Hiroshi, neither.
5. Non-author edit -> 403 with the unchanged message; edit a soft-deleted comment -> 410.
6. Post a `javascript:` link + script tag; show core strips them before the template renders.
7. Suite green.

## Definition of Done (slice)
- US-B03 ACs all green; live-vs-reloaded structural-equality scenario green.
- Comment-render `format!` sites reduced from ≥3 to 1 partial; 0 authz/sanitization in template.
- Acceptance suite passing count unchanged.

## Size / sequence
~3 days, 6 scenarios. P2.
