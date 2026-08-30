# ADR-CANZAN-THEME-001: Ship all three canzan faces, instanced to the axes and weights foundry actually uses

## Status

Accepted (canzan-theme-system DESIGN wave, 2026-08-29)

## Context

Intake D3 and US-CTS-03 require foundry to self-host canzan.net's three type
families — Bricolage Grotesque (display), Public Sans (body), JetBrains Mono
(mono) — from its own origin. `crates/foundry-app/static/VENDOR.md:4` forbids a
CDN at runtime, so self-hosting is the only compliant path, not a preference.

The naive path — commit the woff2 files the Google Fonts API serves — costs
**~210 KB**, measured: Bricolage Grotesque 131,312 B (variable, three axes),
JetBrains Mono 31,340 B (statics 400+500), Public Sans roman+italic ~47,000 B.

Two facts make that unacceptable rather than merely large:

1. **It fails KPI 7**, a DISCUSS *guardrail*: "Added static payload ≤150 KB
   across all new blobs." 210 KB is a 40 % overrun on a guardrail, not a target.
2. **Proportion.** foundry ships **zero** webfonts today, and htmx — its only
   vendored runtime dependency (`VENDOR.md:16`, 2.0.4) — is ~48 KB. Naive
   adoption roughly quintuples static payload, 131 KB of it a *display* face used
   only for headings.

Meanwhile the reference stylesheet uses far less of these fonts than the full
files carry. Measured against `docs/feature/canzan-theme-system/canzan-net-reference.css`:

| Family | Axes shipped upstream | Weights canzan.net actually uses |
|---|---|---|
| Bricolage Grotesque | `opsz` 12–96, `wdth` 75–100, `wght` 200–800 | **600 and 700 only** |
| Public Sans | `wght` 100–900 (+ separate italic file) | 400, 500, 600, 700 |
| JetBrains Mono | `wght` 100–800 | **500 only** |

foundry's own current stylesheet uses weights 400, 600 and 700
(`foundry.8ce38566.css:66,185,279,453,465,493`) plus mono at 400
(`:555`) and 600 (`:270`). Every axis except `wght` is unused by both
stylesheets, and `wdth` and `opsz` carry the bulk of Bricolage's delta data.

All three families are OFL-1.1 and — verified from the licence text itself, not
from secondary description — **none declares a Reserved Font Name**. Each
copyright line reads `Copyright <year> The <Family> Project Authors (<repo URL>)`
and proceeds directly to the licensing sentence, with no "with Reserved Font
Name" clause. Sources: `raw.githubusercontent.com/ateliertriay/bricolage/main/OFL.txt`,
`raw.githubusercontent.com/uswds/public-sans/develop/LICENSE.md`,
`raw.githubusercontent.com/JetBrains/JetBrainsMono/master/OFL.txt`. This closes
feature-delta Unresolved #1 and means a derived subset may keep the upstream
family name in `@font-face`. (Licence-text reading, not legal advice.)

## Decision

Keep canzan.net's exact type identity — all three families — and pay for it by
**instancing the variable axes down and subsetting to latin** offline. One blob
per family, three blobs total.

| Blob | Instancing | Latin woff2 |
|---|---|---|
| Bricolage Grotesque | `opsz=24` and `wdth=100` **pinned**; `wght` narrowed to the range **600:700** | **29,964 B** |
| Public Sans (roman only) | `wght` narrowed to the range **400:700** | **20,664 B** |
| JetBrains Mono | `wght` narrowed to the range **400:500** | **27,180 B** |
| | **total** | **77,608 B (75.8 KiB)** |

Against KPI 7's 150 KB budget that leaves ~50 % headroom even after `theme.js`
(~2.6 KB). Against the naive path it is a **63 % reduction**, and Bricolage alone
falls 131,312 → 29,764 B (**−77 %**).

These are measured figures, not estimates, produced with `fonttools 4.63.0` +
`brotli` on Python 3.14.7. Two caveats are recorded rather than smoothed over:

