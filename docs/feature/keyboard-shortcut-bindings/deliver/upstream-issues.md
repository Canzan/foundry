# DELIVER — Upstream Issues

Back-propagation from the DELIVER wave to prior waves. Per the contract, prior-wave
documents are NOT edited; the delta and its rationale are recorded here.

---

## UI-1 — ADR-001 §2's "zero Alpine consumers" precondition is FALSE (found at step 01-03)

**Source**: `docs/feature/keyboard-shortcut-bindings/design/adr-001-vanilla-dispatch-layer.md` §2

**What DESIGN asserted** (and what step 01-03 carried forward as a precondition):

> Zero app consumers is VERIFIED (`x-data` / `x-on:` / `x-model` / `x-show` / `x-init` /
> `@click` / `Alpine` across `templates/` return zero hits)

**What is actually true**: that grep is correct but **too narrow**. It searched for Alpine
*directives in templates* — of which there are genuinely zero. It never searched for
**contract tests asserting the vendored asset is served**. Three exist, all currently green
and none `@pending`:

| Consumer | What it asserts |
|---|---|
| `crates/foundry-acceptance/tests/features/us-b02-vendored-assets.feature:31` | `@walking_skeleton` — "The vendored htmx, **Alpine**, and stylesheet are served by the binary" |
| `crates/foundry-acceptance/tests/features/us-b01-styled-board.feature:48` | "the board loads the vendored htmx **and Alpine** scripts from the application's own static path" |
| `crates/foundry-app/src/lib.rs:300` | `assert!(static_cache_control_value("/static/vendor/alpine.min.js").contains("immutable"))` |

Plus stale prose referencing alpine.js at `projects.rs:748,795,882`, `csrf.rs:39,163`, and
`views.rs`, which the step's zero-hit `alpine` grep gate also catches.

So: Alpine has **zero runtime consumers** and **three contract consumers**. Deleting the
asset reds a *delivered* feature's walking skeleton. The distinction the ADR missed is
between "nothing uses it" and "nothing asserts it exists".

**Why this matters beyond the mechanics**: `projects.rs:882` reads *"the alpine.js j/k
handler walks"* — a third stale comment describing a handler that never existed, alongside
`keyboard.rs:1-29` and `keyboard.js:16`. The same failure mode this feature exists to close
(a written claim outliving, or preceding, the code it describes) is present in the
codebase in at least three places.

**Resolution (ratified by the user, 2026-07-16)**: **Retire Alpine and amend the two shipped
contracts** to assert only htmx. Rationale: the scenarios assert a dependency that does
nothing — they are testing a fiction, and keeping them green would preserve 44 KB of dead
framework on every page load in order to satisfy an assertion about its own presence. The
`AGENTS.md` dead-code policy ("Remove dead/legacy code outright — do not leave it inert")
governs. ADR-001 §2's *conclusion* (drop Alpine) stands; only its *precondition* (zero
consumers) was wrong, and the correction widens the blast radius rather than reversing it.

**Scope delta to step 01-03** (authorized by the orchestrator, disclosed here):
- `crates/foundry-acceptance/tests/features/us-b01-styled-board.feature` — drop the Alpine arm
- `crates/foundry-acceptance/tests/features/us-b02-vendored-assets.feature` — drop the Alpine arm, retitle the scenario
- `crates/foundry-acceptance/src/steps/` — whichever step module backs those two scenarios
- `crates/foundry-app/src/lib.rs` — remove the alpine.min.js cache assertion at :300
- `crates/foundry-app/src/csrf.rs`, `views.rs`, `projects.rs` — correct stale alpine.js prose

**Process note**: step 01-03 was correctly BLOCKED, not failed, by the crafter. It logged
RED_ACCEPTANCE / RED_UNIT / GREEN / COMMIT as `SKIPPED / BLOCKED_BY_DEPENDENCY` rather than
marking them EXECUTED to satisfy a hook demanding recovery — which would have been the exact
audit fraud the DES rules forbid. Zero source files were modified. The step instructed
"if DELIVER finds ANY Alpine consumer this ADR missed, STOP and re-open ADR-001 rather than
working around it", and that is what happened.

---

## UI-2 — Roadmap step 01-02 claims a scenario that cannot pass at slice 01's scope (found at step 01-02)

**Source**: `docs/feature/keyboard-shortcut-bindings/deliver/roadmap.json` step `01-02`

The scenario **"The overlay lists exactly the seven advertised shortcuts and each is bound"**
was assigned to step 01-02. Its middle arm asserts *"every shortcut it lists is bound and
does something"* — which requires **all seven** keys bound. Slice 01 binds **two** (`?` and
`Esc`); `slice-01-dispatch-layer-and-help-overlay.md` states its own scope as
"KPI-1 advertised-to-working 0/7 → 2/7" and "no character key is bound here".

The roadmap step and the DESIGN slice scope genuinely conflict. The crafter's only two paths
were both violations — bind all seven speculatively (code with no requiring test, contradicting
the slice's stated OUT-of-scope), or assert only over the implemented subset (weakening the
test to force a pass). It escalated instead, correctly.

**Resolution**: the scenario stays `@pending` through slice 01 and **moves to slice 05**,
where all seven keys are bound and the bound-equals-advertised invariant (BR-1, KPI-5) can
hold honestly. To be applied to `roadmap.json` before slice 05 executes.

**Related drift risk**: `keyboard.js:16` comments that bound == advertised "by construction",
but `:73` hardcodes `event.key === "?"`. Nothing structurally prevents drift today — the
claim is aspirational. The invariant is enforced by *that scenario* (a test), not by
construction. When the scenario lands in slice 05, either the comment must be corrected to
say so, or the derive-from-`SHORTCUTS` design must actually be built.

**The roadmap review did not catch either issue**: it verified every scenario had a step, but
not that each step's scenarios *could pass at that step's slice scope*. Worth adding to the
roadmap quality gate.
