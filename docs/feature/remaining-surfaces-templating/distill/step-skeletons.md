# DISTILL Step Skeletons — Remaining-Surfaces Templating

Owner: acceptance-designer (Sentinel). The precise DELIVER wiring each RED
scenario drives toward: which template + view-model moves each surface out of its
inline `format!()`. This is a **move-only** refactor — no new behavior. Mirrors
`docs/feature/htmx-web-tier/distill/step-skeletons.md`.

Source of the surface→template map: DESIGN `architecture.md` §"Surface → template
/ view-model map". Source of byte-stable markers: DESIGN `render-contract.md`.

## Scaffolding posture (minimal, per Feature B precedent)

NO Rust scaffold stubs are created. Per Feature B's DISTILL discipline, RED comes
from MISSING_FUNCTIONALITY observable over real HTTP — a full page that does not
yet link `/static` because it is still a bare-`<head>` `format!()` — NOT from a
missing module. The workspace compiles and the existing suite stays green; the
Askama templates + view-model structs are authored in DELIVER. Authoring them in
DISTILL would either (a) make the scenarios GREEN prematurely, or (b) break the
build with half-wired templates. Both violate the "keep the suite green, defer the
wiring" rule. Askama type-checks every new `#[derive(Template)]` at `cargo build`,
so a missing template/field is a compile error in DELIVER — the strongest posture.

## Per-surface DELIVER wiring

Each row: move the inline `format!()` at `file:line` into the named template
(driven by the named `views.rs` view-model), delete the `format!()`, keep the
handler computing the same values. Full pages `{% extends "base.html" %}` (which
adds the `/static` `<link>` the bare `<head>` lacks); fragments stay BARE.

### US-R01 — project-create (Slice 1, Walking Skeleton)
| Surface | file:line | Template | View-model | Shape |
|---|---|---|---|---|
| create form | `projects.rs::render_create_form` :466 | `project_create.html` (extends `base.html`) | `ProjectCreatePage { team_name, action, csrf, error, raw_name, raw_key }` | full page |
| error fragment | `projects.rs::render_error_fragment` :499 | shared `error_fragment.html` (or `partials/errors/project_create_error.html`) | `views::ErrorFragment { fragment_marker="project-create-error", message }` | bare fragment |

RED flip: the form links the content-hashed `/static` CSS via base. Keep
`method=post`, `action="/team/{slug}/projects"`, hidden `_csrf`, `input[name=name]`,
`input[name=key_prefix]`. Keep `data-hx-fragment="project-create-error"` byte-stable.

### US-R02 — new-issue modal (Slice 2)
| Surface | file:line | Template | View-model | Shape |
|---|---|---|---|---|
| modal fragment | `keyboard.rs::render_modal_fragment` :108 | `partials/new_issue_modal.html` (the ONE partial) | `NewIssueModal { action, csrf, project_name }` | bare fragment |
| full-page fallback | `keyboard.rs::render_modal_full_page` :124 | `new_issue_modal_page.html` (extends `base.html`; `{% include %}` the partial) | `NewIssueModalPage { action, csrf, project_name, team_slug }` | full page |

RED flip: the no-JS full page links `/static` via base AND `{% include %}`s the same
partial. Keep `data-modal="new-issue"`, `role="dialog"`, `aria-modal`, `_csrf`,
`input[name=title][autofocus]`. One-partial rule (NFR-WEBB-MAINT-02).

### US-R03 — issue-create error + state chip (Slice 3)
| Surface | file:line | Template | View-model | Shape |
|---|---|---|---|---|
| create error | `issues.rs::bad_request_fragment` :253 | shared `error_fragment.html` (reuse) | `views::ErrorFragment { fragment_marker="issue-create-error", message }` | bare fragment |
| state chip | `issues.rs` state `<span>` :147 | `partials/state_chip.html` | `StateChip { normalized }` | bare fragment |

RED flip: none — both are GREEN guards today (markers/copy already correct). The
move must PRESERVE: `data-hx-fragment="issue-create-error"` + literal "Title is
required"; `<span class="state" data-state="{normalized}">` (note the normalized
value uses an underscore, e.g. `in_progress`).

### US-R04 — landing + events page (Slice 4)
| Surface | file:line | Template | View-model | Shape |
|---|---|---|---|---|
| dashboard landing | `signin.rs::dashboard_root` signed-in body :243 | `dashboard_root.html` (extends `base.html`) | `DashboardRoot {}` | full page |
| events 401 page | `events.rs::unauthorized_response` :138 | `events_signin_required.html` (extends `base.html`) | `EventsSigninRequired {}` | full page |

