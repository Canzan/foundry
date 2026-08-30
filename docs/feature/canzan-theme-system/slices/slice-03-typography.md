# Slice S03 — foundry reads in canzan's voice: Bricolage, Public Sans, JetBrains Mono

**Story**: US-CTS-03 · **Job**: `job-canzan-theme` · **Effort**: 1 day
**Depends on**: nothing (independent of S02 and S04; sequenced after S01 only to
share its re-hash rhythm) · **Depended on by**: nothing

## What ships

Three self-hosted woff2 families **and the rules that make them visible**, in one
step. This is deliberate: a slice that vendored three blobs and wrote three
`VENDOR.md` rows would change nothing a user could see and would fail the
slice-composition gate as `@infrastructure`-only (D-13). Vendoring and applying
are one slice or they are not a slice.

- Three type tokens with system fallbacks: display (Bricolage Grotesque → Public
  Sans → system), body (Public Sans → system stack), mono (JetBrains Mono →
  `ui-monospace`).
- The three ad-hoc stacks are retired: `--font`, `.dash`'s duplicate system
  stack, and the two bare `ui-monospace` uses (`.card__key`,
  `.keyboard-help dt`).
- The canzan eyebrow/label idiom — mono, ~.6875rem, .18em tracking, uppercase —
  applied to `.column h2` and `.dash__section h2`, **in `--cz-faint`, exactly as
  canzan.net writes it**. S01 rebound that token's value (`#6e756f` / `#78807b`)
  so the idiom clears 4.5:1 at ≈11 px in both palettes; the tier is unchanged
  (D-04).
- Blobs under `static/fonts/`, served by the existing `ServeDir` route. Four
  `VENDOR.md` rows: version, upstream canonical URL, retrieval date, sha256,
  licence.

## Learning hypothesis

> **The system fallback stack is metrically close enough to the canzan faces
> that swapping them in does not reflow the board — so no `size-adjust` /
> `ascent-override` font-metric overrides are needed.**

Falsified if the lane columns or issue cards move between the fallback paint and
the webfont paint. Bricolage Grotesque is a variable display face with wide
apertures; Public Sans is a Libre Franklin derivative. Neither is metrically
matched to `-apple-system` or `Segoe UI`, so the honest expectation is that this
hypothesis is the one most likely to fail in the feature.

**If falsified**: `size-adjust`/`ascent-override` descriptors go into the
`@font-face` blocks, tuned per family. That is a real cost — it must be re-tuned
whenever a font version is bumped — and it should be recorded in `VENDOR.md`
beside the row, so the next person updating a blob knows the descriptor is
coupled to that exact release.

## Why third

Typography moves the coherence KPI (6) but not the comfort KPI (1). It is the
only slice nothing depends on and nothing else depends on it, which is precisely
why it should not go earlier: it is the slice that can move if something forces
it to.

## Watch items

- **`VENDOR.md` requires a licence per blob and this wave verified none.** All
  three families are believed openly licensed, but no release artefact has been
  checked. Pin version + canonical URL + licence before writing the rows; this is
  Unresolved #2 in the feature-delta.
- **No CDN, ever** — `VENDOR.md` states "NO CDN at runtime". Google Fonts is
  prohibited, not merely dispreferred. This is the first vendoring since htmx
  2.0.4 and the first font blob in the repo.
- **Payload guardrail: ≤150 KB added** across all new blobs (KPI 7). Three
  unsubsetted variable fonts would blow this; a latin subset should land it well
  under. Measure, do not assume.
- **No invisible text, ever** (D-14). The fallback paints immediately; the canzan
  face swaps in. A profile that blocks font loading must still render every
  string.
- **`.dash`'s own `font-family`** — coordinate with S02 so exactly one slice
  removes it.
- **Five sites re-hash** (file, `base.html`, `VENDOR.md`, three literals in
  `src/lib.rs` at 329, 346, 365).

## Demo

Open the AUTH board with an empty cache and the network panel visible: three
font requests, all same-origin, zero to `fonts.gstatic.com`. "Identity Platform"
in Bricolage; card titles in Public Sans; `AUTH-7` in JetBrains Mono; the
IN-PROGRESS column header in the eyebrow idiom. Put the window beside
canzan.net and answer the coherence question by eye.

## Done when

US-CTS-03's six scenarios are green, every blob's recorded sha256 recomputes,
the board's column and card positions are unchanged between fallback and
webfont paint, and `git diff --stat crates/foundry-acceptance/` shows additions
only.
