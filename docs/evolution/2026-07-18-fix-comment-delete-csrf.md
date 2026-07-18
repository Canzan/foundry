# Evolution — fix-comment-delete-csrf (the delete button had the same CSRF gap comment-edit did)

**Finalized**: 2026-07-18
**Commit**: DELIVER `59c10be` (1 DES-monitored step). Escalated from `/nw:continue` → `/nw-bugfix` as a
follow-up the `form-error-display-contract` archive had flagged. Trunk-based; DES integrity exit 0. **Not
pushed.**

## Defect (confirmed real, user-facing)

The comment **Delete** button (`partials/comment_card.html:4`) was a bare `hx-delete` with **no CSRF token**.
`csrf_middleware` requires a token for DELETE (`is_safe_method` allows only GET/HEAD/OPTIONS; the module doc
says POST/PUT/PATCH/DELETE are compared). A DELETE has no body to carry `_csrf`, and the button had no
`hx-headers` echo — so in a real browser the delete **403'd before the handler ran**. Comment deletion was
broken for real users. It shipped because the HTTP-lane `us_10` delete tests inject the token manually; no
`@needs-browser` scenario exercised it. Same latent-defect class the browser lane surfaced for comment-*edit*
in `form-error-display-contract`, and the exact follow-up that feature's archive said to audit.

## Fix (the correct idiom for a body-less DELETE)

The Delete button gained
`hx-headers='js:{"x-csrf-token": <read foundry_csrf cookie>}'` — the cookie→header double-submit
`board-dnd.js`/`csrf-upload.js` use; `csrf_middleware` accepts the `x-csrf-token` header. The header is the
right carrier here because a DELETE has no body (unlike the comment-edit PATCH, which was fixed with a body
`_csrf` field). Byte-additive; the Edit button (`hx-get`, safe) and the `can_delete` gate are untouched. ZERO
server change, no route/endpoint/migration (latest stays `0014`).

## The regression is the durable part

A `@needs-browser` scenario deletes a seeded comment in a real browser and asserts the card is **removed from
the DOM** — and the driven-store oracle confirms the soft-delete tombstone (`deleted_at IS NOT NULL`), so it's
a real delete, not a cosmetic swap. RED today (403 → card stays), green after the fix. The DOM-level oracle the
HTTP lane structurally cannot provide, so this defect class can't silently regress again. Shipped HTTP-lane
comment-delete tests (`comment-edit-delete`, 10/10) still green — the server is unchanged.

## Audit completeness

All mutating htmx triggers in `templates/` were swept: comment-edit (`hx-patch`) carries a body `_csrf`
(fixed by `form-error-display-contract` `80754a8`); the comment Edit button is `hx-get` (safe); every other
write form ships `<input name="_csrf">`. Comment delete was the only remaining gap. **No mutating trigger in
the app now lacks CSRF.**
