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
| `vendor/htmx.min.js` | htmx **1.9.12** (last 1.x stable; bump to 2.x is step 04-01) | https://unpkg.com/htmx.org@1.9.12/dist/htmx.min.js | 2026-06-04 | `449317ade7881e949510db614991e195c3a099c4c791c24dacec55f9f4a2a452` |
| `vendor/alpine.min.js` | Alpine.js **3.14.1** | https://cdn.jsdelivr.net/npm/alpinejs@3.14.1/dist/cdn.min.js | 2026-06-04 | `358d9afbb1ab5befa2f48061a30776e5bcd7707f410a606ba985f98bc3b1c034` |
| `css/foundry.css` | hand-authored (this repo) | — (not vendored; authored in-tree) | 2026-06-04 | `870985fc8aef09342bf108e84278d54c024cde03dcfd3f6743eecd610d746d37` |

## Notes

- **htmx 1.9.12** is the last stable 1.x release. The `htmx.org` UMD wrapper
  (`define.amd` / `module.exports` shim) and `version:"1.9.12"` marker near the
  top of the blob confirm the 1.x line. The migration to htmx 2.x is a later
  step (04-01); this step only requires the blob to be served + non-empty.
- **Alpine 3.14.1** records `version:"3.14.1"` in the blob.
- `foundry.css` is hand-authored and served as-authored (gzip via the
  `compression-gzip` tower-http feature handles the wire size); no CSS minifier
  is introduced (DB6).
- Re-verify a hash with: `shasum -a 256 crates/foundry-app/static/vendor/<file>`.
