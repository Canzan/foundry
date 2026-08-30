# Vendored static assets — provenance & integrity

Pure pre-vendored blobs (DB6 / `docs/feature/htmx-web-tier/design/assets.md`):
**NO Node, NO bundler, NO package.json, NO minifier step, NO CDN at runtime.**
Every blob below is served by the binary via `tower_http::services::ServeDir`
(`.nest_service("/static", ...)` in `foundry-app/src/lib.rs`).

## THREE ROW SHAPES, THREE AUDIT PROCEDURES

This file used to make ONE promise — byte-identity with an upstream release —
and as of 04-01 it can no longer make that promise for every row. Rather than
let it become silently false for three assets, the shapes are named. **Check
which shape a row is before auditing it; running the wrong procedure will look
like tampering.**

| Shape | Rows | What "verify" means |
|---|---|---|
| **Upstream-verbatim** | `vendor/htmx.min.js` | Re-download the pinned release, re-hash, compare. The blob IS the upstream artifact. |
| **Authored-in-tree** | `css/foundry.<hash>.css`, `js/theme.js` | Re-hash and compare with the value recorded in the table. There is no upstream release to re-download. (For the CSS the hash is ALSO the filename; `theme.js` is unhashed, so the table row is the only record.) |
| **Derived** (NEW, 04-01) | the three `fonts/*.woff2` | Two SEPARATE claims of different strength — see § Derived assets below. The blob is derived from a named upstream and is **not byte-identical to it**; re-hashing against an upstream digest fails BY DESIGN. |

An auditor (or an air-gapped operator) can verify any row's committed bytes with
`shasum -a 256` against the value recorded here — that claim is unconditional
for all three shapes and is machine-checked by `cargo xtask check-arch` (R3).
What differs between shapes is the *provenance* question: "what IS this file?"

Updating an asset = pin the new release, drop it in, update this file + the
hash, re-run the acceptance suite. For a derived asset, re-run the recipe.

| File | Version | Upstream canonical URL | Retrieved (UTC) | sha256 |
|------|---------|------------------------|-----------------|--------|
| `vendor/htmx.min.js` | htmx **2.0.4** (pinned latest-stable 2.0.x; step 04-01 migration) | https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js | 2026-06-04 | `e209dda5c8235479f3166defc7750e1dbcd5a5c1808b7792fc2e6733768fb447` |
| `css/foundry.6296815a.css` | hand-authored (this repo) | — (not vendored; authored in-tree) | 2026-08-30 | `6296815a6b668801290e36c8e90452a6c39b19d2e1dc66c0a2122d6b7f128445` |
| `js/theme.js` | hand-authored (this repo) — a two-value PORT of canzan-lift `src/ui/assets/theme.js`, NOT vendored from a release | — (not vendored; authored in-tree) | 2026-08-29 | `95a7ffdd0f97d75332fc988dc3a87d53ea758131eff31de48bde6000043e8813` |
| `fonts/bricolage-grotesque.3bd3b180.woff2` | **derived** — `ateliertriay/bricolage@84745e5b` | github.com/ateliertriay/bricolage (OFL-1.1) | 2026-08-29 | `3bd3b180978a3c167fe394da73b34a5dca88b1107d8cb426b7e544a924e2a597` |
| `fonts/public-sans.a2bd64e2.woff2` | **derived** — `uswds/public-sans@v2.001` | github.com/uswds/public-sans (OFL-1.1) | 2026-08-29 | `a2bd64e2c7420ec38a2c794be957e51347858940fd6f652c2fad7212c4caa7a2` |
| `fonts/jetbrains-mono.4e194fb3.woff2` | **derived** — `JetBrains/JetBrainsMono@v2.304` | github.com/JetBrains/JetBrainsMono (OFL-1.1) | 2026-08-29 | `4e194fb3b563af1df4eac36952711cd30ad491e2c6122df81a370f6fc5d6266f` |
| `fonts/OFL-bricolage-grotesque.txt` | licence text, upstream-verbatim | github.com/ateliertriay/bricolage `OFL.txt` | 2026-08-29 | `4b5a7d8f37f5602621c8a8d7358a6a2e71317e6c231c661e15aef0275d3e07ba` |
| `fonts/OFL-public-sans.txt` | licence text, upstream-verbatim | github.com/uswds/public-sans `LICENSE.md` | 2026-08-29 | `82f0d3cad45f264192db156360b4a710fe7060885f6aa261e6539f13cb9eb0d9` |
| `fonts/OFL-jetbrains-mono.txt` | licence text, upstream-verbatim | github.com/JetBrains/JetBrainsMono `OFL.txt` | 2026-08-29 | `30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad` |

## Notes