- The Bricolage figure was measured at **`opsz=14`**, the value every named
  instance of the family uses. This ADR specifies **`opsz=24`** instead, because
  foundry's display type spans **20–30.4 px** — `.modal-header h2` at 1.25rem
  (`foundry.8ce38566.css:369-372`) and `.dash h1` at 1.9rem (`:213-216`), with
  `.column h2` (`:130-133`) moving to the mono eyebrow idiom rather than display.
  `opsz=24` sits at the centre of that band. canzan.net renders its headings with
  `font-optical-sizing: auto`, i.e. at the used pixel size.

  **The size claim is a testable expectation, not a guarantee.** The reasoning is
  that pinning *any* single `opsz` removes the same delta data and leaves glyph
  count and table structure identical, so the result should land within a few
  hundred bytes of the `opsz=14` measurement. That is sound for a display face
  with shallow optical differentiation — but variable-font architecture *permits*
  size-specific glyph substitution and per-instance hinting, and nothing verified
  that Bricolage declines to use them. **The only measurement in hand is at
  `opsz=14`; `opsz=24` is extrapolated.** DELIVER re-measures and **records the
  actual figure in `VENDOR.md` beside the row**, so the next person knows whether
  the assumption held rather than re-deriving the question. Budget ceiling for
  this blob: **32,768 B**.

  If the ceiling is exceeded, **do not stop and escalate** — a one-person team
  cannot afford a stop-the-world gate on a few hundred bytes, and the fallback is
  already known. Descend this pre-authorised ladder and record which rung was
  taken:

  | Rung | Action | Expected |
  |---|---|---|
  | 1 | `opsz=20` — still inside the 20–30.4 px band | ~29–30 KB |
  | 2 | `opsz=14` — the measured, known-good value every named instance uses | **29,764 B, measured** |
  | 3 | Only if rung 2 also exceeds 32,768 B: the assumption that pinning `opsz` is byte-neutral is **false**, which invalidates this ADR's reasoning rather than its arithmetic. *Then* stop, and reopen against alternative B (drop Bricolage). | — |

  Rung 2 is a measured fact, not a hope, so rungs 1–2 cannot both fail on size.
  Reaching rung 3 means something is wrong with the model, and that is worth
  stopping for; missing a ceiling by 400 bytes is not.
- The measurement fetched sources from `google/fonts` at `main` — a moving ref.
  The recipe below pins the **authoritative upstream** at an immutable ref
  instead (see ADR-CANZAN-THEME-002), so the recorded input sha256 must be taken
  at DELIVER against the pinned artefact, not carried over.

**Weight discipline.** A `font-weight` outside a shipped range is silently
synthesised by the browser — a smeared faux-bold that looks like a rendering bug
and has no test. The stylesheet must therefore request only shipped weights:

- display: 600 or 700 only;
- body: 400–700 only;
- mono: 400 or 500 only — which requires `.card__key`'s `font: 600 …`
  (`foundry.8ce38566.css:270`) to move to 500, canzan.net's own mono weight.

**Italic is not shipped.** Public Sans' upright and italic are separate files
upstream (`PublicSans[wght].ttf`, `PublicSans-Italic[wght].ttf`). foundry
contains exactly one italic rule — `.empty-state`
(`foundry.8ce38566.css:173-176`), a muted empty-state string. That one string
takes a synthesised oblique; the ~20 KB italic file is not worth one muted line.

**Offline, occasional, documented.** Instancing and subsetting run on a
maintainer's machine when a font is added or bumped — never in `cargo build`,
never in CI, no Node, no bundler (`VENDOR.md:3-4`, assets.md DB6). The recipe and
its pinned tool versions live in `VENDOR.md` beside the rows, so the transform is
part of the asset's provenance rather than tribal knowledge; how it is recorded
so an auditor can still verify a *derived* blob is ADR-CANZAN-THEME-002.

## Consequences

- Positive: KPI 6 ("3 of 3 type families present") and KPI 7 (≤150 KB) are both
  met, which no other option achieves — the identity is kept *and* the budget is
  kept, with headroom for a fourth blob later.
- Positive: one blob per family means three font requests on a cold load, not
  five or six, and no static-weight matrix to keep in sync with the stylesheet.
