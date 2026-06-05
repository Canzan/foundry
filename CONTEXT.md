# CONTEXT

## Current Task

**Feature B "htmx-web-tier" shipped to trunk** (`main` at `36c0fd3`). Full nWave pipeline (DISCUSS→DESIGN→DISTILL→DELIVER): inline `format!` HTML replaced with **Askama 0.12 templates** + pure-vendored **htmx 2.0.4 / Alpine 3.14.9 / CSS** in `static/` (no JS toolchain), reusing the `foundry-services` seam. 157/157 acceptance scenarios green; scoped mutation 100%. Fixed the `comments.rs:841` OOB-affordance bug. Both halves of the web-tier-extraction split are now done (Feature A = JSON API, shipped `ba791ee`; Feature B = htmx tier, shipped `36c0fd3`).

## Key Decisions

- **Askama 0.12** templating (compile-time typed); **pure vendored assets** (no Node/bundler/CDN); **content-hashed CSS** filename for safe `immutable` caching; **htmx 1→2** as one atomic final slice.
- **Selector-and-substring-identical render contract** — the existing suite (parses DOM via `scraper`) staying green is the move's correctness proof. Browser auth/CSRF/sessions untouched (markup-only).
- **Trunk-based workflow** (AGENTS.md + memory): commit to `main`, no PRs, no CI commit-gate, validate with `cargo xtask ci`. Feature B committed directly to `main` (no branch).

## Next Steps

- **Deferred (documented in `htmx-web-tier/discuss/out-of-scope.md`)**: remaining inline-`format!` surfaces — `projects.rs` create-form + error fragment, `keyboard.rs` new-issue modal, `issues.rs` create-error fragment — a clean follow-up "remaining-surfaces templating" feature.
- **`@all` US-03 DEADLOCK FIXED** (commit `6407946`): root cause was a `FoundryWorld` field-drop-order race (serialization guard released before the restored sqlx pool closed → sibling `pg_restore --clean` blocked on a relation lock). Fix: reorder fields + `After`-hook `pool.close().await` + `lock_timeout=30000` fail-fast + `stdin(null)`. No more hang. **Residual** (local-only, fast, not a hang): US-03 restore fails because local `pg_dump` 18.4 writes a PG17+ GUC the pg16 server rejects — fix by installing a matching **postgresql@16** client (`brew install postgresql@16` + link). CI uses pg16 → green there.
- Optionally delete the stale `feature/web-tier-extraction` branch (Feature A work is on trunk).