- **htmx 2.0.4** is the pinned latest-stable 2.0.x release (step 04-01,
  `design/htmx2-migration.md` DD7). The blob is the core, no-extension htmx 2
  build: it opens `var htmx=function(){...}` (htmx 2 dropped the htmx-1 UMD
  `define.amd` shim — `grep -c define.amd` is 0) and records `version:"2.0.4"`
  near the top. Foundry uses only core directives
  (`hx-get`/`hx-post`/`hx-patch`/`hx-delete`/`hx-target`/`hx-swap`/`hx-swap-oob`),
  no `hx-on`, no extensions — exactly the directive set htmx 2 preserves, so the
  bump is API-compatible for Foundry's usage. The `data-*` render-contract
  markers (`data-column`/`data-issue-key`/`data-comment-list`/`data-comment-id`/
  `data-hx-fragment`) are passive scraper hooks, NOT htmx directives, and are
  left byte-unchanged.
- **Alpine.js was retired** (keyboard-shortcut-bindings, ADR-001) and its blob
  deleted. It was vendored for a keyboard layer that was never written: no
  template ever carried an Alpine directive (`x-data` / `x-on:` / `x-model` /
  `x-show` / `x-init` / `@click`), so the framework was parsed and executed on
  every page load to do nothing. The client keyboard layer that replaced the
  intent is `static/js/keyboard.js` — one app-owned vanilla IIFE, no framework.
  htmx remains the only vendored runtime dependency.
