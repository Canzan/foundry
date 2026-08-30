# Intake — canzan-theme-system

**Wizard**: `/nw:new` · 2026-08-29 · **Starting wave**: DISCUSS
**Classification**: User-facing (UI/UX) · **Context**: Brownfield

## Request

> Look at https://canzan.net/ and apply the themes to the style
> `crates/foundry-app/static/css`

## Decisions taken at intake (user-confirmed)

| # | Decision | Value |
|---|---|---|
| D1 | Theme scope | Light + dark + **three-state toggle** (system/light/dark) |
| D2 | Toggle mechanism | Port `canzan-lift/src/ui/assets/theme.js` **exactly**, so both apps can later share one module |
| D3 | Fonts | **Self-host woff2**, hashed, registered in `VENDOR.md`. No Google Fonts CDN. |
| D4 | Palette | Adopt canzan.net `--cz-*` tokens in foundry now; **record a follow-up to migrate canzan-lift onto the same names** |
| D5 | Pipeline | Full nWave, starting at DISCUSS |

## Reference material gathered

`canzan-net-reference.css` (43 KB, sha256 `44ad42b5…`) is the verbatim
production stylesheet fetched from canzan.net at intake. It is the source of
truth for the token values below.

### Token contract (canzan.net `--cz-*`)

| Token | Light | Dark |
|---|---|---|
| `--cz-bg` | `#fbfbf9` | `#0a0c0b` |
| `--cz-bg-2` | `#f3f4f1` | `#0f1312` |
| `--cz-surface` | `#ffffff` | `#131817` |
| `--cz-line` | `#e3e5e0` | `#1f2523` |
| `--cz-line-strong` | `#cdd1cb` | `#2e3733` |
| `--cz-text` | `#121614` | `#e8ebe8` |
| `--cz-muted` | `#5c645f` | `#8d958f` |
| `--cz-faint` | `#878e89` | `#626a66` |
| `--cz-jade` (accent) | `#1a7a5e` | `#62c9a6` |
| `--cz-jade-soft` | `rgba(26,122,94,.10)` | `rgba(98,201,166,.11)` |
| `--cz-jade-line` | `rgba(26,122,94,.32)` | `rgba(98,201,166,.34)` |

Also: `--radius: 6px`, `--cz-gutter: clamp(20px,5vw,40px)`,
`--cz-shadow` (two-layer, re-bound in dark).

### Typography

- `--cz-display`: **Bricolage Grotesque** → Public Sans → system
- `--cz-body`: **Public Sans** → system stack
- `--cz-mono`: **JetBrains Mono** → ui-monospace
- Eyebrow/label idiom: mono, `.6875rem`, `letter-spacing:.18em`, uppercase, `--cz-faint`

### Components

- `.cz-btn`: inline-flex, `.9375rem`/600, `.78rem 1.15rem`, radius 6, `translateY(-1px)` on hover
- `.cz-btn--primary`: ink fill → **jade** on hover
- `.cz-btn--ghost`: hairline `--cz-line-strong` → jade border + jade text on hover

### Theming mechanism (from canzan-lift)

Three states, because a two-state toggle can never hand the choice back to the
device:

```
system   no data-theme attribute; prefers-color-scheme decides (default)
light    data-theme="light" on <html>; media query overruled
dark     data-theme="dark"  on <html>; media query overruled
```

CSS must be authored as **two** dark blocks — they cannot be merged, since a
media query and an attribute selector cannot express "either":

```css
@media (prefers-color-scheme: dark) { :root:not([data-theme="light"]) { … } }
:root[data-theme="dark"] { … }
```

The script is loaded from `<head>` as a plain render-blocking `<script>` so the
attribute lands **before first paint** — otherwise a dark-preferring user gets a
white flash on every navigation. `localStorage` access is guarded in try/catch
throughout; losing site-data permission costs persistence, never the control.
The button is built in JS (not server-rendered) so it simply does not exist when
scripts are off, rather than existing as a dead control.

## Open questions for DISCUSS

1. **Mount point.** canzan-lift appends the button to `nav.top-nav`; foundry's
   chrome is `.site-header`. Confirm the selector and where in the header the
   control belongs.
2. **Storage key.** canzan-lift uses `canzan-lift.theme`. Foundry needs its own
   (`foundry.theme`?) — but a shared module will need this parameterised.
3. **No-JS obligation.** Does foundry carry canzan-lift's D5 ("every screen and
   every mutating control must work with JavaScript disabled")? Foundry already
   has a no-JS test lane; the toggle must degrade to OS-follows.
4. **Render-contract risk.** `foundry.css` header states the semantic classes
   (`.column`, `.issue-card`, `.comment`) double as the contract the acceptance
   suite selects on. The restyle must not rename or remove them.
5. **Header comment is now false.** The file says "NOT a design system, theming,
   or dark mode." All three become untrue — rewrite it.
6. **a11y re-verification.** NFR-WEBB-A11Y-02 requires ≥4.5:1 (3:1 large). Every
   pair must be re-measured **in both palettes**; canzan-lift documents its dark
   contrast ratios inline and foundry should do the same.
7. **Asset re-hash.** `foundry.8ce38566.css` is content-hashed and referenced by
   templates + `VENDOR.md`. Fonts and `theme.js` add new hashed assets. There is
   a known-stale-hash failure mode in this repo's history — follow the hardened
   re-hash procedure in `VENDOR.md`.
8. **Follow-up (D4).** Migrating canzan-lift onto `--cz-*` is out of scope here
   but must be recorded so "one theme everywhere" does not silently lapse.
