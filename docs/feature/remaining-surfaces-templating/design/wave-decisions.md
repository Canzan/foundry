# Remaining-Surfaces Templating — DESIGN Wave Decisions

Owner: solution-architect (Morgan). Scope: the **deferred move-only follow-up to Feature B**
(`htmx-web-tier`), stories US-R01..US-R06. Interaction mode: **Propose** (inherit-only — near-trivial
DESIGN). LEGACY per-feature layout under `docs/feature/remaining-surfaces-templating/design/`. Companion
documents: `architecture.md`, `render-contract.md`.

## Headline decision

> **This wave INHERITS Feature B's ADR-B01..B07 unchanged. It adds ONLY template files +
> view-model structs — NO new architecture, NO new dependency, NO new infrastructure, NO new external
> integration.** The deliverable is the last `format!()` render sites moved into `.html` templates
> (full pages extend the existing `base.html`; htmx fragments stay bare), driven by new
> `foundry-app::views` structs, keeping the existing acceptance suite green.

## Decisions table (DDD-numbered, this wave)

| # | Decision | Rationale |
|---|---|---|
| DR1 | **Inherit Feature B ADR-B01..B07 wholesale.** Askama 0.12 engine, ONE `base.html`, `views.rs` typed view-models, vendored `/static` assets + `ServeDir`, selector-and-substring-identical render contract, content-hash cache-busting, htmx-2 pin — all unchanged. | This is a move-only follow-up; Feature B already made and ratified every architectural decision. Re-deciding would be waste (Principle 8). |
| DR2 | **The shared `invalid_page` helper → ONE shared `invalid_page.html` template + `views::InvalidPage { heading, message }`.** | `bootstrap.rs::invalid_page` (`:356`) is reused across 7 modules (~17 call sites). One template restyles every not-found/error path at once — highest leverage in the feature (US-R06). The status code stays a handler argument (no DB/authz in template). |
| DR3 | **Per-surface full-page-vs-fragment classification** (table in `architecture.md`). Full pages extend `base.html`; htmx fragments (modals, error divs, OOB rows, state `<span>`) stay BARE. | Inherited Feature B rule (NFR-WEBB-COMPAT-02): extending base on a fragment double-wraps the swap. Verified each surface's current shape in code (full-page surfaces today emit a bare `<!doctype><head>`; fragments emit bare markup). |
| DR4 | **New-issue modal + attachment row use the one-partial rule** (NFR-WEBB-MAINT-02): `partials/new_issue_modal.html` (fragment + full-page fallback both `{% include %}` it); `partials/attachment_row.html` (full list + OOB wrapper both include it). | Generalizes Feature B's `issue_card`/`comment_card` discipline; one definition per repeated component. |
| DR5 | **Reuse ONE shared `error_fragment.html`** for the `project-create-error` (US-R01), `issue-create-error` (US-R03), and `attachment-upload-error` (US-R05) divs, parameterized by `fragment_marker` + `message`. | All three are `<div class="error" data-hx-fragment="…">{msg}</div>` (verified `projects.rs:499`, `issues.rs:253`, `attachments.rs:369`); one template + `views::ErrorFragment` covers them. Each marker stays byte-stable. |
| DR6 | **Keyboard search-results + keyboard-help overlays = OPTIONAL fold-in into US-R02; defer if not cheap.** | Same module/pattern as the modal (lowest marginal risk if folded; zero risk if deferred). They are bare fragments either way. Decided per stories.md US-R02 Technical Notes. Default: fold in if mechanical, else defer to a trivial follow-up. |
| DR7 | **No htmx version bump; no new asset; no new probe; no `cargo deny` re-run required.** | The htmx-2 migration was Feature B's Slice 4 (done). The only active `hx-*` here (attachment OOB, state chip) move AS-IS on the pinned htmx 2.x (nfrs.md §"NOT in scope"). No dependency changes → no `deny.toml` impact. New templates are compile-checked by Askama for free. |

## Reuse Analysis tally (full table in `architecture.md`)

- **EXTEND = 11**: template engine, base layout, view-model module, static assets/route, render
  contract, one-partial rule, data fetch (foundry-services), CSRF contract (untouched), session/auth
  flow (untouched), sanitization/authz (core/handler), asset/template probes — **all reused from
  Feature B unchanged**.
