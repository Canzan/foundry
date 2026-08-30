# Evolution — canzan-theme-system (foundry wears canzan's palette, in the light the room needs)

**Finalized**: 2026-08-29
**Commits**: DISCUSS+DESIGN+DISTILL docs landed inside the DELIVER commits; DELIVER
`b9b9bc3` → `526e090` (10 DES-monitored TDD steps across 5 phases / 4 slices), plus
phase-log chores `339ce22`/`f389d6c`/`4cdb316`. No refactor commits (L1–L6 no-change:
the feature's only new Rust is two self-contained check-arch rule families).
Trunk-based; `des-verify-integrity` reports **all 10 steps have complete DES traces**,
exit 0; adversarial review **APPROVED (0 defects)**, Testing Theater scan clean;
mutation **191/218 viable killed (87.6%**, gate 80%)** on `xtask/src/check_arch.rs`
with survivor analysis. Consolidated review gate (end of DISTILL): Eclipse approved;
Architect and Sentinel conditionally approved, all findings resolved before DELIVER.
`cargo xtask check-arch` green with all five new rules armed. 28/28 acceptance
scenarios green, zero `@pending` (us-cts-01 8/8, us-cts-02 5/5, us-cts-03 5/5,
us-cts-04 9/9, plus the `@oracle-probe` instrument scenario). Feature dir PRESERVED.
**Not pushed.**
**Scope**: colour enters `foundry.6296815a.css` at exactly three regions and nowhere
else; 46 colour literals across 30 rules re-authored onto `--cz-*` tokens; three
competing accent hues collapsed to one; a real dark palette on every screen; a
three-state (system/light/dark) control ported byte-identically from canzan-lift;
three self-hosted, axis-instanced, latin-subset OFL faces (76,416 B total); and five
new `check-arch` rules, each with an injected-violation gold test. No schema change,
no new crate, no build step, no wire surface moved. Diff to pre-existing acceptance
files across the whole feature: **3 registration lines**.

## Business context

Priya Raman, the self-hosting operator, runs foundry beside Grafana, Portainer,
ArgoCD and Element. At 23:40 every one of those has quietly gone dark because her
machine asked them to, and foundry is still `#ffffff` — the one tool on the cluster
that looks unfinished. There was no dark mode in any form: 0%, not a partial one.
And a device preference alone would not have been enough, because her design tooling
keeps that same machine on a Light OS by day; the environment that actually needs
solving is *a light-set device on a dark night*, which only an explicit,
per-application override can serve.

Underneath the visible job sat a structural one. The stylesheet had accumulated 46
colour literals outside the token block across 30 rules — the rail block used **zero**
tokens — so "add a dark palette" was a re-authoring, not an audit, and there was
nothing to stop the next 46 from accumulating. `assets.md` Decision #4a had chosen
content-hashed filenames as the cache key back in `htmx-web-tier` and accepted its
one failure mode on the strength of an "asset-resolution probe" that was specified,
never built, and re-requested by two later features. This feature was the third
request, and it built it.

## Key decisions (D-01 – D-14)

- **D-01 (premise corrections, six of them)** — the code contradicted the intake's
  *facts* while every one of its *decisions* stood. (a) `.site-header` renders in **no**
  template: dead CSS through 43 features; the real signed-in chrome is `aside.sidebar`.
  (b) foundry does **not** carry canzan-lift's blanket no-JS guarantee — it has one
  scripting-disabled scenario, a lane and not a promise. (c) the stylesheet is not
  "mostly tokenised": 46 literals / 30 rules. (d) foundry carried **three** accent hues
  (`#2452c9`, rail indigo, card-key indigo), canzan has one. (e) there is **no**
  automated asset-resolution guard, and the CSS hash is pinned in five hand-maintained
  sites. (f) canzan-lift shares neither the `--cz-*` names nor the type strategy, so
  intake D2's "share one module" is true of the **toggle only**.
- **D-02** — token names adopted verbatim; foundry's eight colour tokens **retired, not
  aliased**. An alias layer would make foundry a *translation* of the contract rather
  than an *adopter* of it.
- **D-03** — two dark blocks, written out, never merged, both setting `color-scheme`.
  CSS cannot express "either" across a media query and an attribute selector.
- **D-04** — `--cz-faint` **rebound**, not abandoned: canzan's own faint tier measures
  3.24:1 light / 3.52:1 dark, fine for a marketing eyebrow and not for a label an
  operator must read. foundry moves the *value* (4.57:1 / 4.83:1) and keeps the
  *structure*, so canzan-lift's eventual migration inherits an unchanged shape.
- **D-05** — translucent tokens never sole-carry text; any tinted, text-bearing surface
  also declares an opaque background. A contrast algorithm that walks ancestors for the
  first non-transparent value reads a translucent panel as its unblended colour.
- **D-06** — `theme.js` ported with logic **byte-identical**; exactly two values differ
  (`STORAGE_KEY`, mount selector), both hoisted to named constants. A third divergence
  is an escalation, not an edit.
- **D-07** — `theme-color` follows the **device only**. Accepted limitation (below).
- **D-08** — the control mounts on the 11 app-shell screens and nowhere else; the 15
  chrome-less templates still *honour* an explicit choice with no button to make one.
- **D-09** — mount at the foot of the rail, inside `.sidebar__user`, a region the
  acceptance suite already selects on.
- **D-10** — the dead `.site-header` rules are **deleted**, not restyled.
- **D-11** — zero selector churn. Amended mid-flight (Divergence 1): its original
  measurable form, "no existing file under `crates/foundry-acceptance/` is modified",
  is **mechanically unsatisfiable by any feature** — registering a step module requires
  a `pub mod` and a force-link line or the module never compiles into the test binary.
  Restated as "no existing **scenario or assertion** is changed"; the intent is unchanged
  and was met exactly (3 registration lines).
- **D-12** — walking skeleton declined: brownfield, 47 shipped features, nothing
  unproven end to end. This feature changes how a shipped path *looks*.
- **D-13** — fonts are never vendored alone; three woff2 blobs plus VENDOR.md rows show
  a user nothing, so the vendoring folds into the typography slice.
- **D-14** — no screen ever renders invisible text while a font loads. `font-display: swap`
  on all three: the browser's default is a *block* period, and a blank board is a worse
  failure than an unstyled one.

## Steps completed (10/10, execution-log.json)

| Step | What landed | Commit |
|---|---|---|
| 01-01 | Asset-integrity guard R1/R2/R3 in check-arch + 3 injected-violation gold tests | `b9b9bc3` |
| 01-02 | Token-seam guard S1/S2 in check-arch + 2 injected-violation gold tests | `8717f13` |
| 01-03 | Device-preference oracle: dark-device sessions, two-armed anti-vacuity probe | `d8facc3` |
| 02-01 | Token seam + both dark blocks; every board and rail surface re-pointed; S2 armed; re-hash #1 | `dd39f36` |
| 02-02 | Brand chrome off its three off-contract literals onto the canzan contract | `9ad2abd` |
| 03-01 | Dashboard, dialog, overlay and chrome-less screens onto the seam; `--cz-shadow` bound; S1 armed; re-hash #2 | `c411289` |
| 04-01 | Three derived OFL faces with seven-field provenance + the type tokens that make them visible; re-hash #3 | `90bfaaa` |
| 05-01 | `theme.js` ported byte-identical, loaded render-blocking before first paint | `f54e168` |
| 05-02 | The control's three states, honest accessible name, hittable target; re-hash #4 | `83f8d76` |
| 05-03 | Degraded lanes (scripting off, storage refused); settings-toast blind spot closed; re-hash #5; closeout | `526e090` |

Step 04-01 halted once at a **criterion-3 stop-gate** — the second-environment probe
found the JetBrains Mono intermediate anchor did not reproduce (host `29b51afe` vs
container `32e0cc84`), and ADR-CANZAN-THEME-002 says a varying step 2 invalidates the
stable-anchor model and stops the step. Nothing was committed until the anchor question
was decided. That is the only mid-step halt in the feature and it did what it exists
to do.

## Lessons

1. **Four tests that proved nothing, caught during delivery. This is the feature's most
   transferable lesson — every one of them was GREEN.**
   - `.column h2` was **dead CSS**: the board renders `section.column > h3`. A contrast
     assertion had been passing against an element that never rendered, clearing the
     floor by accident via inherited `--cz-text`. The rule was re-pointed at `.column h3`
     rather than left as a second dead rule beside the one D-10 had just deleted.
   - `apply_stored_theme_choice` was a **harness shim** re-stamping `data-theme` after
     navigation. Three scenarios were green off the fixture rather than off production
     `theme.js`. Deleted; they now exercise production code.
   - The **type oracle compared each face against its stack's own fallback tail**, which
     is meaningless for mono: two monospace faces agree on advance width by construction
     (45.00 vs 45.15 px — a 0.3% difference presented as a discrimination). Strengthened
     to compare against a family that *cannot exist*.
   - `.site-header` had survived **43 features** as dead CSS with no markup behind it,
     and nothing in the repo would ever have noticed. A trivial "every class in the
     stylesheet appears in at least one template" check would have caught it; it is not
     built here and is the obvious successor to S1.

   The common shape: each assertion was *correct as written* and *vacuous as executed*.
   None would have been found by reading the test; all four were found by asking what
   would have to break for it to go red.

2. **Two reproducibility defects in the font recipe, both found by RUNNING the probe
   rather than reasoning about it.** (i) `varLib.instancer` stamps `head.modified` from
   the wall clock — the anchor differed between two runs *on one machine, seconds apart*:
   5 bytes, `head.modified` plus the derived `checkSumAdjustment`, everything else
   byte-identical. Fixed by `SOURCE_DATE_EPOCH`. (ii) The IUP optimiser makes a
   float-tolerance choice about which `gvar` deltas to store explicitly; on JetBrains
   Mono it chose differently on macOS/arm64 and a Debian container for **4 of 414
   glyphs**, diverging the anchor while the font was provably identical (instancing
   either encoding at wght 400 and 500 gives byte-equal fonts, zero differing outlines).
   Fixed by `--no-optimize`, at a cost of ~200 B of unoptimised `gvar`.

   The second fix is the interesting one: `--no-optimize` **preserved ADR-002's anchor
   model unamended**. The alternative was re-anchoring the Tier-2 audit onto something
   else, which would have amended a just-accepted ADR to accommodate a tool's
   non-determinism. An anchor that changes when nothing changed proves nothing and
   trains an auditor to shrug — which is the failure ADR-002 § alternative C exists to
   refuse.

3. **The provenance model was tested by accident, twice, and held.** A 152-byte brotli
   variance across machines is exactly what ADR-CANZAN-THEME-002 predicts and precisely
   why it anchors the Tier-2 audit on the *intermediate TTF* sha256 rather than the
   woff2 output: woff2 is compressor-dependent, so an upstream-digest comparison fails
   BY DESIGN. Two claims of different strength — integrity (unconditional,
   machine-checked by R3) and provenance (expected, explicitly not guaranteed
   byte-for-byte) — is what let a real variance be *absorbed* instead of *escalated*.

4. **A guard's blind spot is a statement about the guard, not about the repo — but only
   if you say so.** S1 scans served stylesheets, so a colour written inline in a template
   `<style>` block is outside its reach entirely. This was not hypothetical:
   `templates/settings.html` carried three light-only `.toast` literals that painted a
   bright green card on an ink page for every dark-device operator. It sat in the one
   blind spot **both** instruments shared — outside S1 (inline, not served) and outside
   the rendered sweep (which walks board, dashboard, issue, shortcut list and sign-in,
   but not settings). It was fixed by **moving** the rules into the stylesheet rather
   than tokenising them in place: tokenising would have fixed the colour and left the
   blind spot. `settings.html` was the only template in the tree carrying an inline
   `<style>`, so the gap closes with it — and the *limit* remains, written down.

5. **An untested default is the worst coverage shape available.** The harness had no way
   to give a session a device colour preference, and without one a dark-mode scenario can
   only drive dark by stamping an explicit choice — at which point the
   `@media (prefers-color-scheme: dark)` block is green **whether or not it exists**,
   because the attribute selector alone satisfies the assertion. The media path is the
   *default*, the state most operators get. `--force-dark-mode` in
   `goog:chromeOptions.args` was measured end-to-end (both `matchMedia` **and** the
   computed custom property flip, so the cascade genuinely applies), and
   `--enable-features=WebContentsForceDark` was measured to **not** work — recorded so
   nobody later "fixes" the flag to it and silently returns the lane to green-over-nothing.
   The two-armed `@oracle-probe` scenario asserts both arms so the instrument cannot
   itself pass vacuously, and it is the first scenario DELIVER un-pended.

6. **Mutation survivors clustered on the scanners, for one structural reason.** 25 of 27
   survivors are off-by-one and boundary mutations inside the four hand-written byte
   scanners S1/R1/R2 are built on. They survive because **the gold tests assert the
   verdict, not the offsets**: a planted violation is still found when a cursor lands one
   byte early, and a clean tree still passes when it lands one byte late, so the mutant
   changes an intermediate the assertion never reads. That is a real, bounded limitation
   of verdict-level gold tests, recorded rather than chased.

## Measured KPI baselines (no kpi-contracts.yaml in this repo — recorded here)

- **KPI 1 — North Star** (100% of screens render dark; 0 light surfaces): baseline
  **0%**, no dark palette existed in any form. Now demonstrated by the sweep scenarios
  across board, dashboard, dialogs, overlay and sign-in. The one surface the sweep did
  not walk — settings — is the blind spot lesson 4 records and closes. The qualitative
  half (Priya's own report over a week of evening sessions) is a human input no test
  supplies and is stated rather than dressed up as instrumentation.
- **KPI 2 — Guardrail** (0 light frames per navigation): the `<script>` is render-blocking
  in `<head>` with no `defer`/`async`/`type=module`, asserted deterministically against
  the served HTML; the paint-timing comparison is recorded as a **supporting** measurement
  that can pass by luck on a fast loopback. Sampling the painted colours of the first
  frame needs a paint-level capture surface this suite deliberately does not carry — the
  gap is written down, not papered over.
- **KPI 3 — Guardrail** (both palettes meet NFR-WEBB-A11Y-02): **measured in a live
  browser**, computed from `getComputedStyle` with an ancestor walk for the effective
  background — never restated from the token comments. Recorded values:

  | Token pair | Light | Dark |
  |---|---|---|
  | `--cz-text` on `--cz-bg` | 17.62:1 | 16.33:1 |
  | `--cz-muted` on `--cz-bg` | 5.89:1 | 6.38:1 |
  | `--cz-faint` on `--cz-bg` (rebound, D-04) | 4.57:1 | 4.83:1 |
  | `--cz-jade` on `--cz-bg` | 5.08:1 | 9.74:1 |

  Baseline: light was presumed-pass and never re-measured against a new palette; dark was
  0%, there being no dark palette. canzan.net's own `--cz-faint` measures 3.24:1 / 3.52:1
  and would have failed at body size. The three text tiers remain visibly separated after
  the rebind, which was the acceptance condition for D-04 rather than the ratio alone.
- **KPI 4 — Guardrail** (0 existing acceptance files changed): met in substance —
  **3 registration lines** (`src/lib.rs` +1, `tests/acceptance.rs` +2), no scenario and no
  assertion authored upstream changed. The KPI's literal form was unsatisfiable and was
  amended in flight (Divergence 1).
- **KPI 5 — Leading** (dark reachable without an OS change): baseline **0** — impossible;
  the only route was an OS-wide change. Now ≤2 activations of one visible control from any
  app-shell screen, and a third press hands the decision back to the device.
- **KPI 6 — Leading** (foundry and canzan.net read as one product): **1 accent hue**, down
  from 3 — met. **3 of 3 type families present** — met. Colour tokens: **11 of 11 adopted
  by name, 10 of 11 identical by value**, `--cz-faint` deliberately rebound per D-04. The
  KPI as written at DISCUSS said "11 of 11 identical in value", which D-04 — decided in the
  same wave — already contradicted; recorded here as shipped rather than as claimed.
  foundry also ships one **extension** to the contract, `--cz-scrim`, for the dialog and
  overlay layer canzan.net has no equivalent of; it is `--cz-`-prefixed, named for its role,
  bound in all three regions, and is inherited by canzan-lift's eventual migration.
  (The 12th-token falsifier the roadmap predicted was `.sidebar__item--active`; the actual
  one was the scrim.)
- **KPI 7 — Guardrail** (≤150 KB added static payload; 0 cross-origin requests):
  **76,416 B measured** across all three blobs — `bricolage-grotesque.3bd3b180.woff2`
  29,788 B + `public-sans.a2bd64e2.woff2` 20,672 B + `jetbrains-mono.4e194fb3.woff2`
  25,956 B — **51% of the guardrail**, achieved by axis instancing plus latin subsetting.
  **Zero cross-origin requests**: `VENDOR.md`'s "NO CDN at runtime" makes self-hosting
  policy-required, not merely preferred, and an air-gapped operator would experience a
  font host as missing type.

## Permanent artifacts

- `docs/product/architecture/adr-canzan-theme-001-font-axis-instancing-and-subsetting.md`
- `docs/product/architecture/adr-canzan-theme-002-derived-asset-provenance-model.md`
- `docs/product/architecture/adr-canzan-theme-003-asset-integrity-guard-in-check-arch.md`
- `docs/product/architecture/adr-canzan-theme-004-token-seam-and-dark-block-parity.md`
- `docs/product/architecture/brief.md` — "Colour enters the stylesheet at one seam;
  assets are hash-honest by construction" (49 lines)
- `docs/product/jobs.yaml` — `job-canzan-theme`
- `docs/product/personas/persona-instance-operator.yaml` — **directory bootstrapped by
  this feature**, back-declaring the persona four prior feature-deltas had referenced
  inline with no file behind it
- `docs/product/journeys/journey-theme-adoption.yaml` — **directory bootstrapped by this
  feature**
- `docs/architecture/atdd-infrastructure-policy.md` — device-preference and
  degraded-capability browser sessions (driving); derived font blobs + `VENDOR.md`
  provenance rows (driven internal)
- `crates/foundry-app/static/VENDOR.md` — three row shapes with three audit procedures
  (upstream-verbatim, authored-in-tree, **derived**), the two-claim model, and the
  measured reproducibility section naming both flags and what breaks without them
- `tools/fonts/derive-fonts.sh` + `tools/fonts/requirements.txt` — the hermetic recipe
  (`SOURCE_DATE_EPOCH` + `--no-recalc-timestamp` + `--no-optimize`), reproducing
  byte-for-byte across host and container for all three families
- `docs/feature/canzan-theme-system/` — full wave history incl. `feature-delta.md`
  (DISCUSS/DISTILL/DELIVER), `intake.md`, `design/`, `slices/`, the pinned
  `canzan-net-reference.css`, and `deliver/mutation/mutation-report.md`

## Accepted limitations (shipped deliberately — inherit these as decisions, not bugs)

- **`theme-color` follows the device, never an explicit choice (D-07).** An operator on a
  light device who chooses dark gets a dark page inside browser chrome still tinted for
  light. Fixing it means `theme.js` writing the meta, which spends the byte-identical
  port that D-06 and the future shared module rest on — so the fix costs exactly the
  thing the port exists to buy, and belongs *after* the shared module, landing in both
  apps at once.
- **The `localStorage` WRITE guard has no acceptance scenario, BY CONSTRUCTION
  (Divergence 4).** Blocking site data also blocks the session cookie, so no signed-in
  screen is reachable; and the control mounts only inside `partials/sidebar.html`, which
  only `app_shell.html` includes. "Storage is refused" and "the control exists" are
  mutually exclusive — not a harness limitation. Its only protection is the review gate
  step 05-01 established: a two-line cross-repo diff against canzan-lift's `theme.js`,
  verified twice. Two workarounds were considered and rejected: a script-injected
  throwing accessor (tests the stub, not the browser) and quota-filling (a flaky oracle).
  The READ guard, which is the one with a real consequence, **is** covered — scenario 25
  drives a real storage-refused session and asserts the sign-in screen still themes from
  the device with nothing reported.
- **A signed-out visitor gets no theme control at all (D-08).** The 15 chrome-less
  templates honour an explicit choice but offer no way to make one. Flagged at DISTILL as
  an **open product question, deliberately not decided** — it is a D-09 change belonging
  to DISCUSS/DESIGN, and closing it would also happen to make the write guard drivable.
- **S1 cannot see a colour inside a template `<style>` block.** `settings.html` was the
  only template in the tree carrying one and its rules were moved under S1, so the gap
  closes with it — but nothing stops the next template from opening one.

## Open / deferred

- **Migrate canzan-lift onto the shared `--cz-*` token names** — intake D4's "one theme
  everywhere". foundry is the first adopter. Widened by D-01f: canzan-lift declares 57
  unprefixed tokens and ships **no webfont at all** by explicit decision, so unification
  is two jobs, not one. The JS is already shareable; the CSS is not yet. Must not silently
  lapse.
- **Extract `theme.js` as a genuinely shared module** — D-06 leaves it one
  parameterisation away; the remaining work is deciding where a module shared across two
  repos lives. This unblocks the `theme-color` fix above.
- **The browser lane is genuinely flaky on this machine, and it is a real latent problem
  in the harness rather than a property of any feature.** 2–4 scenarios per run, wandering,
  `WaitTimeout`-class, confined to `board-lane-management`, `form-error-display-contract`,
  `fix-comment-delete-csrf`, `issue-edit-modal-close-icon`, `instance-admin-project-rename`
  and `keyboard-shortcut-bindings` — **never this feature's scenarios**. Orphaned
  chromedrivers (PPID 1) worsen it and `pkill -f chromedriver` helps, but a solo run with a
  verified-empty process table still fails 2. Root cause identified: `browser_harness.rs`
  reaps its driver only on a clean exit, so any interrupted run leaks one. **This deserves
  its own feature.**
- **A "every stylesheet class appears in at least one template" check** — the successor to
  S1, and the thing that would have caught `.site-header` 43 features ago.
- **Extending S1 to template `<style>` blocks** — the stated limit above.
- **An automated contrast sweep** as a gate rather than a per-feature scenario.
  NFR-WEBB-A11Y-02 names one and none exists; canzan-lift has one
  (`tests/e2e/test_dark_mode.py`), which is where D-05's opaque-surface rule came from.
- **Two non-arithmetic mutation survivors** (mutation-report.md): `run -> ExitCode` with
  `Default::default()` — `ExitCode::default()` **is** `SUCCESS`, so the mutant makes
  check-arch always pass, and it survives because the gold tests call the rule functions
  directly against staged trees rather than driving `run()`. Its only guard is
  `cargo xtask ci` itself, which is not in the `-p xtask` test command. And
  `is_sha256_hex -> true`, which would let R3 accept a malformed hash string; every gold
  fixture uses well-formed hashes, and the rule still fails on a hash that is well-formed
  but wrong, which is the case R3 exists for.
- **Contributor note, not a repo problem**: `cargo xtask ci` needs PostgreSQL 16+ client
  tools; `postgresql@14` shadows `@16` on this machine's PATH. Prepend
  `/opt/homebrew/opt/postgresql@16/bin`.
