# htmx Web Tier (Feature B) — Template Engine Decision

Owner: solution-architect (Morgan). This is DESIGN's Open Decision #1 (DB5). Interaction mode:
**Propose** — three options with trade-offs for THIS codebase, then a recommendation awaiting user
ratification. Companion: `architecture.md`, `render-contract.md`, `wave-decisions.md` (ADR-B01).

## The decision frame (the constraints that actually discriminate)

This is **not** a generic "pick a Rust template engine" question. The constraints from
`nfrs.md`/`out-of-scope.md` and the code reading narrow it sharply:

1. **≤200 ms P95 render budget, no regression vs the current `format!` path** (NFR-WEBB-PERF-01).
   `format!` is effectively free; the engine must add no measurable latency.
2. **The acceptance suite must stay green** (NFR-WEBB-COMPAT-01/02). The suite reads the DOM via
   `scraper` (CSS selectors + text) plus a few `body.contains("…")` substring checks — see
   `render-contract.md`. So the engine must reproduce **structure + markers + literal copy**, not
   freeze whitespace. All three candidates can do this.
3. **No JS toolchain, air-gap/reproducible image, one binary** (DB6, NFR-WEBB-INFRA-01/02). An engine
   that needs **no runtime template-file I/O** is preferred (templates compiled into the binary), so
   a fresh `docker compose up` cannot fail on a missing/wrong template path at runtime.
4. **Refactor distance from inline `format!`** (the contributor payoff, htmx-web-1). The closer the
   migration is to a mechanical move, the lower the regression risk.
5. **Fragment / partial support for htmx OOB swaps** (US-B01/B03, NFR-WEBB-MAINT-02). One partial
   definition rendered across full-page, htmx-append, edit, and cancel paths.
6. **Already in the tree?** Prefer crates already blessed by the workspace.

**Grounding fact that dominates the decision:** `askama = { version = "0.12" }` is **already declared
in `[workspace.dependencies]`** (`Cargo.toml:38`) and was named as the engine by backend-mvp's
`architecture.md` — but it is **absent from `Cargo.lock`** (no crate depends on it, because
`templates/` is empty). So Askama is the *workspace-blessed-but-unwired intent*. Wiring it is the
lowest-surprise, lowest-process-cost choice.

## Option A — Askama 0.12 (RECOMMENDED)

Compile-time, typed, Jinja-like. Templates are `.html` files; a `#[derive(Template)]` struct binds
fields to template variables; the macro **parses and type-checks the template at `cargo build`** and
generates a `Display`/`render()` impl. `askama_axum` provides `IntoResponse` so a handler returns the
struct directly. License **MIT/Apache-2.0**.

