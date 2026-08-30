# Component Boundaries — canzan-theme-system

Wave: DESIGN | Agent: nw-solution-architect (Morgan) | Date: 2026-08-29

Six components, four new. No new crate, no new dependency edge, no schema change.
Each section states what the component **owns**, what it **must not** do, and the
**contract** it offers — the boundaries DELIVER must not blur. Implementation
(rule bodies, matcher internals, script structure) is the crafter's.

---

## C1 — Token seam (`foundry.<hash>.css`, three regions)

**Owns.** Every colour value in foundry, and the three type-role tokens.
Three regions, and only these three:

| Region | Selector | Role |
|---|---|---|
| Light / default | `:root` | Base bindings + `--radius`, `--gap`, `--cz-gutter`, type tokens |
| Dark by device | `@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) }` | Re-binds colour tokens; sets `color-scheme: dark` |
| Dark by choice | `:root[data-theme="dark"]` | Identical bindings; sets `color-scheme: dark` |

**Must not.** Be merged (CSS cannot express "either" across a media query and an
attribute selector — D-03). Let the three regions declare different *sets* of
custom-property names; they differ in values only.

**Contract.** After S02, no rule outside these three regions writes a colour.
Enforced by **S1** and **S2** (ADR-004). Measured contrast ratios are recorded as
inline comments beside the tokens that produced them, in both palettes — which is
why the S1 scanner must strip `/* … */` before matching.

**Boundary note.** `--radius: 6px` already matches canzan's and is kept. `--gap`
is layout, not colour, and stays. The eight foundry colour tokens (`--bg`, `--fg`,
`--muted`, `--border`, `--surface`, `--accent`, `--accent-contrast`, `--danger`)
are **retired, not aliased** (D-02): an alias layer would leave two live names for
one colour.

---

## C2 — Component rules (~30 rule blocks, same file)

**Owns.** Geometry, spacing, typography application, states. Names `--cz-*`
tokens for every colour.

**Must not.** Write a colour literal (S1). Redefine a colour locally. Rename,
remove or newly require any render-contract selector (D-11) — `.column`,
`.issue-card`, `.comment*`, `.sidebar*`, `.dash`, `.card*`, `.modal*`,
`.keyboard-help`, and the `data-*` markers are the acceptance suite's selector
set. Request a `font-weight` outside a shipped range (ADR-001: display 600/700,
body 400–700, mono 400/500 — anything else is silently synthesised).

**Contract.** Two tinted, text-bearing surfaces — `.card__key` and
`.sidebar__item--active` — declare an **opaque** `background-color` beneath any
jade tint (D-05). Not statically checkable; a review-and-acceptance obligation,
recorded as deliberately unenforced in ADR-004.

**Deletions this feature owns.** `.site-header` and `.site-header .brand`
(`foundry.8ce38566.css:56-69`) — dead in all 32 templates (D-10).

---

## C3 — Type asset layer (`@font-face` + `static/fonts/`)

**Owns.** Three `@font-face` declarations and three derived woff2 blobs plus
three OFL licence texts.

