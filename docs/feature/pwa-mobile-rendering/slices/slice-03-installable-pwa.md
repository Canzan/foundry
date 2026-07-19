# Slice 03 — Installable PWA (manifest + icons + theme-color)

**Goal**: Foundry can be installed to the home screen and launches standalone — ship a valid web app manifest,
icons, theme-color, and the apple meta.

**Story**: US-03. **Depends on**: slice 01 (viewport). Independent of slice 02 but naturally last.

**IN scope**
- `static/manifest.webmanifest` (static asset): `name`, `short_name`, `start_url`, `scope`, `display:
  standalone`, `theme_color`, `background_color`, `icons` (192, 512, maskable).
- Icon assets under `/static` (192/512 png + maskable + apple-touch-icon) — ODD-4.
- `base.html` head: `<link rel="manifest">`, `theme-color` meta, `apple-mobile-web-app-capable` +
  `apple-mobile-web-app-status-bar-style` + `apple-touch-icon`.
- Manifest + icons served with a sensible content-type + cache policy via the existing `/static`
  (`static_cache_control`).
- **ODD-3**: verify current Chrome install criteria; ship a **minimal service worker** ONLY if the prompt
  requires it, and keep it from caching dynamic HTML (no offline in v1). If no SW is needed, ship none.
- fantoccini scenarios: manifest linked + 200 + valid JSON with all fields; icons fetch 200 image; theme-color
  + apple meta present; `display: standalone`. Un-@pend US-03 scenarios.

**OUT of scope**
- Offline / caching of pages or assets; background sync; push notifications.

**Learning hypothesis**: disproves **"manifest + head tags = installable, no service worker needed"** (ODD-3)
if the target browser's install prompt still requires a SW with a fetch handler — in which case a minimal no-op
SW ships (and must not cache dynamic HTML). Confirms the lean path if manifest + HTTPS suffices.

**Acceptance**: `discuss/acceptance-criteria.md` US-03.

**Seams**: `base.html` head; `/static` serving + `static_cache_control` (`lib.rs:~251-325`); the icon asset
pipeline; (optional) a `static/sw.js` + a tiny registration script.

**Falsification**: the manifest-valid scenario RED before the manifest exists; the icons-served scenario RED
against a manifest that references missing icon files; the standalone/theme scenario RED before the head tags.

**Watch items**
- **HTTPS**: the install prompt needs HTTPS (or localhost). Dev is HTTP — note that install is a
  prod/localhost dogfood; the fantoccini lane asserts the *layout facts* (manifest served + valid + standalone
  + theme-color), not the OS prompt itself.
- If a SW ships, it MUST NOT cache dynamic HTML/CSRF'd responses (v1 has no offline goal) — scope it to a no-op
  or static-only, and confirm it doesn't alter htmx.
- New static files change cache headers, not the CSS hash — but if the manifest references the hashed CSS,
  keep them consistent.

**Dependencies**: slice 01. **Effort**: ~1 day (incl. icon assets + the ODD-3 check).
