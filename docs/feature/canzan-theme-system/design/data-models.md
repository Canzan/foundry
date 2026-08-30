# Data Models — canzan-theme-system

Wave: DESIGN | Agent: nw-solution-architect (Morgan) | Date: 2026-08-29

## 0. Database: no change

**No migration. No table, column, index or constraint is added, altered or
dropped.** Theme state is device-local `localStorage`; nothing is persisted
server-side and no server-side per-user preference is introduced (feature-delta
§ Out of Scope). The migration counter is untouched by this feature.

The "data models" this feature owns are four *contracts* instead: the token
contract, the theme-state vocabulary, the `VENDOR.md` row schemas, and the asset
filename grammar. Each is a shared artifact with named consumers and a named
enforcer.

---

## 1. Token contract

Source of truth: `canzan-net-reference.css` (pinned, sha256 `44ad42b5…`),
transcribed into the `:root` block. **11 colour tokens** + radius, gutter, shadow
+ 3 type roles. Consumers: every rule in `foundry.<hash>.css`. Enforcer: **S1**
(no colour beneath the seam) and **S2** (identical name sets across the three
regions).

| Token | Light | Dark | Note |
|---|---|---|---|
| `--cz-bg` | `#fbfbf9` | `#0a0c0b` | page |
| `--cz-bg-2` | `#f3f4f1` | `#0f1312` | recessed |
| `--cz-surface` | `#ffffff` | `#131817` | cards, dialogs, overlay |
| `--cz-line` | `#e3e5e0` | `#1f2523` | hairline |
| `--cz-line-strong` | `#cdd1cb` | `#2e3733` | emphasised border |
| `--cz-text` | `#121614` | `#e8ebe8` | tier 1 — **17.62:1** / **16.32:1** |
| `--cz-muted` | `#5c645f` | `#8d958f` | tier 2 — **5.89:1** / **6.38:1** |
| `--cz-faint` | **`#6e756f`** | **`#78807b`** | tier 3 — **4.57:1** / **4.83:1** — **REBOUND, see D-04** |
| `--cz-jade` | `#1a7a5e` | `#62c9a6` | the one accent; replaces three hues |
| `--cz-jade-soft` | `rgba(26,122,94,.10)` | `rgba(98,201,166,.11)` | translucent — never sole-carries text (D-05) |
| `--cz-jade-line` | `rgba(26,122,94,.32)` | `rgba(98,201,166,.34)` | translucent — ditto |
| `--cz-shadow` | `0 1px 2px rgba(18,22,20,.04), 0 8px 24px rgba(18,22,20,.05)` | `0 1px 2px rgba(0,0,0,.4), 0 12px 32px rgba(0,0,0,.32)` | two-layer; dark is deeper and wider deliberately — a shadow that reads on paper disappears on ink |
| `--radius` | `6px` | — | already matched; kept |
| `--cz-gutter` | `clamp(20px,5vw,40px)` | — | layout |

**The one deliberate divergence from the reference.** canzan.net's `--cz-faint`
is `#878e89` light (**3.24:1**) and `#626a66` dark (**3.52:1**) — both clear 3:1
for large text and non-text, both **fail 4.5:1** at the ~11 px label size
canzan.net's own eyebrow idiom uses. foundry carries NFR-WEBB-A11Y-02 and
canzan.net does not, so foundry **moves the value, not the structure**: the token
name, its tier and its role in the shared contract are unchanged, and the
three-tier hierarchy stays visibly separated in both palettes. Collapsing labels
onto `--cz-muted` was rejected — it passes, but it deletes a tier from the
contract and would make foundry structurally different from canzan.net, which is
a far worse thing to carry into a future unification than a corrected hex value.

**All six tier ratios are recorded as inline comments beside the tokens**, in
both palettes (D-04), so the next reader can check the arithmetic rather than
trust it. This is why S1's scanner must strip block comments before matching.

### Extending the contract — the anticipated 12th token

S01's learning hypothesis is that **11 tokens are not enough**: canzan.net is a
marketing site with no selected navigation item, no keyboard selection ring and no
hover state, so it never needed an *opaque tint*. Its only tinted surface is
`--cz-jade-soft`, which is translucent and therefore barred by D-05 from carrying
text alone. `.sidebar__item--active` is exactly that case and is the named
likeliest falsifier.

