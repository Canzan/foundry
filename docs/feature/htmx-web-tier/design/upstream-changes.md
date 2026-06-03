# htmx Web Tier (Feature B) — Upstream Changes (DESIGN → DISCUSS feedback)

Owner: solution-architect (Morgan). Records where DESIGN's code-grounded reading refines a DISCUSS
assumption. None of these change the stories, scope, slices, or NFR intent — they refine framing and
shape ADRs. Listed so the DISCUSS artifacts and DESIGN stay reconcilable.

## CHG-1 — The acceptance suite reads the DOM, not the bytes (LOWERS a HIGH risk)

- **DISCUSS framing**: risk register + NFR-WEBB-COMPAT-02 frame the compatibility risk as
  *"acceptance scenarios assert on HTML **substrings**; templating changes **whitespace/markup**"* —
  rated **HIGH** probability, with the mitigation "keep asserted substrings byte-stable."
- **DESIGN finding (grounded)**: the structural assertions go through **`scraper`** — a real HTML/DOM
  parser — using **CSS selectors + trimmed visible text**
  (`crates/foundry-acceptance/src/support/html_assertions.rs`: `assert_has`,
  `assert_comment_has_element_with_text` with `text.trim() == expected.trim()`, `collect_attributes`).
  Only error *copy* (e.g. `"Invalid email or password"`, `us_06_signin.rs:273`) and the `data-*`
  *markers* are checked by `body.contains(...)`.
- **Consequence**: whitespace, indentation, and in-tag attribute ORDER changes are **already safe** —
  they do not affect a DOM parser or a substring check on copy/markers. The render contract is
  therefore **selector-and-substring-identical**, NOT byte-identical (ADR-B02). This **lowers** the
  HIGH risk to manageable and is the basis for Decision #2's recommendation (move-only, defer visual
  rework). It refines, does not contradict, the DISCUSS mitigation (keep the asserted *things*
  stable — just not the whitespace around them).
- **Impact on stories/scope**: none. The regression net (the suite) is unchanged and remains binding.

## CHG-2 — "askama" was named but never wired (REDUCES net-new surface)

- **DISCUSS/prior framing**: backend-mvp `architecture.md` named `askama` as the templating engine,
  and `[workspace.dependencies] askama = "0.12"` is declared in the root `Cargo.toml:38`.
- **DESIGN finding**: `askama` is **absent from `Cargo.lock`** — no crate's `[dependencies]`
  references it, so it never resolved. This is consistent with `templates/` being empty: the engine
  was *intended* but never *wired*.
- **Consequence**: wiring Askama in Feature B is the **lowest-process-cost** engine choice (honoring
  an existing workspace pin, not adding an unblessed dep), and it is correctly counted as a CREATE
  NEW (wiring) rather than an EXTEND. Net new runtime dependency for Feature B = ONE (`askama` +
  `askama_axum`). Reinforces ADR-B01.
- **Impact on stories/scope**: none.

## CHG-3 — `tower-http`'s `fs` feature is already enabled (REDUCES net-new surface)

- **DESIGN finding**: the workspace `tower-http` dependency already enables `fs`
  (`Cargo.toml:35`: `features = ["trace", "compression-gzip", "fs", "limit", "request-id", "util"]`),
  so `tower_http::services::ServeDir` is available with **zero new dependency**.
- **Consequence**: static serving (US-B02/B06) adds no dependency — only a `.nest_service("/static",
  ServeDir::new("static"))` line in `build_router`, mirroring the existing `attachment_routes`
  sub-router. Reinforces ADR-B03 and the "ONE net-new dep" accounting.
- **Impact on stories/scope**: none.

## CHG-4 — `render_comment_card_oob` omission is a real (small) UX bug the feature fixes

- **DISCUSS framing**: US-B03 already notes the OOB card omits Edit/Delete "for simplicity" and that
  the one-partial work fixes it.
- **DESIGN confirmation (grounded)**: `comments.rs:841` deliberately elides the buttons from the OOB
  fragment, so a live-appended card visibly differs from a reloaded one. The one-partial rule (DD10,
  NFR-WEBB-MAINT-02) makes the OOB wrapper `{% include %}` the SAME `comment_card.html` with the same
  flags, eliminating the divergence by construction.
- **Consequence**: this is the single in-scope *behavior* change; it is covered by a NEW
  live-vs-reloaded structural-equality scenario (not by editing an existing scenario — ADR-B02).
- **Impact on stories/scope**: none (already anticipated by US-B03); recorded for traceability.

None of CHG-1..CHG-4 require a DISCUSS re-open. They are refinements DESIGN surfaces for the record
and to justify ADR-B01/B02/B03.