- `foundry.6296815a.css` is hand-authored and served as-authored (gzip via the
  `compression-gzip` tower-http feature handles the wire size); no CSS minifier
  is introduced (DB6). **The `.6296815a.` segment is the content hash** (first 8
  hex of the file's sha256) per ADR-B03 / assets.md Decision #4 option 4a: the
  hash IS the cache key, so the blanket `Cache-Control: ...immutable` on `/static`
  is safe even though the file is hand-edited — an edit changes the hash, changes
  the committed filename, changes the URL `base.html` references, and misses stale
  caches correctly. To update the CSS: edit it, recompute
  `shasum -a 256 css/foundry.<old>.css`, rename the file to the new 8-hex prefix,
  then in the SAME commit update the `<link>` in `templates/base.html`, the row
  above, and the hashed-name literals in the `foundry-app` cache-policy tests
  (`src/lib.rs`) — a split commit is red on those tests. The acceptance suite
  discovers the hashed name on disk, so it does not pin the literal.
- **`js/theme.js` is AUTHORED-IN-TREE, not vendored and not derived** — do not
  run the upstream-verbatim procedure on it, there is no release to re-download.
  It is a PORT of canzan-lift's `src/ui/assets/theme.js` in which **exactly two
  values differ**: `STORAGE_KEY` (`"foundry.theme"` rather than
  `"canzan-lift.theme"`) and the mount selector (`.sidebar__user`, foundry's
  vertical rail, rather than `nav.top-nav`, canzan-lift's horizontal strip).
  Nothing else — not a comment, not a space. That two-line identity is the whole
  basis of the future shared module (canzan-theme-system D-06): a third
  divergence and the two files can never be merged into one parameterised
  module. **Verify it, do not assume it**, with
  `diff canzan-lift/src/ui/assets/theme.js crates/foundry-app/static/js/theme.js`
  — the expected output is two changed lines and nothing else. A change to this
  file that is not also made in canzan-lift is a review-blocking divergence.
- `theme.js` stays **UNHASHED** under `/static/js/`, like the four other
  app-owned IIFEs, and therefore takes `no-cache` (revalidate) rather than the
  immutable policy the hashed CSS gets. That costs ONE conditional GET before
  first paint — a latency cost, never a flash risk, because the tag is
  render-blocking and the browser blocks rather than paints. It is also the only
  head script in `base.html` with no `defer`/`async`/`type="module"`; see the
  comment above the tag, and the acceptance scenario that asserts its shape.
- Re-verify a hash with: `shasum -a 256 crates/foundry-app/static/vendor/<file>`.

## Derived assets — the three canzan typefaces (ADR-CANZAN-THEME-001 / -002)

The three woff2 blobs are **derived**: each is the upstream variable font with
its axes instanced down to what foundry actually uses and its charset subset to
latin. That is why they are 76,416 B instead of ~210 KB, and it is why **no
upstream-published sha256 will ever match them**. A derived asset's provenance
is its RECIPE, so the recipe is recorded here in full — and it is executable:
`tools/fonts/derive-fonts.sh`, with `tools/fonts/requirements.txt` pinning the
toolchain. It lives outside the cargo workspace and outside `static/` (a served
directory must not hold a shell script) and is **NOT a build step**: no
`cargo build`, no `xtask ci`, no CI job invokes it. It runs when a maintainer
adds or bumps a font (`VENDOR.md:3-4`, assets.md DB6).

### The two claims, and only one of them is a promise

> **Tier 1 — Integrity. Unconditional, machine-checked, always true.**
> The committed blob's sha256 equals item 7 below. This answers *"is the file
> this binary serves the file that was reviewed?"* — the claim that protects the
> operator. `cargo xtask check-arch` R3 enforces it, so a row that stops being
> true reds CI. It needs only `shasum`, which is what an air-gapped operator has.
>
> **Tier 2 — Provenance. Conditional, human-run, best-effort.**
> Re-deriving from the pinned input reproduces the blob. This answers *"is this
> really Bricolage Grotesque, and only that?"* Byte-identical re-derivation is
> **expected but NOT guaranteed** across brotli builds, platforms and fonttools
> patch releases. We do not promise it. Measured below: on the two environments
> tried, it held completely.

**The three-step audit.** Run `tools/fonts/derive-fonts.sh` and compare:

1. **Input** — sha256 of the file downloaded from the pinned ref, against item
   3. *If this fails, stop: the upstream moved or the record is wrong.*
2. **Intermediate** — instanced + subset TTF, **without** `--flavor=woff2`,
   against item 6. Brotli-independent, and therefore the STABLE ANCHOR: a match
   here proves the font CONTENT is exactly what was recorded — same glyphs, same
   axes, same tables.
3. **Output** — flavoured to woff2, against item 7. A match is full byte-level
   provenance. **A mismatch here after step 2 passed is a compressor difference,
   not a provenance failure**, and should be recorded as such rather than
   treated as tampering.

### Reproducibility: MEASURED, not assumed (04-01)

The recipe was run in two materially different environments — macOS 15 / arm64
(host) and a `python:3.14` Debian container with its own locally-compiled
brotli, the compiled compressor and libc being the variables under test.
Both ran fonttools 4.63.0, Brotli 1.1.0, Python 3.14.7.

**Steps 2 AND 3 reproduced byte-for-byte, for all three families.** Every
anchor and every output hash below is identical on both. ADR-002 predicted step
3 might legitimately vary and declined to promise it; on these two environments
it did not vary. That is recorded as the measurement it is — two environments
are not a proof of universal reproducibility, and the Tier 2 wording above
stands unchanged.

**Two flags make that true, and removing either silently breaks the audit while
every test stays green.** Both were found by measurement at 04-01, after the
first probe FAILED:

- **`SOURCE_DATE_EPOCH=1787961600`** — `varLib.instancer` stamps `head.modified`
  from the wall clock. Without the pin the anchor differed between two runs *on
  one machine, seconds apart*: 5 bytes, `head.modified` plus the derived
  `checkSumAdjustment`, every other table byte-identical.
- **`--no-optimize`** — the IUP optimiser makes a float-tolerance choice about
  which `gvar` deltas to store explicitly and which to leave interpolated. On
  JetBrains Mono it chose differently on the two platforms for 4 of 414 glyphs,
  so the anchor diverged (`2c13f344…` vs `b608c69a…`) while the FONT was
  identical — instancing either encoding at wght 400 and 500 gives byte-equal
  fonts with zero differing outlines. With the flag: `446cb992…` on both.

An anchor that changes when nothing changed proves nothing and trains an auditor
to shrug at step 2, which is the failure mode ADR-002 § alternative C exists to
refuse. The cost of `--no-optimize` is ~200 B of unoptimised `gvar` on the
shipped blob. **Do not remove either flag to reclaim it.**

`varLib.instancer` also emits a benign `OTLOffsetOverflowError` warning on range
instancing and repairs it internally; the outputs are valid woff2. Recorded so
it is not mistaken for a failure.

### Bricolage Grotesque — display

1. **Family / licence** — Bricolage Grotesque, **OFL-1.1**, no Reserved Font
   Name; text shipped at `fonts/OFL-bricolage-grotesque.txt` (clause 2).
2. **Pinned input** — `ateliertriay/bricolage@84745e5b96261ae5f8c6c856e262fe78d1d6efdd`
   (2023-07-19). A commit sha, not a tag: the repo is dormant and has **no
   releases and no tags**. Never a branch, never the `google/fonts` mirror.
   Path: `fonts/variable/BricolageGrotesque[opsz,wdth,wght].ttf`, 408,496 B.
3. **Input sha256** — `413e7357809ddd12fd80a96a8a396de0e401638d4acd3cb3e37532f0472ac682`
4. **Tools** — `fonttools 4.63.0`, `brotli 1.1.0`, `python 3.14.7`,
   `SOURCE_DATE_EPOCH=1787961600`.
5. **Command** — `fonttools varLib.instancer <input> opsz=24 wdth=100 wght=600:700 --no-optimize --no-recalc-timestamp -o <instanced>`, then
   `pyftsubset <instanced> --unicodes="U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD" --layout-features="kern,liga,clig,calt" --output-file=<subset>`, then
   `pyftsubset <subset> --unicodes="*" --layout-features="*" --flavor=woff2 --output-file=<output>`
6. **Intermediate sha256 (THE ANCHOR)** — `f2f8d04b0aff66a40f2e3e3c35d3a77e47242948443d9ffba178900f6115fb2e`
7. **Output sha256** — `3bd3b180978a3c167fe394da73b34a5dca88b1107d8cb426b7e544a924e2a597`,
   **29,788 B**, committed as `fonts/bricolage-grotesque.3bd3b180.woff2`.

**Measured, so the next reader need not re-derive the question.** ADR-001
specified `opsz=24` but had only measured `opsz=14`, and required DELIVER to
re-measure against a **32,768 B ceiling**. The result is **29,788 B — the
ceiling holds with 2,980 B of headroom, and NO rung of the pre-authorised
fallback ladder (`opsz=20`, then `opsz=14`) was taken.** The extrapolation held:
pinning `opsz` removes the same delta data at either value, and Bricolage's
`opsz` axis carries no size-specific glyph data of consequence.

### Public Sans — body

1. **Family / licence** — Public Sans, **OFL-1.1**, no Reserved Font Name; text
   at `fonts/OFL-public-sans.txt`.
2. **Pinned input** — `uswds/public-sans@v2.001` (2022-05-11), path
   `fonts/variable/PublicSans[wght].ttf`.
3. **Input sha256** — `d75a7dc1a27eb9e336d5b33f55489d2ecb5621bf694d5c43b2415bce2ca830a8`
4. **Tools** — as above.
5. **Command** — as above with axes `wght=400:700`; same subset and flavour
   commands verbatim.
6. **Intermediate sha256 (THE ANCHOR)** — `2eea0d13f535597fa3969043a0cf4c1b3130d94c5895895dd23a3dbc9f63b7ec`
7. **Output sha256** — `a2bd64e2c7420ec38a2c794be957e51347858940fd6f652c2fad7212c4caa7a2`,
   **20,672 B**, committed as `fonts/public-sans.a2bd64e2.woff2`.

**Roman only.** Upstream ships upright and italic as separate files; foundry has
exactly one italic rule (`.empty-state`, a muted empty-state string), which
takes a synthesised oblique. A ~20 KB italic file is not worth one muted line.

### JetBrains Mono — mono

1. **Family / licence** — JetBrains Mono, **OFL-1.1**, no Reserved Font Name;
   text at `fonts/OFL-jetbrains-mono.txt`.
2. **Pinned input** — `JetBrains/JetBrainsMono@v2.304` (2023-01-14), path
   `fonts/variable/JetBrainsMono[wght].ttf`.
3. **Input sha256** — `662a196d58f1183bf2d77428b6d5283fe3f45161ab021bea4036bc98e5cac016`
4. **Tools** — as above.
5. **Command** — as above with axes `wght=400:500`; same subset and flavour
   commands verbatim.
6. **Intermediate sha256 (THE ANCHOR)** — `446cb9929b6ddbc02397b949cbd569328fed822597d7792c85bf03c0a1158f26`
7. **Output sha256** — `4e194fb3b563af1df4eac36952711cd30ad491e2c6122df81a370f6fc5d6266f`,
   **25,956 B**, committed as `fonts/jetbrains-mono.4e194fb3.woff2`.

This is the family whose anchor exposed the IUP non-determinism described above.
It is the reason `--no-optimize` is in the recipe.

### Standing notes on the three

- **Total 76,416 B**, against KPI 7's 150 KB guardrail — 49 % under, with room
  for a fourth blob later. The naive path (the woff2 files Google serves) was
  measured at ~210 KB and would have overrun the guardrail by 40 %.
- **No CDN, ever.** `VENDOR.md:4` prohibits an external font host outright
  rather than discouraging it. An air-gapped operator would experience one as
  missing type; the acceptance suite asserts zero cross-origin requests.
- **Latin-only.** Non-latin content (a Cyrillic workspace name, a CJK title)
  falls back to the system stack for those glyphs. Safe by construction — the
  `font-display: swap` fallback path is the same one that renders before the
  blobs arrive — but silent, so it is written down.
- **Weight discipline binds the stylesheet.** Display ships 600:700, body
  400:700, mono 400:500. A rule requesting a weight outside its range is
  silently SYNTHESISED into a faux-bold. Nothing enforces this yet; ADR-001
  recommends folding a weight-range check into the CSS structural check of
  ADR-CANZAN-THEME-004, where the parser already walks the file.
- **Standing risk.** Bricolage's pin is a commit sha on a dormant repo with no
  release process. If that repo disappears the input is unverifiable and only
  Tier 1 survives. The mitigation if it fires is ADR-001 alternative B (drop the
  display face and use Public Sans for display, which is canzan.net's own
  fallback).
