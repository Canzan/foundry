# RCA — comment delete 403s in a real browser (missing CSRF token)

**Origin**: `/nw-bugfix` follow-up from `form-error-display-contract`, which found and fixed the same defect
class on comment-*edit*. This audit checked comment-*delete* (and swept all mutating htmx triggers).

## Defect

The comment **Delete** control (`crates/foundry-app/templates/partials/comment_card.html:4`) is a bare
`hx-delete` with **no CSRF token**:

```html
<button class="comment-delete-button" hx-delete="{{ card.delete_url }}"
        hx-target="#comment-{{ card.id }}" hx-swap="outerHTML">Delete</button>
```

`csrf_middleware` requires a token for state-changing methods — `is_safe_method` (`csrf.rs:125`) allows only
`GET/HEAD/OPTIONS`, and the module doc (`csrf.rs:5`) states *"On POST/PUT/PATCH/DELETE, this middleware
compares the cookie."* A DELETE carries no urlencoded body, so it cannot carry `_csrf` the way a form POST/PATCH
does, and this button also has no `hx-headers` echo and there is no global htmx CSRF injection. So in a real
browser the DELETE reaches `csrf_middleware` with **no supplied token → 403 Forbidden**, before the handler
runs. **Comment deletion is broken for real users.**

## Why it shipped (masked, RCA Root Cause B pattern)

The HTTP-lane comment-delete tests (`us_10_comment_edit_delete`) inject the CSRF token manually (reqwest sets
the header/field), so they pass. Only a real browser exercises the gap — and there was no `@needs-browser`
scenario for comment delete. Same "HTTP-body/HTTP-lane blindness" that hid the comment-edit CSRF gap.

## Sweep (audit completeness)

All mutating htmx triggers in `crates/foundry-app/templates/` checked:
- `comment_edit_form.html` — hx-patch **now carries** body `_csrf` (fixed by form-error-display-contract 80754a8). OK.
- `comment_card.html` Edit button — `hx-get` (safe method, no CSRF needed). OK.
- `comment_card.html` Delete button — `hx-delete`, **no token → the defect**.
- All other write forms carry `<input name="_csrf">`. OK.

Comment delete is the only remaining gap.

## Fix (the correct idiom for a body-less DELETE)

Add to the Delete button:

```
hx-headers='js:{"x-csrf-token": (document.cookie.match(/(?:^|; )foundry_csrf=([^;]+)/) || [])[1]}'
```

the cookie→header double-submit `board-dnd.js`/`csrf-upload.js` already use. `csrf_middleware` accepts the
`x-csrf-token` header (`csrf.rs:181-186`). The header is the right carrier here precisely because a DELETE has
no body to hold `_csrf` (unlike the comment-edit PATCH, which was fixed with a body field). The `foundry_csrf`
cookie is non-HttpOnly by design for exactly this.

## Regression (the durable part)

A `@needs-browser` scenario: seed a comment, delete it in a real browser, assert the comment card is **removed
from the DOM**. Red today (403 → card stays), green after the fix. The DOM-level oracle the HTTP lane
structurally cannot provide — so this defect class cannot silently regress again.

## Risk

Client-side, template-only + one test. No server change, no route/migration. Scoped to the delete button.
The `card.can_delete` gate is unchanged (only the author sees the button). No CSRF surface weakened — it ADDS
a token to a request that had none.
