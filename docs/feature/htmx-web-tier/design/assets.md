# htmx Web Tier (Feature B) — Static Asset Pipeline (Pure Vendored Blobs)

Owner: solution-architect (Morgan). Covers Open Decision #4 (cache-busting without a build step).
Interaction mode: **Propose**. Companion: `architecture.md`, `htmx2-migration.md`, `wave-decisions.md`
(ADR-B03/ADR-B04). Hard constraint from DB6 + `out-of-scope.md`: **PURE PRE-VENDORED BLOBS — NO Node,
NO bundler, NO package.json, NO minifier step, NO CDN, at runtime OR build time.** Asset update = a
manual pinned-blob swap.

## Serving mechanism — `tower_http::services::ServeDir` (zero new dependency)

`tower-http` is **already a workspace dependency with the `fs` feature enabled**
(`Cargo.toml:35`: `features = ["trace", "compression-gzip", "fs", "limit", "request-id", "util"]`).
So `ServeDir` is available with **no new dependency** — the cleanest possible "served by the binary"
story.

Wiring in `build_router` (`lib.rs:166`), mirroring the existing `attachment_routes` sub-router
pattern (`lib.rs:175`):

```text
use tower_http::services::ServeDir;
let router = Router::new()
    .nest_service("/static", ServeDir::new("static"))   // serves crates/foundry-app/static/ only
    .merge(attachment_routes)
    ...
```

- **Path-traversal safe by construction** — `ServeDir` refuses `/static/../secret` and serves only
  files resolved under `static/` (satisfies US-B06 scenario 2 and `architecture.md` security note).
- **Mounted OUTSIDE the CSRF/session layers is fine** — `/static` is GET-only static content; the
  CSRF middleware already no-ops on safe methods (`csrf.rs:57`). No auth on assets (they are public,
  non-secret, vendored).
- **Cache + compression headers**: `ServeDir` emits `Last-Modified`/`ETag` and supports range; the
  `compression-gzip` feature (already on) handles gzip. Long-lived `Cache-Control: immutable` is
  achieved via the content-hash filename (below) — set with `ServeDir::new("static")` plus a
  `SetResponseHeader` layer for `Cache-Control: public, max-age=31536000, immutable` on the
  content-hashed paths (NFR-WEBB-PERF-03: "200 with content type + cache header").
- **Air-gap proof (US-B02 scenario 1):** the blobs live in the image; rendering makes **zero
  external-origin requests** — asserted by the harness network-request count = 0.

## `static/` organization

```text
crates/foundry-app/static/
  vendor/
    htmx-2.0.4.min.js          # pinned htmx 2.x blob (exact version = htmx2-migration.md)
    alpine-3.14.8.min.js       # pinned Alpine blob
  css/
    foundry.css                # hand-written stylesheet (see §CSS strategy)
  VENDOR.md                    # provenance + integrity record (air-gap audit trail)
```

`VENDOR.md` records, per blob: upstream canonical URL, exact version/tag, retrieval date, and a
**sha256** of the committed file. This is the manual-curation discipline DB6 accepts in exchange for
a toolchain-free build: an auditor (or an air-gapped operator) can verify each blob matches the
named upstream release without trusting a build pipeline. Updating an asset = download the new pinned
release, minified-as-published, drop it in, update `VENDOR.md` + the hash, run the suite.

> **Minification without a minifier:** htmx and Alpine ship **pre-minified `.min.js` artifacts** on
> their GitHub releases / official distribution. We vendor THOSE published artifacts directly — no
> local minify step, honoring DB6. The CSS is hand-written and small; it is served as-authored (the
> ≤a-few-KB cost of un-minified hand CSS is negligible and gzip handles it). No CSS minifier is
> introduced.

## CSS strategy (Open sub-decision, recommendation)

**Recommendation: a single hand-written `foundry.css`** (not a utility-class framework, not a
build-time CSS tool). Rationale:
- The scope (`out-of-scope.md`) is "looks intentional, accessible (WCAG 2.2 AA), consistent" — **not
  a design system, theming, or dark mode.** A small hand-written stylesheet meets that with zero
  toolchain.
