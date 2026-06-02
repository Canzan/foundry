# Story Map: htmx Web Tier (Feature B — "Foundry looks like a product")

> Feature B of the web-tier-extraction split. Feature A (JSON API + the
> presentation-neutral `foundry_services` seam + the CI boundary guard) has SHIPPED.
> This map covers the deferred web-tier track: replace inline `format!()` HTML with a
> real template engine + vendored htmx2/Alpine/CSS, reusing the seam Feature A proved.
> Sliced as thin Elephant-Carpaccio slices: each slice ships an independently
> verifiable end-to-end behavior, NOT a technical layer.

## Personas
- **Contributor** (Jamal Okafor, Rust dev) — PRIMARY. Visual/wording changes become a
  one-template edit instead of editing `format!()` strings in handlers.
- **Self-hosting member** (Mei Chen, teammates) — PRIMARY. Sees the first styled screen;
  must perceive NO behavioral change except "it now looks styled".
- **Operator** (Devansh Rao) — one binary, one Postgres, no Node runtime, no CDN,
  air-gap friendly; screenshots Foundry for the team.

## End-to-End Goal
A self-hosted Foundry whose user-facing HTML (board, issue+comments, sign-in) is
rendered from **templates** with **vendored htmx2 + Alpine + CSS** served by the one
binary — looking like a finished product, keyboard-driven, reusing the
presentation-neutral `foundry_services` seam Feature A established, with the existing
acceptance suite staying green throughout, and the htmx-1->2 normalization completed.

---

## Oversize Assessment

Re-run for Feature B in isolation (the original web-tier-extraction oversize was
resolved by splitting A from B; this assesses B alone):

| Signal | Threshold | Feature B | Trips? |
|--------|-----------|-----------|--------|
| User-story count | >10 | 6 (US-B01..B06; B06 is `@infrastructure`, folded) | No |
| Bounded contexts / modules touched | >3 | 1 net-new templating concern in `foundry-app`/`foundry-web` over the existing seam | No |
| Walking-skeleton integration points | >5 | 3 (template engine ↔ existing services seam, static-asset pipeline, acceptance harness) | No |
| Estimated effort | >2 weeks | ~7-10 days across 4 slices | No |
| Independent shippable user outcomes | "multiple that could ship separately" | 1 coherent outcome (a product-grade web tier); slices are increments of it, not separable products | No |
| New auth/security surface | qualitative | none (browser auth unchanged; no new credential) | No |

**Verdict: PASS — right-sized.** 6 stories (1 infra, folded), 1 templating concern,
~7-10 days. No split needed. This is exactly the "Feature B" the web-tier-extraction
split (D8/D9) carved out as independently shippable. Each slice below is ≤ ~2-3 days
and end-to-end demonstrable.

---

## Backbone

The backbone is the **sequence of surfaces moved to templates + styled**, ordered so the
highest-traffic surface proves the templating + asset pipeline first, and the deferred
htmx-2 normalization lands last as a dedicated, low-risk slice.

| Style the board | Templatize issue+comments | Templatize sign-in | Normalize + upgrade htmx |
|-----------------|---------------------------|--------------------|--------------------------|
| board -> template + card partial | issue page + ONE comment-card partial | sign-in/forgot -> shared base layout | normalize hx-* directives; pin vendored htmx2 |
| vendor htmx/Alpine/CSS, no CDN | collapse 4 `format!` comment sites to 1 | preserve cookie/error/CSRF contract | leave data-* render markers untouched |
| static pipeline scaffolding (infra, folded) | preserve authz affordances + sanitization | extend `$LAYOUT_TEMPLATE` | green regression per hx-driven swap |

---

## Walking Skeleton (Slice 1)

The thinnest end-to-end slice that proves the feature's premise: **Foundry's
highest-traffic surface (the board) renders from a real template, styled from vendored
assets, with no external fetch, reusing the existing `foundry_services` seam, while the
acceptance suite stays green.**

| Backbone step | Story | Why it's in the skeleton |
|---------------|-------|--------------------------|
| Style the board | US-B01 (board -> template over the seam) | Without a template-rendered surface, there is no templating pipeline to build on. |
| Style the board | US-B02 (board from vendored htmx/Alpine/CSS, no CDN) | Without the static-asset pipeline, the board renders unstyled — the first-impression problem is unsolved. |
| (infra, folded) | US-B06 (static-asset pipeline scaffolding) | The mechanism US-B02 needs; never ships standalone. |

