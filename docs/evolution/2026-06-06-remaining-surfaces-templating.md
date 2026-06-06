# Evolution — remaining-surfaces-templating (Feature B follow-up)

**Finalized**: 2026-06-06
**Ship commit**: [e3b7af1](../../) — tip of a 7-step run off `a5bdcb2` (roadmap) / `934f316` (first code)
**Wave coverage**: full nWave pipeline, fast-forwarded (DISCUSS → DESIGN → DISTILL → DELIVER end-to-end). Legacy per-feature doc layout; trunk-based (committed directly to `main`).

## Feature summary

The deferred follow-up to Feature B (`htmx-web-tier`). Templatizes Foundry's **remaining inline-`format!()` HTML surfaces** into Askama templates extending the existing `base.html`, reusing Feature B's render pattern, vendored assets, and **selector-and-substring-identical** contract. A pure **move-only refactor** — no behavior change, no new logic, no new dependencies — that finishes the job Feature B started.

North-star outcome: **inline full-page `format!()` HTML sites: 9 → 0**, enforced by a permanent source-tree completion guard.

## What shipped

14 new Askama templates + their view-models in `foundry-app`, replacing inline `format!` across 6 modules:

| Surface | Template(s) | Shape |
|---|---|---|
| Project-create form + error | `project_create.html` + shared `error_fragment.html` | page + bare fragment |
| New-issue modal | `partials/new_issue_modal.html` (one partial) + `new_issue_modal_page.html` | fragment + no-JS page |
| Issue-create error + state chip | reuse `error_fragment.html` + `partials/state_chip.html` | bare fragments |
| Dashboard landing `/` + events 401 | `dashboard_root.html`, `events_signin_required.html` | pages |
| Shared not-found/error | `invalid_page.html` (rewired ~17 callers of `bootstrap.rs::invalid_page`) | page |
| Attachments | `partials/attachment_row.html` + OOB wrapper, `payload_too_large.html` | fragment + 413 page |
| Bootstrap | `bootstrap_dashboard.html`, `bootstrap_claim.html`, `bootstrap_invite.html` | pages |

Three **shared** templates do the heavy lifting: `error_fragment.html` (parameterized by marker — reused by project/issue/attachment errors), `invalid_page.html` (one styled not-found reused app-wide via the `invalid_page` helper), and the one-partial OOB pattern (attachment row, mirroring Feature B's comment card).

### Key decisions

- **Inherits Feature B wholesale** (DESIGN was inherit-only): Askama 0.12, `base.html`, `views.rs` typed view-models, content-hashed `/static` assets, the selector-and-substring-identical render contract. **Zero new architecture, dependencies, or infrastructure** — the deliverables are template files + field-holder view structs.
- **Move-only**: full pages extend `base.html` + link `/static`; htmx fragments stay bare. Control-flow/status contracts preserved exactly — signed-out `/` still 303s, events still 401s, payload-too-large still 413s, bootstrap claim/invite stay CSRF-exempt, the signed invite URL stays byte-stable.
- **Completion guard** (US-R07): a source-tree test (`feature_remaining_surfaces::inline_full_page_sites()`) scans `foundry-app/src/*.rs` and fails if any bare-`<head>` inline full-page `format!` document is (re)introduced. 0 sites at ship.

## How it was built (DELIVER, fast-forwarded)

7 DES-monitored TDD steps, each a `@real-io` cucumber scenario driven to green, each removing inline sites (9→8→8→8→5→4→0):

| Step | Surface | Greens |
|---|---|---|
| 01-01 | project-create form + shared error_fragment (walking skeleton) | us-r01 |
| 02-01 | new-issue modal (one partial + no-JS page) | us-r02 |
| 03-01 | issue-create error (reuse) + state chip | us-r03 |
| 04-01 | dashboard `/` + events 401 + shared invalid_page | us-r04 |
| 05-01 | attachment OOB row + upload-error + 413 + not-found | us-r05 |
| 06-01 | bootstrap pages + rewire invalid_page (~17 callers) | us-r06 |
| 06-02 | completion guard (0 inline sites) | us-r07 |

## Quality at ship

- **Acceptance (`@all` lane)**: 183/183 scenarios, 1541/1541 steps green — every new us-r0* scenario plus the entire existing suite (the regression net that proves the move is selector-identical). The pg16-client fix from the prior session means `@all` runs clean locally.
- **Build/lint**: `cargo build --workspace --tests`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo deny` all clean.
- **Review (proportionate to a move-only feature)**: XSS surface verified clean — the one new `|safe` is on a server-constructed signed invite URL (`state.public_url` + signed token, no user input), byte-identical to prior behavior; user input (`raw_name`/`raw_key`) auto-escapes; the comment-card `|safe` is the pre-existing core-sanitized field. The pattern itself was adversarially reviewed in Feature B.
- **Mutation**: N/A — no new business logic (templates + field-holder view-models + render-switch plumbing); the per-feature mutation surface is empty. Logic-bearing predicates were covered in Features A/B.
- **Completion guard**: 0 inline full-page sites; the guard is a live regression net against reintroduction.

## Residuals / follow-ups

- The optional `keyboard.rs` search-fragment + keyboard-help overlay were noted by DISCUSS as a lowest-risk tail; fold into a future small pass if ever wanted (they were not in the ratified US-R01..R06 scope).
- Visual redesign / mobile / theming remain out of scope (this was a move, not a restyle) — the now-complete template tier makes them straightforward later.

## Pointers

- Spec: `docs/feature/remaining-surfaces-templating/{discuss,design,distill}/`
- DES roadmap + log: `docs/feature/remaining-surfaces-templating/deliver/`
- Completion guard: `crates/foundry-acceptance/src/steps/feature_remaining_surfaces.rs::inline_full_page_sites()`
- Templates: `crates/foundry-app/templates/`
