# Stories — keyboard-fragments-templating

> The optional tail of the web-tier templating arc (deferred from `remaining-surfaces-templating`
> US-R01..R06). Two remaining inline-`format!()` **bare htmx fragments** in `keyboard.rs` move to
> Askama partials, reusing the established pattern + selector-and-substring-identical render
> contract. Move-only; no behavior change. Jobs inherited from `htmx-web-tier` (no new jobs).

## US-K01 — Search-results fragment renders from a template
`job_id: htmx-web-1` (restyle/re-word a screen without touching Rust — inherited from htmx-web-tier)

As a contributor maintaining the UI, I want the keyboard search-results fragment rendered from an
Askama partial (not an inline `format!` string) so its markup lives in a template like every other surface.

### Elevator Pitch
Before: the search-results `<ul>` is built by inline `format!` in `keyboard.rs::render_search_fragment`.
After: hit the search endpoint (htmx) → sees the SAME `<ul class="search-results">` with `<li class="search-result" data-issue-key="KEY-N">` items (and the empty `data-empty="true"` case), rendered from `partials/search_results.html`.
Decision enabled: a maintainer can restyle/re-word search results by editing a template, not Rust.

### Acceptance Criteria
- The search fragment renders from `partials/search_results.html` as a **BARE fragment** (no base.html).
- Selector-and-substring-identical: `ul.search-results`, `li.search-result[data-issue-key="{prefix}-{n}"]`, `.key`, `.title`, and the empty-state `ul.search-results[data-empty="true"]` are byte-stable; titles/keys stay HTML-escaped (Askama auto-escape).
- The existing us-12-keyboard-nav suite stays green (regression net); no inline `format!()` HTML remains in `render_search_fragment`.

## US-K02 — Keyboard-help overlay renders from a template
`job_id: htmx-web-1`

As a contributor maintaining the UI, I want the keyboard-help overlay rendered from an Askama partial so its markup lives in a template.

### Elevator Pitch
Before: the help overlay is built by inline `format!` in `keyboard.rs::show_keyboard_help`.
After: `GET /keyboard-help` → sees the SAME `<section class="keyboard-help" role="dialog" aria-label="Keyboard shortcuts">` with `<dt data-shortcut="K">`/`<dd>` entries, rendered from `partials/keyboard_help.html`.
Decision enabled: a maintainer can restyle/re-word the shortcuts overlay by editing a template, not Rust.

### Acceptance Criteria
- The overlay renders from `partials/keyboard_help.html` as a **BARE fragment** (no base.html — it is an htmx-swapped/overlay dialog).
- Selector-and-substring-identical: `section.keyboard-help[role="dialog"][aria-label="Keyboard shortcuts"]`, `header > h2` "Keyboard shortcuts", `dl > dt[data-shortcut]` + `dd`, byte-stable; key/label stay escaped.
- The existing keyboard-help/us-12 coverage stays green; no inline `format!()` HTML remains in `show_keyboard_help`.

## Out of scope
Same as `htmx-web-tier`/`remaining-surfaces-templating`: not a redesign, not new behavior, no JS toolchain, browser auth/CSRF/sessions untouched. No new jobs/personas (inherits htmx-web-1).
