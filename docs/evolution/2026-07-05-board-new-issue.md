# Evolution — board-new-issue (wiring the inert "New issue" button)

**Finalized**: 2026-07-05
**DELIVER commits**: `b38c6ff` (wiring + board-slug view-model) → `3e86b7b` (modal overlay styling). Prior
waves: DISCUSS `a78be20`, DISTILL `99e6913`, scope correction `6d91b5a`. Committed directly to `main`
(trunk-based, no PRs). Repo legacy multi-file convention (no SSOT); DES step-monitoring exempt (lean mode).
Feature directory PRESERVED.
**Wave coverage**: DISCUSS (DoR passed, 1 story → 1 slice) → DISTILL (5-scenario acceptance SSOT + browser-
dogfood checklist) → DELIVER (1 slice + a follow-up styling pass). DESIGN skipped (client wiring on shipped
seams — the requirements seam table + D1–D5 stood in).
**Scope**: the project board's "New issue" button rendered but was inert (live-verified: click and `c` fired no
request). The create backend was complete and tested (`us-08`, `us-12`); only the **client wiring** was
missing. This feature wires it — button-only (the `c`/`Esc` keyboard layer stays deferred).

## Milestone — you can file an issue from the board

Clicking "New issue" now opens a centered modal, and filing a title drops the card into Backlog without a
full-page reload — the board finally captures work the moment you think of it, the JTBD the empty-state's
"press `c`" hint had only promised.

## What shipped (all client wiring + one view-model surfacing — no new backend logic)

- **`board.html`** — the button gains `hx-get="/team/{{ team_slug }}/project/{{ project_slug }}/issues/new"`
  + `hx-target="#modal-root"` + `hx-swap="innerHTML"`; a `<div id="modal-root"></div>` container was added.
- **`partials/new_issue_modal.html`** — the form gains `hx-post="{{ action }}"` + target/swap (submit becomes
  an htmx request), keeping `method="post"`/`action`/the hidden `_csrf` (the no-JS fallback, D4); its content
  is wrapped in a `.modal-dialog` card.
- **`BoardPage` view-model (`views.rs`) + `build_board_page` (`projects.rs`)** — the ONE `src/` change: added
  `team_slug` + `project_slug`, populated via the existing `slugify(team_name)` / `slugify(project.name)`
  (the project slug is derived the same way `submit_create` stores it). Fixed one unit-test call site. No new
  logic, no migration. (Scope correction — see below.)
- **Modal styling** — `.modal` full-viewport fixed backdrop scrim + flex-centering; `.modal-dialog` white card
  (shadow, radius); styled header/input/Create button. Added to the vendored stylesheet with a content-hash
  bump `foundry.386eb83b.css` → `foundry.4c43c2a8.css` (D3; `base.html` updated, old file removed).

The whole create path is REUSED unchanged: `GET …/issues/new` (modal fragment), `POST …/issues`
(`submit_create`) which on an htmx request returns the **OOB card** appending to `[data-column='backlog']`
(`issues.rs:293`) — so the new card lands in Backlog and the modal closes (its `#modal-root` target empties)
with no extra JS. On a plain (no-JS) POST it redirects to the board (fallback preserved).

## Scope correction (back-propagation)

DISCUSS/DISTILL asserted "template-only, zero backend." DELIVER found `BoardPage` exposes no slugs and no
robust template-only URL could be built (relative `issues/new` resolves wrong on the no-trailing-slash board
path; `name|lower` breaks on multi-word names). D5 was revised to a **near-zero** backend change (2 view-model
fields surfacing existing data); user-authorized 2026-07-05. Recorded in `requirements.md` § Changed
Assumptions + `wave-decisions.md` D5.

## Decisions realized (D1–D6)

| # | Decision | Status |
|---|---|---|
| **D1** | Modal swaps into a `#modal-root` container (button not replaced). | **IMPLEMENTED** |
| **D2** | Close-on-success via the shipped OOB card + emptying the modal target (no JS). | **IMPLEMENTED** |
| **D3** | Empty-title error renders inside the modal (`bad_request_fragment`). | **IMPLEMENTED** |
| **D4** | No-JS fallback preserved — form keeps `method="post"`/`action` alongside `hx-post`. | **IMPLEMENTED** |
| **D5** | Near-zero backend — ONLY the `BoardPage` slug fields (REVISED from "zero"). | **IMPLEMENTED** |
| **D6** | Repo multi-file convention; no SSOT, no migration. | **IMPLEMENTED** |

## How it was built (DELIVER)

Outside-In: un-`@pend` S1–S5 (RED for the right reason — inert button / plain-POST modal), wire the templates
+ surface the slugs (GREEN), then a styling follow-up. The acceptance suite is HTTP-level, so it pins the
**wiring** (button `hx-get` + container; modal form `hx-post` + `_csrf`), the shipped **contracts** (OOB
Backlog card; "Title is required" error fragment), and the **no-JS fallback** end-to-end; the live
click→modal→card→close interaction + the modal styling were verified by **browser dogfood**.

| Commit | Proved |
|---|---|
| `b38c6ff` | button files an issue via htmx; OOB card to Backlog; no-JS fallback intact; 5/5 acceptance |
| `3e86b7b` | modal renders as a centered overlay dialog; render contracts (us-12, us-k01) intact |

## Verification

- **Feature-scoped**: `board-new-issue` 5/5, `us-12` 5/5, `us-k01` 4/4, `dashboard-enhancements` 8/8 (hash-bump
  safe) — all green. `cargo fmt --all --check` + `cargo clippy --all-targets --release -- -D warnings` clean.
- **Full gate**: `cargo xtask ci` — all gates green including the `@all` acceptance lane (recorded at finalize).
- **Browser dogfood**: click "New issue" → centered modal → title → Create → `GEN-1` card in Backlog → modal
  closes, no reload.

## Deferred (out of scope, v1)

The `c`-to-open / `Esc`-to-close / focus-trap keyboard interaction layer (the standalone
`board-keyboard-interaction` feature the empty-state hint still promises); backdrop-click-to-close; creating
directly into a non-Backlog column; drag between columns.
