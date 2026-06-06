# CONTEXT

## Current Task

**Web-tier templating arc COMPLETE** (`main` at `7f63c8b`). `keyboard-fragments-templating` shipped the last two inline-`format!()` fragments (`keyboard.rs` search-results + help overlay) into Askama partials. **Zero inline HTML `format!` remains anywhere in `foundry-app/src`** — every web surface now renders from a template. 174/174 acceptance green; move-only, selector-identical; a new `inline_html_fragment_sites()` guard enforces it.

## Key Decisions

- Four-feature arc, all on trunk: `web-tier-extraction` (JSON API + JWT, `ba791ee`) → `htmx-web-tier` (Askama + vendored htmx2, `36c0fd3`) → `remaining-surfaces-templating` (full pages 9→0, `71c9c72`) → `keyboard-fragments-templating` (last 2 fragments, 0 inline HTML, `7f63c8b`).
- Established pattern (reused throughout): Askama 0.12 typed view-models, `base.html` for pages / bare partials for fragments, pure-vendored `/static` assets (no JS toolchain), selector-and-substring-identical render contract (existing suite = regression net), browser auth/CSRF/sessions untouched.
- Trunk-based (AGENTS.md + memory): commit to `main`, no PRs, no CI commit-gate, `cargo xtask ci` is the local gate (now green end-to-end after the US-03 deadlock fix + postgresql@16 client).

## Next Steps

- None outstanding — the web-tier templating arc is fully delivered and green. `foundry-app` has zero inline HTML; two CI-enforced guards (`inline_full_page_sites` + `inline_html_fragment_sites`) prevent regression.
- Optionally delete the stale `feature/web-tier-extraction` branch (its work is on trunk).
