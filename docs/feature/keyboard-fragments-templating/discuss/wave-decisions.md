# DISCUSS Decisions — keyboard-fragments-templating

## Key Decisions
- [DK1] Move-only templating of the 2 remaining inline-`format!` bare fragments in `keyboard.rs` (`render_search_fragment`, `show_keyboard_help`). Inherits the `htmx-web-tier`/`remaining-surfaces-templating` pattern + selector-and-substring-identical contract.
- [DK2] Jobs inherited (no new jobs): both stories trace to `htmx-web-1` (restyle without touching Rust). `htmx-web-2` (styled first screen) is N/A — these are interaction fragments, not landing pages.
- [DK3] Both surfaces are **bare fragments** (no `base.html`): the search results are htmx-swapped; the help overlay is a `role="dialog"` overlay. They do NOT link `/static` (the host page already does).

## Requirements Summary
- Primary need: the last two inline-`format!` HTML fragments in foundry-app move to Askama partials, making web-tier templating 100% (zero inline HTML anywhere in foundry-app handlers).
- Feature type: user-facing (UI), move-only refactor.
- Walking skeleton: US-K01 (search-results fragment).

## Constraints Established
- Selector-and-substring-identical (existing us-12-keyboard-nav suite is the regression net).
- Browser auth/CSRF/sessions untouched; one binary; no JS toolchain; no new deps (Askama already wired).
- HTML-escaping of key/title/label preserved (Askama auto-escape replaces the manual `html_escape`).

## Scope Assessment: PASS (right-sized — 2 bare fragments, 1 slice).

## Upstream Changes
- None. Pure tail of `remaining-surfaces-templating`; reuses its jobs/contract.