- A utility framework (Tailwind etc.) would require a build step to purge/compile — **forbidden by
  DB6.** A pre-built utility CSS blob would ship a large unused payload. Hand-written CSS keeps the
  blob tiny and the markup classes semantic (`.column`, `.issue-card`, `.comment`) — which the
  acceptance suite already selects on, so semantic classes are *also* the render contract.
- a11y obligations (NFR-WEBB-A11Y-02): contrast ≥4.5:1 (3:1 large), visible focus indicators
  (`:focus-visible`), interactive targets ≥24×24px, labelled inputs. These are stylesheet + template
  responsibilities and are checked by an a11y lint on the board/issue/sign-in render.

## Decision #4 — cache-busting WITHOUT a build step

Three options weighed (all toolchain-free):

**Option 4a — content-hash in the committed filename. RECOMMENDED.**
Commit the asset as `foundry.<sha256-prefix>.css` / `htmx-2.0.4.min.js` (version-pinned name is its
own cache key) and reference that exact name from the base layout. When a blob changes, its committed
filename changes, so the URL changes, so caches miss correctly. Pair with
`Cache-Control: public, max-age=31536000, immutable`.

- Pro: **correct, simple, zero runtime logic, zero build step.** The hash/version IS in the path the
  committer writes; nothing computes it at build or runtime. Immutable caching is safe because a new
  content = a new URL. Air-gap-clean.
- Con: the contributor renames the file + updates the one `<link>`/`<script>` reference on an asset
  swap. This is a 2-line manual edit — and the **asset-resolution probe** (below) catches a
  forgotten rename by reding CI, so it cannot silently ship a stale path. Acceptable given DB6's
  manual-curation premise. For the vendor blobs the **version in the filename** (`htmx-2.0.4.min.js`)
  already serves as the cache key with no separate hash needed; the hash form is used for the
  hand-edited `foundry.css` where the version is less obvious.

**Option 4b — version query string (`/static/foundry.css?v=3`).**
- Pro: filename stays stable; bump a query param.
- Con: some proxies/CDNs (and historically some browsers) ignore query strings for caching; the
  `?v=` is hand-maintained too, with the same forget-to-bump risk but **without** the
  self-correcting "the URL is wrong → 404 → probe reds" safety of a renamed file. Weaker than 4a.

**Option 4c — `ServeDir` defaults only (ETag/Last-Modified, no busting).**
- Pro: zero work.
- Con: cannot use `immutable`/long max-age safely (the URL never changes), so every deploy risks a
  stale cached asset until revalidation; revalidation round-trips defeat the "cacheable" NFR's
  intent. Acceptable as a *fallback* but not the target.

**Recommendation: 4a** — content-hash/version in the committed filename + `immutable` long-cache,
guarded by the asset-resolution probe. It is the only option that gives *both* aggressive caching
*and* correctness with no build step, and its one failure mode (a forgotten rename) is caught by CI.

## Asset-resolution probe (Earned Trust — US-B02 scenario 3)

A small CI check (an `xtask check-assets` subcommand — `xtask` already exists and anticipates such
subcommands per Feature A `boundary-guard.md` — or a `#[test]` in `foundry-app`):

1. Parse the base layout template's vendored `<link href>` / `<script src>` `/static/...` references.
2. Assert each referenced file exists under `static/` on disk.
3. (Optional, stronger) assert each vendored blob's sha256 matches `VENDOR.md`.

A typo (`/static/htmx.js` when the file is `/static/vendor/htmx-2.0.4.min.js`) **reds CI** → the
broken-asset board never ships (the exact US-B02 scenario 3 contract). Gold test: rename a blob and
assert the check goes red (proving the probe bites — Principle 12 self-application).

## Dockerfile / deployment note (for platform-architect)

The `static/` directory must be **`COPY`'d into the image** alongside the binary (the templates are
compiled in via Askama, but the static blobs are read from disk at runtime by `ServeDir`). No CDN, no
asset-build stage, no Node layer. `docker compose` topology unchanged (foundry + postgres). This is
the only deployment delta Feature B introduces.
