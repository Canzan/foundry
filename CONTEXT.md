# CONTEXT

## Current Task

**Feature "remaining-surfaces-templating" shipped to trunk** (`main` at `71c9c72`). Fast-forwarded nWave pipeline (DISCUSS→DESIGN→DISTILL→DELIVER): templatized the last inline-`format!()` HTML surfaces into Askama templates extending `base.html`, reusing Feature B's contract. **Inline full-page `format!` sites 9 → 0**, enforced by a source-tree completion guard. `@all` suite 183/183 green; move-only, selector-identical; zero new deps. This completes the web-tier templating arc: Feature A (JSON API, `ba791ee`) + Feature B (htmx tier, `36c0fd3`) + remaining surfaces (`71c9c72`).

## Key Decisions

- **Three shared templates** carry the reuse: `error_fragment.html` (parameterized marker), `invalid_page.html` (rewired ~17 `bootstrap.rs::invalid_page` callers app-wide), one-partial OOB (attachment row). DESIGN was inherit-only (Askama/base.html/render-contract/assets from Feature B).
- **Move-only**: control-flow/status contracts preserved exactly (signed-out `/` 303, events 401, payload 413, bootstrap CSRF-exempt, signed invite URL byte-stable). Selector-identical; existing suite is the regression net.
- **Trunk-based** (AGENTS.md + memory): all 9 commits direct to `main`, no branch, no PRs. Mutation N/A for move-only (no new logic); review proportionate (XSS surface verified clean — one `|safe` on a server-constructed invite URL).

## Next Steps

- **Local gate is green**: `cargo xtask ci` `@all` lane passes end-to-end (US-03 deadlock fixed `6407946` + postgresql@16 client installed). pg client on PATH is now 16.14.
- **Optional tail**: `keyboard.rs` search-fragment + keyboard-help overlay (lowest-risk, was out of US-R01..R06 scope) — fold into a future small pass if wanted.
- Optionally delete the stale `feature/web-tier-extraction` branch (Feature A work is on trunk).
