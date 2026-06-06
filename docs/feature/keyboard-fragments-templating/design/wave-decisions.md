# DESIGN Decisions — keyboard-fragments-templating

> Inherit-only. Reuses `htmx-web-tier` ADR-B01..B07 (Askama 0.12, `views.rs` typed view-models,
> bare-fragment rendering, content-hashed `/static` assets) and the `remaining-surfaces-templating`
> render contract. Zero new architecture, dependencies, or infrastructure — deliverables are two
> template files + two field-holder view structs.

## Surface → template / view-model map (verified `keyboard.rs`)

| Story | Current site | New template | View-model | Shape |
|---|---|---|---|---|
| US-K01 | `keyboard.rs::render_search_fragment` (~:230) | `partials/search_results.html` | `SearchResults { items: Vec<SearchResultRow>, empty: bool }` (or render the empty case in-template) | **bare fragment** |
| US-K02 | `keyboard.rs::show_keyboard_help` (~:252) | `partials/keyboard_help.html` | `KeyboardHelp { entries: Vec<ShortcutEntry> }` | **bare fragment / overlay** |

`SearchResultRow { key, title }` (key = `{prefix}-{n}`); `ShortcutEntry { key, label }`. Askama auto-escapes all fields (replaces the manual `html_escape` calls). Both templates are BARE (no `{% extends "base.html" %}`).

## Render contract (cite remaining-surfaces-templating / htmx-web-tier)
Selector-and-substring-identical. Byte-stable markers:
- search: `ul.search-results`, `li.search-result[data-issue-key]`, `.key`, `.title`, empty `ul.search-results[data-empty="true"]`.
- help: `section.keyboard-help[role="dialog"][aria-label="Keyboard shortcuts"]`, `header>h2`, `dl>dt[data-shortcut]`+`dd`.
The existing us-12-keyboard-nav suite (search + help coverage) is the regression net.

## Reuse Analysis
| Component | Decision | Justification |
|---|---|---|
| Askama engine + `views.rs` pattern + bare-fragment rendering | EXTEND | reused unchanged from Feature B / remaining-surfaces |
| `partials/search_results.html` + `partials/keyboard_help.html` + 2 view structs | CREATE NEW | the feature's only deliverable (template files + field-holders), not architecture |

## Technology Stack
- Unchanged. No new crate/dep (Askama already in `Cargo.toml`/`Cargo.lock`).

## Constraints Established
- Bare fragments (no base.html); auth/CSRF/sessions untouched; one binary; no JS toolchain; selector-identical.

## Open decisions
- None (inherit-only).
