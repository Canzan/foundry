# Slice S02 — Every remaining screen matches: dashboard, dialogs, shortcut overlay, sign-in

**Story**: US-CTS-02 · **Job**: `job-canzan-theme` · **Effort**: 0.75 day
**Depends on**: S01 (token block + both dark blocks) · **Depended on by**: nothing

## What ships

The four surface groups S01 deliberately left alone, because none of them ever
used a token:

| Group | Rules | Literals |
|---|---|---|
| Dashboard | `.dash`, `.dash__welcome/workspace/section/empty`, `.card a`, `.card a:hover`, `.card__key`, `.actions a`, `.actions button` (+ hovers) | 21 |
| Dialog | `.modal` backdrop, `.modal-dialog` shadow, `.modal-close` hover | 2 (+ 2 already tokenised) |
| Keyboard help | `#kb-overlay-root:not(:empty)` scrim, `.keyboard-help`, `dt`, `dd` | 6 |
| Chrome-less screens | the 15 templates extending `base.html` directly | 0 own rules — they inherit `body` |

After this slice the file contains colour values **only** inside the token block
and the two dark blocks.

## Learning hypothesis

> **foundry's non-board screens carry no colour decision the board did not
> already answer — so this slice is pure substitution: zero new tokens, zero new
> contrast pairs to argue about.**

Falsified if any of these needs a value S01 did not already bind. Two candidates:
the `.card__key` chip (a tinted, text-bearing surface — the same shape as
S01's `.sidebar__item--active` problem, so if S01's hypothesis held, this one
should too) and the two scrims, which are translucent black over an already-dark
page and may need a different alpha rather than a different colour.

**If falsified**: the count of tokens the canzan contract is missing for
application use rises above one, which strengthens the case for treating the
extension as a contract-level change (see S01) rather than a local patch.

## Why second, not third or fourth

S01 alone ships a *partial* dark mode, and a partial dark mode is worse than a
coherent light one: `?` is a reflex press and a white card over an ink board is
the most visible defect this feature could ship. S01 + S02 together fully
deliver the north star for `env-night-triage`. Nothing depends on S02, so it
could in principle go later — it goes second because leaving the gap open is
the thing that makes the feature feel unfinished.

## Watch items

- **The `?` overlay is the highest-visibility surface in the slice.** It is the
  one an operator opens by reflex, and it currently paints `#ffffff` with a
  `rgba(28,28,34,.45)` scrim — over an ink board that would be a white card in a
  grey haze.
- **`.card__key` must take an opaque tint** (D-05). `#4f46e5` on `#eef2ff` becomes
  jade on an opaque tinted surface, not a `--cz-jade-soft` wash.
- **`--cz-shadow` values are in hand** — light
  `0 1px 2px rgba(18, 22, 20, 0.04), 0 8px 24px rgba(18, 22, 20, 0.05)`, dark
  `0 1px 2px rgba(0, 0, 0, 0.4), 0 12px 32px rgba(0, 0, 0, 0.32)`. Bind them as a
  token and use it on `.card a:hover` and `.modal-dialog`. Note the dark layer is
  deliberately deeper and wider: a shadow tuned for paper vanishes on ink.
- **Form controls.** The dialog has real `<input>` and `<textarea>` elements. If
  S01's `color-scheme` is doing its job, the caret, placeholder and native
  chrome follow for free; verify rather than assume, and do not paper over a
  missing `color-scheme` with per-property overrides.
- **`.dash` carries its own `font-family`** (a second system stack). That
  declaration is replaced here or in S03 — pick one and do not leave both
  touching it.
- **Sign-in has no chrome at all** — no rail, no header, no mount point. It is
  correct for it to have no control and still honour an explicit choice (D-08);
  the scenario exists to make that intentional rather than accidental.

## Demo

Dark palette on → press `?` on the AUTH board (dark list, mono keycaps) → Esc →
click Home (dark cards, jade AUTH key) → press `c` (dark dialog, dark scrim) →
Esc → Sign out (dark sign-in page, no toggle on it).

## Done when

US-CTS-02's five scenarios are green, `grep` finds no colour literal outside the
token block anywhere in the file, and `git diff --stat crates/foundry-acceptance/`
shows additions only.
