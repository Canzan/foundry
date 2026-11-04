# Story map — integrations-wayfinding

Backbone (the admin's or lead's activities, left to right):

| Notice a project should post to chat | Get to the right place | Understand what's missing | Fix it, or ask the right person | Pick the project's channel |
|---|---|---|---|---|
| board → project Integrations (v0.18.1) | **US-IW-01** sidebar entry (admin) | **US-IW-03** project page connection line | **US-IW-02** route form: instance / key / workspace, with link for admin | existing route form (unchanged) |
| dashboard Quick actions (v0.18.1) | | **US-IW-02** per-kind wording | | |

No walking skeleton. This is an existing codebase and every story is a thin
change to a page that already exists.

## Release slices

One release (v0.18.2) holding all three stories. Each story is an independent
slice and can ship alone:

1. **US-IW-02**, the route form's wording and links. This is the dead end the
   user actually hit.
2. **US-IW-03**, the project page connection line. It prevents reaching that
   dead end, and answers "do I configure this per project?".
3. **US-IW-01**, the sidebar entry. It gives an admin a general way in from
   every page.

## Priority Rationale

- **US-IW-02 first.** It is the reported failure. It misleads the only user
  ("ask an admin" when they are the admin) and hides the real cause
  (instance not set up). This also breaks the journey's rule that every
  refusal names the next step.
- **US-IW-03 second.** It turns the shared-connection model from implicit into
  visible, which is the misunderstanding behind the user's request to
  "configure once, choose a channel per project". It shares the access facts
  that US-IW-02 needs (is the viewer an admin, and the state of each kind).
- **US-IW-01 last.** It has the widest reach (every page) but the lowest
  urgency: v0.18.1 already gives two ways in. It touches the shared sidebar,
  so its access check sits on every page render. DESIGN must confirm that
  check is cheap or already loaded with the navigation context.
