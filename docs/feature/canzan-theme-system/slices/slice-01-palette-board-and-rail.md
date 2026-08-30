# Slice S01 — The board and its rail wear canzan's palette, light and dark

**Story**: US-CTS-01 · **Job**: `job-canzan-theme` · **Effort**: 1 day
**Depends on**: nothing · **Depended on by**: S02, S04

## What ships

The `--cz-*` token block and **both** dark blocks, plus every base and
board/rail rule re-pointed onto them. After this slice a dark-preferring
operator's board is dark — frame, rail, columns, cards, buttons, inputs, focus
ring and selection ring.

Surfaces re-authored: `:root` · `body` · `a` · `:focus-visible` · `button` /
`.button` / `input[type=submit]` · `label` · text inputs and `textarea` ·
`.board` · `.column` · `.column h2` · `.issue-card` · `.issue-card.kb-selected`
· `.empty-state` · `.comment*` · `.app-shell*` · `.sidebar*` (all 15 literals)
· the `@media (max-width: 480px)` rail override.

Also: the dead `.site-header` / `.site-header .brand` rules deleted (D-10); the
header comment rewritten (it currently claims the file is "NOT a design system,
theming, or dark mode" — all three become false).

**Brand chrome, in this slice** (D-07). Three literals live outside the
stylesheet and outside the `--cz-*` contract, and they land here because they
are device-following markup that ships with the palette — the toggle in S04
explicitly does *not* touch them:

| Site | Today | Becomes |
|---|---|---|
| `templates/base.html:11` | one `<meta name="theme-color" content="#1c1c22">` | a media-scoped pair — light `#fbfbf9`, dark `#0a0c0b` |
| `manifest.webmanifest` `theme_color` | `#1c1c22` | a canzan value |
| `manifest.webmanifest` `background_color` | `#ffffff` | a canzan value |

**AC**: the media pair and both manifest values are on the contract; **both
manifest keys remain declared** (`pwa-mobile-rendering.feature:100` asserts
presence, not value); and S10 stays green —
`feature_pwa_mobile.rs:883` uses `document.querySelector('meta[name="theme-color"]')`,
which takes the **first** match and only checks its content is non-empty, so a
pair is safe and D-11 is not violated.

## Learning hypothesis

> **The eleven `--cz-*` tokens are sufficient to express an *application shell*
> — a selected navigation item, a keyboard selection ring, a hover state — that
> canzan.net, a marketing site with none of those, never needed.**

Falsified if we must invent a twelfth token. The likeliest failure is
`.sidebar__item--active`: canzan.net's only "tinted surface" is
`--cz-jade-soft`, which is translucent and therefore barred from carrying text
alone (D-05), so an active nav item needs an opaque tint the contract does not
supply.

**If falsified**: the "shared contract" claim behind intake D4 is weaker than
assumed, and canzan-lift's eventual migration will need the same addition. Record
the new token as a *proposed extension to the canzan contract*, not as a foundry
local — otherwise the two apps diverge at exactly the point unification matters.

## Why this slice is first

It is the only slice that moves the north-star KPI off zero (no dark palette
exists in any form today), and it is the feature's only hard dependency edge:
S02 and S04 both consume what it introduces.

## Watch items

- **46 literals, and 15 of them are in the rail.** Re-pointing only the eight
  already-tokenised rules ships a white stripe down the left of a dark app. The
  rail block uses zero tokens today.
- **Three accents collapse into one.** `--accent` #2452c9, rail indigo
  #5b5bd6/#ecedff/#3a3ad1, card-key indigo #4f46e5/#eef2ff → jade. The card-key
  pair is S02's; the rail's are this slice's.
- **The selection-ring comment goes stale.** It cites `#2452c9 on #ffffff ≈ 7:1`.
  Jade is 5.08:1 on paper and 9.74:1 on ink — still far past 1.4.11's 3:1, but
  the number in the file must change with the colour.
- **`--cz-faint` is rebound, not abandoned** (D-04). canzan.net's values fail
  4.5:1 in both palettes (3.24 / 3.52); foundry binds `#6e756f` light (**4.57**)
  and `#78807b` dark (**4.83**). `.column h2` keeps using `--cz-faint` — the tier
  survives, only the hex moves. Record all six ratios inline (text / muted /
  faint × two palettes): light 17.62 / 5.89 / 4.57, dark 16.32 / 6.38 / 4.83.
  Check by eye that muted and faint still read as different tiers; if they do
  not, the rebind bought accessibility at the cost of the hierarchy and needs
  re-tuning, not acceptance.
- **Five sites re-hash in one commit**: the file (rename), `base.html`,
  `VENDOR.md`, and three literals in `foundry-app/src/lib.rs` (329, 346, 365).
  No automated guard existed at DISCUSS. ADR-CANZAN-THEME-003's R2 rule builds one in this
  feature (DELIVER obligation #0); until it lands, this discipline is manual.

## Demo

OS set to Dark → open `/team/backend/project/identity-platform` → the whole
frame is ink, the rail included. Flip the OS to Light → reload → paper and jade,
same geometry. Set `data-theme="dark"` by hand on a Light OS → ink. Remove it →
paper.

## Done when

US-CTS-01's six scenarios are green, every re-bound pair's measured ratio is
recorded inline in both palettes, and `git diff --stat crates/foundry-acceptance/`
shows additions only.
