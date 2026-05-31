# Story Map: Web-Tier Extraction

> DRIVER-CORRECTED (2026-05-30). The headline outcome is the **first-class JSON API** (read +
> write, machine-token auth) so external agents/integrations can drive Foundry programmatically.
> The JSON-API track now LEADS; the htmx web-tier template track follows as the peer-consumer
> track. Sliced as thin Elephant-Carpaccio slices: each slice ships an independently verifiable
> end-to-end behavior, NOT a technical layer. Slice 1 proves the JSON API as a real consumer of
> a presentation-neutral core on the smallest real read surface (the board's issues).

## Personas
- **Integrator / automation author / agent builder** (Devansh Rao scripting; an agent builder) —
  the PRIMARY persona post-correction: drives Foundry over the JSON API with a machine token.
- **Member** (Mei Chen, Hiroshi) — uses board / issue / sign-in; must perceive NO change
  except "it now looks styled"; also sees API-created changes appear in real time.
- **Contributor** (Jamal Okafor, Rust dev) — visual changes become a one-template edit with a
  structural safety net (secondary track).
- **Operator / workspace admin** (Devansh) — still one binary, one Postgres, no Node, no new
  services; also issues and revokes machine tokens.

## End-to-End Goal
A self-hosted Foundry whose **machine-facing JSON API** (read + write, machine-token auth) and
**user-facing HTML** are PEER consumers of one presentation-neutral core — the JSON tier served
by `foundry-api`, the HTML tier rendered from templates by `foundry-web`, both calling the
existing core/store **in-process**, in **one binary**, with the existing acceptance suite
staying green throughout.

---

## Oversize Assessment + Proposed Split (for the user to ratify in DESIGN)

Promoting the JSON API to a first-class read+write surface plus a new machine-token auth surface
trips the Elephant-Carpaccio oversize gate (see `stories.md` Scope Assessment and
`wave-decisions.md` D8). **Three signals trip**: two independent shippable outcomes, a net-new
auth surface, and ~16-22 days effort.

**Recommended split (keep everything in this discuss set for now; ratify before DESIGN):**

| Proposed feature | Stories | Outcome | Ships independently? |
|------------------|---------|---------|----------------------|
| **A — "Programmatic Foundry"** | US-W05a, US-W05b, US-W05c, US-W06 | First-class JSON API (read+write) + machine-token auth + peer-consumer core separation + boundary guard | Yes — delivers the primary job (jtbd-web-4) on its own |
| **B — "Foundry looks like a product"** | US-W01, US-W02, US-W03, US-W04 | htmx web-tier template/asset build-out (board, issue+comments, sign-in) + later htmx2 | Yes — delivers the secondary jobs (jtbd-web-1/3) on its own |

Both share the SAME presentation-neutral core seam; Feature A proves it first (Slice 1), and
Feature B reuses it. If the user does NOT split, the single feature still ships in the slice
order below (API-first). **No separate feature directories created yet** — this is a flagged
recommendation only.

---

## Backbone

The backbone is the **sequence of capabilities proven across the boundary**, ordered so the
headline JSON API is proven first and the web tier follows as the peer consumer.

| Read as JSON | Authenticate machines | Write as JSON | Lock the boundary | Move the board | Move issue+comments | Move sign-in |
|--------------|-----------------------|---------------|-------------------|----------------|---------------------|--------------|
| board issues as JSON over a presentation-neutral core seam | machine-token issuance + use | create/update issues+comments via JSON | boundary guard (web≠DB, api≠HTML) | board template + card partial | issue page + comment-card partial | sign-in/forgot templates |
| no HTML from api; core neutral | additive to browser session/CSRF | rule-parity with the UI; same outbox | folds into the write slice | vendor htmx/Alpine/CSS | preserve authz affordances | preserve cookie/error contract |

---

## Walking Skeleton (Slice 1)

The thinnest end-to-end slice that proves the headline premise: **a real machine consumer can
read Foundry data as JSON from a presentation-neutral core.** The board's issues are served by
`foundry-api` as a JSON array, obtained through the same core/store call the web board will use,
with no HTML in the response and the existing acceptance suite green.

| Backbone step | Story | Why it's in the skeleton |
|---------------|-------|--------------------------|
| Read as JSON | US-W05a | Without a JSON endpoint reading core, there is no API to build on and no proof that core is presentation-neutral. |

End-to-end demonstrable value: **Devansh runs `curl -H "Accept: application/json" .../issues`
and gets a JSON array of the Auth v2 board's issues — served by foundry-api from the same core
call the UI uses, with zero HTML — while the full existing acceptance suite stays green.**

> US-W05a is a single story but it is clearly user-visible (a real machine consumer gets real
> JSON), so Slice 1 passes the slice-level Elevator-Pitch rule on its own.

---

## Release Slices

Every slice contains at least one user-visible value story (no slice is all `@infrastructure`).
Slices are sliced by capability/outcome, each end-to-end.

### Slice 1 — Walking Skeleton: "Foundry data is readable as JSON from a neutral core"
- **Stories**: US-W05a (read the board's issues as JSON over a core seam)
- **End-to-end value**: a real JSON read endpoint, served by foundry-api from the same core call
  the UI uses, no HTML, acceptance suite green.
- **Target jobs**: jtbd-web-4 (PRIMARY — programmatic read), jtbd-web-2 (core is presentation-neutral).
- **Dependencies**: none (entry slice).
- **Learning hypothesis**: *Core can feed a non-HTML JSON consumer through the same call path
  the UI uses — i.e. core is genuinely presentation-neutral, not HTML-shaped.* If false, the
  whole first-class-API premise is wrong, so this is the riskiest assumption.

### Slice 2 — "Agents can authenticate and DRIVE Foundry over JSON"
- **Stories**: US-W05b (machine-token auth), US-W05c (create/update issues+comments via JSON),
  US-W06 (boundary guard, folded in)
- **End-to-end value**: an unattended client holds a machine token and creates+updates issues
  and comments via JSON, with the same authz/validation/sanitization as the UI, changes visible
  to the UI in real time — and CI now fails if api emits HTML or web reaches the DB.
- **Target jobs**: jtbd-web-4 (PRIMARY — the full programmatic-drive outcome), jtbd-web-2 (durable boundary).
- **Dependencies**: Slice 1 (the JSON read surface + core seam).
- **Learning hypothesis**: *A machine-token auth surface can be added additively (without
  perturbing the browser session/CSRF model) AND API writes can reuse core's write+outbox path
  with full rule-parity to the UI.* This validates that the API is genuinely first-class, not a
  privileged or divergent back door.

### Slice 3 — "The board is a real, styled web peer of the same core"
- **Stories**: US-W01 (board over the core seam in foundry-web), US-W02 (board template + vendored assets)
- **End-to-end value**: a styled board, rendered by foundry-web from a template, reading the
  SAME core seam the JSON API proved; acceptance suite green; web tier provably DB-free.
- **Target jobs**: jtbd-web-2 (web is a peer consumer), jtbd-web-3 (first-screen trust), jtbd-outcome-4 (no latency regression).
- **Dependencies**: Slice 1 (the presentation-neutral core seam already exists and is proven).
- **Learning hypothesis**: *The same core seam that feeds JSON can feed a template-rendered HTML
  board inside the ≤200 ms budget while keeping the substring-asserting acceptance tests green.*

### Slice 4 — "Issue & comments read like a product"
- **Stories**: US-W03 (issue detail + comment thread → templates)
- **End-to-end value**: the issue page and comment thread render from one comment-card partial
  across full-render / htmx-append / edit / cancel paths; authz affordances and sanitization unchanged.
- **Target jobs**: jtbd-web-1 (one-template visual change), jtbd-outcome-4 (in-issue discussion feel).
- **Dependencies**: Slice 3 (the web seam + base layout + card-partial pattern exist).
- **Learning hypothesis**: *The four `format!` comment-render sites can collapse into ONE partial
  without diverging the live-updated card from the reloaded card.*

### Slice 5 — "First impression: sign-in looks trustworthy"
- **Stories**: US-W04 (sign-in / forgot-password → templates)
- **End-to-end value**: full-page auth screens render from the shared layout; cookie and
  non-enumerable-error contracts unchanged.
- **Target jobs**: jtbd-web-3 (first-screen trust), jtbd-outcome-7 (template-driven upgrade ease).
- **Dependencies**: Slice 3 (base layout + static pipeline).
- **Learning hypothesis**: *Full-page templates share one base layout with the fragment surfaces,
  so CSS/asset consistency is automatic (Nielsen #4) without duplication.*

---

## Slice-Level Validation Checklist

Per the reviewer Dimension 0 slice-level rule: every *released* slice must contain at least one
user-visible value story (not entirely `@infrastructure`).

| Released slice | User-visible stories | Infra-only stories | Pass? |
|----------------|----------------------|--------------------|-------|
| 1 — Read as JSON | US-W05a (1) | 0 | YES |
| 2 — Auth + writes + guard | US-W05b, US-W05c (2) | US-W06 (1, folded in) | YES (writes are user-visible) |
| 3 — Styled board | US-W01, US-W02 (2) | 0 | YES |
| 4 — Issue & comments | US-W03 (1) | 0 | YES |
| 5 — Sign-in | US-W04 (1) | 0 | YES |

US-W06 never ships as a standalone release; it rides Slice 2. No released slice is
all-infrastructure. No re-slicing required.

---

## Priority Rationale

Order: **Slice 1 (read JSON) → Slice 2 (auth + writes + guard) → Slice 3 (board) → Slice 4
(issue/comments) → Slice 5 (sign-in)**. The flip from the old order (web-first) is the heart of
the driver correction.

1. **Slice 1 first (Walking Skeleton — JSON read)** — validates the riskiest assumption behind
   the now-PRIMARY job: *can core feed a non-HTML JSON consumer through the same path the UI
   uses?* If core is secretly HTML-shaped, the first-class-API premise collapses. Tie-break:
   Walking Skeleton beats riskiest-assumption beats highest-value (Maurya).
2. **Slice 2 second (machine-token auth + writes)** — completes the PRIMARY outcome
   (jtbd-web-4): an agent can authenticate and actually DRIVE Foundry. This is the headline
   value and carries the second-riskiest assumption (machine-token auth is additive; API writes
   reach rule-parity with the UI). The boundary guard (US-W06) folds in here because the write
   surface is exactly when an eroded api≠HTML / web≠DB boundary would do the most damage.
3. **Slice 3 third (board → web tier)** — the web tier is now the SECONDARY peer consumer of the
   core seam Slices 1-2 already proved presentation-neutral. Doing the board first in the web
   track validates the template render path on the highest-traffic surface.
4. **Slice 4 fourth (issue & comments)** — highest-value contributor outcome (jtbd-web-1): four
   `format!` comment sites collapsing to one partial; the most tangled surface.
5. **Slice 5 last (sign-in)** — first-impression trust (jtbd-web-3) but lowest extraction risk
   (full-page, no fragment swaps), so it trails the harder fragment surfaces.

Value × Urgency / Effort intuition: Slice 1 high-value/high-urgency (de-risks the headline job)
→ P1. Slice 2 highest-value (delivers the headline)/high-effort → P2. Slice 3 medium-value/
medium-effort → P3. Slice 4 high-value/medium-effort → P4. Slice 5 medium-value/low-effort → P5.

> If the user ratifies the split, Feature A = Slices 1-2 (+US-W06), Feature B = Slices 3-5. The
> slice order above is the single-feature delivery sequence and is also the cross-feature
> sequence (A before B) if split.

---

## Notes on Story Granularity

- 8 stories total: US-W05a/b/c (primary JSON-API track), US-W01..W04 (secondary web track),
  US-W06 (`@infrastructure`, folded into Slice 2).
- All stories are individually right-sized (S/M/M-L; ≤3-4 days, 3-7 scenarios). US-W05c is at
  the upper bound (6 scenarios, M-L) — split issues-writes from comments-writes if it grows.
- No story spans multiple slices.
- US-W06 is the only `@infrastructure` story and is explicitly NOT a standalone slice.
- Per-slice briefs live in `docs/feature/web-tier-extraction/slices/`.
