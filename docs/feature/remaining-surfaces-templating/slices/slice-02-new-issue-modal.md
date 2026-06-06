# Slice 02 — New-issue modal (fragment + full-page fallback)

- **Story**: US-R02
- **Job**: htmx-web-1
- **Type**: move-only refactor
- **Effort**: ≤1 day
- **Learning hypothesis**: The fragment-vs-full-page split (fragment stays bare,
  full page extends `base.html`, both share ONE modal partial) holds for an
  htmx-swapped surface without breaking the swap.

## Surfaces

| Site | Kind | Today | Target |
|------|------|-------|--------|
| `keyboard.rs::render_modal_fragment` | fragment | inline `<div class="modal" data-modal="new-issue" role="dialog">` | `partials/new_issue_modal.html` (bare partial) |
| `keyboard.rs::render_modal_full_page` | full page | bare `<head>` wrapper around the modal | full page extends `base.html`, `{% include %}`s the SAME partial |

## Done when
- [ ] Modal markup in ONE `partials/new_issue_modal.html`; both paths include it.
- [ ] Fragment path emits a bare fragment (no `base.html`); full-page path extends `base.html`.
- [ ] `data-modal="new-issue"`, `role="dialog"`, `aria-modal`, `_csrf`, `action`, `autofocus` title input selector-identical.
- [ ] `cargo test -p foundry-acceptance` passing count does not drop; no scenario edited.
- [ ] No inline HTML `format!()` left in `render_modal_fragment`/`render_modal_full_page`.

## Notes
- One-partial rule (NFR-WEBB-MAINT-02); keyboard operability preserved (NFR-WEBB-A11Y-01).
- Optional fold-in: `render_search_fragment` + `show_keyboard_help` overlay (same module,
  lowest risk) — fold here if cheap, else defer to a one-line follow-up.
