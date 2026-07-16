# ADR-001: Vanilla document-delegated dispatch layer; Alpine dropped (ODD-2)

## Status
Accepted — 2026-07-15 (Morgan, DESIGN wave). Feature-local. Resolves **ODD-2** and **Risk R9**.

## Context
`keyboard.rs:1-30` documents *"Three routes that back the **alpine.js** keyboard-shortcut handlers"* and
`keyboard.rs:19-24` describes *"the alpine.js bootstrap"* that GETs `/keyboard-help` once and caches it.
**None of it exists.** Verified: `static/js/` contains exactly `board-dnd.js` and `csrf-upload.js`; the only
`keydown` in the tree is inside the vendored `alpine.min.js`.

The house pattern is unambiguous and doubly precedented — `board-dnd.js:17` and `csrf-upload.js:19` are
vanilla IIFEs with `"use strict"`, loaded as external same-origin `defer` scripts with no inline handlers,
using **`document`-level delegation**. `board-dnd.js:67` says why in its own comment: *"Delegated on
document so htmx-appended cards … are draggable without re-wiring."*

Alpine **is** vendored and loaded (`base.html:7`) but **unused**: a search for `x-data`, `x-on:`, `x-model`,
`x-show`, `x-init`, `@click` and `Alpine` across `crates/foundry-app/templates/` returns **zero** hits.

NFR-5 requires vendored-only, CSP-safe, no inline handlers. NFR-6 requires handlers that survive htmx swaps
with no re-wiring.

## Decision
1. **Vanilla.** `static/js/keyboard.js` — one IIFE, `"use strict"`, external same-origin, `defer` from
   `base.html`, no inline handlers, **one `document`-delegated `keydown`**. Exactly the `board-dnd.js`
   idiom, for exactly the reason `board-dnd.js` gives.
2. **Drop Alpine.** Remove `<script src="/static/vendor/alpine.min.js" defer></script>` (`base.html:7`).
   Zero app consumers; per AGENTS.md (*"Remove dead/legacy code outright — do not leave it inert"*) it is
   carry, not insurance. **This is a deliberate scope addition**, declared here rather than smuggled: it is
   the same dead-code judgement ODD-1 applies to `#kb-items`, and leaving Alpine loaded while writing the
   vanilla layer that its own doc comment promised would enshrine the confusion this ADR exists to end.
   Retiring the vendored asset itself (`static/vendor/alpine.min.js`, `static/VENDOR.md`) rides along.
3. **Correct the doc (R9).** Rewrite `keyboard.rs:1-30` to describe the vanilla layer that now exists. The
   "public by design" rationale at `:19-24` stays true and stays — only the "alpine.js" attribution is
   wrong.

## Alternatives Considered
- **Alpine `x-on:keydown.window` / `x-data` on `<body>`** — REJECTED on three independent grounds, any one
  sufficient:
  1. **CSP.** Alpine's standard build evaluates attribute expressions via the `Function` constructor,
     requiring `unsafe-eval` under a strict CSP. `board-dnd.js:1-17` names CSP-safety as a design
     property of the house pattern. (Foundry sets no CSP header **today** — verified: no
     `Content-Security-Policy` anywhere in `crates/`. So this is a *future* cost, not a present breakage —
     stated honestly rather than overclaimed. Adopting Alpine would foreclose a strict CSP; vanilla does
     not.)
  2. **Delegation.** Alpine's idiom is per-element/per-component binding within an `x-data` scope. NFR-6
     wants one document-level listener that htmx swaps cannot break. `@keydown.window` can approximate it
     but only by putting behaviour in template attributes — markup the guard chain (BR-2) must then be
     duplicated into or called from.
  3. **Guard placement.** BR-2 requires the chain evaluated **once, before dispatch, with no exemptions**.
     Alpine's per-binding modifiers invite exactly the "seven scattered `if`s" the DISCUSS names as the
     failure mode (NFR-2, R2).
- **Keep Alpine loaded but unused (write vanilla, change nothing else)** — REJECTED. It is ~15 KB of
  dead vendored JS on every page and it is the *source* of R9: the next reader finds Alpine loaded and a
  doc comment claiming Alpine handlers, and re-derives the wrong mental model. The dead-code policy is a
  standing repo rule, not a per-feature preference.
- **Adopt Alpine properly and rewrite `board-dnd.js`/`csrf-upload.js` into it** — REJECTED. A framework
  migration of two shipped, working, tested scripts, motivated by a stale comment. Enormous blast radius,
  zero user value, and it would fight NFR-6 and NFR-8 (drag) for no reason.
- **A build step / bundler** — REJECTED. The repo has an explicit no-toolchain posture (inherited; see
  `navigation-bar-linear-ui/design/adr-004`, which rejects a build step even for CSS hashing). The layer is
  small and dependency-free; a bundler would be pure carry.

## Consequences
- **Positive**: one house idiom, not two. `keyboard.js` reads like `board-dnd.js`, so the delegation
  guarantee (NFR-6) is obvious rather than argued. Zero dependencies, zero build step, no `eval`.
- **Positive**: the guard chain (ADR-002) can be structural — one chain in one function, unreachable-around
  — which is the single most important property in the feature (BR-2, NFR-2).
- **Positive**: dropping Alpine removes the tree's only `unsafe-eval`-shaped dependency, leaving a strict
  CSP available as a cheap future win, and deletes ~15 KB from every page load.
- **Negative / accepted**: dropping Alpine is a scope addition beyond "bind seven keys" (one `base.html`
  line + the vendored asset + its `VENDOR.md` entry). Declared, not smuggled. If DELIVER finds any Alpine
  consumer this ADR missed, **stop and re-open this decision** rather than working around it.
- **Negative / accepted**: vanilla means writing the selection/overlay/stack plumbing Alpine would have
  given for free (~a few hundred lines). Accepted — that plumbing is this feature's actual content, and
  ADR-004's key-based model makes most of it fall out rather than be written.
- **Probe (Earned Trust)**: `keyboard.js` sets `document.documentElement.dataset.kbReady = "1"` at init.
  This is not decoration — it is the layer *demonstrating* it loaded and bound, and it is what the browser
  lane waits on (ADR-007) and what US-02's paired assertion uses as its "the layer is live" precondition
  (D15). `defer` ordering against htmx/Alpine bootstrap was slice 01's named risk; the marker retires it by
  making readiness observable instead of assumed.
