# CONTEXT

## Current Task

**`keyboard-shortcut-bindings` SHIPPED + finalized** (`main`, 32 commits `3e3aa84`→`bdb7543`, **not pushed**).
The ask was "bind the `c` key"; investigation found **all seven advertised shortcuts were dead** — the
client keyboard layer was never written while the server routes shipped, routed and green, and the
port-to-port suite **could not press a key**. The absence was *decided, not missed*:
`us-12-keyboard-nav.feature` recorded a "no-Playwright decision" putting key handling "OUT of automated
scope" and described handlers living "in alpine.js" that never existed. KPI-1: **0/7 → 7/7**.
Shipped: `static/js/keyboard.js` (915 lines, ONE vanilla document-delegated IIFE); a **structural** guard
chain that names zero shortcuts (consumability is a platform fact — `key.length === 1` ||
`NATIVE_TEXT_ENTRY_KEYS`); a DOM-derived Esc layer stack (help → modal → search → no-op); key-based
selection (never an index) with a WCAG-AA ring + `aria-activedescendant` composite re-projected on
`htmx:afterSwap`; and — the root-cause fix — a **real-browser lane** (fantoccini + chromedriver,
`@needs-browser`, included in `cargo xtask ci`'s `all`). Alpine retired; `#kb-items` carrier retired
(33 sites). Reuse over reconstruction throughout: `c`/`Enter` click the shipped `hx-get` triggers, so
zero client CSRF, **zero new routes/endpoints/migrations** (latest remains `0014`).
Verified: browser **38/38** (263 steps), default **514/514** (3692 steps), fmt/clippy/deny/check-arch
clean, DES **15/15** steps complete. Archive: `docs/evolution/2026-07-17-keyboard-shortcut-bindings.md`.

## Key Decisions

- **Seven defects found by execution (UI-1..UI-7), five inside DESIGN artifacts; zero by any review.**
  Crafters blocked **five times — every one correct**. UI-3: ADR-002 made `Esc` unable to close the modal
  it opened, while `keyboard.rs` advertises it as literally "Close modal" (fixed by narrowing the
  predicate's *domain*, not a carve-out). UI-4: the acceptance runner was **green over undefined steps** —
  the instrument had the disease it was built to diagnose (fixed: `.fail_on_skipped()`). UI-7: ADR-005 §3
  was unreachable under guard 4. Full record: `deliver/upstream-issues.md`.
- **A green can be an artefact of the instrument.** Blur-on-arrival ran **6/6 green** while destroying a
  human's typing — `send_keys()` batches keystrokes with no round-trip. Found only by typing at
  150ms/char. Same lesson as UI-4, UI-6, and the feature's own origin. Two scenarios that *couldn't
  falsify* were caught the same way and strengthened.
- **Twelve features through the full nWave pipeline, all on trunk**, each in `docs/evolution/`; all
  feature workspaces PRESERVED. Legacy multi-file layout; no `docs/product/` SSOT; no PRs; DES hook
  requires the 5-phase contract; verify with full-workspace `cargo fmt --all --check` +
  `cargo clippy --all-targets --release -- -D warnings`.

## Next Steps

- Optional: `git push` (this finalize did NOT push).
- **UI-5 (open)**: guard 1 (IME) is **unfalsifiable** — deleting it leaves `@ime` green because guard 4
  already inerts `c`. Guard 1 is correct and must stay; retarget the scenario at `Escape` (which an IME
  uses to cancel composition). DISTILL's call.
- **Also open**: ADR-008's trap-B mechanism is **inverted** (05-05 proved both forms red identically) —
  needs a DESIGN correction; `#kb-search-panel` and `#kb-overlay-root` have **no CSS** at all; two lane
  flakes (leaked postgres testcontainers → `PoolTimedOut`; the no-JS scenario ~2-3/10). A
  `pipx upgrade nwave-ai` reverts the `des-init-log` `project_id` patch that unblocks DES subagents.
- **Carried**: Prometheus `foundry_token_mutations_total` exporter; per-workspace backup (OD-5);
  key-rotation UX; nightly scoped mutation pass on the web adapter.
