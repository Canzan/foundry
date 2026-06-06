# Remaining-Surfaces Templating — Outcome KPIs

> Inherits Feature B's outcome logic. Because this is a move-only refactor of a
> shipped UI, the KPIs are maintainability/consistency outcomes (the contributor
> payoff for htmx-web-1, the styled-consistency payoff for htmx-web-2), not new
> end-user-behavior metrics. One KPI table suffices (single epic, 6 thin stories).

## Feature: remaining-surfaces-templating

### Objective
Finish the inline-`format!()`→template move so EVERY Foundry web surface renders
from a template extending the shared `base.html` — making any surface restyle-able
in one template file (htmx-web-1) and every screen styled-consistent and offline
(htmx-web-2) — with the acceptance suite green throughout.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Contributors editing any remaining surface | change a surface's markup/wording by editing a template, not handler `format!()` | 100% of remaining surfaces' on-screen text greppable in `templates/`, 0% in handler `format!()` | ~15 inline `format!()` HTML sites across 6 modules | grep for on-screen strings in `templates/` vs `.rs` | Leading |
| 2 | Self-hosters landing on a remaining surface | see a styled page linking `/static`, not bare-`<head>` raw HTML | 0 bare-`<head>` `format!()` full pages remain in foundry-app | ~7 bare-`<head>` full-page sites | grep for `<!doctype` inside `format!` strings → 0 | Leading |
| 3 | The maintainer | keep the regression net intact through the move | `foundry-acceptance` `[Summary]` passing count does not drop after any slice | current green count | `cargo test -p foundry-acceptance --release` | Guardrail |
| 4 | Self-hoster | server-render latency unchanged on moved surfaces | P95 ≤200 ms, no regression vs `format!` | Feature B board bench parity | reuse backend-mvp NFR-PERF-01 harness on touched surfaces | Guardrail |

### Metric Hierarchy
- **North Star**: 0 inline `format!()` HTML render sites remain in `foundry-app`
  (the cut is complete — every surface is template-driven).
- **Leading Indicators**: per-slice reduction in inline `format!()` sites; per-slice
  count of full pages now extending `base.html`.
- **Guardrail Metrics**: acceptance suite passing count (must not drop); P95 render
  latency (must not regress); zero new runtime dependency/service.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| 1 (markup in templates) | source tree | grep on-screen text in `templates/` vs `.rs` | per slice | contributor/reviewer |
| 2 (no bare-`<head>` pages) | source tree | grep `<!doctype` in `format!` strings | per slice | contributor/reviewer |
| 3 (suite green) | CI / local | `cargo test -p foundry-acceptance --release` | per slice + CI | CI |
| 4 (render budget) | bench harness | criterion + synthetic HTTP load on touched surfaces | once, spot-check | contributor |

### Hypothesis
We believe that moving the remaining inline-`format!()` surfaces into Askama
templates extending the shared `base.html` will let contributors restyle any
screen in one file and give self-hosters a consistently styled UI. We will know
this is true when 0 inline `format!()` HTML render sites remain in `foundry-app`
and the acceptance suite stays green with no render-latency regression.
