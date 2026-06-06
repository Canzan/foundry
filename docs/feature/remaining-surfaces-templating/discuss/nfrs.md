# Remaining-Surfaces Templating — Non-Functional Requirements

> This feature INHERITS Feature B's NFRs wholesale. It is the same move-only
> refactor (inline `format!()` → Askama templates extending the EXISTING
> `base.html`, linking the EXISTING `/static` assets) applied to the surfaces
> Feature B deferred. We CITE Feature B's NFRs rather than re-derive them. The
> binding regression net is the existing `foundry-acceptance` suite.
>
> **Source of truth**: `stories.md` for functional behavior; this file for NFRs
> (by reference); `out-of-scope.md` for deferred items. NFR ids reuse Feature B's
> `WEBB` namespace because they are the SAME requirements — see
> `docs/feature/htmx-web-tier/discuss/nfrs.md` for full text.

## Inherited NFRs (cited — apply unchanged to the remaining surfaces)

| NFR | What it requires (see Feature B nfrs.md) | Applies to (this feature) |
|-----|------------------------------------------|---------------------------|
| **NFR-WEBB-BND-01** | Web/template tier gains no DB access; templates render data already fetched via core/services. | All stories — the remaining surfaces already fetch in the handler; templates only render. |
| **NFR-WEBB-BND-03** | Sanitization/authz stay in core/handler, not templates. | US-R06 (invite token, signed-in flag computed in handler). |
| **NFR-WEBB-BND-04** | One binary, in-process, no new service, no Node, no CDN. | All stories — reuse the existing vendored `/static`; no new infra. |
| **NFR-WEBB-PERF-01** | P95 render ≤200 ms, no regression vs `format!`. | All stories — Askama compiled-in, parity with `format!`; these surfaces are small. |
| **NFR-WEBB-PERF-03** | Static assets cacheable, served locally, no external origin. | All stories — reuse Feature B's already-vendored, already-cached assets. |
| **NFR-WEBB-COMPAT-01** | Existing acceptance scenarios stay green; `[Summary]` passing count does not drop. | All stories — the binding regression net. |
| **NFR-WEBB-COMPAT-02** | Render contract preserved: asserted CSS-selectable elements, `data-*` markers, `hx-*` directives, literal copy reproduced; whitespace/attr-order free. | All stories — every `data-hx-fragment`, `data-modal`, `data-state`, `hx-swap-oob` target, and error copy is byte-stable. |
| **NFR-WEBB-COMPAT-03** | CSRF contract unchanged (`_csrf` field, `/bootstrap` exemption, header path, 403). | US-R01, US-R02, US-R06 (forms with `_csrf`; `/bootstrap` exemption). |
| **NFR-WEBB-COMPAT-04** | Session contract unchanged. | US-R04, US-R06 (signed-in detection, redirects). |
| **NFR-WEBB-A11Y-01** | Keyboard operability preserved (the `c`-to-create modal, focus). | US-R02 (new-issue modal stays keyboard-reachable). |
| **NFR-WEBB-A11Y-02** | Semantic HTML, labelled inputs, contrast ≥4.5:1, targets ≥24px. | US-R01 (form labels), US-R02 (modal), US-R06 (claim form). |
| **NFR-WEBB-MAINT-01** | On-screen markup lives in templates, not handler `format!()`; full pages extend ONE base layout. | All stories — this is the feature's whole point; greps for on-screen text land in `templates/`. |
| **NFR-WEBB-MAINT-02** | One partial per repeated component. | US-R02 (new-issue modal: one partial, fragment + full-page paths), US-R05 (attachment row: one partial, full + OOB paths), US-R06 (one shared `invalid_page`). |
| **NFR-WEBB-INFRA-01** | No new runtime services/deps; assets vendored + served by the binary. | All stories — reuse existing assets; add no dependency. |

## Feature-specific clarifications (not new NFRs — scoping of the inherited ones)

### Render-contract markers preserved by this feature (the byte-stable set)
The remaining surfaces carry these `data-*` markers, `hx-*` directives, and copy
that the suite (or Alpine) reads. They MUST be reproduced byte-stable (the
selector-and-substring-identical contract, Feature B render-contract.md):
- `data-hx-fragment="project-create-error"` (US-R01)
- `data-modal="new-issue"`, `role="dialog"`, `aria-modal="true"` (US-R02)
- `data-hx-fragment="issue-create-error"` + copy "Title is required"; `class="state" data-state="{state}"` (US-R03)
- the events "sign-in required" copy + `/sign-in` link; the signed-in landing copy (US-R04)
- `data-hx-fragment="attachment-upload-error"`; `hx-swap-oob="beforeend:[data-attachment-list]"`; `<li class="attachment" data-filename>` (US-R05)
- the `_csrf` field + `/bootstrap?token=…` action; the signed invite URL; the `invalid_page` heading/message structure (US-R06)

### Fragment vs full-page (the only render-shape rule)
Inherited from Feature B: htmx fragments (modals, error divs, OOB rows, state
`<span>`) emit BARE fragments and must NOT extend `base.html`; only FULL pages
extend `base.html`. Violating this double-wraps the swap. Enforced per story AC.

## What is NOT in scope here (vs Feature B's NFRs)
- **htmx-2 normalization (NFR around htmx-web-3 / US-B05)** is NOT carried — it was
  Feature B's dedicated slice and is done there. The remaining surfaces carry only
  the attachment OOB swap and the state-change fragment as active `hx-*`; those
  move AS-IS, no version bump (already on Feature B's pinned htmx).