- **CREATE NEW = 2**, and both are the feature's *deliverable*, not architecture: (1) the **new
  template files** (project_create, new_issue_modal[+page], state_chip, dashboard_root,
  events_signin_required, attachment_row[+oob], payload_too_large, bootstrap_{dashboard,claim,invite},
  shared invalid_page, shared error_fragment) — a *move* of markup out of Rust, no new behavior;
  (2) the **new view-model structs** added to the existing `views` module.
- **Genuinely-new components flagged:** NONE beyond template files + view structs. **Zero new
  engines, zero new infrastructure, zero new dependencies.**

## Technology stack — UNCHANGED (cite Feature B)

No change to Feature B's stack (`docs/feature/htmx-web-tier/design/wave-decisions.md` §"Technology
stack"). For the record, the load-bearing entries this feature touches:

| Concern | Choice | Version | New dep? |
|---|---|---|---|
| Template engine | `askama` + `askama_axum` | 0.12 (Feature B) | **no — already wired** |
| Static serving | `tower-http` (`fs`) | 0.6 (Feature B) | no |
| Vendored htmx / Alpine | pinned blobs | Feature B pins | no |
| CSS | hand-written `foundry.css` | Feature B | no |
| HTTP / routing | `axum` | 0.8 | no |

**Net new runtime dependencies introduced by this feature: ZERO.**

## Constraints honored (NFR traceability — all inherited)

- **Browser auth / CSRF / sessions UNCHANGED** (NFR-WEBB-COMPAT-03/04/05, DB7): `_csrf` field emitted
  where present today, `/bootstrap` CSRF exemption preserved (claim form has no `_csrf` — reproduce
  as-is), signed-out 303 redirect (US-R04) and 401 (events) and all status codes (200/400/401/413/
  SEE_OTHER) move byte-for-behavior. Only markup moves.
- **Selector-and-substring-identical** (NFR-WEBB-COMPAT-01/02): every `data-hx-fragment`,
  `data-modal`, `data-state`, `data-filename`, `hx-swap-oob` target, and error copy byte-stable; no
  existing scenario edited; suite passing count must not drop.
- **One binary, no JS toolchain, no CDN, no new service** (NFR-WEBB-BND-04, INFRA-01): reuse the
  existing vendored `/static`; add nothing. `docker compose` topology unchanged.
- **No DB in the render path** (NFR-WEBB-BND-01); sanitization/authz stay in core/handler
  (NFR-WEBB-BND-03).
- **≤200 ms P95, no regression** (NFR-WEBB-PERF-01): Askama compiled-in; these surfaces are small.
- **Fragments bare, full pages extend `base.html`** (the only render-shape rule).
- **Maintainability** (NFR-WEBB-MAINT-01/02): on-screen markup lands in `templates/`; one-partial rule
  for the modal + attachment row; one shared `invalid_page` + one shared `error_fragment`.
- Default architecture preserved: modular monolith with dependency inversion; the web tier is a
  driving adapter rendering over `foundry-services`. No crate split (Feature B DD9 stands).

## Priority validation (reviewer Dimension 5)

- **Q1 largest bottleneck?** The job is htmx-web-1/2 applied to the last deferred surfaces (the
  highest-visibility being `dashboard_root` `/` — the first post-sign-in screen — and the
  high-leverage shared `invalid_page`). The design leads with those. **YES.**
- **Q2 simpler alternatives?** This IS the simplest path — pure inheritance, no new decisions. The
  only sub-choices (shared error fragment DR5, shared invalid_page DR2, optional overlay fold-in DR6)
  are documented. **ADEQUATE.**
- **Q3 constraint prioritization?** The one real risk — surfaces with weak/no acceptance coverage —
  is identified up front and routed to DISTILL (render-contract.md §coverage). Not inverted.
- **Q4 data-justified?** Every surface→template mapping is grounded in verified file:line evidence
  and confirmed acceptance coverage. **JUSTIFIED.**

## Open decisions

Effectively none — this is inherit-only. Two trivial sub-choices left to the crafter/DISTILL, neither
architectural:

1. **DR6 — keyboard search/help overlays in US-R02:** fold in if the move is mechanical; otherwise
   defer to a trivial follow-up. Either way: bare fragments, same pattern. (Recommendation: fold in.)
2. **DR5 vs per-surface error templates:** the design recommends ONE shared `error_fragment.html`
   (parameterized by marker + message); a crafter may instead keep three tiny per-surface templates if
   that reads cleaner — a file-organization choice, not architecture. Marker byte-stability is the only
   hard constraint.

Both are non-blocking. There are **zero open architectural questions** and **zero new dependencies**
for the user to ratify — Feature B's ratified ADR-B01..B07 govern this feature entirely.
