# htmx Web Tier (Feature B) — Outcome KPIs

> Feature B of the web-tier-extraction split. KPIs follow the Gothelf/Seiden
> "[Who] [Does what] [By how much]" formula. Because this is a refactor-with-a-styling-payoff,
> several KPIs are necessarily *capability* metrics (the behavior change is "a contributor CAN
> now edit one template" / "a self-hoster sees a styled screen"), measured by code/PR
> inspection and acceptance assertions rather than production telemetry — appropriate for a
> self-hosted single-binary tool with no central analytics.

## Feature: htmx-web-tier

### Objective
Make Foundry's server-rendered web UI look and feel like a finished product, and make its
markup a one-template edit — without adding a SPA, a new service, or a CDN, and without
regressing a single existing behavior.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| 1 | Contributors | Make a board/comment/sign-in markup change touching only a template | 100% of visual-only PRs touch zero handler `format!` HTML and zero store files | 0% (markup is in `format!` inside handlers today) | PR file-path diff on visual PRs | Leading |
| 2 | First-time self-hosting teams | See a styled board/issue/sign-in on first open, fully offline | 0 external-origin requests; styled-board check green on a no-egress host | Unstyled (empty `static/`), 0% styled | Acceptance network-request assertion + visual/asset checks | Leading |
| 3 | Members (Mei/Hiroshi) | Experience no behavioral regression after templating | 100% of previously-green acceptance scenarios stay green (incl. after htmx-2 bump) | N/A (suite green today) | `cargo test -p foundry-acceptance --release` passing count | Guardrail |
| 4 | Members | See a live-posted comment card identical to a reloaded one | live-vs-reloaded card divergence eliminated (was: OOB card omits affordances) | Divergent today (`render_comment_card_oob` omits buttons) | live-vs-reloaded structural-equality scenario (US-B03) | Leading |
| 5 | Maintainers | Upgrade htmx by swapping one vendored, pinned file | 1 pinned htmx 2.x file (was 0/unpinned); 1 consistent directive convention; 100% hx interactions regression-covered | htmx unvendored/unpinned; directives ad-hoc per handler | code inspection + suite green after bump | Leading |

### Metric Hierarchy
- **North Star**: % of visual-only changes that touch only a template (KPI 1) — the direct
  measure of the primary contributor job (htmx-web-1).
- **Leading Indicators**: styled-screen-offline check (KPI 2); live-vs-reload card parity
  (KPI 4); one pinned htmx version (KPI 5).
- **Guardrail Metrics** (must NOT degrade):
  - Acceptance suite passing count (KPI 3) — no behavioral regression.
  - P95 render latency ≤200 ms (NFR-WEBB-PERF-01) — no Linear-feel regression.
  - 0 new runtime services / 0 external origins (NFR-WEBB-INFRA-01) — no infra drift.
  - Boundary guard green (NFR-WEBB-BND-01) — web tier gains no DB pool.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| 1 | Git/PR diffs | file-path inspection on visual PRs | per PR | maintainer |
| 2 | Acceptance harness | external-origin request count on a no-egress host | per CI run | CI |
| 3 | `foundry-acceptance` | `[Summary]` passing count | per CI run | CI |
| 4 | `foundry-acceptance` | live-vs-reloaded structural-equality scenario | per CI run | CI |
| 5 | repo + acceptance | one vendored htmx file + version record; suite green | per CI run after US-B05 | maintainer/CI |

### Hypothesis
We believe that moving Foundry's board, issue+comments, and sign-in HTML into templates with
vendored htmx/Alpine/CSS, for contributors and self-hosting teams, will achieve a
product-grade, one-template-editable web UI.
We will know this is true when contributors make visual changes touching only templates (KPI
1), self-hosters see a styled screen offline (KPI 2), and the acceptance suite stays 100%
green throughout (KPI 3).

### Handoff to DEVOPS (platform-architect)
- **Instrument**: external-origin request count on page render (must be 0); render-latency
  bench harness (reuse backend-mvp NFR-PERF-01); acceptance passing-count trend.
- **Alerting thresholds**: any external origin on a rendered page = fail; P95 render >200 ms =
  fail; acceptance passing count drop = fail.
- **Baseline to capture before release**: current `format!`-path render latency (for the
  ≤200 ms no-regression comparison).