End-to-end demonstrable value: **Mei opens the Auth v2 board on an air-gapped VM and
sees a styled, interactive board — rendered by a template from data fetched through the
existing `foundry_services` seam, with htmx/Alpine/CSS loaded from the binary's own
`static/` path and zero external origins — while the full existing acceptance suite
stays green.**

> Slice 1 contains two user-visible stories (US-B01, US-B02), so it passes the
> slice-level Elevator-Pitch rule independent of the folded infra story (US-B06).

---

## Release Slices

Every slice contains at least one user-visible value story (no slice is all
`@infrastructure`). Slices are sliced by surface/outcome, each end-to-end.

### Slice 1 — Walking Skeleton: "The board is a styled, templated product surface"
- **Stories**: US-B01 (board -> template over the seam), US-B02 (vendored htmx/Alpine/CSS,
  no CDN), US-B06 (static-asset pipeline scaffolding, `@infrastructure`, folded in).
- **End-to-end value**: a styled board, template-rendered from the existing seam, assets
  vendored and served by the binary, 0 external origins, acceptance suite green.
- **Target jobs**: htmx-web-1 (board markup becomes a one-template edit), htmx-web-2
  (first styled screen), jtbd-outcome-4 (no latency regression).
- **Dependencies**: none (entry slice; reuses Feature A's shipped seam).
- **Learning hypothesis**: *A real template engine + a vendored static-asset pipeline can
  render the highest-traffic surface (the board) inside the ≤200 ms budget, fully offline,
  while keeping the substring-asserting acceptance tests green.* If the chosen engine
  blows the latency budget or the asset pipeline needs a runtime service, the whole
  "looks like a product without new infra" premise is at risk — so this is the riskiest
  assumption.

### Slice 2 — "Issue & comments read like a product, from one card partial"
- **Stories**: US-B03 (issue detail + comment thread -> templates; collapse the 4 `format!`
  comment-render sites into ONE comment-card partial).
- **End-to-end value**: the issue page and comment thread render from one comment-card
  partial across full-render / htmx-append / edit / cancel paths; authz affordances and
  markdown sanitization unchanged; the live-appended card is structurally identical to a
  reloaded one (fixing today's OOB-omits-buttons divergence).
- **Target jobs**: htmx-web-1 (one-template comment change), jtbd-outcome-4 (in-issue feel).
- **Dependencies**: Slice 1 (template engine + base layout + card-partial pattern exist).
- **Learning hypothesis**: *The four comment-render `format!` sites
  (`render_issue_page`, `render_comment_card`, `render_comment_card_oob`, the inline
  edit-form) can collapse into ONE partial without diverging the live-updated card from
  the reloaded card, while keeping sanitization/authz in the handler/core.*

### Slice 3 — "First impression: sign-in looks trustworthy"
- **Stories**: US-B04 (sign-in / forgot-password -> templates extending the shared base layout).
- **End-to-end value**: full-page auth screens render from the one base layout; cookie,
  CSRF, and non-enumerable-error contracts unchanged.
- **Target jobs**: htmx-web-2 (first-screen trust), jtbd-outcome-7 (template-driven upgrade ease).
- **Dependencies**: Slice 1 (base layout + static pipeline).
- **Learning hypothesis**: *Full-page templates share one base layout with the fragment
  surfaces, so CSS/asset consistency is automatic (Nielsen #4) without duplication, and
  the security-critical sign-in contracts (cookie attrs, non-enumerable error, CSRF)
  survive the move untouched.*

### Slice 4 — "htmx is consistent and ready to upgrade to 2"
- **Stories**: US-B05 (normalize htmx directives across templates; vendor + pin an htmx 2
  version; regression-test every hx-driven interaction).
- **End-to-end value**: all htmx directives use one consistent convention; htmx is
  vendored at a single pinned 2.x version; every existing hx-driven swap (create-card OOB,
  comment edit/delete/cancel, SSE fragment) still works; the data-* render-contract
  markers are left untouched.
- **Target jobs**: htmx-web-3 (low-risk htmx-2 upgrade), jtbd-outcome-7 (upgrade friction).
- **Dependencies**: Slices 1-3 (all surfaces templated, so there is one place per
  directive to normalize). **This is sequenced as a DEDICATED normalization slice AFTER
  the surfaces are templated — see "Decision: htmx-2 sequencing" below.**
- **Learning hypothesis**: *Once every surface is templated, the htmx directive set is
  small and centralized enough that normalizing it and bumping to htmx 2 is a single
  controlled change with a green regression scenario per interaction — and the version
  pin lands cleanly because assets are already vendored.*

---

## Decision: htmx-2 sequencing — dedicated slice, not per-surface

The brief asks: is the htmx 1->2 migration per-surface as each is templated, or a
dedicated normalization slice? **Recommendation: a dedicated slice (Slice 4), NOT
per-surface.** Rationale, grounded in the code:

- The htmx-2 surface is **small and centralized after templating**: the active directives
  are a handful of bare `hx-*` attributes (`hx-patch`/`hx-get`/`hx-target`/`hx-swap`/
  `hx-delete` in the comment edit-form; `hx-swap-oob` in the create card and comment OOB).
  Once Slices 1-3 move these into templates, they live in a few partials, not scattered
  across handlers.
- Doing the version bump **per-surface would mean N partial upgrades with N regression
  passes** and a window where surfaces run mixed htmx versions — higher risk, not lower.
- A dedicated slice lets the htmx-2 bump be **one atomic, fully regression-tested change**
  with the version pinned once, after the directives are already consistent.
- Slices 1-3 must therefore **preserve existing htmx behavior as-is** (no version bump,
  no directive rewrite beyond moving the same attributes into templates). The data-*
  scraper markers (`data-hx-fragment`, `data-comment-list`, `data-column`,
  `data-issue-key`) are render-contract, NOT htmx directives, and are left byte-stable in
  every slice including Slice 4.

> OPEN for the user/DESIGN: the htmx VERSION (specific 2.x pin) remains a DESIGN decision
> (carried from web-tier-extraction D3). Slice 4 establishes the requirement and the
> regression net; DESIGN picks the version.

---

## Slice-Level Validation Checklist

Per the reviewer Dimension 0 slice-level rule: every released slice must contain at least
one user-visible value story (not entirely `@infrastructure`).

| Released slice | User-visible stories | Infra-only stories | Pass? |
|----------------|----------------------|--------------------|-------|
| 1 — Styled board | US-B01, US-B02 (2) | US-B06 (1, folded in) | YES |
| 2 — Issue & comments | US-B03 (1) | 0 | YES |
| 3 — Sign-in | US-B04 (1) | 0 | YES |
| 4 — htmx normalize/upgrade | US-B05 (1) | 0 | YES |

US-B06 (static-asset pipeline scaffolding) never ships as a standalone release; it rides
Slice 1. No released slice is all-infrastructure. No re-slicing required.

---

## Priority Rationale

Order: **Slice 1 (styled board) -> Slice 2 (issue/comments) -> Slice 3 (sign-in) ->
Slice 4 (htmx normalize/upgrade)**.

1. **Slice 1 first (Walking Skeleton — styled board)** — validates the riskiest
   assumption (a template engine + vendored asset pipeline can render the hottest surface
   in budget, offline, suite green) on the highest-traffic surface, and delivers BOTH
   primary jobs' first payoff (a one-template board edit for the contributor; a styled
   first screen for the self-hoster). Tie-break: Walking Skeleton beats
   riskiest-assumption beats highest-value (Maurya).
2. **Slice 2 second (issue & comments)** — highest-value contributor outcome (htmx-web-1):
   the most tangled surface (four `format!` comment-render sites) collapses to one partial,
   and it fixes a real (small) UX divergence (live card vs reloaded card). Higher
   value/complexity than sign-in, so it precedes it.
3. **Slice 3 third (sign-in)** — first-impression trust (htmx-web-2) but lowest
   extraction risk (full-page, no fragment swaps), so it trails the harder fragment
   surfaces. It also establishes the shared base layout cleanly for any later full-page
   surfaces.
4. **Slice 4 last (htmx normalize + upgrade)** — supporting job (htmx-web-3). Must come
   last because it needs every surface already templated so the directive set is
   centralized; doing it earlier would fragment the upgrade across surfaces (see the
   sequencing decision above).

Value × Urgency / Effort intuition: Slice 1 high-value/high-urgency (de-risks the
templating + asset premise) -> P1. Slice 2 high-value/medium-effort -> P2. Slice 3
medium-value/low-effort -> P3. Slice 4 medium-value/low-effort but dependency-gated last
-> P4.

---

## Notes on Story Granularity

- 6 stories total: US-B01..B05 (user-visible value), US-B06 (`@infrastructure`, folded
  into Slice 1).
- All stories are individually right-sized (S/M; ≤3 days, 3-6 scenarios).
- No story spans multiple slices.
- US-B06 is the only `@infrastructure` story and is explicitly NOT a standalone slice.
- Per-slice briefs live in `docs/feature/htmx-web-tier/slices/`.
- Story IDs use the `US-B0x` namespace to distinguish Feature B from web-tier-extraction's
  `US-W0x` (Feature A).
