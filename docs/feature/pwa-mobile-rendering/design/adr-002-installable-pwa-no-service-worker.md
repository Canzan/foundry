# ADR-002 — Installable via manifest + icons, NO service worker in v1

**Status**: Accepted (2026-07-19) · **Resolves**: ODD-3, ODD-4 · **Story**: US-03

## Context

"Modern PWA" is often assumed to require a service worker. It does not, for *installability*: modern Chrome's
install criteria dropped the "must register a service worker with a fetch handler" requirement — a valid
manifest served over HTTPS with `name`/`short_name`, `icons` (192 + 512), `start_url`, and a standalone-ish
`display` is sufficient for the install prompt. iOS Safari's "Add to Home Screen" never used a service worker
— it uses the `apple-mobile-web-app-*` meta + `apple-touch-icon`. A service worker on an htmx app also brings
real correctness hazards (stale cached fragments, CSRF'd responses, the hashed-asset cache story).

## Decision

**Ship installability via a static manifest + icons + head meta. NO service worker, NO offline, in v1.**

- `static/manifest.webmanifest` (served by the existing `ServeDir`): `name`, `short_name`, `start_url`,
  `scope`, `display: standalone`, `theme_color`, `background_color`, and `icons` (192, 512, and a `maskable`
  icon).
- `static/icons/…`: 192 + 512 png, a maskable variant, and `apple-touch-icon.png` (ODD-4) — DELIVER generates
  from a simple Foundry mark.
- `base.html` head: `<link rel="manifest">`, `theme-color`, `apple-mobile-web-app-capable` + status-bar-style,
  `apple-touch-icon`.
- **No `sw.js`, no registration, no caching.** v1 has no offline requirement.

Verify at DELIVER: `ServeDir`/mime_guess serves `.webmanifest` as `application/manifest+json` — if not, use
`manifest.json` (browsers honor the linked manifest regardless of exact content-type).

## Alternatives considered

- **Ship a minimal service worker "to be a real PWA"** — rejected for v1: unnecessary for the install prompt
  on current Chrome, and a SW that caches anything on an htmx/CSRF app risks serving stale fragments or
  breaking auth. Adds a caching contract with no v1 payoff.
- **Full offline caching (app-shell + fragments)** — deferred to a follow-up feature: it's a genuine, larger
  design problem (which fragments are safe to cache, cache invalidation vs the hashed assets, CSRF, the
  htmx-swap story). Out of scope keeps this feature honest.
- **`manifest.json` with `application/json`** — acceptable fallback; `.webmanifest` +
  `application/manifest+json` is the standard, chosen if `ServeDir` provides it.

## Consequences

- Installable on Android Chrome (manifest + HTTPS) and iOS Safari (apple meta) with zero offline machinery and
  zero htmx-caching risk.
- **HTTPS caveat**: the OS install prompt requires HTTPS (or localhost); dev is HTTP. So the true prompt is a
  prod/localhost dogfood; the fantoccini lane asserts the layout/asset facts (manifest linked + 200 + valid
  fields + icons served + `display:standalone` + theme-color), which is what's mechanically checkable
  headlessly.
- If a future need for offline arises, a service worker is added as its own feature with its own caching
  contract — not smuggled in here.
- No new route, no migration; the manifest + icons are static files under the existing `/static`.
