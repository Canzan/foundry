# Evolution — htmx-web-tier (Feature B: "Foundry looks like a product")

**Finalized**: 2026-06-04
**Ship commit**: [da8b836](../../) — tip of a 10-commit run off `247198b` (roadmap) / `5aea95b` (first code)
**Wave coverage**: full nWave pipeline — DISCUSS → DESIGN → DISTILL → DELIVER (legacy per-feature doc layout, trunk-based: committed directly to `main`).

## Feature summary

The second half of the `web-tier-extraction` split. Replaces Foundry's inline `format!()`-string HTML (scattered across `foundry-app` handlers) with a real **Askama 0.12 template tier** + **pure-vendored htmx 2 / Alpine / CSS** served from `static/`, reusing the `foundry-services` seam that Feature A proved. Server-rendered htmx, one binary, no SPA, no JS toolchain.

Feature A ("Programmatic Foundry", the JSON API) shipped the presentation-neutral core seam; Feature B is the htmx web-tier track that was deferred and explicitly flagged "validate the strawman before DESIGN." DISCUSS re-validated the web-track jobs (promoting `htmx-web-1`/`htmx-web-2`, retiring `jtbd-web-2` as already-done-by-Feature-A, adding `htmx-web-3` for the htmx-2 upgrade).

## What shipped

```
foundry-app (one binary)
├── templates/              NEW — Askama: base.html, board.html, issue.html,
│     signin.html, forgot.html, forgot_sent.html,
│     partials/{issue_card, comment_card, comment_edit_form}.html,
│     partials/oob/comment_card_oob.html, partials/errors/issue_400.html
├── static/                 NEW — vendored, pinned, content-addressed:
│     vendor/htmx.min.js (2.0.4), vendor/alpine.min.js (3.14.9),
│     css/foundry.870985fc.css, VENDOR.md (provenance + sha256)
├── src/views.rs            NEW — typed #[derive(Template)] view-models
└── src/{projects,comments,signin}.rs — render switched format! → Askama
```

### Key decisions (ratified with the user)

- **Askama 0.12** (DESIGN ADR-B01, user-ratified): compile-time-typed templates, markup moves out of Rust (the primary job), no runtime template I/O. Was already declared-but-unwired in `Cargo.toml`.
- **Pure vendored assets** (DISCUSS DB6, user-ratified): minified htmx/Alpine/CSS committed directly to `static/` — NO Node, NO bundler, NO package.json, NO CDN, at runtime OR build time. Asset update = manual pinned-blob swap.
- **Selector-and-substring-identical render contract** (DESIGN ADR-B02): the acceptance suite parses the DOM via `scraper`, so whitespace is free; structure + selectors + copy + `data-*` markers are preserved. The existing board/comment/sign-in suite staying green IS the move's correctness proof.
- **htmx 1→2 as a dedicated final slice** (DISCUSS DB4): template all surfaces on htmx 1.x first, then bump to pinned 2.0.x + normalize directives as one atomic regression-gated change. Foundry's directive set is core-only (no `hx-on`, no extensions) → API-compatible.
- **No `foundry-web` crate extraction** (DESIGN, user-ratified): templating stayed inside `foundry-app`; web≠DB is already guard-enforceable from Feature A.
- **Content-hashed CSS filename** (`foundry.870985fc.css`): so the `immutable` 1-year cache header is safe on the hand-authored stylesheet (fixed a Phase-4 review blocker).

## How it was built (DELIVER)

7 DES-monitored TDD steps across 4 slices, each a `@real-io` cucumber scenario driven to green:

| Step | Outcome |
|------|---------|
| 01-01 | Wire Askama + askama_axum; vendor htmx 1.x/Alpine/CSS blobs + VENDOR.md; mount `/static` |
| 01-02 | base + board + issue-card templates + `views.rs`; switch `show_board` (walking skeleton) |
| 01-03 | render-failure → clean 500 (cfg-gated test seam, no half-page) |
| 02-01 | issue page + comment-card partial; sanitized `body_html` via `\|safe` only |
| 02-02 | OOB comment card renders through the SAME partial — fixes the `comments.rs:841` affordance-omission bug |
| 03-01 | sign-in + forgot via shared base layout; CSRF/cookie/non-enumerable-error untouched |
| 04-01 | bump vendored htmx to pinned **2.0.4**; normalize directives (data markers byte-stable) |

Then: Phase-4 adversarial review (Sonnet) → 1 blocker + 3 high, all fixed test-first (content-hash CSS cache-bust, render-fail flag teardown, template the forgot-success POST, real htmx-file-count check, Alpine→3.14.9); Phase-5 mutation (scoped affordance predicate + board builder **100% kill**, 2 survivor-killing tests added).

## Quality at ship

- **Acceptance**: 157/157 scenarios, 1352/1352 steps green. All 9 Feature-B RED scenarios driven to green; the existing board/comment/sign-in suite stayed green throughout (the selector-identity proof).
- **The bug fix**: a live htmx-OOB comment card is now structurally identical to the reloaded card (both carry Edit/Delete via the one shared partial), computed by the same ADR-006/007 predicate.
- **Build/lint**: `cargo build --workspace --tests`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo deny check` all clean.
- **Mutation**: scoped logic 100% (14/14 viable); the security-adjacent `can_edit`/`can_delete` predicate fully pinned.
- **Review strengths**: `|safe` bounded to exactly the one pre-sanitized field (XSS surface correct); sanitization stays in `foundry_core`; render-failure seam cfg-gated out of release; one-partial rule by construction; CSRF/sign-in/session contracts byte-identical; real e2e tests over HTTP.

## Residuals / follow-ups (documented in `discuss/out-of-scope.md`)

- **Remaining inline-`format!` surfaces** — `projects.rs::render_create_form`/`render_error_fragment`, the `keyboard.rs` new-issue modal, and the `issues.rs` issue-create error fragment still emit bare-`<head>` HTML. Knowingly OUT of the ratified US-B01..B05 scope (board, issue+comments, sign-in); a clean follow-up "remaining-surfaces templating" feature.
- **Visual redesign / mobile / theming / dark mode** — out of scope; this was a move-only refactor (markup to templates), not a restyle. The template + asset pipeline makes those easier later.
- **xtask check-assets probe** — the "exactly one htmx file" contract is now checked in the acceptance step; a dedicated CI asset-provenance probe remains a nice-to-have.

## Pointers

- Spec: `docs/feature/htmx-web-tier/discuss/`, `design/`, `distill/`
- DES roadmap + execution log: `docs/feature/htmx-web-tier/deliver/`
- Mutation report: `docs/feature/htmx-web-tier/deliver/mutation/mutation-report.md`
- Templates + assets: `crates/foundry-app/templates/`, `crates/foundry-app/static/` (+ VENDOR.md)
