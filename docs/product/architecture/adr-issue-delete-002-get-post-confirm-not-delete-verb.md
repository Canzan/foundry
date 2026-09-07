# ADR-ISSUE-DELETE-002: The delete confirm is a GET+POST pair, not an HTTP DELETE, and its dialog has two carriers

## Status

Accepted (issue-card-delete DESIGN wave, 2026-09-05)

## Context

foundry has two shipped shapes for a destructive web action, and they disagree.

**The lane shape**: `get(show_delete_lane_dialog).post(submit_delete_lane)` — a
GET that renders a confirm dialog and a POST that performs the write, both under
`csrf_middleware`. **The comment shape**: `.delete(submit_delete_comment)` — an
HTTP `DELETE` verb driven by `hx-delete`, with the token in an `HX-CSRF` header
because the request body is empty (ADR-009).

The comment shape is unreachable without JavaScript: no HTML form can emit a
`DELETE`. That was acceptable for a control living inside a comment card on a
JS-rendered surface. It is not acceptable here, because
`persona-instance-operator` records an `env-hardened-profile` — "a browser
profile with scripting disabled and/or site data blocked, used when opening
foundry from a machine she does not own" — and notes that "foundry already has a
scripting-disabled acceptance lane exercising exactly this". The full issue page
is a plain server-rendered page; making its only destructive action require
JavaScript would regress that lane.

But the lane shape has an unexamined gap of its own: the lane dialog is reached
from `hx-get` buttons in the `⋯` menu, so it *also* has no no-JS path — its
GET route returns a bare `div.modal` fragment which, navigated to directly,
renders as an unstyled floating box with no page chrome.

## Decision

**The route pair is `GET`+`POST` at
`/team/{team}/project/{project}/issues/{n}/delete`**, registered beside the
shipped `issues/{n}/edit` pair and covered by the same layer-wide
`csrf_middleware`. No new HTTP verb, no `hx-delete`, no `HX-CSRF` header path.

**The dialog has two carriers over one body.** `partials/delete_issue_modal.html`
is the fragment htmx swaps into `#modal-root`;
`templates/delete_issue_modal_page.html` `{% extends "base.html" %}` and
`{% include %}`s that same partial for a direct navigation. `show_delete_form`
branches on `is_htmx` to choose. This is the pattern `new_issue_modal_page.html`
already established over `partials/new_issue_modal.html` — the house had solved
this and the lane dialog simply predates it.

**One handler per verb, not per surface.** `submit_delete` branches on `is_htmx`
for its *rendering* only: htmx → cleared `#modal-root` plus the out-of-band
`#board-columns` refresh; non-htmx → `303` to the board. Both branches call the
same `foundry_services::issues::delete_issue` use case, so the popup and the
full page cannot drift — there is nothing between them to drift.

The Delete control on the edit popup is rendered **outside** the `<form>`,
beside the `×`, inheriting the protection ADR-MODAL-CLOSE-001 D-12 established
for the close trigger: an element outside the form can never submit it. This
feature writes **no JavaScript**; every control is an attribute (`hx-get`,
`hx-post`, `data-action="close-modal"`) resolved by the existing delegated
listeners, so BR-4's "`Escape` has exactly one owner" holds by construction.

## Alternatives Considered

- **A. `hx-delete` on `DELETE …/issues/{n}`, mirroring `comment-edit-delete`.**
  Rejected. Unreachable without JavaScript, which regresses the
  scripting-disabled lane on foundry's most plainly server-rendered page. It
  would also need the `HX-CSRF` header path, and `fix-comment-delete-csrf`
  records that HTTP-lane token injection can mask a real browser 403 on exactly
  that path — a sharper edge to re-tread for no gain.
- **B. `GET`+`POST`, but return the bare fragment from the GET in all cases.**
  Rejected. A direct navigation renders an unstyled floating `div` with no page
  chrome. The acceptance criterion ("works with scripting disabled") would pass
  on a technicality while the operator saw something visibly broken.
- **C. Drop the no-JS promise and match the lane dialog's htmx-only reality.**
  Rejected. It revokes an AC and abandons the environment the persona
  explicitly records — and it is the whole reason `GET`+`POST` was chosen over
  the `DELETE` verb, so accepting it would reopen decision A.
- **D. Two handlers, one per surface.** Rejected. It puts the same use-case call
  in two places so that only the response rendering differs, which is the drift
  this design is shaped to prevent. `issues::submit_create` already branches on
  `is_htmx` inside one handler; this follows it.
- **E. `{% include %}` `delete_lane_modal.html` and parameterise it over both
  lanes and issues.** Rejected. The bodies genuinely differ — the lane dialog
  offers a two-fate choice with a destination `<select>`, the issue dialog
  offers one action — and a template parameterised over both would be more
  coupling than the ~20 lines it saved.

## Consequences

- **Positive.** The whole delete path works with scripting disabled: a plain
  link to a real page containing a plain `<form method="post">`. CSRF is covered
  by registration alone, with no per-route work and no header path. Two carriers
  over one partial means the fragment and the page cannot disagree. No
  JavaScript is added, so `keyboard.js` keeps its exactly-one-`keydown`,
  exactly-one-`click` budget and BR-4 is untouched.
- **Negative.** foundry now has *three* destructive-action shapes: comment
  (`DELETE` verb, htmx-only), lane (`GET`+`POST`, htmx-only in practice), and
  issue (`GET`+`POST`, both carriers). This ADR makes the third the intended
  target rather than a fourth accident, but converging the first two is not
  attempted here and remains unclaimed work.
- **Negative.** The lane dialog's missing no-JS carrier is now *visible* as a
  gap rather than merely unnoticed. Adding one is a three-line template change
  following DDD-6, deliberately left out of this feature's scope.
- The `303` on the non-htmx POST is a redirect to the board, never a re-render
  of the issue page: a page for a resource that no longer exists is the
  invisible-card failure in another guise.
