# Slice 01 — Project-create form + error fragment (Walking Skeleton)

- **Story**: US-R01
- **Job**: htmx-web-1 (edit markup in a template, not Rust)
- **Type**: move-only refactor
- **Effort**: ≤1 day
- **Learning hypothesis**: A full page + its error fragment can move to
  Askama/`base.html` selector-identical with the acceptance suite staying green —
  proving the move pattern on a real surface before repeating it.

## Surfaces

| Site | Kind | Today | Target |
|------|------|-------|--------|
| `projects.rs::render_create_form` | full page | inline `format!()`, bare `<!doctype><html><head><title>`, no `/static` | `project_create.html` extends `base.html`, view-model in `views.rs` |
| `projects.rs::render_error_fragment` | fragment | inline `<div class="error" data-hx-fragment="project-create-error">` | error-fragment template/partial; marker byte-stable |

## Why it is the walking skeleton
Cheapest surface that exercises EVERY mechanic the later slices reuse: extending
`base.html`, linking `/static`, emitting the `_csrf` hidden field, and keeping a
`data-hx-fragment` marker byte-stable. Green here = pattern proven.

## Done when
- [ ] Markup lives in `project_create.html` extending `base.html`; page links `/static`.
- [ ] Error fragment renders from a template; `data-hx-fragment="project-create-error"` byte-stable.
- [ ] `_csrf`, `method=post`, `action`, name/key inputs selector-identical.
- [ ] `cargo test -p foundry-acceptance` passing count does not drop; no scenario edited.
- [ ] No inline HTML `format!()` left in `render_create_form`/`render_error_fragment`.

## Notes
- Fragment stays a bare fragment; only the full form page extends `base.html` (DR5).
- Reuses Feature B's shipped Askama + `base.html` + `views.rs`; DESIGN near-trivial.