| Blob | Instancing | Measured |
|---|---|---|
| Bricolage Grotesque (display) | `opsz=24` (centre of foundry's 20–30.4 px display band), `wdth=100` pinned; `wght` 600:700 | 29,764 B at `opsz=14`; **re-measure at 24**, ceiling 32,768 B |
| Public Sans (body, roman only) | `wght` 400:700 | 20,664 B |
| JetBrains Mono | `wght` 400:500 | 27,180 B |
| | **total** | **77,608 B** vs a 150 KB guardrail |

**Must not.** Reference a CDN or any cross-origin host (`VENDOR.md:4` — prohibited,
not merely discouraged). Introduce a build step: instancing runs offline from
`tools/fonts/derive-fonts.sh`, never from `cargo build` or CI. Use relative `src`
URLs — absolute `/static/fonts/…` is required so ADR-003's R1 matcher is uniform.
Ship italic (one `.empty-state` rule takes synthesised oblique instead).

**Contract.** `font-display: swap` plus a system fallback stack on every role, so
a font that never arrives costs a typeface and never a word (D-14). The fallback
must be metrically close enough that swap-in does not move the board — this is
S03's *learning hypothesis*, and DISCUSS honestly expects it to be the one most
likely to fail. If falsified, `size-adjust` / `ascent-override` descriptors go
into the `@font-face` blocks **and are recorded in `VENDOR.md` beside the row**,
because they are coupled to that exact derived blob.

---

## C4 — Theme-state adapter (`theme.js`)

A driven adapter over two substrates that lie: `localStorage` (throws when site
data is blocked) and the document at parse time.

**Owns.** Resolving `system | light | dark` and stamping `data-theme` on `<html>`
**before first paint**; building the `.theme-toggle` control; persisting the
choice best-effort.

**Must not.** Carry `defer`, `async` or `type="module"` — it is the only one of
six head scripts that must not, and copying the five neighbours is named by
DISCUSS as the single most likely regression in the feature. A comment at the tag
must state why it differs. Be server-rendered (it must be *absent* with scripting
off, not dead). Register a document-level listener — `Escape` has exactly one
owner, `keyboard.js::closeTopLayer()` (brief.md BR-4); this script listens on its
own button only. Diverge from canzan-lift's source by more than **two lines**
(`STORAGE_KEY` → `"foundry.theme"`, mount → `.sidebar__user`), both hoisted to
named constants so a future shared module takes them as parameters (D-06).

**Contract.** "System" **removes** the attribute rather than writing a third
value — absence is what hands the decision back to the device, and it is the
mechanism the stylesheet's `:not([data-theme="light"])` guard depends on. This is
one mechanism written in two files; S2 protects the CSS half, the byte-identical
port protects the JS half.

**Degradation, all specified by DISCUSS.** Storage throws → mode still applies for
the session, persistence lost, nothing surfaced. Mount absent → early return, no
control, palette still correct. Scripting off → no control at all, page follows
the OS exactly as before the feature.

**If a third divergence proves necessary — escalate, do not absorb.** S04's
learning hypothesis names the likely falsifier: canzan-lift appends to a
*horizontal* `nav.top-nav` where `margin-left: auto` pushes the control to the
strip's end, while foundry's rail is a *vertical* `<aside>` where that property
does nothing. The design's position is that **CSS must absorb this, not the
script** — `.sidebar__user` is already `margin-top: auto`, so the rail's far end
already exists and the control simply appends into it. If CSS genuinely cannot,
the correct response is to **stop and decide whether the future shared module
takes a layout parameter, or whether both apps converge on one chrome shape** —
and to record that decision. Quietly adding a third constant is the one forbidden
outcome: byte-identity is the entire basis of intake D2's shared module, so
spending it silently costs the thing the port exists to buy.

---

## C5 — Brand chrome (`base.html:11` meta pair + `manifest.webmanifest`)

**Owns.** `<meta name="theme-color">` as a **media-scoped pair** (light `#fbfbf9`,
dark `#0a0c0b`) and the manifest's `theme_color` / `background_color`, moved off
the three off-contract literals (`#1c1c22` ×2, `#ffffff`).

**Must not.** Drop either manifest key — `pwa-mobile-rendering.feature:100`
asserts they are *declared*; values may move, keys may not. Grow behaviour in
`theme.js` to make the meta follow an explicit override.

**Accepted limitation (D-07, user-confirmed).** An explicit toggle override does
**not** update browser chrome; only the device preference does. A static JSON
manifest cannot follow a runtime choice at all, and making the meta follow one
would spend exactly the byte-identical-port property intake D2's shared module
depends on. Visible only in the override case, only in browser chrome. Do not
"fix" it.

**Safe by evidence.** `feature_pwa_mobile.rs:883` uses `document.querySelector`,
which returns the **first** match and checks only that content is non-empty — so
a media pair does not break S10 and does not violate D-11.

---

## C6 — Asset & seam guard (`xtask/src/check_arch.rs`) — NEW

Two functions in the existing `fn(&Path) -> Vec<String>` rule shape, appended to
the violations vec at `check_arch.rs:56-64`.

| Fn | Rule | Assertion | Gold test plants |
|---|---|---|---|
| `check_static_asset_integrity` | **R1** | every `/static/…` reference resolves | a renamed hashed CSS → must name `base.html` + the dangling path |
| | **R2** | every `<stem>.<8hex>.<ext>` filename equals its own sha256 prefix | an appended byte, no rename → must name the file |
| | **R3** | every `VENDOR.md` row's recorded sha256 recomputes | a row with a wrong sha256 → must name the row |
| `check_stylesheet_token_seam` | **S1** | no colour literal outside the three regions | `color: #ff0000` below the seam → must name file:line |
| | **S2** | the three regions declare an identical set of `--*` names | a token deleted from the media block only → must name the token + region |

**Five rules, five gold tests.** The counts must match; an earlier draft listed
four and silently omitted R3, which is the rule that machine-checks ADR-002's
provenance model. Each test also asserts the rule stays **silent on a clean
tree** — a rule that always fires is as useless as one that never does.

**Owns.** Its own input set — **derived by scanning, never enumerated**, so the
guard cannot itself go stale. Adding a font blob and referencing it from
`@font-face` enrols it automatically.

**Must not.** Scan `crates/foundry-acceptance/` (it holds deliberate
non-resolving fixtures at `feature_b_web_tier.rs:486` and `:495`). Region-skip
`#[cfg(test)]` (the three protected hash literals live inside it,
`lib.rs:312-373`) — an explicit departure from `check_no_static_lane_list`'s
posture at `check_arch.rs:602`, and the reason must be in the doc comment. Grow a
hand-maintained allowlist.

**Contract.** Runs in `run_ci` gate 3 (`main.rs:191-195`) **and** `run_smoke`
(`main.rs:270-308`), so it bites pre-commit. On violation it names the offending
`file:line` and exits non-zero, matching the house diagnostic shape. Each rule
carries an **injected-violation gold test** (Principle 12c) as an **`xtask` unit
test** in the `#[cfg(test)]` module beside `check_arch.rs`, staging a fixture tree
with the existing `tempfile` dev-dependency (`xtask/Cargo.toml:19-21`) and calling
the rule function directly. `check-arch` has no driving port — it is a pure
function of a directory — so a cucumber scenario was the wrong shape, and DISTILL
Decision 4 places infrastructure outside acceptance scope. The gold tests are a
**DELIVER obligation** and land in the same commit as their rules.

---

## Interaction contracts (what breaks if a seam is crossed)

| From → To | Contract | Failure if violated |
|---|---|---|
| C4 → C1 | `data-theme` present/absent on `<html>` before first paint | Present but late → a light frame on every navigation for exactly the operators the feature serves (KPI 2) |
| C1 ↔ C1 | The three regions declare identical token *name* sets | Divergence breaks **only** dark-by-device; the toggle masks it in testing (S2 exists for this) |
| C2 → C1 | Colour only by token name | Erosion back to 46 literals (S1 exists for this) |
| C1 → C3 | `@font-face` src at absolute `/static/fonts/…` | R1 cannot match relative URLs; the fonts become unguarded |
| C5 → device | Media-scoped, first-match-lenient | A single non-media meta would still pass S10 but lose the device follow |
| C6 → all | Derived input set, no allowlist | A hand-maintained list is the staleness the guard exists to kill |
| Everything → acceptance | Zero selector churn beyond `.theme-toggle*` | KPI 4 fails; the feature's central claim becomes false |