RED flip: both link `/static` via base. UNCHANGED handler control flow: the
signed-out `/` branch keeps `303 SEE_OTHER → /sign-in` with empty body; the events
page keeps its `401` status, the "Sign-in required…" copy, and the `/sign-in` link.

### US-R05 — attachment surfaces (Slice 5)
| Surface | file:line | Template | View-model | Shape |
|---|---|---|---|---|
| OOB row | `attachments.rs::render_attachment_row_oob` :385 | `partials/attachment_row.html` + `partials/oob/attachment_row_oob.html` wrapper | `AttachmentRow { filename, size_label }` | bare fragment |
| upload error | `attachments.rs::bad_request_fragment` :369 | shared `error_fragment.html` (reuse) | `views::ErrorFragment { fragment_marker="attachment-upload-error", message }` | bare fragment |
| 413 too-large | `attachments.rs::payload_too_large` :353 | `payload_too_large.html` (extends `base.html`) | `PayloadTooLarge { limit_mb }` | full page |
| not-found | `attachments.rs::not_found_page` :349 | shared `invalid_page.html` (already delegates) | `views::InvalidPage` | full page |

RED flip: the 413 page links `/static` via base. PRESERVE: `413` status +
"Upload too large" copy; `data-hx-fragment="attachment-upload-error"`;
`hx-swap-oob="beforeend:[data-attachment-list]"` + `<li class="attachment"
data-filename>` (regression-net-only, `us_11`). The missing-file path is the
`UploadError::Missing` → `bad_request_fragment("Upload is missing a file part")`
case; the over-limit path is `UploadError::TooLarge` → `payload_too_large`.

### US-R06 — bootstrap + claim + invite + shared invalid_page (Slice 6)
| Surface | file:line | Template | View-model | Shape |
|---|---|---|---|---|
| bootstrap dashboard | `bootstrap.rs::dashboard` :205 | `bootstrap_dashboard.html` (extends `base.html`) | `BootstrapDashboard { signed_in }` | full page |
| claim form | `bootstrap.rs::render_claim_form` :338 | `bootstrap_claim.html` (extends `base.html`) | `BootstrapClaim { token }` | full page |
| invite-link page | `bootstrap.rs::create_invite` body :286 | `bootstrap_invite.html` (extends `base.html`) | `BootstrapInvite { invite_url }` | full page |
| shared invalid_page | `bootstrap.rs::invalid_page` :356 (~17 callers) | `invalid_page.html` (extends `base.html`) | `views::InvalidPage { heading, message }` | full page |

RED flip: bootstrap dashboard + the shared invalid_page link `/static` via base.
PRESERVE: "Workspace dashboard" copy; the `<h1>{heading}</h1><p>{message}</p>`
invalid_page shape; the claim form has NO `_csrf` field (`/bootstrap` is
CSRF-exempt — reproduce as-is); the `/bootstrap?token=…` action; the signed invite
URL. The shared invalid_page move restyles every not-found/error path at once.

### Shared templates introduced (DESIGN DR2 + DR5)
- `error_fragment.html` — `<div class="error" data-hx-fragment="{{ fragment_marker }}">{{ message }}</div>`,
  reused by US-R01 / US-R03 / US-R05. A crafter MAY instead keep three tiny
  per-surface error templates; marker byte-stability is the only hard constraint.
- `invalid_page.html` — `<h1>{{ heading }}</h1><p>{{ message }}</p>` extending
  `base.html`, reused across ~7 modules / ~17 call sites (US-R06, high leverage).

### Optional tail (DR6 — fold-in or defer)
`keyboard.rs::render_search_fragment` (:226) → `partials/search_results.html`;
`keyboard.rs::show_keyboard_help` (:248) → `partials/keyboard_help.html`. Both bare
fragments, same pattern. No DISTILL scenario authored (out-of-scope tail);
the completion guard (US-R07) does NOT count them (they are fragments, no `<head>`).

## Completion guard (US-R07)

`feature_remaining_surfaces::inline_full_page_sites()` scans `foundry-app/src/*.rs`
for `<!doctype` string literals (the unambiguous tell of an inline FULL PAGE; bare
fragments have no `<head>`). It currently reports **9** sites:
`events.rs:142, signin.rs:243, bootstrap.rs:213, bootstrap.rs:285, bootstrap.rs:341,
bootstrap.rs:358, keyboard.rs:132, projects.rs:479, attachments.rs:355`. The guard
flips GREEN when DELIVER has moved every full-page surface into a template — i.e.
the north-star KPI (0 inline `format!()` full pages) is met. It is the LAST guard
to go green.
