# Journey Note — Remaining-Surfaces Templating

> LIGHTWEIGHT by design. These are the SAME screens Feature B already studied —
> no new user journeys, no new emotional arc. This note records only the one
> thing that changes per surface: the markup moves from an inline `format!()`
> with a bare `<head>` into an Askama template extending the EXISTING `base.html`
> and linking the EXISTING `/static` assets. Behavior, routes, auth, and the
> render contract are unchanged. The acceptance suite is the regression net.

## The single journey these surfaces join

Feature B established the experience: every full page extends `base.html`, which
links the vendored `/static` stylesheet + htmx/Alpine, so the screen looks
intentional and works offline. Fragments (htmx swaps, OOB rows, error divs) stay
bare fragments — they are swapped INTO an already-styled page, so they must NOT
re-wrap in `base.html`. This feature simply brings the remaining surfaces into
that same experience.

Two personas, unchanged from Feature B:
- **Jamal (contributor)** wants to edit any surface's markup in `templates/`, not
  in handler `format!()` literals (job `htmx-web-1`).
- **Mei (self-hoster member)** wants every screen she lands on — not just the
  board — to look styled and load from the binary's own assets (job `htmx-web-2`).

## Per-surface: before → after

Each row is a move-only transform. "Before" = today's inline `format!()`.
"After" = an Askama template/partial reusing Feature B's base layout + contract.

| # | Surface (handler site) | Kind | Before | After |
|---|------------------------|------|--------|-------|
| US-R01 | `projects.rs::render_create_form` | full page | bare `<!doctype><html><head><title>` form, no assets | `project_create.html` extends `base.html`, links `/static`; same `_csrf`, same `method=post action`, same labels |
| US-R01 | `projects.rs::render_error_fragment` | fragment | inline `<div class="error" data-hx-fragment="project-create-error">` | `partials/error_fragment.html` (or view-model), `data-hx-fragment` byte-stable |
| US-R02 | `keyboard.rs::render_modal_fragment` | fragment | inline `<div class="modal" data-modal="new-issue">…` | `partials/new_issue_modal.html`; `data-modal`, role/aria, `_csrf`, `action` preserved |
| US-R02 | `keyboard.rs::render_modal_full_page` | full page | bare `<head>` wrapper around the modal | full page extends `base.html`, includes the SAME modal partial (one-partial rule) |
| US-R03 | `issues.rs::bad_request_fragment` | fragment | inline `<div class="error" data-hx-fragment="issue-create-error">` | error-fragment template; `data-hx-fragment` + "Title is required" copy byte-stable |
| US-R03 | `issues.rs` state-change `<span>` | fragment | inline `<span class="state" data-state="{s}">` | tiny state-fragment template; `data-state` byte-stable |
| US-R04 | `signin.rs::dashboard_root` (`GET /`) | full page | bare `<head>` "Foundry / signed in" | `dashboard_root.html` extends `base.html`; redirect-when-signed-out unchanged |
| US-R04 | `events.rs::unauthorized_response` | full page | bare `<!doctype>` "Sign-in required to subscribe to events" | template extends `base.html`; same copy + `/sign-in` link; 401 status unchanged |
| US-R05 | `attachments.rs::bad_request_fragment` | fragment | inline `data-hx-fragment="attachment-upload-error"` | error-fragment template; marker byte-stable |
| US-R05 | `attachments.rs::render_attachment_row_oob` | fragment | inline `<div hx-swap-oob="beforeend:[data-attachment-list]">…<li class="attachment">` | `partials/attachment_row.html` + OOB wrapper; `hx-swap-oob` target + `.attachment`/`data-filename` byte-stable |
| US-R05 | `attachments.rs::payload_too_large` | full page | bare `<head>` "Upload too large" | template extends `base.html`; 413 status + copy unchanged |
| US-R06 | `bootstrap.rs::dashboard` | full page | bare `<head>` "Workspace dashboard" | template extends `base.html` |
| US-R06 | `bootstrap.rs::render_claim_form` | full page | bare `<head>` claim form | template extends `base.html`; `_csrf`/action/`/bootstrap` exemption unchanged |
| US-R06 | `bootstrap.rs::create_invite` invite-link page | full page | bare `<head>` "Invite link" + URL | template extends `base.html`; signed invite URL unchanged |
| US-R06 | `signin.rs::invalid_page` (shared not-found/error helper) | full page | bare `<head>` `<h1>{heading}</h1><p>{msg}</p>` | shared `invalid_page.html` extends `base.html`; used by not-found/error helpers across handlers |

### Surfaces deliberately NOT touched (already done / out of scope)
- `render_board` (projects.rs), `render_signin_form` (signin.rs), the issue page +
  comment cards — **already templated by Feature B**. Confirmed in code: `render_board`
  builds a view-model and calls Askama; `render_signin_form` returns `views::SigninPage`.
- `keyboard.rs::render_search_fragment` and `show_keyboard_help` overlay are inline
  `format!()` too; they are grouped as OPTIONAL tail items (see `story-map.md`) — same
  pattern, even lower risk, can fold into US-R02 if cheap or defer.

## Emotional note (brief, no arc diagram needed)

Nothing to design here: the contributor's confidence and the self-hoster's
first-impression trust were already designed in Feature B. The only emotional
delta is *removing a jarring inconsistency* — the moment a self-hoster clicks
from the styled board to an unstyled bootstrap/landing/error page. Finishing the
move makes the whole surface coherent. That is the entire emotional payoff.
