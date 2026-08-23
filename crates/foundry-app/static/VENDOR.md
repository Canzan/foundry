# Vendored static assets — provenance & integrity

Pure pre-vendored blobs (DB6 / `docs/feature/htmx-web-tier/design/assets.md`):
**NO Node, NO bundler, NO package.json, NO minifier step, NO CDN at runtime.**
Each blob below is the upstream-published, pre-minified artifact, committed
verbatim and served by the binary via `tower_http::services::ServeDir`
(`.nest_service("/static", ...)` in `foundry-app/src/lib.rs`).

An auditor (or an air-gapped operator) can verify each file matches the named
upstream release by re-computing its sha256 and comparing the value recorded
here. Updating an asset = download the new pinned release, drop it in, update
this file + the hash, re-run the acceptance suite.

| File | Version | Upstream canonical URL | Retrieved (UTC) | sha256 |
|------|---------|------------------------|-----------------|--------|
| `vendor/htmx.min.js` | htmx **2.0.4** (pinned latest-stable 2.0.x; step 04-01 migration) | https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js | 2026-06-04 | `e209dda5c8235479f3166defc7750e1dbcd5a5c1808b7792fc2e6733768fb447` |
| `css/foundry.8ce38566.css` | hand-authored (this repo) | — (not vendored; authored in-tree) | 2026-08-22 | `8ce38566aada1b12c9eb247d0d58f9a387ba19926a567dd7ad53569bf5b0fadf` |

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
- `foundry.8ce38566.css` is hand-authored and served as-authored (gzip via the
  `compression-gzip` tower-http feature handles the wire size); no CSS minifier
  is introduced (DB6). **The `.8ce38566.` segment is the content hash** (first 8
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
- Re-verify a hash with: `shasum -a 256 crates/foundry-app/static/vendor/<file>`.
