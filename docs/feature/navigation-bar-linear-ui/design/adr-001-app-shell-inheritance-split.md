# ADR-001: App-shell template inheritance split for shared chrome

## Status
Accepted

## Context
The shared Linear-style sidebar must appear on ~12 authenticated app pages and must be **absent** on
10 pre-auth/utility pages (FR-2, NFR-3, AC-02). Today all 21 page templates extend a bare
`base.html` (`<head>` + `<body>{% block content %}`), and the engine is **Askama 0.12** (ADR-B01),
which compiles templates into the binary and type-checks template variables against the page's
`#[derive(Template)]` struct at build time.

The core question: **how is "sidebar here / no sidebar there" decided, so that it cannot be
forgotten and cannot leak the rail onto a pre-auth page?**

## Decision
Introduce an **intermediate layout** `templates/app_shell.html`:

```
{% extends "base.html" %}
{% block content %}
<div class="app-shell">
  {% include "partials/sidebar.html" %}
  <div class="app-shell__content">{% block app_content %}{% endblock %}</div>
</div>
{% endblock %}
```

Authed app pages change `{% extends "base.html" %}` → `{% extends "app_shell.html" %}` and rename
their body block `{% block content %}` → `{% block app_content %}`. Pre-auth/utility pages are left
**untouched** on `{% extends "base.html" %}`. Chrome membership is therefore **structural** — a
property of which parent a template names — not a runtime boolean evaluated per request.

Askama supports the required multi-level inheritance (base → shell → page) with a block nested
inside another block, and `{% include %}` renders against the page struct's context, so
`sidebar.html` reads the `nav` field every authed page struct embeds (see ADR-002).

## Alternatives considered

1. **Per-page runtime boolean (`show_nav` / typed marker) inside `base.html`.**
   `base.html` would wrap the sidebar in `{% if show_nav %}` and every page struct would carry the
   flag. *Rejected:* the exclusion invariant becomes a value a developer can set wrong; a new
   pre-auth page that forgets `show_nav = false` (or defaults it true) leaks the rail — exactly the
   NFR-3 regression we must prevent. It also forces the flag onto all 21 structs, including pre-auth
   ones we otherwise never touch. Weakest safety.

2. **`{% include "partials/sidebar.html" %}` guarded by a marker, added into `base.html`.**
   Same failure mode as (1) at the template layer, plus it couples `base.html` (the pre-auth parent)
   to the sidebar partial and its `nav.*` context — meaning pre-auth structs would need a `nav` field
   or Askama would fail to compile. *Rejected as primary*, but retained as the **documented fallback**
   if a specific Askama 0.12 nested-block limitation surfaces at implementation time: in that case,
   keep `base.html` as-is and have each authed page `{% include "partials/sidebar.html" %}` at the
   top of its own `{% block content %}`. This still works because the `nav` field lives on the authed
   page struct only — but it re-introduces per-page include boilerplate the shell avoids.

3. **Copy the sidebar markup into each authed template.** *Rejected outright:* violates the feature's
   entire purpose (consolidation) and the maintainability driver; 12 divergent copies.

## Consequences
- **Positive:** exclusion is unforgeable — a pre-auth page cannot show the rail without someone
  re-parenting it to `app_shell.html` on purpose. One definition of the chrome. Askama makes a page
  that extends the shell but omits the `nav` field a **build error** (strong Earned-Trust posture).
- **Positive:** pre-auth output is guaranteed unchanged (NFR-3) because those templates are not edited.
- **Negative:** a mechanical migration of ~12 templates (re-parent + rename one block) plus one field
  per page struct. Low risk, high fan-out — the feature's main labor.
- **Negative / to verify:** relies on Askama 0.12 resolving a block nested inside an overridden block
  across two inheritance levels. If that specific behavior is limited in the pinned version, fall back
  to alternative (2) (per-page include) — a mechanism change only; the render contract and CSS are
  identical. The crafter confirms with a one-template spike before mass migration.
</content>
