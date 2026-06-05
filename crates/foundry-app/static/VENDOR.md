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
| `vendor/alpine.min.js` | Alpine.js **3.14.1** | https://cdn.jsdelivr.net/npm/alpinejs@3.14.1/dist/cdn.min.js | 2026-06-04 | `358d9afbb1ab5befa2f48061a30776e5bcd7707f410a606ba985f98bc3b1c034` |
| `css/foundry.css` | hand-authored (this repo) | — (not vendored; authored in-tree) | 2026-06-04 | `870985fc8aef09342bf108e84278d54c024cde03dcfd3f6743eecd610d746d37` |

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
- **Alpine 3.14.1** records `version:"3.14.1"` in the blob.
- `foundry.css` is hand-authored and served as-authored (gzip via the
  `compression-gzip` tower-http feature handles the wire size); no CSS minifier
  is introduced (DB6).
- Re-verify a hash with: `shasum -a 256 crates/foundry-app/static/vendor/<file>`.
