# Technology Stack — canzan-theme-system

Wave: DESIGN | Agent: nw-solution-architect (Morgan) | Date: 2026-08-29

**Zero new runtime dependencies. Zero new crates in the workspace. Zero
build-step additions.** Everything below is either a committed asset, an existing
house tool, or a maintenance-time script that no build or CI job invokes.

**One dependency edge is added, and it is not a new crate.** R2 and R3
(ADR-CANZAN-THEME-003) recompute sha256, and `xtask` today depends only on
`anyhow` (`xtask/Cargo.toml:16-17`). `sha2 = "0.10"` is **already a workspace
dependency** (`Cargo.toml:63`), so `xtask` gains `sha2 = { workspace = true }` —
a new edge to a crate already in the graph, not a new crate in it. `cargo deny
check bans` is therefore unaffected. The alternative — shelling out to
`shasum`/`sha256sum`, which would match the house idiom that already shells to
`cargo deny` in `check_dependency_direction` — was rejected as
platform-dependent (`sha256sum` on Linux, `shasum -a 256` on macOS) for a guard
that must run identically on a maintainer's laptop and in CI.

## 1. Runtime assets (served to browsers)

| Asset | Version / pin | Licence | Upstream | Bytes |
|---|---|---|---|---|
| Bricolage Grotesque, derived | `ateliertriay/bricolage@84745e5b96261ae5f8c6c856e262fe78d1d6efdd` (2023-07-19 — **no releases, no tags**) | **OFL-1.1**, no Reserved Font Name | github.com/ateliertriay/bricolage | 29,764 B\* |
| Public Sans, derived (roman only) | `uswds/public-sans@v2.001` (2022-05-11) | **OFL-1.1**, no Reserved Font Name | github.com/uswds/public-sans | 20,664 B |
| JetBrains Mono, derived | `JetBrains/JetBrainsMono@v2.304` (2023-01-14) | **OFL-1.1**, no Reserved Font Name | github.com/JetBrains/JetBrainsMono | 27,180 B |
| `theme.js` | authored in-tree, ported from canzan-lift | project licence | — | ~2,600 B |
| **Total added cold-load payload** | | | | **≈80 KB** (guardrail 150 KB) |

\* measured at `opsz=14`; ADR-001 specifies `opsz=24` and requires DELIVER to
re-measure against a 32,768 B ceiling.

**Licence verification.** Each family's OFL text was read directly from the
authoritative upstream, not from a summary. All three copyright lines read
`Copyright <year> The <Family> Project Authors (<repo URL>)` and proceed straight
to the licensing sentence — **no "with Reserved Font Name" clause in any of the
three**. The derived subsets may therefore keep the upstream family names in
`@font-face`. This closes feature-delta Unresolved #1. (Licence-text reading, not
legal advice.) Each family's `OFL.txt` ships as
`crates/foundry-app/static/fonts/OFL-<family>.txt`, satisfying OFL-1.1 clause 2.

**Provenance caveat.** The byte measurements above were produced from sources
fetched from the `google/fonts` mirror at `main` — a moving ref. The pins in the
table are the **authoritative** upstreams at immutable refs. DELIVER re-takes the
input sha256 against those pins (ADR-002). Note also that `upstream_info.md`
files inside `google/fonts` are AI-generated audit notes and are **not** a
provenance source.

**No CDN, ever.** `VENDOR.md:4` states "NO CDN at runtime". Google Fonts is
prohibited, not dispreferred. US-CTS-03 S2 asserts zero cross-origin font
requests.

## 2. Maintenance-time toolchain (never in a build or CI path)

`tools/fonts/derive-fonts.sh` + `tools/fonts/requirements.txt`, outside the cargo
workspace and outside `static/` (a served directory must not hold a shell script).

| Tool | Version pinned | Licence | Why |
|---|---|---|---|
| `fonttools` | 4.63.0 | MIT | `varLib.instancer` pins/narrows axes; `pyftsubset` subsets and flavours to woff2 |
| `brotli` | pinned in `requirements.txt` | MIT | woff2 compression backend — **must** be pinned, because woff2 output is compressor-dependent |
| Python | 3.14.7 | PSF | host for the above |

All OSS, all MIT/PSF, all mature with active maintenance. No proprietary tool is
used anywhere in this feature.

### The recipe (measured, reproducible)

Per family: instance the axes, then subset to latin, then flavour to woff2.
Bricolage is the worked example; the other two differ only in axis arguments.