This design does not pre-empt the hypothesis — it governs the outcome:

- **If a 12th token is needed, it is a proposed extension to the canzan contract,
  not a foundry local.** It takes the `--cz-` prefix, is named for its role rather
  than its use site (`--cz-jade-tint`, not `--cz-sidebar-active-bg`), is bound in
  **all three regions**, and is recorded in the feature's DELIVER notes as a
  contract extension canzan-lift's eventual migration inherits. A foundry-local
  token would divide the contract at exactly the point unification matters
  (intake D4).
- **S2 makes the "all three regions" part structural rather than remembered**: a
  token added to `:root` and one dark block reds the build. This is the token
  contract's own extension mechanism being protected by the same check that
  protects its steady state.
- **What must not happen** is a literal. The tempting shortcut for one active nav
  item is a hardcoded opaque hex; S1 forbids it, which is the point.

### Type roles

| Token | Stack | Weights shipped |
|---|---|---|
| `--cz-display` | `"Bricolage Grotesque", "Public Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` | **600, 700** |
| `--cz-body` | `"Public Sans", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif` | **400–700** |
| `--cz-mono` | `"JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace` | **400, 500** |

Requesting a weight outside these ranges yields a silently synthesised face
(ADR-001). Three ad-hoc stacks are retired: `--font` (`:23`), `.dash`'s duplicate
(`:209`), and the two bare `ui-monospace` uses (`:270`, `:555`). `.card__key`'s
`font: 600 …` moves to 500 — canzan.net's own mono weight.

**Eyebrow/label idiom** (`.column h2`, `.dash__section h2`): mono, `.6875rem`,
`letter-spacing: .18em`, uppercase, `--cz-faint` — exactly as canzan.net writes
it, at the rebound value.

---

## 2. Theme-state vocabulary

A three-value closed set. Source of truth: `theme.js`'s `ORDER` constant.

| State | `data-theme` on `<html>` | Resolved by |
|---|---|---|
| `system` | **attribute absent** | `@media (prefers-color-scheme: dark)` + the `:not([data-theme="light"])` guard |
| `light` | `data-theme="light"` | `:root` defaults; the media block is excluded by the `:not()` guard |
| `dark` | `data-theme="dark"` | `:root[data-theme="dark"]` |

Cycle: `system → light → dark → system`. **"System" removes the attribute rather
than writing a third value.** This is not an implementation detail — absence is
what hands the decision back to the device, and it is one mechanism written in
two files: the CSS `:not()` guard and the JS `apply()`. S2 protects the CSS half;
the two-line-diff port constraint protects the JS half.

Storage key: `foundry.theme` (canzan-lift uses `canzan-lift.theme`; a shared
module must parameterise it). Every access try/catch-guarded — a storage failure
costs persistence, never the control, and surfaces nothing.

**Vocabulary risk**: a fourth state, or writing `"system"` as a value, breaks the
stylesheet silently. The set is closed by design and by the `ORDER.indexOf(value)
=== -1 → "system"` normalisation in the ported script.

---

## 3. `VENDOR.md` row schemas — three shapes

`VENDOR.md` currently makes one promise stated in terms of byte-identity with an
upstream release (`:5-12`). That promise cannot hold for a derived asset.
ADR-002 adds a third shape and requires the document's preamble to state that
three shapes now exist with three different audit procedures.

| Shape | Used by | Records | Audit |
|---|---|---|---|
| **Vendored-verbatim** | `vendor/htmx.min.js` | version, upstream URL, retrieval date, sha256 | re-download from upstream, re-hash, compare |
| **Authored-in-tree** | `css/foundry.<hash>.css`, **`js/theme.js`** (new) | "hand-authored (this repo)", sha256 | re-hash, compare; for the CSS the hash is also the filename |
| **Derived** (NEW) | the three woff2 blobs | 7 fields, below | three-step, with a documented fallback |

### Derived row — 7 fields, in a per-blob block beneath the table

A recipe does not fit a table cell, and pretending it does is how recipes drift
from reality.

