# Evolution — keyboard-fragments-templating

**Finalized**: 2026-06-06
**Ship commit**: `12bce74` (single DELIVER step) — feature run off `1ce71f7` (roadmap)
**Wave coverage**: full nWave pipeline, fast-forwarded; lean (compact DISCUSS/DESIGN authored by the orchestrator, agent-delegated DISTILL + DELIVER). Trunk-based (committed directly to `main`).

## Feature summary

The optional tail of the web-tier templating arc (deferred from `remaining-surfaces-templating` US-R01..R06). The last two inline-`format!()` **bare htmx fragments** in `keyboard.rs` — `render_search_fragment` (the search-results `<ul>`) and `show_keyboard_help` (the shortcuts dialog) — move to Askama partials. Move-only; byte-identical markup. **Result: zero inline `format!()` HTML anywhere in `foundry-app/src` — web-tier templating is 100%.**

## What shipped
- `crates/foundry-app/templates/partials/search_results.html` ← `views::SearchResults { items: Vec<SearchResultRow{key,title}> }` (incl. empty `data-empty="true"` state). BARE fragment.
- `crates/foundry-app/templates/partials/keyboard_help.html` ← `views::KeyboardHelp { entries: Vec<ShortcutEntry{key,label}> }`. BARE `role="dialog"` overlay.
- `keyboard.rs` `render_search_fragment` + `show_keyboard_help` switched to Askama; manual `html_escape` replaced by Askama auto-escape.

## Key decisions
- **Inherit-only DESIGN** (Askama + `views.rs` + bare-fragment pattern from Feature B / remaining-surfaces). Zero new deps/architecture.
- **No new user-visible delta** (bare fragments, byte-identical markup, no `/static` link). So the move is covered by the **existing us-12-keyboard-nav suite** as the regression net, plus a new **`inline_html_fragment_sites()` guard** asserting 0 inline-HTML sites in `keyboard.rs` (RED 3 → GREEN 0). DISTILL also tightened us-12's coverage of the search-list wrapper / `.key` / empty-state and the help dialog container/heading (3 scenarios, green throughout).

## Quality at ship
- Acceptance: **174/174 scenarios, 1470/1470 steps green**. us-k01 4/4 (guard at 0 sites, search populated/empty, help dialog); us-12 green.
- Build/fmt/clippy clean. Grep confirms **0 inline HTML `format!` in `foundry-app/src`**.
- Mutation: N/A (move-only, no new logic). Review: proportionate — auto-escaped fields, no `|safe`, browser auth/CSRF/sessions untouched.

## Pointers
- Spec: `docs/feature/keyboard-fragments-templating/{discuss,design,distill}/`
- DES log: `docs/feature/keyboard-fragments-templating/deliver/`
- Guard: `crates/foundry-acceptance/src/steps/feature_remaining_surfaces.rs::inline_html_fragment_sites()`

## The web-tier templating arc — complete
1. `web-tier-extraction` — Programmatic JSON API + machine-token auth (`ba791ee`)
2. `htmx-web-tier` — Askama + vendored htmx2 web tier (`36c0fd3`)
3. `remaining-surfaces-templating` — remaining full-page surfaces, inline full-page sites 9→0 (`71c9c72`)
4. `keyboard-fragments-templating` — last 2 fragments; **0 inline HTML in foundry-app** (`12bce74`)
