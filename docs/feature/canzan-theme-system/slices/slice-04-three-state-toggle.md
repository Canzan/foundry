# Slice S04 — One control, three states: follow the device, or overrule it

**Story**: US-CTS-04 · **Job**: `job-canzan-theme` · **Effort**: 0.75 day
**Depends on**: S01 (both dark blocks — the attribute selector is half the
mechanism) · **Depended on by**: nothing

## What ships

`static/js/theme.js` — canzan-lift's control, ported with its logic
byte-identical and **exactly two values changed**: `STORAGE_KEY` becomes
`"foundry.theme"`, and the mount selector becomes foundry's. Both are hoisted to
named constants at the top of the IIFE so a future shared module takes them as
parameters (D-06). A reviewer can diff the two files and see two differing lines.

Plus: a plain render-blocking `<script>` in `base.html`'s head; the
`.theme-toggle` / `__glyph` / `__mode` rules in both palettes; the control added
to the mobile ≥44 px selector list; a `VENDOR.md` row in the **authored-in-tree**
shape (like `foundry.<hash>.css`), not the vendored-blob shape.

Mount: inside `.sidebar__user`, at the foot of the rail (D-09) — canzan-lift's
own rationale, that the control belongs "away from navigation, because it
changes how the desk LOOKS and never where she is". `.sidebar__user` is already
`margin-top: auto`; it *is* the far end.

## Learning hypothesis

> **A `<head>` render-blocking script that stamps `data-theme` before paint
> removes the light-flash entirely, and building the button in JS satisfies the
> no-JS lane with zero lane-specific branching — so the ported module needs
> exactly two foundry-specific values and nothing else.**

Falsified if a third divergence proves necessary. The known candidate is the
mount itself: canzan-lift appends to a horizontal `nav.top-nav` where
`margin-left: auto` pushes the control to the strip's end; foundry's rail is a
vertical `<aside>` where that property does nothing. If the CSS cannot absorb
that difference, the *script* must — and a third divergence means the "share one
module later" premise (intake D2) needs re-examining before the shared module is
attempted, not after.

**If falsified**: stop and decide whether the shared module takes a layout
parameter or whether both apps converge on one chrome shape. Do not quietly add
a third constant.

## Why last

Three reasons, in order of weight.

1. **It serves the narrower environment.** `env-night-triage` is already answered
   by the device after S01 + S02; only `env-light-os-dark-room` needs a control.
2. **The flash risk is introduced here and contained here.** Before the toggle
   exists, `prefers-color-scheme` resolves inside a render-blocking stylesheet
   and *cannot* flash. KPI 2 comes into existence with this slice.
3. **Nothing half-painted is left to reveal.** A toggle over an incomplete dark
   palette makes the incompleteness interactive — the operator presses a button
   and is shown the gaps.

Toggle-first was considered and rejected for reason 3. It is the flashiest demo
and the strongest instinct, which is why the rejection is recorded rather than
assumed.

## Watch items

- **THE regression risk: `defer`.** Five scripts already sit in `base.html`'s
  head and **all five correctly carry `defer`**. `theme.js` must not. Copying the
  surrounding lines is the single most likely mistake in the whole feature, and
  it fails silently — everything works, there is just a white flash on every
  navigation for exactly the operators the feature exists for. Put a comment at
  the tag saying why it differs.
- **Three states, not two.** "System" **removes** the attribute rather than
  writing a third value. That is one mechanism with the stylesheet's
  `:root:not([data-theme="light"])` guard — absence is what hands the decision
  back to the device. Changing either half alone breaks system-follow silently.
- **A wrong mount selector yields no control, silently**, because the ported
  script returns early when the mount is absent. Nothing on screen says the
  feature shipped. The intake named `.site-header`, which exists in **no**
  template (D-01a).
- **The control is absent from 15 screens and that is correct** (D-08). They
  still honour an explicit choice, because the attribute is stamped from
  origin-wide storage on every document.
- **foundry does not have a blanket no-JS guarantee.** It has one
  scripting-disabled scenario (`keyboard-shortcut-bindings.feature:113`). This
  slice adds the **second**. Do not write acceptance text that implies a D5-style
  commitment foundry has not made (D-01b).
- **Storage failures are silent by design.** A blocked-site-data profile throws
  on the first `localStorage` touch; every access is guarded, and the cost is
  persistence, never the control, and never a visible error.
- **`theme-color` does not follow an explicit choice** (D-07) — a known,
  user-accepted limitation, not an oversight. The media pair and the manifest
  values ship in **S01**, where the device-following markup belongs; this slice
  owns only the decision *not* to extend it. **Do not "fix" it here.** Making the
  meta track an override means adding behaviour to `theme.js`, which is the third
  divergence — and byte-identity is the entire basis of the future shared module
  (intake D2). The correct fix lands in both apps at once, after the module
  exists. Record it as an accepted limitation in the DELIVER notes rather than
  leaving the next reader to rediscover it as a bug.

## Demo

OS on Light → open the AUTH board → the rail's foot reads `◐ System` → click:
`☀ Light` → click: `☾ Dark`, board repaints to ink → navigate to `/report`, to
`/`, reload: all ink, no white frame anywhere → click: `◐ System`, back to
paper. Then: a scripting-disabled profile → no control at all, page follows the
OS. Then: a blocked-storage profile → control present, works, forgets on reload,
says nothing about it.

## Done when

US-CTS-04's seven scenarios are green, `diff` against canzan-lift's `theme.js`
shows exactly two differing lines, the `<script>` tag carries no
`defer`/`async`/`type=module`, and `git diff --stat crates/foundry-acceptance/`
shows additions only.