| # | Field | Why it is load-bearing |
|---|---|---|
| 1 | Family + licence + in-tree path of the OFL text | OFL-1.1 clause 2 |
| 2 | **Pinned input** — authoritative repo at an **immutable ref** | tag where one exists; **commit sha where none does** — Bricolage has no releases. Never a branch. Never the `google/fonts` mirror |
| 3 | **sha256 of the input** as downloaded | proves the audit started from the real upstream |
| 4 | **Tool + exact versions** (`fonttools 4.63.0`, `brotli`, `python 3.14.7`) | woff2 output is compressor-dependent; a recipe without a pinned compressor is not a recipe |
| 5 | **Exact command line**, verbatim, every flag, the full unicode-range | — |
| 6 | **sha256 of the intermediate** instanced+subset TTF, *before* woff2 flavouring | **the stable anchor.** Brotli-independent, so it is the step that actually proves font content |
| 7 | **sha256 of the committed output** woff2 | machine-checked by R3; its first 8 hex are the filename |

### The two claims, separated

> **Tier 1 — Integrity. Unconditional, machine-checked, always true.** The
> committed blob's sha256 equals field 7. Answers *"is the file this binary
> serves the file that was reviewed?"* Enforced by **R3**.
>
> **Tier 2 — Provenance. Conditional, human-run, best-effort.** Re-deriving from
> field 2 with field 4 *should* reproduce the blob. **Byte-identical
> re-derivation is expected but NOT guaranteed** across brotli builds, platforms
> and fonttools patch releases. We do not claim it.

Audit procedure: (1) fetch input, verify field 3 — *if this fails, stop*;
(2) instance and subset **without** `--flavor`, compare field 6 — a match proves
the font content is exactly what was recorded; (3) flavour to woff2, compare
field 7 — a mismatch *after step 2 passed* is a compressor difference, not a
provenance failure, and is recorded as such.

**The reproducibility assumption is itself probed.** DELIVER re-runs
`derive-fonts.sh` on a **second machine** and records in `VENDOR.md`, as measured
fact, whether steps 2 and 3 reproduced byte-for-byte. If step 2 varies, the
intermediate is not the stable anchor this model assumes and the model is
revisited.

---

## 4. Asset filename grammar

```
<stem>.<8 lowercase hex>.<ext>        content-hashed  →  Cache-Control: immutable
<stem>.<ext>                          unhashed        →  Cache-Control: no-cache (for /static/js/)
```

The 8 hex are the first 8 of the file's **own** sha256 — "the hash IS the cache
key" (assets.md Decision #4a, ADR-B03). **R2 recomputes it**, so the grammar is
enforced rather than trusted: a file edited without being renamed now reds CI
instead of being pinned stale in every browser for a year at an immutable URL.

| Asset | Name | Policy |
|---|---|---|
| stylesheet | `css/foundry.<8hex>.css` | immutable |
| fonts | `fonts/<family>.<8hex>.woff2` | immutable — **required**: a derived blob has no upstream version to pin, so the content hash is its only honest cache key |
| `theme.js` | `js/theme.js` | `no-cache`, matching the four existing IIFEs. Render-blocking, so this costs one conditional GET before paint — a latency cost, not a flash risk (architecture-design.md §3.1) |
| OFL texts | `fonts/OFL-<family>.txt` | never requested by a browser; not in the KPI 7 cold-load measurement |

### Hash reference sites — the model R1+R2 replaces

Five hand-maintained sites per re-hash, three re-hashes across four slices:

| # | Site | Caught today by | Caught after |
|---|---|---|---|
| 1 | the committed filename | — | R2 |
| 2 | `templates/base.html:6` | `feature_pwa_mobile.rs:266-306` (same-hash only) | R1 + R2 |
| 3 | `VENDOR.md` row | nothing | R3 |
| 4–6 | `lib.rs:329, 346, 365` | the cache-policy tests catch a *wrong* hash, not a *forgotten* one | R1 |

The existing partial guard compares `base.html` and `lib.rs` to each other and
**never stats the file**, so both can name the same wrong hash and pass. R1 is
the first check that touches the disk; R2 is the first that touches the bytes.