- Negative: the blobs are **derived**, so they are not verifiable against an
  upstream-published sha256. This breaks `VENDOR.md:9-12`'s stated audit model
  and is the entire subject of ADR-CANZAN-THEME-002. It is the real price of this
  decision and is paid there, not waved away here.
- Negative: `opsz` is frozen. If a future screen sets display type far outside
  17–30 px — a large marketing headline, or 12 px display text — it renders at
  the wrong optical size. Cheap to revisit (re-run the recipe with a different
  pin); recorded so the next person knows the axis was a choice.
- Negative: latin-only. Any future non-latin content (a workspace named in
  Cyrillic, a CJK issue title) falls back to the system stack for those glyphs.
  Acceptable and in fact *safe* — D-14's "a font that never arrives costs a
  typeface, never a word" already guarantees the fallback path renders. Recorded
  because it is a silent, not a loud, limitation.
- Negative: three weight constraints now bind the stylesheet (display 600/700,
  body 400–700, mono 400/500), and nothing enforces them. A future rule
  requesting `font-weight: 800` synthesises silently. Recommended follow-up: fold
  a weight-range check into the CSS structural check of
  ADR-CANZAN-THEME-004, where the parser already walks the file.
- Neutral: `.empty-state` renders in synthesised oblique rather than a drawn
  italic. Visible only to someone comparing letterforms on an empty board.

## Alternatives considered

- **A. Ship the Google-served woff2 files verbatim** — Rejected. 210 KB against a
  150 KB guardrail, and it quintuples a payload whose only prior runtime
  dependency is 48 KB. Its one real merit is that it keeps `VENDOR.md`'s
  upstream-verbatim audit model intact and makes ADR-CANZAN-THEME-002
  unnecessary — a genuine simplification, and the strongest argument for this
  option. Rejected because a guardrail overrun is not tradeable against
  documentation convenience, and because 131 KB for a headings-only face is
  disproportionate on its own terms.
- **B. Drop Bricolage Grotesque; use Public Sans for display too** — Rejected,
  though it is the *simplest* option and costs 0 KB extra: canzan.net's own stack
  already falls back `"Bricolage Grotesque", "Public Sans", …`, so this is a
  supported degradation, not a hack. Rejected because KPI 6 explicitly requires
  "3 of 3 type families present", and because the display face is the half of the
  identity a stranger notices first — US-CTS-03's whole decision ("can foundry be
  shown beside canzan.net as one product?") is answered by the headings. Worth
  keeping on the shelf: if the Bricolage blob ever misses its 32,768 B ceiling,
  this is the fallback, not a bigger budget.
- **C. Two static instances of Bricolage at wght 600 and 700** — Rejected on
  measurement. 18,876 + 18,560 = **37,436 B** across two blobs and two requests,
  versus **29,764 B** in one — 26 % larger and one extra round trip, because two
  statics duplicate outlines, hinting and tables that one narrowed variable
  shares. The same result held for JetBrains Mono (statics 400+500 = 38,868 B
  versus 27,180 B for the range). The intuition that "static is smaller than
  variable" is false at these axis widths and was tested rather than assumed.
- **D. Pin `opsz`/`wdth` but keep the full `wght` 400:800 range** — Rejected, but
  narrowly: 33,624 B versus 29,764 B, so narrowing `wght` to 600:700 buys only
  3,860 B (~11 %). Almost all the saving comes from pinning `opsz` and `wdth`,
  not from the weight range. Rejected anyway because the wider range buys nothing
  — no rule in either stylesheet requests display weights outside 600–700 — and
  an unused range is an invitation to request a weight the design never chose.
- **E. Keep `opsz` as a live axis for true optical sizing** — Rejected. It is the
  typographically superior option and would match canzan.net's
  `font-optical-sizing: auto` exactly, but `opsz` and `wdth` carry most of
  Bricolage's 131 KB; keeping `opsz` would give back the great majority of the
  saving for a refinement invisible across a 17–30 px band.
- **F. Ship a single JetBrains Mono static at weight 500** — Rejected, at a cost
  of 7,404 B (19,776 B versus 27,180 B). It is the most faithful option, since
  canzan.net uses mono at exactly one weight, and it is the smallest. Rejected
  because it forces foundry's existing 400-weight keycaps
  (`foundry.8ce38566.css:554-561`) up to 500, and a heavier `dt` widens the
  `auto` column of the keyboard-help grid (`:547-553`). The feature's governing
  constraint is that nothing moves; 7.4 KB against 50 % headroom is the correct
  side to err on.


