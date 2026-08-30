<!-- markdownlint-disable MD024 -->
# Feature Delta: canzan-theme-system

foundry adopts canzan.net's `--cz-*` palette and typography, gains a real dark
palette on every screen, and gains a three-state (system / light / dark) theme
control ported from canzan-lift — without renaming a single class the
acceptance suite selects on.

## Wave: DISCUSS

Agent: nw-product-owner (Luna) | Date: 2026-08-29 | Density: lean + ask-intelligent
| UX research depth: comprehensive | Feature type: user-facing | Walking skeleton: no (brownfield)

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/feature/canzan-theme-system/intake.md` | ✓ | Primary input. D1–D5 taken as given. Four of its factual premises are corrected below (D-01) — the decisions themselves stand. |
| `docs/feature/canzan-theme-system/canzan-net-reference.css` | ✓ | Pinned reference (43 KB, sha256 `44ad42b5…`), minified single-line. Token values taken from the intake's transcription; the two `--cz-shadow` layer values were initially hidden by the minification and have since been recovered from this same file (see Unresolved § Closed). |
| `docs/product/jobs.yaml` | ✓ | Read. Four prior jobs; `job-canzan-theme` appended by this wave. |
| `docs/product/architecture/brief.md` | ✓ | Read. No section governs presentation; inherited commitments listed below. |
| `docs/product/personas/` | ⊘→✓ | Did not exist. **Bootstrapped by this wave**, back-declaring `persona-instance-operator` which four prior feature-deltas referenced inline. |
| `docs/product/journeys/` | ⊘→✓ | Did not exist. **Bootstrapped by this wave** with `journey-theme-adoption`. |
| `docs/product/outcomes/registry.yaml` | ✓ | Read for shape. This feature registers no new operation or invariant — see Driving Ports. |
| `crates/foundry-app/static/VENDOR.md` | ✓ | Read. "NO CDN at runtime" makes intake D3 policy-required, not merely preferred. |
| `/Users/jeffbailey/Projects/canzan/canzan-lift/src/ui/assets/theme.js` | ✓ | Read in full (137 lines). Port source. |
| `/Users/jeffbailey/Projects/canzan/canzan-lift/src/ui/assets/app.css` | ✓ | Read (2177 lines). Two hard constraints inherited from it: the opaque-surface contract (D-05) and inline-documented contrast ratios. |
| DISCOVER artifacts | ⊘ | None. Risk noted: job grounded in code reading + operator context, not interviews. Same precedent as all four prior jobs. |
| DIVERGE artifacts | ⊘ | None. The design direction was chosen at intake (adopt canzan.net; port canzan-lift). JTBD run inside this wave. |

### [REF] Persona

**Priya Raman — self-hosting operator, instance super-admin, team member on her
own boards.** `persona-instance-operator`, now declared in
`docs/product/personas/persona-instance-operator.yaml`. Runs foundry beside
Grafana, Portainer, ArgoCD and Element, all federated through one Keycloak
realm. Member of team Backend in workspace Canzan Labs, where "Identity
Platform" (AUTH) lives. Works the board daily with both mouse and the shipped
bindings (`j`/`k`/`Enter`/`Esc`/`?`/`c`).

Three of her environments matter here, and only the first is served by a device
preference alone:

- **`env-night-triage`** — late triage from a dark room, OS set to Dark. Every
  other cluster service has gone dark; foundry has not.
- **`env-light-os-dark-room`** — the same machine kept on a Light OS by day
  because her design tooling needs it, used for a late review anyway. The
  device preference is the wrong answer for this one app at this one hour.
  *This environment is the entire reason a toggle exists.*
- **`env-hardened-profile`** — scripting disabled and/or site data blocked.

Marco (`persona-team-member-foil`) plays no part in this feature: theming has
no authorization dimension.

### [REF] JTBD

**job_id: `job-canzan-theme`** (appended to `docs/product/jobs.yaml`)

One-liner: *When I open foundry late — after Grafana, Portainer and ArgoCD have
all quietly gone dark because my machine asked them to — I want foundry to
follow that same preference, and to let me overrule it for this one app when my
device is set the other way, so I can work a long triage session without glare
and without foundry looking like the one tool that was never finished.*

All four stories below trace N:1 to this job. Forces (full text in `jobs.yaml`):

| Force | Summary | Where it lands |
|---|---|---|
| Push | No dark mode exists at all; 46 colour literals sit outside the token block; three competing accent hues | US-CTS-01, US-CTS-02 |
| Pull | canzan.net publishes a complete two-palette contract; canzan-lift ships a working three-state control | Adopt, do not invent — D-02, D-06 |
| Anxiety | The restyle silently breaks the render contract; dark ships unmeasured; the toggle introduces a flash | US-CTS-01 S5, US-CTS-01 S6, US-CTS-04 S3 |
| Habit | Board geometry is muscle memory; an untouched control must keep meaning "whatever the device says" | D-11 (zero selector churn), US-CTS-04 S1 (third press returns to System) |

### [REF] Locked Decisions

| ID | Decision | Rationale / source |
|---|---|---|
| **D-01** | **Premise corrections — code contradicts the intake in six places. The intake's *decisions* all stand; these are its *facts*.** (a) **`.site-header` renders in NO template.** Zero matches across all 32 templates. It is dead CSS at `foundry.css:56-69`. The signed-in chrome is `aside.sidebar` (`partials/sidebar.html`); the signed-out screens have no chrome at all. Intake open question 1 is premised on a stale fact. (b) **foundry does NOT carry canzan-lift's D5.** It has exactly ONE scripting-disabled scenario — `@no-js`, `keyboard-shortcut-bindings.feature:113` — covering one path (board → rail link → `/keyboard-help`), driven by a real Chrome content-setting in `browser_harness.rs:192` (`new_session_without_scripting`). Intake open question 3 is answered **NO**: there is a no-JS *lane*, not a blanket guarantee. (c) **The stylesheet is not "mostly tokenised".** 46 colour literals across 30 rules sit outside `:root`; the rail block (lines 423-518), the dashboard/card/actions block (202-332), the modal backdrop and the keyboard-help overlay use **zero** tokens. Intake open question 7 ("some may hardcode colors") understates it — this is a re-authoring, not an audit. (d) **foundry carries three accent hues today, not one:** `--accent` #2452c9 (base rules), rail indigo #5b5bd6 / #ecedff / #3a3ad1, card-key indigo #4f46e5 / #eef2ff. Canzan has one (jade). Consolidation is part of the work, not a side effect. (e) **There is no automated asset-resolution guard.** `cargo xtask check-arch` has no asset rule; the "asset-resolution probe" promised in `design/assets.md` Decision #4a is not in the tree. The hash literal is pinned in **three** places in `foundry-app/src/lib.rs` (329, 346, 365). (f) **canzan-lift shares neither the palette names nor the type strategy.** It declares 57 unprefixed tokens (`--page-bg`, `--panel`, `--signal`…), not `--cz-*`, and explicitly ships **no webfont at all** ("a typeface that arrives over the network is a typeface that sometimes does not arrive"). So "so both apps can later share one module" (intake D2) is true of the **toggle only**. | Template grep (0 hits for `site-header`/`top-nav`); `keyboard-shortcut-bindings.feature:113`, `browser_harness.rs:185-235`; literal audit of `foundry.8ce38566.css`; `xtask/src/check_arch.rs` (no asset rule); `canzan-lift/src/ui/assets/app.css:1-96,104-218`. |
| **D-02** | **Token names adopted verbatim; foundry's colour tokens are RETIRED, not aliased.** `--bg`, `--fg`, `--muted`, `--border`, `--surface`, `--accent`, `--accent-contrast`, `--danger` are deleted, and every rule that used them names a `--cz-*` token directly. An alias layer would make foundry's stylesheet a *translation* of the contract rather than an *adopter* of it, and would leave two live names for one colour for the next reader. `--radius: 6px` already matches canzan's and is kept as-is. `--gap` is layout, not colour, and stays. `--font` is replaced by `--cz-body` in US-CTS-03. Retiring names changes declaration **values** only — no selector moves (D-11). | Intake D4; the "one theme everywhere" goal is falsified by a shim. |
| **D-03** | **Two dark blocks, written out, never merged, both setting `color-scheme`.** `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { … } }` and `:root[data-theme="dark"] { … }`. Identical bindings; duplicated deliberately because a media query and an attribute selector cannot be combined into one rule meaning "either". `color-scheme: dark` in both, so native scrollbars, form widgets and the caret follow — foundry has real `<input>`/`<textarea>` surfaces that canzan.net does not. | canzan-lift `app.css:220-315`, verbatim mechanism; intake §Theming mechanism. |
| **D-04** | **foundry rebinds `--cz-faint` to a WCAG-AA-passing value in both palettes; the token name and its role in the shared contract are unchanged.** Measured at DISCUSS and confirmed: canzan.net's `--cz-faint` is **3.24:1** light (#878e89 on #fbfbf9) and **3.52:1** dark (#626a66 on #0a0c0b) — both clear 3:1 (large text, non-text) and both **fail 4.5:1** at body size, which is the size canzan.net's own eyebrow idiom uses it at (`.6875rem`, ≈11 px). foundry carries NFR-WEBB-A11Y-02 and canzan.net does not. **foundry therefore moves the value, not the structure:** light `#6e756f` (**4.57:1**), dark `#78807b` (**4.83:1**). The three-tier hierarchy canzan.net designed — text > muted > faint — survives and stays visibly separated (light 17.62 / 5.89 / 4.57; dark 16.32 / 6.38 / 4.83). Labels and eyebrows keep using `--cz-faint` exactly as canzan.net intends. All four ratios are recorded inline beside the tokens, canzan-lift's idiom. **Rejected alternative:** collapsing labels onto `--cz-muted`. It passes, but it deletes a tier from the contract and would have made foundry structurally different from canzan.net — a much worse thing to carry into unification than a corrected hex value. | NFR-WEBB-A11Y-02; ratios computed at DISCUSS and confirmed by the coordinator; re-verification required in DELIVER (KPI 3). |
| **D-05** | **Translucent tokens never sole-carry text.** `--cz-jade-soft` (rgba .10/.11) and `--cz-jade-line` (rgba .32/.34) are the contract's only non-opaque tokens. canzan-lift states the opposing rule as a hard contract with its dark-mode contrast suite, which resolves a computed background by walking up for the first non-transparent value and therefore reads a translucent panel as its *unblended* colour — rendering a perfectly readable page as a failure. foundry adopts the same discipline: any element that both bears text and takes a jade tint must ALSO declare an opaque `background-color` beneath it. Applies concretely to `.sidebar__item--active` and `.card__key` — the two tinted, text-bearing surfaces in the file. | canzan-lift `app.css` header, "EVERY SURFACE COLOUR IS OPAQUE"; forward-compatibility with any contrast suite foundry later adopts. |
| **D-06** | **`theme.js` is ported with its logic byte-identical; exactly two values differ.** `STORAGE_KEY` becomes `"foundry.theme"`, and the mount selector becomes foundry's (D-09). Both are hoisted to named constants at the top of the IIFE so a future shared module takes them as parameters. **No third divergence is permitted** — any behaviour change is a DESIGN decision, not an implementation liberty, and must be applied to both apps or to neither. The three-state cycle, the "system removes the attribute" rule, the try/catch-guarded storage, the accessible-name-carries-state pattern and the build-in-JS-not-in-the-renderer choice all transfer unchanged. | Intake D2; `canzan-lift/src/ui/assets/theme.js`. |
| **D-07** | **`<meta name="theme-color">` follows the device only, not an explicit override. Confirmed by the user as an accepted limitation.** Ship the declarative media pair (light `#fbfbf9`, dark `#0a0c0b`) and move the three off-contract literals — `base.html:11`'s `#1c1c22`, and the manifest's `theme_color: #1c1c22` / `background_color: #ffffff` — onto canzan values. Making the meta follow an explicit choice would require adding behaviour to `theme.js`, and **keeping `theme.js` a byte-identical port is the entire basis of intake D2's future shared module** — so the cost of closing this gap now is the thing the port exists to buy. **Known, accepted limitation:** *an explicit toggle override does not update browser chrome.* It is visible only in the override case, only in browser chrome, and becomes cheap to fix once the shared module exists — at which point the behaviour lands in both apps at once, which is the only correct way to add it. | D-06; `static/manifest.webmanifest` is static JSON and cannot follow a runtime choice at all. `pwa-mobile-rendering.feature:100` asserts the manifest **declares** both keys, so the values may move but the keys must stay. |
| **D-08** | **The control appears on the 11 app-shell screens and nowhere else.** The ported script returns early when its mount is absent (canzan-lift's own behaviour, unchanged). The 15 templates extending `base.html` directly — sign-in, forgot, invite-accept, bootstrap, unsubscribe, error pages — get no control. They **still honour an explicit choice**, because `data-theme` is stamped from origin-wide `localStorage` on every document. So the sign-in page is dark for an operator who chose dark, with no button on it. No new chrome is invented to host a control on 15 pages that have none. | Template inventory (11 extend `app_shell.html`, 15 extend `base.html`); `signin.html` has `<h1>` + form and nothing else. |
| **D-09** | **Mount point: the foot of the rail** — inside `.sidebar__user`, alongside the existing Notifications / Keyboard shortcuts / Sign-out group. Rationale taken verbatim from canzan-lift's stylesheet: the control belongs "at the far end of the strip, away from navigation, because it changes how the desk LOOKS and never where she is". `.sidebar__user` is already `margin-top: auto` — it *is* the rail's far end. It is also the group the acceptance suite already knows (`feature_navigation_bar.rs:210` selects `.sidebar__user`), so the control lands inside an asserted region rather than beside one. | canzan-lift `app.css:725-731`; `partials/sidebar.html:10-16`. Answers intake open question 1 on corrected facts (D-01a). |
| **D-10** | **The dead `.site-header` / `.site-header .brand` rules are deleted** in US-CTS-01. Verified: no template renders the class, no acceptance selector references it. Giving it a dark palette would be the only work in this feature whose result could never be seen. | D-01a. |
| **D-11** | **Selector churn is zero.** No class and no `data-*` attribute in the render contract is renamed, removed, or newly required. `.theme-toggle`, `.theme-toggle__glyph` and `.theme-toggle__mode` are the only additions, and they are new markup, not renamed markup. Measurable form: **no existing *scenario or assertion* under `crates/foundry-acceptance/` is changed** across the whole feature. The only permitted edits to existing files are the registration lines a new step module structurally requires (`pub mod` in `src/lib.rs`, force-link in `tests/acceptance.rs`); without them the module never compiles into the test binary. Verified precedent: `board-lane-management`'s landing commit `1f100bf` touched 27 files under `crates/foundry-acceptance/` including `world.rs` and 18 existing step modules; this feature's diff is 3 registration lines. **Amended 2026-08-29** — as originally written ("no existing file is modified") the criterion was mechanically unsatisfiable by any feature. | `foundry.css:1-10` render-contract statement; the selector inventory in Shared Artifacts. |
| **D-12** | **Walking skeleton: NO** (confirmed at dispatch). Brownfield, 47 shipped features, running app: the stylesheet is already authored, already served by `ServeDir`, already content-hashed, already cache-policied and already asserted against. There is no end-to-end path left to prove. Delivery is four slices in outcome order, each independently shippable and each visible. | Dispatch Decision 2. |
| **D-13** | **Fonts are folded into the typography slice and are never vendored alone.** A slice that adds three woff2 blobs plus VENDOR.md rows changes nothing a user can see and would fail the slice-composition gate as `@infrastructure`-only. US-CTS-03 vendors **and** applies them in one shippable step. | Elephant Carpaccio slice-composition hard gate. |
| **D-14** | **No screen ever renders invisible text while a font loads.** The system stack paints immediately and the canzan face swaps in; a font that never arrives costs a typeface, never a blank board. (Format/subset choices are technical notes for DESIGN, not requirements — see System Constraints.) | Material honesty: a self-hosted asset on the same origin as the HTML will arrive, but the failure must still be benign. |

### [REF] Journey

Full journey: `docs/product/journeys/journey-theme-adoption.yaml` (5 steps, 9
shared artifacts, per-step failure modes and integration checkpoints).

Emotional arc: **Problem Relief** — braced (she has pre-flinched at the 23:40
white page for months) → surprised, then curious (it arrived dark without
asking; the *rail* is dark too, which is the thing half-done dark modes get
wrong) → at home and in control (the one decision she might still want has a
visible control that can also hand the decision back).

```text
[Trigger]                [Step 1]              [Step 2]               [Step 3]                [Goal]
23:40. Every other  →  Opens the AUTH   →  j,j,Enter,Esc,?   →  On a light-OS      →  She stops thinking
service on the         board. Frame,        Ring, cards and      night, presses        about theme at all.
cluster went dark      rail, columns        ? overlay all        ◐ System → ☀ →       /report and / match;
because the OS         and cards paint      legible on ink.      ☾ Dark. Board        the choice survives
asked. foundry is      on ink #0a0c0b.      Geometry has         repaints. Third      reload and navigation.
still #ffffff.         Feels: surprised     not moved.           press hands it       Feels: settled
Feels: braced          Sees: no white       Feels: trusting      back to the device.  Sees: zero light
                       stripe on the left   Sees: jade ring      Feels: in control    frames, anywhere
```

Error paths, all owned by named scenarios:

| Path | Outcome | Owner |
|---|---|---|
| Scripting disabled | The control does not exist (rather than existing dead); the page follows the OS exactly as before the feature | US-CTS-04 S4 |
| `localStorage` throws (site data blocked) | The mode still applies for the session; only persistence is lost, silently | US-CTS-04 S5 |
| Mount selector absent (a non-shell screen) | The script returns early; no control, no error, palette still correct | D-08, US-CTS-04 S4 |
| A font never arrives | The system stack keeps painting; no invisible text | D-14, US-CTS-03 S5 |
| A surface was missed in the audit | A light rectangle in a dark app — the failure the sweep scenarios exist to catch | US-CTS-01 S6, US-CTS-02 S5 |

### [REF] Scope Assessment: PASS — 4 stories, 1 module cluster, estimated 3.5 days

Signals checked against the oversized set (any 2+ fires a split):

| Signal | Threshold | Actual | Fired |
|---|---|---|---|
| Story count | >10 | 4 | no |
| Bounded contexts / modules | >3 | 1 — the `foundry-app` presentation surface (`static/`, `templates/base.html`, `templates/partials/sidebar.html`, 4 test literals in `src/lib.rs`) | no |
| Walking-skeleton integration points | >5 | n/a — no skeleton (D-12) | no |
| Estimated effort | >2 weeks | ~3.5 days | no |
| Independent shippable outcomes | multiple that could ship separately | 4 — and they DO ship separately, as four slices. This is carpaccio, not oversizing: they share one job and one release narrative. | no |

Zero signals fired. No split proposed. Slice composition verified: **no slice is
`@infrastructure`-only** — the fonts-vendoring work that would have been one is
folded into US-CTS-03 per D-13.

### [REF] Shared Artifacts

| Artifact | Source of truth | Consumers | Risk |
|---|---|---|---|
| `--cz-*` token contract (11 colours + radius/gutter/shadow) | `canzan-net-reference.css` (pinned, sha256 `44ad42b5…`), transcribed into `foundry.<hash>.css` `:root` | Every rule in `foundry.css`; canzan-lift's future migration (D4 follow-up) | **HIGH** — a transcription typo makes foundry silently off-brand and the "one theme" claim false, with no test to notice |
| CSS content hash `<hash>` | The file's own sha256, first 8 hex, in its committed filename | `templates/base.html:6` `<link>`; the `VENDOR.md` row; **three** literals in `foundry-app/src/lib.rs` (329, 346, 365) | **HIGH** — five hand-maintained sites; **no automated guard existed at DISCUSS** (D-01e), and ADR-CANZAN-THEME-003's **R2** rule (filename's 8-hex prefix must equal the file's own sha256) closes it *within this feature* as DELIVER obligation #0. Until R2 lands the discipline is manual. A split commit is red on the cache-policy tests and ships a stale immutable-cached URL |
| Render contract (semantic classes + `data-*`) | `foundry.css:1-10` header + the selector constants under `crates/foundry-acceptance/src/` | Every acceptance step file. Classes: `.app-shell`, `.board`, `.column`, `.issue-card`, `.comment*`, `.modal*`, `.modal-close`, `.full-page-link`, `.sidebar`, `.sidebar__nav`, `.sidebar__item`, `.sidebar__user`, `.keyboard-help`, `.search-result(s)`, `.dash`, `.card`, `.card__key`, `.card__title`, `.actions`, `.error`, `.title`, `.report-event`, `.change-event`, `.transition`, `.actor-count`, `.kb-selection-instruction`. Attributes: `data-column`, `data-issue-key`, `data-comment-list`, `data-kb-ready`, `data-kb-selected`, `data-modal`, `data-action`, `data-error-slot`, `data-hx-fragment`, `data-state`, `data-status`, `data-lane-count` and ~30 more | **HIGH** — the restyle's hard boundary. This is what D-11 protects |
| `data-theme` on `<html>` | `theme.js` at `<head>` parse time, seeded from `localStorage` | The `:root[data-theme="dark"]` block; the `:not([data-theme="light"])` guard; every screen | **HIGH** — landing after first paint converts every navigation into a white flash for exactly the operators the feature exists for |
| Theme state vocabulary `system\|light\|dark` | `theme.js` `ORDER` | Attribute values; the `:not()` guard; the button's glyph, word and accessible name | **HIGH** — "system removes the attribute" and the stylesheet's `:not()` guard are **one mechanism written in two files** |
| Storage key `foundry.theme` | `theme.js` `STORAGE_KEY` const | `localStorage` only | MEDIUM — must differ from canzan-lift's `canzan-lift.theme`; a shared module must parameterise it (D-06) |
| Toggle mount selector | `theme.js` `MOUNT` const → `.sidebar__user` | The 11 app-shell templates | MEDIUM — a wrong selector yields **no control, silently**, because the ported script returns early by design |
| Brand chrome colour | `<meta name="theme-color">` pair in `base.html:11` + static `theme_color`/`background_color` in `manifest.webmanifest` | OS/browser chrome; PWA splash; `feature_pwa_mobile.rs:883` (S10) and `pwa-mobile-rendering.feature:100` | MEDIUM — three off-contract literals (`#1c1c22` twice, `#ffffff` once). The manifest cannot follow a runtime choice (D-07). The existing assertion is **lenient** — `document.querySelector('meta[name="theme-color"]')` takes the first match and checks its content is non-empty — so a media-scoped pair is safe and D-11 is not violated. The manifest's **keys** must remain even though the values move |
| Font blobs + sha256 + upstream URL + licence | `static/VENDOR.md` rows | The `@font-face` `src` paths; any auditor or air-gapped operator | **HIGH** — VENDOR.md's entire purpose. An unrecorded blob breaks the audit silently |

### [REF] Story Map

**User**: Priya Raman (`persona-instance-operator`) · **Goal**: work a late
triage session in the light the room needs, in an app that reads as canzan.

| Arrive | Read and act | Adjust the light | Move between screens | Come back |
|---|---|---|---|---|
| Board + rail paint in the device's palette · **S01** | Cards, columns, selection ring legible in both palettes · **S01** | *(nothing — the device decides)* · **S01** | Report + settings shells match · **S01** | The device preference is honoured, always · **S01** |
| Dashboard, dialogs, overlay, sign-in paint too · **S02** | `?` overlay and the new-issue dialog legible in both · **S02** | | Dashboard matches · **S02** | |
| The type is canzan's · **S03** | Issue keys and keycaps in mono · **S03** | | | |
| | | A three-state control at the foot of the rail · **S04** | No frame of the wrong palette on navigation · **S04** | The explicit choice persists; scripting-off and blocked-storage degrade cleanly · **S04** |

**Walking skeleton**: none (D-12). Every activity is already served
end-to-end by shipped code; this feature changes how it looks, not whether it
connects. S01 is nonetheless the *foundation* slice — S02 and S04 both consume
the token block and the two dark blocks it introduces.

#### Release slices

| Slice | Story | Outcome targeted | KPI | Effort |
|---|---|---|---|---|
| **S01** | US-CTS-01 | A dark-preferring operator's board **and rail** are dark | KPI 1, 3, 4, 6 | 1 d |
| **S02** | US-CTS-02 | No screen is left light-only — the theme is finished, not partial | KPI 1, 3 | 0.75 d |
| **S03** | US-CTS-03 | foundry reads in canzan's voice, from its own origin | KPI 6, 7 | 1 d |
| **S04** | US-CTS-04 | An operator on a light-set device can overrule it, per app | KPI 2, 5 | 0.75 d |

#### Priority Rationale

Order is **S01 → S02 → S03 → S04**, by outcome impact and dependency, not by
effort or by visibility.

1. **S01 first** because it is the only slice that moves the north-star KPI from
   zero: today no dark palette exists in any form, so a dark-preferring operator
   is unserved on every screen. S01 also introduces the token block and both
   dark blocks, which S02 and S04 consume — it is the only hard dependency edge
   in the feature.
2. **S02 second** because S01 alone ships a *partial* dark mode, and a partial
   dark mode is worse than a coherent light one: the `?` overlay is a reflex
   press, and a white card over a dark board is the most visible defect this
   feature could ship. S02 closes it. S01+S02 together fully deliver the
   north star for `env-night-triage`.
3. **S03 third** because typography moves the coherence KPI (6) but not the
   comfort one (1). It is independent of every other slice and could move if
   something forces it, which is exactly why it should not go earlier: nothing
   depends on it.
4. **S04 last**, despite being the most demonstrable, for two reasons. It serves
   the narrower environment (`env-light-os-dark-room`) — the OS already answers
   for `env-night-triage`. And the flash risk (KPI 2) is *introduced* by S04 and
   contained in S04: before the toggle exists, `prefers-color-scheme` resolves
   inside a render-blocking stylesheet and cannot flash. Landing S04 on a
   codebase where both palettes are already complete means the toggle has
   nothing half-painted to reveal.

Deliberately rejected ordering: **toggle-first**. It is the flashiest demo and
the strongest instinct. It was rejected because a toggle over an incomplete dark
palette makes the incompleteness *interactive* — the operator would press a
button and be shown the gaps, which is a worse first impression than a device
preference she never chose to test.

### [REF] User Stories

Four stories, one per slice, all tracing N:1 to `job-canzan-theme`.

#### US-CTS-01: The board and its rail wear canzan's palette, in whichever light the device asks for

##### Elevator Pitch

- **Before:** Priya's macOS is set to Dark. She opens
  `/team/backend/project/identity-platform` at 23:40 and gets a `#ffffff` page —
  foundry is the only service on her cluster that ignores the setting, and the
  stylesheet's own header states dark mode is out of scope.
- **After:** she opens the same URL and the board renders on ink (`#0a0c0b`)
  with `#131817` cards, `#e8ebe8` text, jade `#62c9a6` links and a jade
  selection ring — and the sidebar rail is dark too, not a white stripe down the
  left.
- **Decision enabled:** whether to keep triaging tonight. She can read AUTH-7
  without turning the monitor down or reaching for a contrast extension that
  mangles the column layout.

##### Problem

Priya works the AUTH board most evenings from a dark room, on a machine set to
Dark. Grafana, Portainer, ArgoCD and Element all follow that. foundry does not,
and cannot: all 637 lines of its stylesheet were authored light-only, and 46 of
its colour values are literals sitting outside the token block, so even the
parts that look themeable are not. Her only workarounds are dimming the display
(which affects every window) or a browser extension (which reflows the board).
She experiences this as the tool having been left unfinished — which is
reinforced by foundry carrying three unrelated accent hues that appear nowhere
else in canzan.

##### Domain Examples

1. **Happy path (device says dark)** — Priya's OS is Dark. She opens
   `/team/backend/project/identity-platform`. The page frame, the rail, the four
   lane columns and the AUTH-7 / AUTH-12 / AUTH-3 cards all render on the ink
   palette. The rail's "Board" item shows as active in jade, not indigo. Nothing
   on screen is `#ffffff`.
2. **Edge (device says light)** — Marco's shared workstation is set to Light.
   The same board renders on paper `#fbfbf9` with `#ffffff` cards, `#121614`
   text and jade `#1a7a5e` links — the canzan palette, not foundry's old blue.
   The board's geometry is pixel-identical to the day before.
3. **Boundary (explicit choice with no control yet)** — an operator who set
   `foundry.theme = "dark"` in `localStorage` by hand (the toggle ships in
   US-CTS-04) sees `<html data-theme="dark">` and gets the ink palette on a
   Light OS. The attribute path works before the button that drives it exists.

##### UAT Scenarios (BDD)

###### Scenario: A dark-preferring operator's whole board is dark, rail included

- Given Priya's device preference is dark
- When she opens the "Identity Platform" board
- Then the page background, the sidebar rail, the lane columns and every issue card render in the dark palette
- And no surface on the screen renders in a light palette colour

###### Scenario: A light-preferring operator sees canzan's paper-and-jade palette

- Given the device preference is light
- When the "Identity Platform" board is opened
- Then the page renders on canzan's paper background with canzan's jade accent
- And foundry's former blue and indigo accents appear nowhere on the screen

###### Scenario: An explicit dark choice overrules a light device

- Given the document carries an explicit dark theme choice
- And the device preference is light
- When the "Identity Platform" board is opened
- Then the board renders in the dark palette

###### Scenario: An explicit light choice overrules a dark device

- Given the document carries an explicit light theme choice
- And the device preference is dark
- When the "Identity Platform" board is opened
- Then the board renders in the light palette

###### Scenario: Everything Priya already knows how to do still works and still looks the same shape

- Given the restyled board with AUTH-12 in the In-Progress lane
- When Priya presses `j` to select AUTH-12, `Enter` to open it and `Esc` to close it
- Then AUTH-12 carries the keyboard selection ring as a visible outline in both palettes
- And the selection ring adds a shape, not only a colour, so it reads without colour vision
- And every card sits in the same column at the same position as before the restyle

###### Scenario: Text and controls stay legible in both palettes

- Given the restyled stylesheet
- When every foreground-on-background pair used by the board and rail is measured
- Then body-size text pairs reach at least 4.5:1 in both palettes
- And large text and control boundaries reach at least 3:1 in both palettes
- And the measured figure for each pair is recorded beside the token that produced it

##### Acceptance Criteria

- [ ] Every colour used by the board, the rail and the base element rules resolves from the `--cz-*` token block; after this story no rule in those blocks writes a colour literal (D-02, D-01c).
- [ ] Both dark blocks exist, written out separately, each setting `color-scheme`; removing the `data-theme` attribute returns control to the device (D-03).
- [ ] `--cz-faint` is bound to `#6e756f` light / `#78807b` dark, so the contract's third tier passes 4.5:1 at label size in both palettes while staying visibly separated from `--cz-muted`. All six tier ratios (text / muted / faint × both palettes — light 17.62 / 5.89 / 4.57, dark 16.32 / 6.38 / 4.83) are recorded as inline comments beside the tokens (D-04).
- [ ] No text-bearing surface takes a translucent jade tint without an opaque background declared beneath it (D-05).
- [ ] The keyboard selection ring remains an `outline` — never a background or border swap — so it costs no layout space, survives forced-colours mode, and reads without colour. Its contrast rationale comment is rewritten for the jade pairs (5.08:1 light, 9.74:1 dark), replacing the stale `#2452c9 on #ffffff ≈ 7:1` claim.
- [ ] The dead `.site-header` and `.site-header .brand` rules are deleted (D-10).
- [ ] The stylesheet header comment no longer claims the file is "NOT a design system, theming, or dark mode"; it states the three-state mechanism and the opaque-surface rule instead (intake Q5).
- [ ] The three off-contract brand-chrome literals are retired: `base.html:11`'s single `<meta name="theme-color" content="#1c1c22">` becomes a media-scoped pair (light `#fbfbf9`, dark `#0a0c0b`), and the manifest's `theme_color` / `background_color` move off `#1c1c22` / `#ffffff` onto canzan values. **Both manifest keys remain declared** — `pwa-mobile-rendering.feature:100` asserts their presence, not their values (D-07).
- [ ] **No existing file under `crates/foundry-acceptance/` is modified.** The full acceptance suite passes unchanged, including S10, whose `querySelector` on `meta[name="theme-color"]` takes the first match and checks only that its content is non-empty (D-11).
- [ ] The stylesheet's new content hash is updated in the same commit in `base.html`, the `VENDOR.md` row, and all three literals in `foundry-app/src/lib.rs` (D-01e).

##### Size

1 day | 6 scenarios | job_id: `job-canzan-theme` | slice `S01`

#### US-CTS-02: Every remaining screen matches — dashboard, dialogs, shortcut overlay, sign-in

##### Elevator Pitch

- **Before:** with the board dark, pressing `?` opens the shortcut list as a
  white card floating over an ink board, and clicking Home lands on a dashboard
  whose project cards are still `#ffffff` on `#e5e7eb` — two screens of glare
  inside a dark app.
- **After:** pressing `?` on the AUTH board opens the shortcut list on `#131817`
  with `#e8ebe8` text and mono keycaps on `#1f2523`; clicking Home renders the
  project cards on the same dark surfaces with `AUTH` keyed in jade; opening the
  new-issue dialog with `c` gives a dark card over a dark scrim.
- **Decision enabled:** whether the dark theme is finished enough to leave on.
  She stops mentally routing around the two screens that still flash white,
  which is the difference between a feature she uses and one she tolerates.

##### Problem

US-CTS-01 leaves foundry's four non-board surface groups untouched, because
none of them ever used a token: the dashboard block (`.dash*`, `.card*`,
`.actions*`, 21 literals), the modal backdrop and dialog shadow (2), the
keyboard-help overlay (6), and the sign-in and other chrome-less screens. In a
dark app these render as light rectangles. A half-dark interface is worse than
a coherent light one — the glare is now *unexpected*, which is what makes it
feel broken rather than merely bright.

##### Domain Examples

1. **Happy path (the reflex press)** — Priya, on a dark board, presses `?`. The
   shortcut list renders on `#131817` with `#e8ebe8` definitions and JetBrains
   Mono keycaps on `#1f2523` hairlined in `#2e3733`. The scrim behind it is a
   dark translucent layer, not `rgba(28,28,34,.45)` over ink.
2. **Edge (the tinted key)** — she clicks Home. The dashboard lists "Identity
   Platform" and "Homelab Ops" as cards on `#131817`; the `AUTH` key chip is
   jade on an **opaque** tinted surface, not `#4f46e5` on `#eef2ff`, and not a
   translucent jade wash (D-05).
3. **Boundary (no chrome to theme)** — she signs out. `/signin` extends
   `base.html` directly and has no rail, no header and no toggle. It still
   renders on ink, because the explicit choice is stamped from origin-wide
   storage on every document (D-08).

##### UAT Scenarios (BDD)

###### Scenario: The shortcut overlay is legible over a dark board

- Given Priya is on the "Identity Platform" board in the dark palette
- When she opens the keyboard shortcut list
- Then the list, its keycaps and the scrim behind it all render in the dark palette
- And the shortcut text and keycap text each reach at least 4.5:1 against the surface behind them

###### Scenario: The signed-in dashboard matches the board

- Given Priya is in the dark palette
- When she opens the dashboard at "/"
- Then the project cards, the section labels and the action controls render in the dark palette
- And the "AUTH" project key renders on an opaque tinted surface, not a translucent one

###### Scenario: The new-issue dialog and its backdrop are dark

- Given Priya is on the "Identity Platform" board in the dark palette
- When she opens the new-issue dialog
- Then the dialog card, its label, its text input and the backdrop behind it all render in the dark palette
- And the input's own text and caret are legible without selecting the field

###### Scenario: A screen with no chrome still honours the chosen theme

- Given an operator whose explicit theme choice is dark
- When she opens the sign-in page, which has no rail and no theme control
- Then the sign-in page renders in the dark palette

###### Scenario: No surface anywhere is left light-only

- Given the restyled stylesheet
- When every rule in the file is checked
- Then no rule outside the token block writes a colour value
- And every screen foundry renders has a defined appearance in both palettes

##### Acceptance Criteria

- [ ] All 46 pre-existing colour literals outside `:root` are gone; after this story the file contains colour values **only** inside the token block and the two dark blocks (D-01c, and the canzan-lift discipline that makes a palette a re-binding of names rather than a second stylesheet).
- [ ] The dashboard, the modal backdrop and dialog, the keyboard-help overlay, and the `.actions` controls each have a defined appearance in both palettes.
- [ ] `.card__key` and `.sidebar__item--active` — the two tinted, text-bearing surfaces — declare an opaque background beneath any jade tint (D-05).
- [ ] Form inputs and textareas inherit the palette, including caret and placeholder, via `color-scheme` (D-03) rather than per-property overrides.
- [ ] The 15 templates that extend `base.html` directly render correctly in both palettes with no control present (D-08).
- [ ] `.card a:hover` and `.modal-dialog` take `--cz-shadow`, bound to canzan.net's two-layer value and re-bound in dark — light `0 1px 2px rgba(18, 22, 20, 0.04), 0 8px 24px rgba(18, 22, 20, 0.05)`, dark `0 1px 2px rgba(0, 0, 0, 0.4), 0 12px 32px rgba(0, 0, 0, 0.32)`. The dark binding is deeper and wider deliberately: a shadow that reads on paper disappears on ink.
- [ ] Every newly-bound pair's measured contrast is recorded inline beside its token, in both palettes.
- [ ] **No existing *scenario or assertion* under `crates/foundry-acceptance/` is changed** (registration lines excepted — see D-11) (D-11).

##### Size

0.75 day | 5 scenarios | job_id: `job-canzan-theme` | slice `S02` | depends on US-CTS-01

#### US-CTS-03: foundry reads in canzan's voice — Bricolage, Public Sans and JetBrains Mono, from foundry's own origin

##### Elevator Pitch

- **Before:** foundry renders in whatever UI font the operating system supplies.
  Put beside canzan.net it reads as a different product, and the board's column
  headers are the only typographic signal the interface has.
- **After:** opening the AUTH board shows "Identity Platform" set in Bricolage
  Grotesque, card titles in Public Sans, and `AUTH-7` and the `j`/`k` keycaps in
  JetBrains Mono — all served from `/static/fonts/`, with a request log showing
  zero calls to `fonts.gstatic.com`.
- **Decision enabled:** whether foundry can be shown alongside canzan.net and
  canzan-lift as one product. She can put two windows side by side and answer
  it — which is the social dimension of the job, and the only one a palette
  alone does not close.

##### Problem

Colour is half of canzan's identity; the type is the other half, and it is the
half a stranger notices first. foundry has no typographic system at all: three
different font-family declarations (a system sans in `--font`, a second system
sans hardcoded on `.dash`, and `ui-monospace` on two rules), no `@font-face`
anywhere in the repo, and no font blobs under `static/`. Meanwhile `VENDOR.md`
forbids a CDN at runtime — so the Google Fonts route is not merely
discouraged, it is prohibited, and self-hosting is the only compliant path.

##### Domain Examples

1. **Happy path** — Priya opens the AUTH board. The project heading renders in
   Bricolage Grotesque, the card titles in Public Sans, and the `AUTH-7` key in
   JetBrains Mono. She opens the network panel: three font requests, all to
   `foundry.internal/static/fonts/`, none cross-origin.
2. **Edge (the eyebrow idiom, at a corrected value)** — the "IN-PROGRESS" column
   header and the dashboard's "PROJECTS" label take canzan's eyebrow idiom
   exactly as canzan.net writes it: mono, small, wide-tracked, uppercase, in
   `--cz-faint`. The token is the same and the tier is the same; only the value
   moved (`#6e756f` light, `#78807b` dark) so that the idiom clears 4.5:1 at
   ≈11 px in both palettes (D-04).
3. **Boundary (the font does not arrive)** — an operator opens the board from a
   profile that blocks font loading. Every string still renders, in the system
   fallback stack, at the correct size and weight. No blank text, no missing
   glyphs, no reflowed columns.

##### UAT Scenarios (BDD)

###### Scenario: Headings, body text and keys each carry their intended typeface

- Given the restyled stylesheet with canzan's type stacks
- When Priya opens the "Identity Platform" board
- Then the project heading renders in the canzan display face
- And the card titles render in the canzan body face
- And the "AUTH-7" issue key renders in the canzan mono face

###### Scenario: No request for a typeface ever leaves foundry's own origin

- Given a fresh browser profile with an empty cache
- When Priya loads the board and then the dashboard
- Then every font requested is served from foundry's own origin
- And no request is made to any external font host

###### Scenario: Column and section labels are legible at label size

- Given the board renders its lane headers in the canzan label idiom
- When the header text colour is measured against the surface behind it
- Then it reaches at least 4.5:1 in both palettes

###### Scenario: Every served font blob is provenance-recorded and verifiable

- Given the vendored font files
- When an auditor recomputes each blob's checksum
- Then each matches the value recorded for it, alongside its upstream source and its licence

###### Scenario: A typeface that has not arrived costs a typeface, never a word

- Given a browser that has not yet loaded, or cannot load, the canzan typefaces
- When Priya opens the board
- Then every string is rendered immediately in the fallback stack
- And no text is invisible at any point during loading

###### Scenario: The board does not reflow when the typefaces arrive

- Given the board rendered in the fallback stack
- When the canzan typefaces finish loading and are applied
- Then the lane columns and issue cards occupy the same positions as before

##### Acceptance Criteria

- [ ] Three type roles exist as tokens — display, body, mono — each with a system fallback stack, and every `font-family` in the file names one of them. The three ad-hoc stacks (`--font`, `.dash`'s duplicate, the two bare `ui-monospace` uses) are gone (D-02).
- [ ] Font blobs are served from `/static/` by the existing `ServeDir` route. No CDN, no `@import` from a font host, no build step, no minifier (`VENDOR.md` DB6).
- [ ] Each blob has a `VENDOR.md` row carrying version, upstream canonical URL, retrieval date, sha256 **and licence**. Fonts are the first vendored blobs since htmx; the row shape follows the existing vendored-blob rows, not the "authored in-tree" row.
- [ ] Text is never invisible while a font loads (D-14), and the fallback stack is close enough in metrics that applying the webfont does not move the board's columns or cards.
- [ ] The label/eyebrow idiom uses `--cz-faint` — canzan.net's own token for the tier — at the rebound value that clears 4.5:1 at label size in both palettes (D-04).
- [ ] **No existing *scenario or assertion* under `crates/foundry-acceptance/` is changed** (registration lines excepted — see D-11) (D-11).
- [ ] The stylesheet re-hash propagates to all five sites (`base.html`, `VENDOR.md`, three literals in `src/lib.rs`) in one commit.

##### Size

1 day | 6 scenarios | job_id: `job-canzan-theme` | slice `S03` | independent of S02 and S04

#### US-CTS-04: One control, three states — follow the device, or overrule it

##### Elevator Pitch

- **Before:** Priya keeps her OS on Light because her design tooling needs it,
  so foundry is white all evening. The only way to get a dark board is to change
  the whole operating system, which changes every other window too.
- **After:** a control at the foot of the sidebar rail reading `◐ System`. One
  click makes it `☀ Light`, a second `☾ Dark` and the board repaints to ink, a
  third returns it to `◐ System` and the device decides again — and the choice
  survives a reload and a jump to `/report`, with no frame of white in between.
- **Decision enabled:** how foundry should be lit *right now*, independently of
  the device — and, by cycling back to System, how to hand that decision back
  rather than owning it forever.

##### Problem

After US-CTS-01 through 03, foundry answers the device correctly and completely.
It still cannot answer Priya. Her device says Light because her design tooling
needs it; the room says otherwise at 22:00. A two-state toggle would not solve
this — once pressed, it can never return the decision to the device, which is
the setting she wants most of the time. canzan-lift solved exactly this and
shipped the solution; foundry needs the same mechanism, in the same shape, so
the two can later become one module.

The pre-paint concern is the load-bearing risk in this story. Five scripts
already sit in `base.html`'s head and **all five correctly carry `defer`**.
`theme.js` must not. It is the only render-blocking script foundry will have,
and copying the surrounding lines is the single most likely regression in this
feature.

##### Domain Examples

1. **Happy path (the cycle)** — Priya's OS is Light. She opens the AUTH board
   and clicks the rail control twice: `◐ System` → `☀ Light` → `☾ Dark`. The
   board repaints to ink on the second click. She clicks a third time and it
   reads `◐ System` again; the board returns to paper, because the device says
   Light.
2. **Edge (persistence across navigation)** — still on Dark-by-choice, she
   clicks "Change report", then Home, then reloads. All three arrive dark, with
   no white frame at any point. She closes the tab and reopens foundry the next
   evening: still dark.
3. **Error/boundary (the hardened profile)** — she opens the same board from a
   profile with scripting disabled. No control appears at the foot of the rail
   — not a greyed one, not a dead one, none — and the page follows the OS
   exactly as it did before this feature existed. From a second profile with
   site data blocked, the control **is** present and does change the palette;
   only the choice's survival across a reload is lost, and nothing reports an
   error.

##### UAT Scenarios (BDD)

###### Scenario: The control cycles through following the device, light, dark, and back to following the device

- Given Priya is on the "Identity Platform" board and has never used the theme control
- And the control shows that foundry is following her device
- When she activates it once, then again, then a third time
- Then it moves to light, then to dark, then back to following her device
- And on each step the page repaints to the palette the control names

###### Scenario: A chosen theme survives navigation and reload

- Given Priya has chosen dark while her device prefers light
- When she navigates to the change report, then to the dashboard, then reloads
- Then every one of those pages renders dark

###### Scenario: A chosen dark page never flashes light

- Given Priya has chosen dark while her device prefers light
- When she navigates to any foundry page
- Then the correct palette is already in effect when the page is first painted
- And no light-palette frame is rendered at any point during the navigation

###### Scenario: With scripting disabled the control does not exist and the device decides

- Given a browser session with scripting disabled
- When Priya opens the "Identity Platform" board
- Then the board renders in the palette her device prefers
- And no theme control is present anywhere on the page

###### Scenario: With site data blocked the control still works for this session

- Given a browser session that refuses access to site storage
- When Priya activates the theme control and selects dark
- Then the page repaints to dark
- And no error is surfaced to her
- And the choice is not expected to survive a reload

###### Scenario: The control says which theme is active and which the next press will select

- Given the control currently shows that foundry is following the device
- When its accessible name is read
- Then it states that foundry is following the device and names the theme the next press selects
- And after each press the name updates to describe the new state and the next one

###### Scenario: The control is reachable and large enough to hit

- Given the board rendered at desktop width and again at phone width
- When Priya reaches the control by keyboard and by touch
- Then it is focusable in document order with a visible focus indicator
- And its target is at least 24×24 at desktop width and at least 44×44 at phone width

##### Acceptance Criteria

- [ ] Three states, cycling `system → light → dark → system`. "System" **removes** the attribute rather than writing a third value — the mechanism the stylesheet's `:not([data-theme="light"])` guard depends on (D-03, D-06).
- [ ] The theme attribute is on `<html>` **before first paint on every navigation**. The script is a plain external `<script>` in `<head>` with **no** `defer`, `async` or `type="module"`, and a comment at the tag states why it differs from the five deferred scripts beside it.
- [ ] The control is built in JavaScript and never server-rendered, so with scripting off it is absent rather than dead (D-06). A scripting-disabled scenario asserts this — foundry's **second** such scenario; it does not assume a blanket no-JS guarantee that does not exist (D-01b).
- [ ] Every storage access is guarded; a storage failure costs persistence, never the control, and surfaces no error (D-06).
- [ ] The control's accessible name carries the active state **and** the next state, updated on every press; the glyph is decorative and hidden from assistive technology.
- [ ] The control mounts at the foot of the rail and is absent from the 15 chrome-less screens, which still honour an explicit choice (D-08, D-09).
- [ ] Ported logic is byte-identical to canzan-lift's apart from the storage key and the mount selector, both hoisted to named constants (D-06). A reviewer can diff the two files and see exactly two differing lines.
- [ ] The control joins the mobile touch-target rule (≥44 px at ≤480 px) and meets ≥24×24 at desktop width; its `:focus-visible` ring is visible in both palettes.
- [ ] `theme.js` gets a `VENDOR.md` row in the **authored-in-tree** shape (like `foundry.<hash>.css`), not the vendored-blob shape — it is app-owned, ported by hand, not a pinned upstream release.
- [ ] **No existing *scenario or assertion* under `crates/foundry-acceptance/` is changed** (registration lines excepted — see D-11) (D-11).

##### Size

0.75 day | 7 scenarios | job_id: `job-canzan-theme` | slice `S04` | depends on US-CTS-01 (both dark blocks)

### [REF] System Constraints

- **The render contract is the hard boundary.** foundry's semantic classes and
  `data-*` markers double as the selectors the acceptance suite uses. No
  existing one may be renamed, removed or newly required. `.theme-toggle*` are
  the only additions in the whole feature (D-11).
- **After this feature, no rule beneath the token block writes a colour.** This
  is what makes a palette a re-binding of names rather than a second stylesheet
  to keep in sync, and it is why no component can redefine a colour locally.
  Inherited verbatim from canzan-lift.
- **Two dark blocks, written out, never merged**, both setting `color-scheme`
  (D-03).
- **Translucent tokens never sole-carry text** (D-05). Any jade-tinted,
  text-bearing surface declares an opaque background beneath.
- **Measured contrast is recorded inline beside the token that produced it**, in
  both palettes — canzan-lift's idiom, so the next reader can check the
  arithmetic rather than trust it.
- **Re-hash discipline. No automated guard existed at DISCUSS; ADR-CANZAN-THEME-003's R2 builds one in this feature (DELIVER obligation #0), after which a stale hash is caught by `cargo xtask check-arch` rather than by discipline.** Any stylesheet edit changes
  its sha256, therefore its committed filename, therefore its URL. The same
  commit must update `templates/base.html`, the `VENDOR.md` row, and **three**
  literals in `foundry-app/src/lib.rs` (329, 346, 365) — five hand-maintained
  sites counting the rename. `check-arch` has no asset rule and the promised
  asset-resolution probe is not in the tree (D-01e), so this is manual
  discipline backed only by red cache-policy tests.
- **`--cz-shadow` is a bound token, not a literal**, and is re-bound in dark:
  light `0 1px 2px rgba(18, 22, 20, 0.04), 0 8px 24px rgba(18, 22, 20, 0.05)`;
  dark `0 1px 2px rgba(0, 0, 0, 0.4), 0 12px 32px rgba(0, 0, 0, 0.32)`. Values
  taken from the pinned reference stylesheet.
- **New static blobs need VENDOR.md rows.** Three woff2 files take the vendored
  shape (version, upstream URL, retrieval date, sha256, licence); `theme.js`
  takes the authored-in-tree shape.
- **`theme.js` is a vanilla IIFE** matching `board-dnd.js`, `csrf-upload.js`,
  `form-errors.js` and `keyboard.js` — no framework, no module system. It is the
  only `<head>` script that must NOT carry `defer`.
- **No build step, no bundler, no minifier, no CDN at runtime** (DB6 /
  `VENDOR.md`). Fonts are served by the existing `ServeDir` route from the same
  origin as the HTML.
- **Technical notes for DESIGN** (not requirements): woff2-only is expected to
  be sufficient for foundry's htmx-2 browser baseline; a latin subset and
  `font-display: swap` are the obvious means of satisfying D-14; font-metric
  overrides (`size-adjust` / `ascent-override`) may or may not be needed to
  satisfy the no-reflow criterion — US-CTS-03's hypothesis is that they are not.
- **`prefers-reduced-motion`.** canzan's `.cz-btn` idiom uses `translateY(-1px)`
  on hover. Any motion adopted from the reference must be suppressed under
  `prefers-reduced-motion: reduce`.
- **Mobile floor.** Primary controls are ≥44 px in their smaller dimension at
  ≤480 px (pwa-mobile-rendering AC-02.4); the desktop floor is ≥24×24
  (NFR-WEBB-A11Y-02). `.theme-toggle` joins the mobile selector list.
- **Test lanes.** HTTP acceptance lane for head/asset/markup facts; fantoccini
  `@needs-browser` for palette, computed-style, toggle interaction and the
  scripting-disabled scenario (via `new_session_without_scripting`).
- **Mutation testing.** The repo gate is ≥80 % on modified files per feature.
  This feature adds essentially **no Rust** — four test literals in
  `src/lib.rs`. The mutation scope will be near-empty, and DELIVER should record
  that honestly rather than manufacture a score.

### [REF] Outcome KPIs

**Objective**: every foundry screen renders on the first frame in the light
level the operator's environment demands, in canzan's palette and voice — and
nothing she already knows how to do moves.

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|---|---|---|---|---|---|
| 1 | Operators whose device prefers dark | Complete a board session without dimming the display or installing a contrast extension | 100 % of foundry screens render dark; 0 light surfaces remain | **0 %** — no dark palette exists in any form | Sweep scenarios (US-CTS-01 S6, US-CTS-02 S5) + Priya's own report over a week of evening sessions | **North Star** (Leading) |
| 2 | Operators on any device preference | Navigate between foundry pages without seeing a frame of the wrong palette | 0 light frames per navigation | n/a — the mechanism does not exist yet | The `<script>` is render-blocking in `<head>` (no `defer`/`async`/`module`) and the palette is already resolved in the first post-navigation snapshot (US-CTS-04 S3) | Guardrail |
| 3 | Every text/background and control-boundary pair in foundry | Meet NFR-WEBB-A11Y-02 in **both** palettes | 100 % of body pairs ≥4.5:1; large text and control boundaries ≥3:1 | Light: presumed-pass, never re-measured against a new palette. Dark: **0 %** — no dark palette | Ratios computed and recorded inline beside each token; re-verified per slice; `--cz-faint` rebound to a passing value per D-04, with the three-tier separation preserved | Guardrail |
| 4 | The acceptance suite | Pass with no existing step or feature file edited | 0 existing files changed under `crates/foundry-acceptance/` | n/a | `git diff --stat crates/foundry-acceptance/` shows additions only | Guardrail |
| 5 | Operators on a light-set device | Get a dark foundry without changing the operating system | Reachable in ≤2 activations of one visible control, from any app-shell screen | **0** — impossible; the only route is an OS-wide change | US-CTS-04 S1 + Priya's report | Leading |
| 6 | Anyone shown foundry beside canzan.net | Read them as one product | 11 of 11 colour tokens identical in value to the pinned reference; 3 of 3 type families present; 1 accent hue, not 3 | 0 of 11 tokens; 0 of 3 families; **3** competing accents | Diff the `:root` block against `canzan-net-reference.css`; grep `@font-face`; grep for non-token colour literals | Leading |
| 7 | Cold first paint of the board | Not get slower because of the restyle | Added static payload ≤150 KB across all new blobs; **0** cross-origin requests | 0 KB of fonts, 0 cross-origin (nothing to regress — which is why this is a guardrail, not a target) | Sum the referenced blobs from a cold board load; assert no request leaves the origin (US-CTS-03 S2) | Guardrail |

**Metric hierarchy** — North Star: KPI 1. Leading: 5, 6. Guardrails: 2, 3, 4, 7.

**Hypothesis**: we believe that adopting canzan.net's published palette and
canzan-lift's shipped three-state control, rather than authoring either, will
give the operator a foundry that follows her environment at no cost to the
render contract. We will know this is true when a dark-preferring operator
completes an evening triage session on every foundry screen without a light
surface (KPI 1) and the acceptance suite passes with zero existing files touched
(KPI 4).

**Homelab-scale honesty**: single-digit-operator instance, no analytics tooling.
Every KPI above is verified by an acceptance assertion, a file diff, a checksum,
or the operator's own report — never by a dashboard. KPIs 1 and 5 have a
qualitative component (her report) that no test can supply, and that is stated
rather than dressed up as instrumentation.

### [REF] DoD

- All UAT scenarios green: head/asset/markup facts in the HTTP lane; palette,
  computed-style, toggle interaction and scripting-disabled in the
  `@needs-browser` lane.
- Both palettes demonstrated live on all four surface groups — board, dashboard,
  dialogs, overlay — plus the sign-in page.
- Every foreground/background pair's measured ratio recorded inline in the
  stylesheet, in both palettes, with `--cz-faint` rebound per D-04 and its three
  tiers demonstrably separated.
- Zero colour literals outside the token block, provable by grep — including the
  three off-contract brand-chrome literals in `base.html` and the manifest.
- `git diff --stat crates/foundry-acceptance/` shows additions only.
- `theme.js` diffed against canzan-lift's: exactly two differing lines.
- Every new blob has a `VENDOR.md` row whose recorded sha256 recomputes.
- The stylesheet hash is consistent across `base.html`, `VENDOR.md` and all three
  literals in `src/lib.rs`; grep for the previous 8-hex prefix returns zero hits.
- `cargo xtask ci` green (check-arch, deny, cache-policy tests); merged to main.

### [REF] Out of Scope

- **Migrating canzan-lift onto `--cz-*` names or onto self-hosted webfonts.**
  The D4 follow-up. Recorded here so "one theme everywhere" does not silently
  lapse (intake open question 8), and widened by D-01f: canzan-lift shares
  neither the token names nor the type strategy, so unification is two jobs, not
  one. foundry adopts the contract's *structure* unchanged — D-04 moves one hex
  value, not a tier — so the shape canzan-lift migrates onto is canzan.net's own.
- **Extracting `theme.js` into a genuinely shared module** consumed by both
  repos. D-06 *prepares* for it (both site-specific values hoisted to named
  constants) but does not do it.
- **`<meta name="theme-color">` following an explicit override** (D-07) — a
  known, user-accepted limitation, not an oversight.
- **Any layout, spacing or geometry change.** Colour and type only. The board's
  shape is muscle memory and must not move.
- **Any new render-contract class** beyond `.theme-toggle*`.
- **A server-side, per-user theme preference.** This is device-local
  `localStorage`; nothing is persisted to the store and no schema changes.
- **A forced-colours-specific palette** beyond what the existing `outline`
  discipline already guarantees.
- **A CSS build step, preprocessor, minifier or bundler** (DB6, permanently).
- **Theming user-supplied content** — markdown comment bodies, attachment
  previews, uploaded images.
- **Retiring `.dash`'s separate structure.** Its duplicate `font-family`
  declaration is replaced; its markup and its class names are not touched.
- **A high-contrast or a third palette.** Two palettes, three states.

### [REF] WS Strategy

Walking skeleton **declined** (D-12). Brownfield: 47 shipped features, a running
app, a stylesheet already authored, served, hashed, cache-policied and asserted
against. There is no unproven end-to-end path — this feature changes how a
shipped path *looks*.

Delivery is four slices in outcome order (S01 → S02 → S03 → S04; rationale in
Story Map § Priority Rationale). One hard dependency edge: S02 and S04 both
consume the token block and the two dark blocks introduced by S01. S03 is
independent of S02 and S04 and can move without consequence.

Each slice is independently shippable, ≤1 day, and user-visible; the slice
composition gate passes with no `@infrastructure`-only slice, because the
fonts-vendoring work that would have been one is folded into US-CTS-03 (D-13).

Per-slice briefs: `docs/feature/canzan-theme-system/slices/`.

### [REF] Driving Ports

**None.** This feature adds no behaviour to the core and registers no new
outcome in `docs/product/outcomes/registry.yaml` — there is no operation to
invoke and no invariant to enforce in the domain. Stating that explicitly is the
point: a presentation feature that grew a driving port would have grown scope.

Surfaces touched instead:

1. `crates/foundry-app/static/css/foundry.<new-hash>.css` — rewritten header,
   new token block, two dark blocks, all 46 literals retired, dead
   `.site-header` removed, `.theme-toggle*` added.
2. `crates/foundry-app/static/js/theme.js` — new, app-owned vanilla IIFE ported
   from canzan-lift.
3. `crates/foundry-app/static/fonts/` — new, three woff2 blobs.
4. `crates/foundry-app/static/VENDOR.md` — four new/updated rows.
5. `crates/foundry-app/templates/base.html` — the `<link>` hash, the
   render-blocking `<script>`, the `theme-color` meta pair.
6. `crates/foundry-app/static/manifest.webmanifest` — `theme_color`
   (`#1c1c22` → canzan), `background_color` (`#ffffff` → canzan). Keys stay;
   only values move.
7. `crates/foundry-app/src/lib.rs` — **three** hashed-name literals in the
   cache-policy tests (329, 346, 365). **The only Rust in the feature.**

### [REF] Pre-requisites

None outstanding. Everything this feature stands on is shipped: the `ServeDir`
static route and its immutable cache policy; `base.html`'s head; the
`app_shell.html` + `partials/sidebar.html` chrome; the vanilla-IIFE idiom
(four existing files); the `@needs-browser` fantoccini harness and its
`new_session_without_scripting` scripting-disabled mode; and `VENDOR.md`'s
re-hash procedure.

Two DESIGN-owned inputs must be closed before DELIVER — both tracked as
Unresolved below, neither blocking DoR: the `--cz-shadow` literal values, and
the pinned font release URLs, versions and licences.

### [REF] DoR Validation

| DoR Item | US-CTS-01 | US-CTS-02 | US-CTS-03 | US-CTS-04 | Evidence |
|---|---|---|---|---|---|
| 1. Problem in domain language | PASS | PASS | PASS | PASS | Each Problem names Priya's concrete pain — the 23:40 white page, the white `?` card over an ink board, the side-by-side comparison that fails on type, the device that answers the wrong question |
| 2. Persona specific | PASS | PASS | PASS | PASS | `persona-instance-operator` (Priya Raman), now declared in SSOT, with three named environments; US-CTS-04 is scoped to `env-light-os-dark-room` specifically |
| 3. 3+ domain examples, real data | PASS | PASS | PASS | PASS | Real screens and values throughout: `/team/backend/project/identity-platform`, AUTH-7/12/3, `#0a0c0b`/`#131817`/`#e8ebe8`/`#62c9a6`/`#1a7a5e`, `foundry.theme`, `/report`, `/signin`, `?`/`j`/`k`/`c` |
| 4. UAT 3–7 scenarios G/W/T | PASS (6) | PASS (5) | PASS (6) | PASS (7) | Embedded above; all titles are business outcomes — no class, file or protocol name appears in any scenario title |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | Every AC maps to ≥1 scenario and to a D-decision; the solution-neutral rule is held (ACs say "before first paint", "no request leaves the origin", not "use `<script>` without `defer`" — that lives in System Constraints) |
| 6. Right-sized | PASS 1 d | PASS 0.75 d | PASS 1 d | PASS 0.75 d | ≤1 day each, 5–7 scenarios each, each demonstrable in one session |
| 7. Technical notes / constraints | PASS | PASS | PASS | PASS | System Constraints + D-01…D-14 + Driving Ports surface list |
| 8. Dependencies resolved or tracked | PASS | PASS | PASS | PASS | S02/S04 depend on S01 (token block + dark blocks); S03 independent. Two open DESIGN inputs tracked under Unresolved (`--cz-shadow` literals, font release pins) — tracked, not unspecified |
| 9. Outcome KPIs measurable | PASS | PASS | PASS | PASS | Seven KPIs with baselines; five measurable by assertion/diff/checksum, two carrying an explicitly-labelled qualitative component |

**DoR Status: PASSED** — all 9 items, all 4 stories.

Anti-pattern sweep: no "Implement-X" titles (each names a user outcome); no
generic data (no `user123`, no `test@test.com` — real URLs, real issue keys,
real hex values); no technical AC (implementation lives in System Constraints,
not in acceptance criteria); no oversized story (max 7 scenarios, max 1 day); no
abstract requirement without examples (3 per story, each with a distinct
happy/edge/boundary role). Every non-`@infrastructure` story carries an Elevator
Pitch naming a real screen and concrete observable output; there are no
`@infrastructure` stories in this feature.

Peer review by `nw-product-owner-reviewer` **not invoked** in this lean subagent
run — the orchestrator gates handoff. Same precedent as
`instance-admin-project-rename` and `board-lane-management`.

### [REF] Unresolved

**Two** items. Neither blocks DoR; both are DESIGN's.

1. **Font release pins are not verified.** `VENDOR.md` requires a version, an
   upstream canonical URL, a retrieval date, a sha256 and a licence per blob.
   Bricolage Grotesque, Public Sans and JetBrains Mono are all believed to be
   openly licensed, but this wave verified no release artefact. DESIGN pins them
   before DELIVER. Tracked under DoR item 8, not resolved.
2. **There is no automated stale-hash guard.** `design/assets.md` Decision #4a
   promises an "asset-resolution probe" that reddens CI on a forgotten rename;
   it is not in `crates/` or `xtask/`. The three cache-policy literals in
   `src/lib.rs` are the only backstop, and they catch a *wrong* hash, not a
   *forgotten* `VENDOR.md` row. Accepted risk for this feature (three re-hashes
   across four slices); flagged as a repo-level gap in Triggered suggestions.

#### Closed since first draft

- **`--cz-shadow` literal values** — found in the pinned reference (minification
  had hidden them from line-oriented extraction). Light
  `0 1px 2px rgba(18, 22, 20, 0.04), 0 8px 24px rgba(18, 22, 20, 0.05)`; dark
  `0 1px 2px rgba(0, 0, 0, 0.4), 0 12px 32px rgba(0, 0, 0, 0.32)`. Recorded in
  System Constraints and US-CTS-02's AC. DESIGN is not blocked.
- **The `theme-color` assertion is lenient** —
  `crates/foundry-acceptance/src/steps/feature_pwa_mobile.rs:883` does
  `document.querySelector('meta[name="theme-color"]')` and asserts the content is
  non-empty. `querySelector` returns the **first** match; it is not an exact-one
  selector. D-07's media pair does not break S10 and does not violate D-11.
- **`--cz-faint`'s AA failure** — closed by rebinding the value rather than
  abandoning the tier (D-04, amended).

### [REF] Inherited commitments

| Origin | Commitment | Impact here |
|---|---|---|
| `foundry.css:1-10` | Semantic classes double as the render contract the acceptance suite selects on | D-11: zero selector churn; the AC appears in all four stories |
| `assets.md` DB6 / `VENDOR.md` | No Node, no bundler, no minifier, no CDN at runtime; every served blob is hash-verifiable against a named upstream | Fonts self-hosted and VENDOR-recorded (US-CTS-03); no build step introduced |
| `assets.md` Decision #4a / ADR-B03 | The content hash IS the cache key; an edit changes the filename, the URL and every reference, in one commit | Three re-hashes across four slices, each touching five sites (D-01e) |
| NFR-WEBB-A11Y-02 | Contrast ≥4.5:1 (3:1 large), visible `:focus-visible`, targets ≥24×24, labelled inputs | Re-measured in **both** palettes; forces D-04's *value* divergence from canzan.net (the tier structure is kept) |
| pwa-mobile-rendering AC-02.4 / FR-6 | Primary tap targets ≥ ~44 px at mobile width | `.theme-toggle` joins the mobile selector list |
| pwa-mobile-rendering FR-5 / S10 | `theme-color` meta + apple meta present; manifest declares `theme_color` and `background_color` | D-07 moves all three values onto the contract; the keys and the meta stay. The S10 assertion is lenient (first-match, non-empty content), so the media pair is safe |
| keyboard-shortcut-bindings ADR-004 / NFR-7 | The selection ring is an `outline` — not colour alone, survives forced colours, costs no layout space | Preserved verbatim; only its colour and its contrast comment change |
| keyboard-shortcut-bindings NFR-4 / ODD-8 | A scripting-disabled lane exists (one scenario) | This feature adds foundry's **second** such scenario; it does not assume a blanket guarantee (D-01b) |
| `brief.md` § dialog layers (BR-4) | One close mechanism; `Escape` has exactly one owner; new affordances are attributes, never listeners | `theme.js` registers a `click` listener on its **own** button only — no document-level handler, no `Escape` involvement |
| canzan-lift `app.css` | Every surface colour is opaque; measured ratios recorded inline beside the tokens | D-05 and the inline-ratio requirement |
| canzan-lift `theme.js` | Three states; system removes the attribute; storage is best-effort; the button is built in JS | D-06, ported with exactly two differing lines |

### [REF] Triggered suggestions (ask-intelligent, not expanded)

1. **Unify canzan-lift onto the shared contract** — the D4 follow-up, widened by
   D-01f: canzan-lift shares neither the `--cz-*` names (it has 57 unprefixed
   tokens) nor the type strategy (no webfonts at all, by explicit decision). Two
   jobs, not one. Must not silently lapse. **Note the contract itself survives
   this feature intact:** D-04 was resolved by moving one hex value, not by
   dropping a tier, so foundry adopts canzan.net's *structure* verbatim and
   canzan-lift inherits an unchanged shape with one corrected value — a
   materially easier migration than if foundry had collapsed a tier.
2. **Extract `theme.js` as a genuinely shared module** — D-06 leaves it one
   parameterisation away; the remaining work is deciding where a module shared
   across two repos lives. This is also the unblocking step for item 3.
3. **Make `theme-color` follow an explicit override** — D-07's accepted,
   user-confirmed limitation. Deliberately deferred: the fix must land in both
   apps at once, and doing it before the shared module exists would spend
   exactly the byte-identical-port property the module depends on.
4. **Build the asset-resolution probe** — Unresolved #2. A repo-level gap
   predating this feature: `design/assets.md` promises it, nothing implements
   it, and this feature re-hashes three times.
5. **An automated contrast sweep** — NFR-WEBB-A11Y-02's stated test is "an
   automated a11y lint on the templates; contrast check on the vendored
   stylesheet", and no such check exists. canzan-lift has one
   (`tests/e2e/test_dark_mode.py`), which is where D-05's opaque-surface rule
   comes from. Adopting it would turn KPI 3 from a recorded comment into a gate.
6. **Retire the dead-CSS class of defect** — `.site-header` sat in the file for
   43 features with no markup behind it. A trivial "every class in the
   stylesheet appears in at least one template" check would have caught it.

## Wave: DISTILL

Agent: nw-acceptance-designer (Quinn) | Date: 2026-08-29 | Density: lean
| Framework: cucumber-rs (Rust) | Feature scope: core | Integration: real services
| Infrastructure testing: declined (see Divergences) | Walking skeleton: none (D-12)

### [REF] Prior Wave Consultation

| Artifact | Status | Note |
|---|---|---|
| `docs/feature/canzan-theme-system/feature-delta.md` § DISCUSS (lines 1-928) | ✓ read in full | D-01…D-14, US-CTS-01…04 with 24 embedded UAT scenarios, System Constraints, KPIs 1-7 |
| `design/architecture-design.md`, `component-boundaries.md`, `technology-stack.md`, `data-models.md` | ✓ read | C1-C6 boundaries; §9 handoff; the interaction-contract table |
| `docs/product/architecture/adr-canzan-theme-001…004` | ✓ read (indexed via architecture-design §3) | Font instancing, derived-asset provenance, the check-arch guard, the token seam |
| `slices/slice-01…04` | ✓ present | Slice order S01→S02→S03→S04 confirmed; consumed via the DESIGN slice→architecture map |
| `crates/foundry-acceptance/src/support/browser_harness.rs` | ✓ read in full | ADR-003 commentary at `:126-138`; `new_session`, `open_mobile_session`, `new_session_without_scripting` |
| `crates/foundry-acceptance/src/steps/feature_pwa_mobile.rs` | ✓ read (header + idioms) | The model for this module — browser lane, layout-fact assertions, shipped-Background reuse |
| `crates/foundry-acceptance/tests/features/pwa-mobile-rendering.feature` | ✓ read in full | Tag vocabulary, `@pending` discipline, harness-note convention |
| `crates/foundry-acceptance/tests/acceptance.rs` | ✓ read | `filter_run` excludes `@pending` from every lane; `fail_on_skipped()` is on |
| `docs/feature/canzan-theme-system/devops/` | ⊘ not found | DEVOPS was not run. Proceeding on the shipped lane definitions — WARN, not BLOCK |
| `docs/product/kpi-contracts.yaml` | ⊘ not found | Soft gate. KPIs are carried inline in DISCUSS instead; `@kpi` scenarios map to them directly |

### [REF] Reconciliation result

**Passed — 0 contradictions between DISCUSS and DESIGN.** DESIGN adopts every
DISCUSS decision it touches and adds only new material (the four ADRs and the
`check-arch` guard). The one place the two waves point in different directions —
whether the guard's gold tests are acceptance scenarios — is not a contradiction
between waves but between DESIGN and this dispatch's Decision 4; it is recorded
under Divergences rather than silently resolved.

There are no `wave-decisions.md` files in this feature; DISCUSS and DESIGN both
write into `feature-delta.md` and `design/` respectively, which is this repo's
shipped convention (`board-lane-management` precedent).

### [REF] Device-preference oracle — the load-bearing test decision

The trap this feature offered is pwa-mobile-rendering's ADR-003 one layer over.
The harness had **no** way to give a session a device colour preference (zero
matches for `setEmulatedMedia`, `cdp`, `Emulation.`, `prefers-color-scheme`
anywhere under `crates/`). Without one, a dark-mode scenario can only drive dark
by stamping an explicit choice on the document — and then the
`@media (prefers-color-scheme: dark)` block is **green whether or not it exists**,
because the attribute selector alone satisfies the assertion. The media path is
the **default** state, the one most operators get. An untested default is the
worst coverage shape available, and it is precisely the shape this feature exists
to fill.

**Mechanism adopted: `--force-dark-mode` injected into `goog:chromeOptions.args`
at session creation.** Empirically verified twice — raw headless Chrome via
`--dump-dom`, and the real path through chromedriver 151.0.7922.138 over W3C
`POST /session` + `execute/sync`:

| Flags | `matchMedia(...).matches` | Computed CSS custom property |
|---|---|---|
| *(none)* | `false` | `LIGHT` |
| `--force-dark-mode` | `true` | `DARK` |
| `--enable-features=WebContentsForceDark` | `false` | `LIGHT` |

Both the `matchMedia` result **and** the computed custom property flip, so the
media block genuinely applies — this is not merely the JS API reporting a
preference while the cascade ignores it.

`--enable-features=WebContentsForceDark` measurably does **not** work. It is
Chrome's auto-darkening feature, a different thing. Recorded so nobody later
"fixes" the flag to it and silently returns the lane to green-over-nothing.

**Why not CDP.** `POST /session/{id}/goog/cdp/execute` with
`Emulation.setEmulatedMedia` was the first candidate and was rejected. fantoccini
0.21.5 *does* expose `Client::issue_cmd` (`session.rs:338`) and `session_id`
(`client.rs:110`), so CDP was reachable — recorded so this is not rediscovered as
a blocker later. It was rejected on **determinism**: a runtime call can race page
load where a session-creation capability cannot, and the capability needs no
side-channel HTTP client and no new dependency. It also matches the idiom
`open_mobile_session` already established for `mobileEmulation.deviceMetrics`.

**Anti-vacuity guard, mandatory.** The baseline is `false`/`LIGHT`, so the guard
discriminates. `device_prefers_dark(&client)` reads
`window.matchMedia('(prefers-color-scheme: dark)').matches`, and every
dark-by-device `Given` asserts it **before** asserting anything about foundry's
own rendering. The `@oracle-probe` scenario asserts *both* arms — dark session
`true`, default session `false` — so the probe itself cannot pass vacuously. It is
the first scenario DELIVER un-pends; if the instrument cannot prove a dark device,
nothing below it is worth running.

**Confidence: high.** The mechanism is measured end-to-end on the exact
chromedriver this repo runs (151.0.7922.138, confirmed present on this machine),
through the same protocol path the harness uses. The residual risk is a future
Chrome changing the flag's semantics — which is exactly what the `@oracle-probe`
scenario exists to catch, loudly.

### [REF] Flash-of-wrong-theme oracle — layered, with a recorded gap

**There is no sound way to sample the painted colours of the first frame** with
the instruments this suite uses. Doing it honestly needs a paint-level capture
surface (CDP screencast or equivalent) that this lane deliberately does not carry.
Rather than write a scenario that passes either way, the flash scenario asserts
two things at declared strengths:

| | Assertion | Strength | RED-trigger |
|---|---|---|---|
| **(a)** | The theme script is fetched and run **before the browser is permitted to paint** — it sits in the head and carries no attribute that would defer it | **Load-bearing, deterministic** | The tag is moved to the foot of the body, or given `defer` / `async` / `type=module` — the single most likely regression in this feature per DISCUSS |
| **(b)** | The script's fetch completed before the page's **first contentful paint**, read from the browser's own paint timing | **Supporting, measured** | A deferred script losing the race to FCP |

**(b) can pass by luck on a fast loopback even with a deferred script.** It can
produce a false GREEN; it cannot produce a false RED. It is recorded as a
supporting measurement and must not be promoted to load-bearing, nor may (a) be
deleted in its favour. This is the honest position: the *mechanism* that
guarantees the outcome is asserted deterministically, the *outcome* is measured
weakly, and the gap between them is written down rather than papered over.

Rejected: adding a light-palette CSS animation as a paint tripwire. It would give
a genuine first-frame oracle, but only by shipping production CSS that exists
solely for the test — which this feature's own discipline forbids.

### [REF] Scenario list — 28 scenarios, with the RED-trigger for each

Every scenario carries `@pending`. `acceptance.rs::filter_run` excludes `@pending`
from **every** lane, so the suite stays green until DELIVER un-pends them one at a
time in slice order. `@real-io` on all 28 (real Postgres, real chromedriver, real
HTTP — no mocks at acceptance level, per Decision 3). `@needs-browser` on 27; the
brand-chrome scenario is a markup fact and runs in the HTTP lane.

| # | Scenario | Tags | Turns RED when |
|---|---|---|---|
| 1 | The browser under test can be given a device that prefers dark | `@oracle-probe @lane-probe @slice1` | The device-preference capability stops taking effect. **Without this, every dark-by-device assertion below could pass while measuring the light palette twice** |
| 2 | A dark-preferring operator's whole board is dark, rail included | `@slice1 @us-cts-01` | The device-driven dark block is absent or its guard is wrong — the board keeps painting paper on a dark device |
| 3 | A light-preferring operator sees canzan's paper-and-jade palette | `@slice1 @us-cts-01` | The canzan tokens were not adopted, or one of the three retired accent hues survives |
| 4 | An explicit dark choice overrules a light device | `@slice1 @us-cts-01 @error` | The explicit-choice dark block is missing, so only the device can produce dark |
| 5 | An explicit light choice overrules a dark device | `@slice1 @us-cts-01 @error` | The device-driven block is written without the `:not([data-theme="light"])` exception. **The one mechanism written in two files; nothing else in the suite catches this** |
| 6 | The keyboard selection ring reads as a shape, not only a colour | `@slice1 @us-cts-01 @error` | The ring is restyled as a background fill or border swap — looks right, breaks forced-colours and costs layout space |
| 7 | Board and rail text stays legible in both palettes | `@slice1 @us-cts-01 @kpi` | The faint tier reverts to canzan.net's own value (3.24:1 light / 3.52:1 dark) |
| 8 | Everything the operator already selects on is still on the page | `@slice1 @us-cts-01 @error` | Any semantic class or `data-*` marker renamed or dropped by the restyle (KPI 4's render-contract half) |
| 9 | The installed app's brand colours come from the canzan contract | `@slice1 @us-cts-01` *(HTTP lane)* | The off-contract brand literals survive, or a manifest key is dropped while its value moves |
| 10 | The shortcut overlay is legible over a dark board | `@slice2 @us-cts-02 @kpi` | The overlay block keeps its own colour values — a white card over an ink board |
| 11 | The signed-in dashboard matches the board | `@slice2 @us-cts-02` | The dashboard block keeps its 21 literals, or the key chip takes a translucent tint with no opaque surface beneath |
| 12 | The new-issue dialog and the layer behind it are dark | `@slice2 @us-cts-02` | Dialog and backdrop keep their own values |
| 13 | A screen with no chrome still honours the chosen theme | `@slice2 @us-cts-02 @error` | The theme is applied from the rail's chrome rather than the document, so the 15 rail-less screens stay light |
| 14 | No surface anywhere is left light-only | `@slice2 @us-cts-02 @error` | Any single rule missed by the audit — the light rectangle in a dark app |
| 15 | Headings, body text and keys each carry their intended typeface | `@slice3 @us-cts-03` | The declarations exist but no blob is served, so nothing loads. **Asserting the declarations alone would be green over nothing** |
| 16 | No request for a typeface ever leaves foundry's own origin | `@slice3 @us-cts-03 @kpi` | A typeface pulled from an external host (KPI 7 asserts 0 cross-origin) |
| 17 | Column and section labels are legible at label size | `@slice3 @us-cts-03 @kpi` | The faint tier reverts; lane headers fail at ≈11 px in both palettes |
| 18 | A typeface that has not arrived costs a typeface, never a word | `@slice3 @us-cts-03 @error` | A face declared without a swap policy — the browser's default blocking period makes text **invisible**, a blank board rather than an unstyled one |
| 19 | The board does not move when the typefaces arrive | `@slice3 @us-cts-03 @error` | A fallback stack whose metrics shift the columns or cards when the real faces land |
| 20 | An operator who has never chosen a theme is not given one | `@slice4 @us-cts-04` | The control writes a third value for follow-the-device instead of removing the attribute. **Looks correct in the toggle and silently breaks dark-by-device for everyone** |
| 21 | The control cycles through following the device, light, dark, and back | `@slice4 @us-cts-04` | A two-state toggle, which can never hand the decision back to the device |
| 22 | A chosen theme survives navigation and reload | `@slice4 @us-cts-04` | The choice is held in the page rather than on the origin |
| 23 | A chosen dark screen never flashes light | `@slice4 @us-cts-04 @kpi` | The theme tag is deferred or moved out of the head — see the layered oracle above |
| 24 | With scripting disabled the control does not exist and the device decides | `@slice4 @us-cts-04 @error` | The control is server-rendered, so with scripting off it is present but **dead** — worse than absent |
| 25 | With site storage refused the screen still themes and nothing is reported | `@slice4 @us-cts-04 @error` | The first read of the stored choice is unguarded, so the script dies at parse time, taking the device-driven palette down on every screen |
| 26 | A stored choice that means nothing is treated as no choice at all | `@slice4 @us-cts-04 @error` | An unrecognised stored value is applied verbatim, leaving the document in a state no palette matches |
| 27 | The control says which theme is active and which the next press will select | `@slice4 @us-cts-04` | The control ships labelled with a bare glyph |
| 28 | The control is reachable and large enough to hit | `@slice4 @us-cts-04` | The control ships without joining the mobile touch-target rule, or with no visible focus indicator in one palette |

**Coverage.** US-CTS-01 → 8 · US-CTS-02 → 5 · US-CTS-03 → 5 · US-CTS-04 → 9 ·
oracle probe → 1. All four stories covered. **Error/edge ratio 11/28 = 39 %**,
fractionally under the 40 % heuristic — because a scenario that could not be
staged was **removed** rather than kept as decoration (see Divergence 4). The
floor is a smell detector, not a quota; adding a scenario to reach it would be
exactly the padding the anti-vacuity discipline forbids. Five `@kpi` scenarios map
to KPI 2, 3 (×3) and 7.

**One DISCUSS scenario was partially realised.** US-CTS-04 S5 ("with site data
blocked the control still works") covers two guards that fail differently and are
reached differently. The **read** guard is scenario 25. The **write** guard is
structurally undrivable and has no scenario — Divergence 4. Scenario 26
(unrecognised stored value) is new: it is the other half of the
absence-is-the-contract mechanism that scenario 20 protects.

### [REF] Anti-vacuity ledger

Grouped, because the reasoning repeats. Every scenario above has a named
RED-trigger; these are the four places where the *obvious* assertion would have
been decoration and was replaced.

| Tempting assertion | Why it is green over nothing | What is asserted instead |
|---|---|---|
| Stamp an explicit dark choice and assert the page is dark | The attribute selector satisfies it. The `@media` block — the **default** path — is never exercised | A real device preference (`--force-dark-mode`), guarded by the `matchMedia` probe |
| Assert the `@font-face` rules exist | A declaration with no blob behind it passes | The faces **report as loaded**, the real heading/title/key resolve to them, and the blobs appear in resource timing as same-origin |
| Assert the palette changed after activating the control with storage refused | The attribute is stamped **before** the write throws, so the palette changes whether or not the write is guarded | Plus: **nothing is reported to the operator**, via an unhandled-error recorder installed before the activation |
| Assert the six contrast ratios recorded in the token comments | Asserts a human's arithmetic against itself | Ratios **computed** from colours resolved in the live browser, ancestor-walked for the effective background |

### [REF] Test placement

| Artifact | Path | Precedent |
|---|---|---|
| Scenarios (SSOT) | `crates/foundry-acceptance/tests/features/canzan-theme-system.feature` | One file named for the feature — `board-lane-management.feature`, `pwa-mobile-rendering.feature` |
| Step scaffolds | `crates/foundry-acceptance/src/steps/feature_canzan_theme.rs` | One `feature_*.rs` step module per feature; cucumber-rs requires globally-unique step text |
| Module registration | `src/lib.rs` (+1), `tests/acceptance.rs` (+2) | Mechanically required — see Divergences |

The `.feature` file is the scenario SSOT; this section is a pointer and a
structured summary. The Python/pytest-bdd paths in the `nw-distill` skill do not
apply — this repo is cucumber-rs over Rust, and there is no `tests/common/`
state-delta port because Mandates 8-10 are Python-pilot bindings with no Rust
equivalent in this tree. Layer discipline is honoured in substance: these are all
layer 4-6 tests (real browser, real Postgres, real HTTP), where the mandates
themselves permit traditional assertions and forbid PBT.

### [REF] WS strategy

**Declined (D-12), inherited unchanged.** Brownfield: 47 shipped features, a
running app, a stylesheet already authored, served, hashed, cache-policied and
asserted against. There is no unproven end-to-end path.

The `@oracle-probe` scenario is **not** a walking skeleton and is deliberately not
tagged as one. It proves the *instrument*, not the product — the same role
`pwa-mobile-rendering`'s `@lane-probe` played, minus the skeleton claim.

### [REF] Adapter coverage

This feature adds no driven adapter. The surfaces it touches are static assets and
templates served by the shipped `ServeDir` route.

| Surface | `@real-io` scenario | Covered by |
|---|---|---|
| Stylesheet served by `ServeDir` | YES | 2-14, 17 (every palette scenario resolves real computed styles from the real served file) |
| Font blobs served by `ServeDir` | YES | 15, 16 (loaded-font entries + same-origin resource timing) |
| `theme.js` served by `ServeDir` | YES | 20-29 |
| `base.html` head + manifest | YES | 9 (HTTP lane) |

### [REF] Scaffolds

`crates/foundry-acceptance/src/steps/feature_canzan_theme.rs` — **79 step
definitions**, 81 attributes (two steps carry both a `given` and a `then` arm),
every body calling `scaffold()`, which **panics**. Marker: `SCAFFOLD: true` in the
module header; `__SCAFFOLD__` in the panic message. `grep -r "SCAFFOLD: true"
crates/foundry-acceptance/src/steps/` returns this file until DELIVER finishes.

Panicking rather than returning is deliberate: a step that quietly returned would
let an un-pended scenario pass while asserting nothing.

No production scaffolds were created. The feature's production surface is CSS,
HTML, a manifest and one vanilla script — none of which has an import graph that
could produce a BROKEN classification.

### [REF] Pre-DELIVER RED classification

The fail-for-the-right-reason gate was **executed**, not asserted. The
`@oracle-probe` scenario was temporarily un-pended and run against real
infrastructure (Docker + Postgres testcontainer + chromedriver 151.0.7922.138):

```text
  ✔> Given a workspace "Acme" exists with a member "Mei" on team "Backend"
  ✔> And a project "Sandbox" with key prefix "GEN" exists under "Backend"
  ✔> And Mei is signed in
  ✘  Given a browser session whose device preference is dark
     Matched: crates/foundry-acceptance/src/steps/feature_canzan_theme.rs:176:1
     Step panicked: __SCAFFOLD__ canzan-theme-system: step not yet implemented
  1 scenario (1 failed) · 4 steps (3 passed, 1 failed)
```

Classification: **`MISSING_FUNCTIONALITY`** — correct RED. The shipped Background
steps passed against real infrastructure, and the first new step failed at its own
scaffold, not at an import, a fixture or an undefined-step skip. The scenario was
re-pended afterwards; the lane now reports `0 features / 0 scenarios / 0 steps`
and the suite is green.

Mechanical verification, all passing: `cargo check --tests` clean · `cargo clippy
--tests` clean · `cargo fmt` no-op · **124/124 step occurrences in the feature file
resolve to a defined step, with zero orphaned definitions** (checked by matching every emitted regex against every
Gherkin line — zero unmatched, so no scenario can fail as BROKEN when un-pended).

### [REF] Divergences and upstream issues

**1. D-11's measurable form is mechanically impossible. BLOCKER for the AC as
written; harmless in substance.**

D-11 and KPI 4 state: *"no existing file under `crates/foundry-acceptance/` is
modified"*, verified by `git diff --stat` showing additions only. **No feature can
satisfy this.** Registering a step module requires `pub mod` in `src/lib.rs` and a
force-link `use` in `tests/acceptance.rs`, or the module is never compiled into the
test binary — an uncompiled step module is exactly the green-over-nothing this repo
refuses. The most recent feature proves it: `board-lane-management`'s landing commit
touched `crates/foundry-acceptance/src/lib.rs` and 18 existing step files.

**RESOLVED 2026-08-29 (coordinator).** Verified: `board-lane-management`'s landing
commit `1f100bf` touches **27** files under `crates/foundry-acceptance/`, including
`world.rs`, `support/harness.rs`, `tests/acceptance.rs` and 18 existing step modules.
D-11 and KPI 4 are amended in place to *"no existing **scenario or assertion** is
changed"*, with the structurally-required registration lines explicitly excepted.
The intent — zero selector churn, no existing assertion weakened — is unchanged and
still binding; only the unsatisfiable measurable form was replaced. This feature's
actual diff to existing files is 3 registration lines. DESIGN inherits the same
wording at architecture-design §4 and §8 and is being corrected to match.

This wave's actual diff to existing acceptance files is **3 lines**: `lib.rs` +1,
`acceptance.rs` +2. Both are registration, not assertion.

Proposed restatement, preserving the intent exactly:

> **No existing scenario or assertion under `crates/foundry-acceptance/` is
> changed.** New step modules may add their registration line to `src/lib.rs` and
> their force-link line to `tests/acceptance.rs`; `git diff` on every other file in
> the crate shows no change.

DESIGN inherits the same error at architecture-design §4 and §8 ("the gold tests
land in new acceptance files so `git diff --stat` still shows additions only").

**2. DESIGN promises four acceptance gold-test scenarios that this wave did not
write.** DESIGN §8/§9 and ADR-003 place injected-violation gold tests for R1, R2,
S1 and S2 in **new acceptance files**. Dispatch Decision 4 ruled infrastructure
testing out of DISTILL scope: `check-arch` has no driving port, and the acceptance
suite should stay about user-visible behaviour. **Decision 4 stands and this wave
wrote none of them.** Recording it rather than resolving it silently: the guard
rules still need their gold tests — DELIVER owns them, at the level ADR-003
specifies — and the Architect reviewer will see DESIGN's promise unmet in DISTILL's
output. This is a scope divergence, not a coverage gap.

**3. Two ACs are untestable as written at acceptance level.**

| AC | Why | Where it belongs |
|---|---|---|
| US-CTS-01: *"every card sits in the same column at the same position as before the restyle"* | "Before the restyle" is not observable from a test that runs after it. There is no baseline to compare against | Reframed as scenario 8 (the render contract still resolves) + scenario 19 (geometry is stable across typeface substitution, which *is* a within-run comparison) |
| US-CTS-04: *"`theme.js` diffed against canzan-lift's: exactly two differing lines"* | Cross-repo file comparison is not a property of the running system and has no driving port | A DELIVER review check, already in the DoD |

**4. The write guard is structurally undrivable at acceptance level. No scenario
exists for it, deliberately.**

Measured: Chrome's site-data content setting makes **both** reads and writes of
stored state throw `SecurityError` (chromedriver 151, real `http://` origin;
baseline with no prefs: both succeed). The **read** guard is therefore fully
drivable and is scenario 25. The **write** guard is not drivable at all:

| Fact | Evidence |
|---|---|
| Blocking site data also blocks the session cookie, so no signed-in screen is reachable | The same content setting governs both |
| The control mounts at `.sidebar__user` | `templates/partials/sidebar.html:10` (D-09) |
| The rail is included by exactly one template | `templates/app_shell.html:4` — the only `{% include "partials/sidebar.html" %}` in the tree |
| The sign-in screen has no rail | `templates/signin.html:1` extends `base.html`; 15 templates do, only 11 extend `app_shell.html` |

So "site storage is refused" and "the theme control exists" are mutually exclusive
**by construction**, not by harness limitation. Every page reachable under the pref
has no toggle to click.

Two workarounds were considered and rejected:

- **Stub a throwing storage accessor by script injection.** Rejected: it tests the
  stub, not the browser. It would be the only assertion in this lane not exercising
  a real substrate, in a suite whose whole discipline is refusing exactly that.
- **Fill storage to its quota.** A real exception from the real implementation, so
  the objection above does not apply — but quota semantics vary by platform, and
  overwriting an existing short key with a shorter one may not throw at all. A flaky
  oracle for a failure mode whose only symptom is an uncaught error in a console no
  operator reads.

**What is lost, stated plainly:** if the write of the chosen theme ships unguarded,
no acceptance scenario catches it. The blast radius is small — the theme attribute
is stamped *before* the write, so the palette still changes and the operator sees
nothing wrong. DELIVER owns this at code-review level; the byte-identical-port
requirement (D-06) is the real protection, since canzan-lift's write is already
guarded and any divergence is a reviewable diff.

**Open product question, flagged not decided.** A signed-out visitor gets no theme
control at all today — the 15 chrome-less screens honour an explicit choice but
offer no way to make one. That is arguably a product gap independent of testing,
and closing it would also make the write guard drivable. It is a **D-09 change** and
belongs to DISCUSS/DESIGN. DISTILL raises it; DISTILL does not decide it.

**5. `--cz-shadow` and the font release pins** were tracked Unresolved at DISCUSS
and are closed by DESIGN (shadow values recovered; ADR-001/002 pin the releases).
No DISTILL blocker remains.

### [REF] DELIVER obligations

Three, all recorded in the step module header so they travel with the code.

1. **Build the device-preference session helpers before anything else.** Add a
   `ColorScheme` peer to `browser_harness.rs`'s existing `Scripting` enum and let
   `open_session` take both, yielding `new_dark_session()`,
   `new_dark_session_without_scripting()` and `device_prefers_dark()` alongside the
   two shipped constructors. Un-pend scenario 1 first and confirm both arms.
2. **Build the storage-refused session with the measured pref — no fallback needed.**
   `profile.default_content_setting_values.cookies = 2` via `goog:chromeOptions.prefs`
   makes **both** reading and writing stored state throw `SecurityError`, measured
   against a real `http://` origin under chromedriver 151 (a `file://` probe would not
   have exercised content settings at all). Baseline with no prefs: both succeed. The
   earlier "unverified assumption" note and its weaker fallback are withdrawn — the
   mechanism is confirmed and there is nothing to fall back to.
3. **Compute contrast; never restate it.** Resolve the foreground, walk ancestors
   for the first non-transparent background, convert to relative luminance, compare.
   KPI 3 explicitly requires re-verification in DELIVER, and the ancestor walk is the
   very algorithm that makes D-05's opaque-surface rule necessary.

### [REF] Pre-requisites

All shipped. `InProcHarness` + the shared Postgres testcontainer; the
`@needs-browser` fantoccini lane and its chromedriver preflight in `cargo xtask ci`;
`new_session_without_scripting`; the `ServeDir` static route; the shipped
HTTP-lane seed steps this feature's Background reuses. chromedriver 151.0.7922.138
— the version the oracle was measured against — is the version `cargo xtask ci`
preflights.

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|---|---|---|---|
| DISCUSS#D-03 | Two dark blocks, written out, never merged; "system" removes the attribute | n/a | Scenarios 5 and 20 are the only tests in the suite that catch a broken `:not([data-theme="light"])` guard or a third written value — the one mechanism living in two files |
| DISCUSS#D-04 | `--cz-faint` rebound to a WCAG-AA-passing value in both palettes | n/a | Scenarios 7 and 17 compute the ratios in the browser rather than restating the six recorded figures, satisfying KPI 3's re-verification requirement |
| DISCUSS#D-05 | Translucent tokens never sole-carry text | n/a | Scenario 11 asserts the project key chip resolves to an opaque surface, because the contrast algorithm reads a translucent panel as its unblended colour |
| DISCUSS#D-06 | `theme.js` ported byte-identical apart from two constants | n/a | Testable half lands as scenarios 20-28; the cross-repo two-line diff is a DELIVER review check, not an acceptance assertion (Divergence 3) |
| DISCUSS#D-11 | Zero selector churn across the whole feature | n/a | Scenario 8 asserts the render contract still resolves; the AC's "no existing file modified" form is mechanically impossible and restated in Divergence 1 |
| DISCUSS#D-12 | Walking skeleton declined — brownfield, nothing unproven end to end | n/a | No `@walking_skeleton` scenario authored; the oracle probe proves the instrument only and is tagged `@lane-probe` |
| DISCUSS#D-14 | No screen ever renders invisible text while a font loads | n/a | Scenario 18 asserts the swap policy is declared, because the browser's default blocking period produces a blank board rather than an unstyled one |
| DESIGN#C4 | `theme.js` must not carry `defer`/`async`/`module`; absent with scripting off, not dead | ADR-004 | Scenario 23 assertion (a) is the deterministic guard on the deferral regression; scenario 24 drives a dark device so "the device decides" discriminates |
| DESIGN#C5 | Brand chrome media pair; both manifest keys stay declared | n/a | Scenario 9 asserts both, in the HTTP lane, without disturbing the lenient first-match assertion at `feature_pwa_mobile.rs:883` |
| DESIGN#C6 | Asset and seam guard with injected-violation gold tests | ADR-003 | **Not authored here** — Decision 4 places them outside acceptance scope; DELIVER owns them (Divergence 2) |
| pwa-mobile-rendering ADR-003 | Headless Chrome lies about the viewport; inject real device semantics or prove nothing | n/a | The direct precedent for this wave's device-preference oracle and its anti-vacuity guard |
| keyboard-shortcut-bindings NFR-4 / ODD-8 | A scripting-disabled lane exists (one scenario) | n/a | Scenario 24 is foundry's second; it assumes no blanket no-JS guarantee, per D-01b |

## Wave: DISTILL / [REF] Consolidated review gate

Run 2026-08-29, end of DISTILL, all waves visible. **Three reviewers, not four** —
Forge (platform) was skipped because DEVOPS never ran for this feature and there
are no infrastructure artifacts to review; the skip and its reason are recorded
here rather than left as a silent omission.

| Reviewer | Dimension | Verdict | Critical | High | Medium |
|---|---|---|---|---|---|
| Eclipse (`nw-product-owner-reviewer`) | Product / DoR / JTBD | **approved** | 0 | 2 | 3 |
| Architect (`nw-solution-architect-reviewer`) | DESIGN / ADRs | **conditionally approved** | 1 | 2 | 3 |
| Sentinel (`nw-acceptance-designer-reviewer`) | Acceptance / anti-vacuity | **conditionally approved** | 0 | 3 | 1 |

### Blocking finding, resolved before DELIVER

**R3 had no gold test.** ADR-003 defines three rules in
`check_static_asset_integrity` (R1 references resolve, R2 filename hash honest,
R3 `VENDOR.md` rows true) and ADR-004 adds S1/S2 — **five** rules — but only four
gold tests were specified (`adr-canzan-theme-003:130`,
`architecture-design.md:383`). R3 is the rule that machine-checks ADR-002's
provenance model, so the load-bearing integrity claim of the font strategy would
have shipped as an unverified assertion. This contradicted ADR-003's own
Consequences ("not optional and not deferrable"). Corrected to five gold tests.

### Correction applied to a stale premise

Eclipse flagged re-hash discipline as HIGH with "no automated guard exists,
accepted risk". That was true **at DISCUSS** and is no longer true: ADR-003's R2
builds the guard inside this feature. Four statements (`feature-delta.md:147`,
`:653`, `slice-01:85`, and the shared-artifact row) were reworded to say the gap
existed at DISCUSS and closes here as DELIVER obligation #0. Left as written they
would have handed DELIVER a false premise and invited it to accept a risk the
feature had already funded a fix for.

### Carried into DELIVER as obligations, not findings

Sentinel's three HIGH items are all implementation risks rather than defects in
the specification — each is a way DELIVER could produce a green-over-nothing test
from a correct scenario:

1. **The dark-device `Given` must internally call `device_prefers_dark()`** before
   staging anything. Nine scenarios (6, 7, 8, 10, 11, 12, 14, 20, 26) carry no
   explicit `Then`-step guard and rely entirely on it. If DELIVER omits it, all
   nine pass while rendering light.
2. **Flash assertion (a) must inspect the served HTML** for a `<script>` in
   `<head>` with no `defer`/`async`/`type=module` — **not** infer from timing. If
   implemented as timing it becomes indistinguishable from (b) and the only
   deterministic guard against the regression DISCUSS named is lost.
3. **Contrast must be computed** from `getComputedStyle` with an ancestor walk for
   the effective background, never compared against the six ratios recorded in the
   token comments. Asserting a human's arithmetic against itself proves nothing.

Eclipse adds two of its own, both code-review gates: `theme.js` byte-identical to
canzan-lift except `STORAGE_KEY` and the mount selector (this is the real
protection for the write guard that has no scenario — Divergence 4), and the
five-site re-hash landing in one commit.

Architect adds: re-measure Bricolage at `opsz=24` as the **first** task in S03
rather than a late one, so a ceiling miss is cheap; record the measured figure in
`VENDOR.md` so the next reader knows whether the `opsz=14` extrapolation held.

---

## Wave: DELIVER

### [NEW] Closeout — step 05-03

All 28 scenarios in `crates/foundry-acceptance/tests/features/canzan-theme-system.feature`
are green in their lanes. Zero `@pending` tags remain. The DISTILL scaffold helper
`scaffold()` was deleted from `steps/feature_canzan_theme.rs`, and its deletion is
the compiler's own proof that no step still stands on it — `-D warnings` reds on a
`scaffold` that is never called, so the function could not have been left behind.
The diff to pre-existing acceptance files across the whole feature is the three
registration lines (`src/lib.rs` +1, `tests/acceptance.rs` +2) and nothing else:
no scenario and no assertion authored upstream was changed.

**One quality-gate line is reported as written and not met, because meeting it
would have been a boundary violation.** The gate reads
`grep -r "SCAFFOLD: true" crates/foundry-acceptance/src/steps/` returns nothing.
It returns three hits, and none of them is this feature:
`feature_mwt_slice_04_non_enumerability.rs:242`,
`feature_recipient_notification_preferences.rs:32`,
`feature_notification_delivery_providers.rs:31`. Those are other features'
RED-ready scaffolds behind their own `@pending` tags. Scoped to this feature —
`grep "SCAFFOLD" steps/feature_canzan_theme.rs` — the result is empty, which is
what the gate meant.

### [NEW] Accepted limitations, stated plainly

These are limits the feature SHIPS WITH. They are written here so the next reader
inherits them as decisions rather than rediscovering them as bugs.

**1. `theme-color` follows the device, never an explicit choice (D-07).**
An operator on a light device who chooses dark gets a dark page inside a browser
chrome still tinted for light. The meta tag is read by the UA at parse time and
foundry does not rewrite it when the toggle is pressed. Accepted at DISCUSS;
unchanged here.

**2. The `localStorage` WRITE guard has no acceptance scenario, and cannot have
one (Divergence 4).** The mechanism that makes the write guard testable —
refusing site storage — also refuses the session cookie, so no signed-in screen
is reachable. And the theme control mounts only at `.sidebar__user`, inside
`partials/sidebar.html`, which only `app_shell.html` includes. "Storage is
refused" and "the control exists" are therefore mutually exclusive BY
CONSTRUCTION, not by harness limitation. Its protection is the review gate step
05-01 established instead: `static/js/theme.js` is byte-identical to canzan-lift's
`src/ui/assets/theme.js` apart from exactly two lines — `STORAGE_KEY` (`:41`) and
the mount selector (`:95`). That diff was re-verified in this step and is
unchanged. A third divergence is an escalation, not an edit.

The READ guard, which is the one with a real consequence, IS covered: scenario 25
drives a real storage-refused session and asserts the sign-in screen still themes
from the device with nothing reported. Unguarded, that read kills `theme.js` where
it stands and takes the device-driven palette down on EVERY page for that operator.

**3. S1 cannot see a colour inside a template `<style>` block — and that is now a
statement about the guard, not about the repo.** S1 (`check_stylesheet_colour_seam`)
scans `served_stylesheets(root)`. A colour literal written inline in a template is
outside its reach entirely. This was not hypothetical: `templates/settings.html`
carried three light-only literals on `.toast` (`border #b6d4c2`,
`background #eafaf1`, `color #14532d`), which painted a bright green card on an ink
page for every dark-device operator who opened notification settings. It sat in the
one blind spot both instruments share — outside S1 (inline, not served) and outside
scenario 14's rendered sweep (which walks board, dashboard, issue, shortcut list and
sign-in, but not settings).

Fixed by MOVING the rules into the stylesheet rather than tokenising them in place.
Tokenising inline would have fixed the colour and left the blind spot; moving them
puts them under S1. `settings.html` was the only template in the repo carrying an
inline `<style>`, so the gap closes with it — but the LIMIT REMAINS: nothing stops
the next template from opening a `<style>` block, and S1 will not notice. Extending
the scanner to template `<style>` blocks is the obvious follow-up and is NOT done
here.

The move forced re-hash #5, landed in this one commit across all five sites:
`static/css/foundry.64398394.css` → `foundry.6296815a.css`, `templates/base.html:6`,
`src/lib.rs` (×3), and the `VENDOR.md` row (filename, date, sha256). `check-arch`
R2 and R3 verify the rename and the row.

The toast keeps its affirmative reading as the jade hairline
(`border: 1px solid var(--cz-jade-line)`) and takes an OPAQUE card
(`background: var(--cz-surface)`, `color: var(--cz-text)`) rather than the
translucent `--cz-jade-soft`, because it carries text and D-05's opaque rule
applies to any surface a reader has to read off.

**4. The first-frame colour is still not observable.** Unchanged from DISTILL: the
flash scenario asserts the served `<script>` tag's shape (deterministic,
load-bearing) plus a paint-timing comparison (supporting, can pass by luck on a
fast loopback). Sampling the painted colours of the first frame needs a paint-level
capture surface this suite deliberately does not use.

### [NEW] Degradation lanes — how each one was made to discriminate

Both scenarios in 05-03 are single-example against a real browser substrate, which
is where the PBT mandate places degradation wiring. Neither would mean anything
without its anti-vacuity guard, so each guard is named:

| Scenario | Would pass vacuously if… | Guard |
|---|---|---|
| Scripting off, dark device | the device were light (an unthemed page is already light); or scripting were silently still on | `device_prefers_dark()` in the Given; `.sidebar__user` present AND `[data-kb-ready]` absent in the Then |
| Site storage refused, dark device | storage were not actually refused (it would be a second copy of the plain dark sign-in scenario) | the Given navigates to the REAL origin — Chrome's site-data setting is per-origin, so `about:blank` would report success — and asserts `localStorage.getItem` throws `SecurityError` |
| …and "nothing is reported" | the recorder recorded nothing at all | after the assertion, a deliberate uncaught `__RECORDER_PROBE__` throw must come back from the recorder |

**The storage-refused mechanism was MEASURED, not assumed** (chromedriver 151,
`goog:chromeOptions.prefs`, real `http://` origin):

```
no prefs                                READ=ok             WRITE=ok
cookies=2                               READ=SecurityError  WRITE=SecurityError
cookies=2 + block_third_party_cookies   READ=SecurityError  WRITE=SecurityError
```

Composing `cookies=2` with `--force-dark-mode` was measured too: `matchMedia`
still reports dark, so the capabilities do not interfere. No fallback is
documented because none is needed. A script-injected throwing storage accessor was
rejected (it would assert against the stub, not the browser, and would be the only
assertion in this lane not exercising a real substrate); quota-filling was rejected
as a flaky oracle.

**The unhandled-error recorder is a session capability, not an in-page hook.**
`goog:loggingPrefs: {"browser": "ALL"}` is set in `open_session`, so it is armed
before the first navigation and catches an error thrown while a `<head>` script is
still being parsed — which is exactly the failure mode scenario 25 guards. An
in-page `window.onerror` cannot do that: it would have to be installed by a script
running after the one it watches, and it would be destroyed by the navigation it
is meant to observe. `unhandled_script_errors()` filters the log to
`source == "javascript"`; the same log carries a `source: "network"` SEVERE entry
for the favicon the test origin does not serve, on every navigation, which is a
harness artefact and not something foundry reports to an operator.

**D-01b honoured.** This is foundry's SECOND scripting-disabled scenario. It
asserts what the board does with scripting off; it does not assert a blanket no-JS
guarantee the project has never made.

### [NEW] Mutation testing

**PASS — 191/218 viable mutants killed (87.6%)** on `xtask/src/check_arch.rs`,
the feature's only mutatable production surface. Full analysis of the 27 survivors,
and the reason they are not being chased, in
`docs/feature/canzan-theme-system/deliver/mutation/mutation-report.md`.
The feature is NOT recorded as unmutatable.