```sh
python3 -m venv venv && ./venv/bin/pip install -r tools/fonts/requirements.txt

# 1. Fetch the pinned input and record its sha256 (VENDOR.md item 3).
curl -fLo BricolageGrotesque.ttf \
  "https://raw.githubusercontent.com/ateliertriay/bricolage/84745e5b96261ae5f8c6c856e262fe78d1d6efdd/fonts/variable/BricolageGrotesque%5Bopsz%2Cwdth%2Cwght%5D.ttf"

# 2. Pin opsz and wdth; narrow wght to the range the design actually uses.
./venv/bin/fonttools varLib.instancer BricolageGrotesque.ttf \
  opsz=24 wdth=100 wght=600:700 \
  -o bricolage-instanced.ttf

# 3. Subset to the Google "latin" range. NOTE: no --flavor here — this
#    intermediate is brotli-independent and its sha256 is VENDOR.md item 6,
#    the stable anchor of the Tier-2 audit.
./venv/bin/pyftsubset bricolage-instanced.ttf \
  --unicodes="U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,U+2074,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD" \
  --layout-features="kern,liga,clig,calt" \
  --output-file=bricolage-subset.ttf

# 4. Flavour to woff2. Output sha256 is VENDOR.md item 7; its first 8 hex
#    become the filename segment (ADR-003 R2).
./venv/bin/pyftsubset bricolage-subset.ttf \
  --unicodes="*" --layout-features="*" --flavor=woff2 \
  --output-file=bricolage-grotesque.<8hex>.woff2
```

Axis arguments for the other two: `wght=400:700` (Public Sans,
`fonts/variable/PublicSans%5Bwght%5D.ttf`), `wght=400:500` (JetBrains Mono,
`fonts/variable/JetBrainsMono%5Bwght%5D.ttf`). Both have a single `wght` axis, so
no pinning is needed.

`varLib.instancer` emits a benign `OTLOffsetOverflowError` warning on range
instancing and repairs it internally; the outputs are valid woff2. Recorded so it
is not mistaken for a failure.

**Upstream path caveat.** The exact `fonts/variable/…` paths in the authoritative
repos are UNVERIFIED — the measurement used the `google/fonts` mirror layout.
DELIVER confirms each URL resolves before recording it.

## 3. Existing stack, reused unchanged

| Component | Reused for | Change |
|---|---|---|
| `tower_http::services::ServeDir` at `/static` (`lib.rs:407-411`) | Serving the font blobs and `theme.js` | none |
| `static_cache_control` middleware (`lib.rs:300-310`) | Fonts get `immutable` via their content-hashed names; `theme.js` gets `no-cache` like the four existing IIFEs | none — §3.1 of architecture-design.md records the consequence |
| `compression-gzip` (tower-http) | The stylesheet's wire size | none. woff2 is already brotli-compressed; re-gzipping it gains nothing |
| Askama templates | `base.html` head, `partials/sidebar.html` mount | 3 edits |
| `cargo xtask check-arch` (`check_arch.rs`, 8 rules) | Host for 5 new rules | +2 functions, +1 dependency edge (`sha2`, already in the workspace) |
| Vanilla-IIFE idiom (`board-dnd.js`, `csrf-upload.js`, `form-errors.js`, `keyboard.js`) | `theme.js` matches it exactly | +1 file |
| fantoccini `@needs-browser` lane + `new_session_without_scripting` | Palette, computed style, toggle, scripting-disabled | none |

**Rejected, permanently:** any CSS preprocessor, bundler, minifier, PostCSS,
Node, `package.json`, or CSS-in-JS. `VENDOR.md:3-4` and assets.md DB6 forbid them
by standing decision, and this feature does not reopen it. The consequence is
recorded honestly in ADR-004: `stylelint` is the right tool for the token-seam
check in any other project, and the constraint — not the tool's quality —
decided against it.

## 4. Browser platform features relied on

| Feature | Baseline | Fallback if absent |
|---|---|---|
| CSS custom properties | universal in the htmx-2 baseline | none needed |
| `prefers-color-scheme` | universal | light palette (the `:root` defaults) |
| `color-scheme` | broad | native widgets stay light; text remains legible via explicit tokens |
| `font-display: swap` | broad | text paints in the fallback stack regardless |
| woff2 | universal in the htmx-2 baseline | none shipped — woff2-only is sufficient (System Constraints) |
| `localStorage` | universal, **may throw** | try/catch → session-only mode, silently (D-06) |
| `<meta name="theme-color" media=...>` | partial | browsers ignoring `media` take the first match — which is why `querySelector`-based S10 stays green |

## 5. Open-source posture

Every technology selected is OSS: OFL-1.1 fonts, MIT tooling, and existing
MIT/Apache Rust crates. **No proprietary component is introduced anywhere.** The
one place a commercial option exists — a hosted font CDN — is prohibited by
`VENDOR.md:4` and would also fail US-CTS-03 S2.
