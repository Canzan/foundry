# Story Map: Remaining-Surfaces Templating

## User: Jamal Okafor (contributor) + Mei Chen (self-hoster member)
## Goal: every remaining inline-`format!()` HTML surface renders from an Askama template extending the existing `base.html`, selector-and-substring-identical, with the acceptance suite green throughout.

> This is a MOVE-ONLY refactor reusing Feature B's shipped engine, `base.html`,
> `/static` assets, and render contract. The backbone is the SURFACE LIST, not a
> user workflow — each surface is an independent thin vertical slice. There is no
> end-to-end "flow" to skeleton across; the walking skeleton is therefore the
> FIRST surface proved on a real page (the project-create form), establishing the
> move pattern that every later slice repeats.

## Backbone (surfaces, grouped by handler module)

| Project surfaces | Issue/keyboard surfaces | Auth/landing surfaces | Attachment surfaces | Bootstrap surfaces |
|------------------|-------------------------|-----------------------|---------------------|--------------------|
| project-create form (full page) | new-issue modal (fragment) | dashboard landing `/` (full page) | upload-error fragment | bootstrap dashboard (full page) |
| project-create-error fragment | new-issue modal (full page) | events sign-in-required page (full page) | attachment-row OOB fragment | claim form (full page) |
| | issue-create-error fragment | | upload-too-large page (full page) | invite-link page (full page) |
| | state-change `<span>` fragment | | attachment not-found page | shared `invalid_page` helper |

---

## Walking Skeleton (Slice 1) — prove the move on one real surface

**US-R01 — project-create form + project-create-error fragment → Askama template.**
Thinnest slice that proves the pattern end-to-end on a real, full-page surface
that has BOTH a full page (bare `<head>` today) AND an error fragment: it exercises
extending `base.html`, linking `/static`, emitting the `_csrf` field, and keeping a
`data-hx-fragment` marker byte-stable — every mechanic later slices reuse. If the
acceptance suite stays green after US-R01, the pattern is validated and the rest are
mechanical repeats.

---

## Slices (one surface or one tight pair each, ≤1 day, move-only)

Each slice is independent (no inter-slice dependency beyond "the Feature B engine
exists, which it does"). They can ship in any order after the skeleton; the
sequence below is by surface visibility/impact and risk.

### Slice 1 (Walking Skeleton) — US-R01 — project-create form + error fragment
- **Job**: htmx-web-1. **Surfaces**: `projects.rs::render_create_form`, `render_error_fragment`.
- **Learning hypothesis**: "A full page + its error fragment can move to Askama/base.html selector-identical with the suite staying green — proving the move pattern on a real surface."

### Slice 2 — US-R02 — new-issue modal (fragment + full-page fallback)
- **Job**: htmx-web-1. **Surfaces**: `keyboard.rs::render_modal_fragment`, `render_modal_full_page`.
- **Learning hypothesis**: "The fragment-vs-full-page split (fragment stays bare, full page extends base.html, both share ONE modal partial) holds for an htmx-swapped surface without breaking the swap."

### Slice 3 — US-R03 — issue-create-error + state-change fragments
- **Job**: htmx-web-1. **Surfaces**: `issues.rs::bad_request_fragment`, the state-change `<span>`.
- **Learning hypothesis**: "Tiny mutating-response fragments (`data-hx-fragment`, `data-state`) move to templates with their markers + error copy byte-stable."

### Slice 4 — US-R04 — dashboard landing `/` + events sign-in-required page
- **Job**: htmx-web-2. **Surfaces**: `signin.rs::dashboard_root`, `events.rs::unauthorized_response`.
- **Learning hypothesis**: "The two highest-visibility bare-`<head>` landing pages become styled via base.html without changing their redirect/401 behavior."

### Slice 5 — US-R05 — attachment surfaces
- **Job**: htmx-web-1. **Surfaces**: `attachments.rs` upload-error fragment, `render_attachment_row_oob`, `payload_too_large`, `not_found_page`.
- **Learning hypothesis**: "An OOB swap fragment (`hx-swap-oob` target preserved) plus its full-page error pages move cleanly, generalizing the fragment+page split to a second module."

### Slice 6 — US-R06 — bootstrap dashboard + claim form + invite-link page + shared `invalid_page`
- **Job**: htmx-web-2. **Surfaces**: `bootstrap.rs::dashboard`, `render_claim_form`, `create_invite` invite-link page, `signin.rs::invalid_page`.
- **Learning hypothesis**: "The first-run/bootstrap pages and the shared not-found/error helper move to base.html, finishing the cut so NO bare-`<head>` `format!()` HTML remains; the shared `invalid_page` template restyles every not-found path at once."

### Optional tail (fold-in or defer) — keyboard search + help overlay
- `keyboard.rs::render_search_fragment`, `show_keyboard_help`. Same pattern, lowest
  risk/visibility. Fold into Slice 2 if cheap, else a trivial follow-up. Tracked, not
  a blocking slice.

---

## Priority Rationale

Priority is by **surface visibility/impact × risk-derisk value**, not effort
(all slices are ≈equal, ≤1 day, move-only). Tie-break: Walking Skeleton first.

1. **US-R01 (Walking Skeleton)** — first, because it derisks the *whole feature*:
   it is the cheapest surface that exercises every mechanic (full page + base.html
   + `/static` + `_csrf` + a `data-hx-fragment` error fragment). Green here ⇒ the
   pattern is proven; everything after is repetition.
2. **US-R02 / US-R03** — next, because they are interaction surfaces (the `c`-to-create
   modal, the create/state-change fragments) on the contributor hot path; proving the
   fragment-vs-full-page split early derisks the remaining fragment moves.
3. **US-R04** — high *visibility* (the `/` landing is the first thing a signed-in
   self-hoster sees; the events page is a common dead-end), so styling it removes the
   most jarring unstyled moment. htmx-web-2 payoff.
4. **US-R05** — attachment surfaces: moderate visibility, introduces the second OOB
   fragment move; lower urgency than the landing.
5. **US-R06** — bootstrap/first-run + the shared `invalid_page`: important for a clean
   first-run impression and finishes the cut (no bare `<head>` left), but lowest
   day-to-day visibility, so last. The shared `invalid_page` move is high-leverage
   (restyles all not-found paths at once) which is why it is bundled here rather than
   scattered.

> Dependencies: NONE between slices. Every slice depends only on Feature B's shipped
> Askama engine + base.html + `/static` + views.rs, which already exist. Slices may
> be reordered or parallelized freely after the skeleton.

---

## Oversize Assessment: PASS — 6 stories (all move-only, ≤1 day each), 1 bounded context (foundry-app rendering layer), ~5-7 days

Elephant Carpaccio gate run against the 5 oversize signals:

| Signal | Threshold | This feature | Trips? |
|--------|-----------|--------------|--------|
| User stories | >10 | 6 | No |
| Bounded contexts/modules | >3 | 1 (foundry-app web-render layer; reuses Feature B's seam) | No |
| Walking-skeleton integration points | >5 | 0 new (engine/base/assets all exist) | No |
| Estimated effort | >2 weeks | ~5-7 days (move-only, mechanical) | No |
| Multiple independent shippable outcomes | yes → split | Already pre-split into 6 independent per-surface slices | No |

**Verdict: right-sized.** No split needed. Each surface is already its own thin
vertical slice. If anything, the slices are unusually small (each demonstrable in a
single session via the acceptance suite) — exactly the desired carpaccio. No oversize
signal trips.