## Measured at the specified configuration (2026-08-29, coordinator)

The figures above were originally measured at `opsz=14`, the value every named
instance of Bricolage uses, and extrapolated to the specified `opsz=24`. That
extrapolation is now **measured, not assumed**. The recipe in
`design/technology-stack.md` was run verbatim — `fonttools 4.63.0`, Python
3.14.7 — against each family's **pinned upstream ref**, not the `google/fonts`
mirror:

| Family | Axes | Intermediate TTF | woff2 |
|---|---|---|---|
| Bricolage Grotesque | `opsz=24 wdth=100 wght=600:700` | 69,456 B | **29,964 B** |
| Public Sans | `wght=400:700` | — | **20,696 B** |
| JetBrains Mono | `wght=400:500` | — | **25,768 B** |
| | | **total** | **76,428 B (74.6 KiB)** |

> **SUPERSEDED at DELIVER 04-01 — the figures above came from a NON-HERMETIC
> run.** The recipe as written here was not deterministic: `varLib.instancer`
> stamps `head.modified` from the wall clock, and its IUP optimiser makes a
> float-tolerance choice that differs across platforms. Both were found by the
> ADR-002 second-environment probe, which FAILED on first attempt. The recipe now
> pins `SOURCE_DATE_EPOCH=1787961600` and passes `--no-optimize
> --no-recalc-timestamp`; `--no-optimize` costs ~200 B of unoptimised `gvar` and
> is load-bearing for the audit. The deterministic figures, **identical on macOS
> and in a Debian container, at both the intermediate anchor and the woff2
> output**:
>
> | Family | Axes | Intermediate anchor | woff2 |
> |---|---|---|---|
> | Bricolage Grotesque | `opsz=24 wdth=100 wght=600:700` | `f2f8d04b0aff…` | **29,788 B** |
> | Public Sans | `wght=400:700` | `2eea0d13f535…` | **20,672 B** |
> | JetBrains Mono | `wght=400:500` | `446cb9929b6d…` | **25,956 B** |
> | | | **total** | **76,416 B (74.6 KiB)** |
>
> The conclusions are unchanged and slightly strengthened: **Bricolage lands at
> 29,788 B against the 32,768 B ceiling — 2,980 B of headroom, no fallback rung
> taken** — and the total is 49 % under KPI 7. Full provenance in
> `crates/foundry-app/static/VENDOR.md § Derived assets`.

**The ceiling holds and no fallback rung is needed.** `opsz=24` costs **29,964 B**
against the pre-authorised 32,768 B ceiling — 2,804 B of headroom. The step from
`opsz=14` (29,612 B measured here) to `opsz=24` is **+352 B**, inside the "few
hundred bytes" the extrapolation predicted. Bricolage's `opsz` axis does not
carry size-specific glyph data of any consequence.

The total is **76,428 B**, marginally *better* than the 77,608 B this ADR
estimated, and 49 % under KPI 7's 150 KB guardrail.

**Both previously-unverified facts are now verified.** All three
`fonts/variable/…` paths resolve at their pinned refs (`ateliertriay/bricolage`
at `84745e5b…`, `uswds/public-sans@v2.001`, `JetBrains/JetBrainsMono@v2.304`).
The Bricolage blob at the pinned commit is **byte-identical** to the
`google/fonts` mirror copy (same sha256, 408,496 B) — so the mirror was not a
source of error here, though ADR-002's rule to pin the authoritative ref stands
on reproducibility grounds regardless.

**One small discrepancy, and it is the provenance model working as designed.**
The original `opsz=14` figure was 29,764 B; re-running the identical recipe on
this machine produced 29,612 B — a 152 B (0.5 %) difference from brotli build
variation across environments. This is precisely what ADR-002 predicts, and
precisely why its Tier-2 audit anchors on the sha256 of the **intermediate TTF**
rather than the woff2 output. The intermediate is the stable artefact; the woff2
byte count is not, and the model already says so.