- **Compile-time safety: STRONGEST.** A typo'd field, a missing template, or a type mismatch is a
  *build error*, not a runtime 500. This directly satisfies US-B06 scenario 3 ("a referenced missing
  template fails fast") at compile time — the best possible Earned-Trust posture (Principle 12).
- **Render budget: EXCELLENT.** Templates compile to Rust that writes into a buffer — essentially the
  same machine code `format!` produces, no runtime parse, **no runtime file I/O**. Bench (NFR-WEBB-
  PERF-01) is expected to show parity with the `format!` baseline. Best fit for the ≤200 ms budget
  and the air-gap/one-binary ethos (the template is *in* the binary; a missing-template-at-runtime
  failure mode does not exist).
- **Acceptance compatibility: EXCELLENT.** Jinja `{% block %}`/`{% include %}`/`{% if %}`/`{% for %}`
  reproduce the current DOM and the `data-*` markers exactly; the literal error strings live as plain
  text in the template. Auto-escaping is ON by default (matches the current `html_escape` calls); the
  one deliberate exception is the already-sanitized comment `body_html`, embedded with Askama's
  `|safe` filter (mirrors `comments.rs:820`, where `body_html` is embedded verbatim).
- **Refactor distance: SMALL.** Each `render_*` `format!` block maps to a template + a view-model
  struct; the handler swaps `Html(render_x(...))` for `views::X { ... }`. Mechanical, surface by
  surface — exactly the slice plan.
- **Partials/OOB: NATIVE.** `{% include "partials/comment_card.html" %}`; OOB variants are a thin
  wrapper template that includes the same partial inside `<div hx-swap-oob="…">`. ONE definition,
  every path (NFR-WEBB-MAINT-02) — fixes the `render_comment_card_oob` divergence.
- **Already in the tree?** Declared in the workspace manifest (intent); wiring adds `askama` +
  `askama_axum` to `Cargo.lock`. No license-set change anticipated (MIT/Apache-2.0).
- **Cons (honest):** template changes require **recompilation** (no runtime hot-reload by default;
  `askama` does offer a dev-time reload feature but the production posture is compiled-in). For a
  contributor this means "edit template → `cargo build`/`cargo test`" — acceptable, and the
  acceptance suite is the feedback loop anyway. Compile times rise slightly with template count
  (negligible at Feature B's ~10 templates).

> **Naming note for the crafter:** the `askama`/`rinja` ecosystem briefly forked and partially
> re-merged. The workspace pins `askama = "0.12"`; DESIGN recommends honoring that pin. If the
> resolved 0.12 line has diverged from `askama_axum` compatibility at implementation time, the
> crafter may pin the matching `askama`/`askama_axum` pair (a version-pin detail, not an architecture
> change) — recorded as a low-severity flag in `upstream-changes.md`.

## Option B — Maud 0.26

HTML-as-a-Rust-macro: markup is written *inside* Rust with the `html! { ... }` macro; compile-time,
no separate template files. License **MIT**.

- **Compile-time safety: STRONG** (it is Rust; a bad reference is a compile error). Comparable to
  Askama on this axis.
- **Render budget: EXCELLENT** (compiles to buffer writes, like Askama).
- **Closest to the *current* inline approach** — markup stays in Rust, just structured. This is its
  headline appeal and its headline problem for Feature B.
- **Refactor distance: SMALLEST mechanically** — but it **does not satisfy the feature's primary
  job.** htmx-web-1 / NFR-WEBB-MAINT-01 is explicitly *"on-screen text and markup live in template
  FILES, not handler code; grepping for on-screen text lands in `templates/`."* Maud keeps markup in
  `.rs` files — a contributor restyling the board still edits Rust. That is the exact pain Feature B
  exists to remove. Choosing Maud would technically tidy the markup while **missing the point of the
  feature** (a resume/comfort-driven choice the reviewer would flag).
- **Partials/OOB:** Rust functions returning `Markup` — works, but the "one partial" lives in Rust,
  not a template, again contradicting MAINT-01.
- **Already in the tree?** No (not in `Cargo.lock`, not in the workspace manifest).
- **Verdict:** rejected for Feature B specifically because it does not move markup out of Rust — the
  feature's whole reason for existing. (It would be a fine choice for a project whose goal was *not*
  contributor-facing template files.)

## Option C — Minijinja 2.x

Runtime Jinja interpreter; templates loaded from disk (or embedded), parsed at runtime; supports
hot-reload. License **Apache-2.0**.

- **Compile-time safety: WEAKEST.** Template errors (typo'd variable, missing template) surface at
  **runtime** — exactly the failure mode US-B06 scenario 3 wants to avoid, and the one Askama
  eliminates at build time. Mitigable with a startup "render every template once" probe, but that is
  re-inventing what Askama gets for free.
- **Render budget: GOOD but not free.** Runtime template parse/interpret adds per-render overhead
  vs `format!`/Askama. Templates can be precompiled/cached, and embedded via `include_str!` to avoid
  runtime file I/O — but it is still an interpreter on the hot path. Most at risk of nibbling the
  ≤200 ms budget under load (the board with 1,000 issues, NFR-WEBB-PERF-01's load profile).
- **Acceptance compatibility: EXCELLENT** (full Jinja).
- **Hot-reload** is its genuine advantage — a contributor edits a template and refreshes without
  recompiling. But Feature B's feedback loop is the acceptance suite (which recompiles anyway), and
  hot-reload pulls toward runtime template loading, which **fights the air-gap/one-binary/no-runtime-
  surprise posture** (DB6, NFR-WEBB-INFRA). Embedding templates to keep one binary throws away the
  hot-reload benefit, leaving a runtime interpreter with weaker safety than Askama.
- **Already in the tree?** No.
- **Verdict:** viable, but strictly dominated for THIS codebase: it trades Askama's compile-time
  safety and zero-runtime-overhead for a hot-reload convenience the feature does not need and a
  runtime-loading posture the constraints discourage.

## Comparison summary

| Axis (weight for Feature B) | Askama (A) | Maud (B) | Minijinja (C) |
|---|---|---|---|
| Markup in template FILES (htmx-web-1 — the feature's point) | YES | **NO (in Rust)** | YES |
| Compile-time safety / missing-template = build error | **STRONGEST** | STRONG | weakest (runtime) |
| ≤200 ms render budget, no runtime template I/O | **EXCELLENT** | EXCELLENT | good (interpreter) |
| Acceptance-suite reproduction (DOM + markers + copy) | EXCELLENT | EXCELLENT | EXCELLENT |
| Refactor distance from `format!` | small | smallest | small |
| Partial/OOB one-definition support | native include | Rust fn | native include |
| Already workspace-blessed | **YES (manifest)** | no | no |
| Air-gap / one-binary fit | **EXCELLENT** | excellent | mixed (pulls to runtime load) |

## Recommendation — **Askama 0.12** (ADR-B01)

Askama wins on every axis that discriminates for Feature B: it **moves markup into template files**
(the feature's primary job, which Maud fails), it has the **strongest compile-time safety** (a
missing/typo'd template is a build error, the best Earned-Trust posture, which Minijinja lacks), it
adds **no runtime template I/O** (best fit for the ≤200 ms budget and the air-gap/one-binary ethos),
and it is **already the workspace-blessed intent** (`Cargo.toml:38`) so wiring it is the
lowest-process-cost, lowest-surprise path. The one cost — recompile-on-template-change — is
acceptable given the acceptance suite is the feedback loop regardless.

**Pin:** `askama = "0.12"` (honor the existing workspace pin) + `askama_axum` (matching version for
the `IntoResponse` integration). Both MIT/Apache-2.0. Net runtime dependencies added by Feature B:
this pair (the only one). Re-run `cargo deny check`.

This decision is presented for ratification (Propose mode); if the user prefers Maud (markup-in-Rust)
or Minijinja (hot-reload), the render contract and slice plan are unaffected — only the mechanism
changes. DESIGN's recommendation is Askama.
