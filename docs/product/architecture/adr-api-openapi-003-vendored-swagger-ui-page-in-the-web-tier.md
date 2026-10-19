# ADR-API-OPENAPI-003: The API page is a standalone foundry-app template over vendored swagger-ui 5.33.3, with an authored initializer that pins every risky option

## Status

Accepted (2026-10-18). Proposed by the api-openapi-document DESIGN wave. The user chose decision 6
(a), a small swagger-ui plugin that supplies the D14 Authorize copy (OQ-U3), as recommended.

The `'unsafe-eval'` question (OQ-U4) is still open. Slice 03 step 0 (the static grep) and step 1
(the browser probe) settle it, and it goes to the user only if `eval` turns out to be needed.

## Context

D2 asks for a self-hosted Swagger UI page with "Try it out". D10 and D11 add the rules:

- the page loads only its own document, from its own origin;
- the token is kept in memory only and sent to this origin only;
- no CDN, no validator;
- assets are vendored under ADR-CANZAN-THEME-002/003.

D12 says no other page's request set changes. OQ-2, OQ-3 and OQ-4 were answered: the page is
public, it lives at `/docs/api`, and it gets its own strict policy.

Facts:

- `check_api_no_html` (`check_arch.rs:192`) forbids HTML in `crates/foundry-api/src`.
- `/static` is served by `ServeDir` (`foundry-app/src/lib.rs:414-418`). Like every route registered
  before `.layer(csrf_middleware)` and `.layer(session_layer)` (`:821-825`), it is **inside** both
  layers. Only the `foundry_api::routes()` merge (`:835`) sits outside them. This ADR originally
  said otherwise (review H1, corrected).
- The cache policy is path-aware (`static_cache_control_value`, `:272`). `…/js/….js` is
  `no-cache`, and everything else is `immutable`. That is honest only at content-addressed or
  version-addressed URLs.
- check-arch's R1 scans `src/*.rs`, `templates/*.html` and the static CSS for `/static/` references.
  R3 re-hashes every `VENDOR.md` row whose first cell has a file extension.
- `base.html` loads `theme.js`, htmx and five app scripts.

swagger-ui facts (primary sources checked 2026-10-18; see the feature-delta DESIGN Technology
Choices):

- latest 5.33.3, released 2026-10-15, Apache-2.0 with a NOTICE file;
- the only repository advisory is GHSA-qrmm-w75w-3wpx (`?url=` spoofing), fixed in 4.1.3;
  CVE-2021-46708 (clickjacking) is fixed in 4.1.3; nothing is open against 5.33.3 (GitHub
  advisories, Snyk);
- `swagger-ui-bundle.js` is 1.59 MB and `swagger-ui.css` is 186 kB;
- the stock `swagger-initializer.js` points at `petstore.swagger.io`;
- the default `validatorUrl` is `https://validator.swagger.io/validator`, whose badge sends the spec
  URL to swagger.io;
- `persistAuthorization` (default false) is the only path that writes `localStorage`
  (`"authorized"`);
- `queryConfigEnabled` defaults to false (since 4.1.3);
- `deepLinking` defaults to false;
- the bundle injects an inline `<style>` element (issue #10655);
- whether 5.33.x still evaluates code through `new Function` (issues #5817, #7540; fix PR #10559
  not shown as merged) is **unverified**.

**Which of these facts were verified, and how (review M4).**

- Read in primary sources: the option defaults, the `validatorUrl` values `none` / `localhost` /
  `127.0.0.1` named in swagger-ui's configuration docs as disabling validation, and
  `persistAuthorizationIfNeeded`'s single `localStorage.setItem("authorized", …)` in
  `src/core/plugins/auth/actions.js`.
- **Not yet read in the 5.33.3 release bytes:**
  - that the bundle makes no other storage write;
  - that `swagger-ui.css` loads no font or remote stylesheet;
  - the `validatorUrl` helper code itself;
  - `eval` / `new Function` use.

They are therefore verified at **slice 03 step 0, before any row lands**. The step downloads the
four files and greps them for `localStorage`, `sessionStorage`, `document.cookie`, `@import`,
`@font-face`, `url(`, `eval(`, `new Function` and the validator helper. It records the result (with
offsets) in `VENDOR.md`'s swagger-ui note. A finding contradicting this ADR stops the slice for a
design decision.

These facts are not only promised. Three of them are also enforced after vendoring:
- rule `api-docs-page` (vi) fails on a remote `url(` or `@import` in the CSS;
- `img-src 'self' data:` and `connect-src 'self'` block a validator request even if the option
  were ignored;
- the browser lane counts violations.

## Decision

1. **Route and tier.** `GET /docs/api` is a foundry-app handler. It is mounted **after**
   `.layer(session_layer)`, beside the `.merge(foundry_api::routes())` call, so neither the session
   layer nor `csrf_middleware` wraps it. It reads no session and sets no cookie. A foundry-app test
   asserts no `Set-Cookie`, both with no cookie and with a signed-in session. The page is public (OQ-2), and nothing about the caller can change
   it. foundry-api is untouched, and `check_api_no_html` is unchanged.
2. **Standalone template**, `templates/api_docs.html`, rendered by askama. It does **not** extend
   `base.html`, so it loads none of theme.js, htmx or the app scripts, and no other page changes
   (D12). Its markup:
   - `<html lang="en">`;
   - `<title>Foundry API</title>`;
   - the D14 heading line, with `{version}` from `views::RELEASE_VERSION`, and the document path as
     a link;
   - a `<noscript>` with the D14 copy and link;
   - `<div id="swagger-ui">`;
   - three external references: `swagger-ui.css`, `swagger-ui-bundle.js` and the initializer, each
     loaded with `defer`.

   It has no inline script, no inline handler and no `style` attribute in Foundry's own markup.
   Because the heading line sits outside swagger-ui's container, it and its link stay visible when
   the document fails to load (D14).
3. **Vendored files, at a version-addressed directory** (so `immutable` is honest, ADR-B03):

   | File | Row shape |
   |---|---|
   | `static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js` | upstream-verbatim |
   | `static/vendor/swagger-ui-5.33.3/swagger-ui.css` | upstream-verbatim |
   | `static/vendor/swagger-ui-5.33.3/LICENSE.txt` | upstream-verbatim bytes of `LICENSE`, renamed so R3's extension rule enrols it (the `OFL-*.txt` precedent) |
   | `static/vendor/swagger-ui-5.33.3/NOTICE.txt` | upstream-verbatim bytes of `NOTICE`, renamed for the same reason; Apache-2.0 §4(d) requires redistributing it |

   Each file gets a `VENDOR.md` row: the version, `https://unpkg.com/swagger-ui-dist@5.33.3/<file>`,
   the retrieval date and the sha256. A note under the table records:
   - the advisory check (date, sources, result "no open advisory against 5.33.3");
   - the total size, about 1.79 MB against NFR-AOD-3's 2.5 MB ceiling;
   - why the other dist files are not vendored. The standalone preset carries the "Explore" URL bar
     that loads arbitrary specs. The `.map` files are 3.5 MB of developer aids. `index.html` and the
     stock initializer point at petstore. The `oauth2-redirect` files are unused, because the API
     has no OAuth flow.

   The bundle's trailing `sourceMappingURL` comment stays (verbatim). It makes a browser with
   DevTools open request a same-origin `.map`, which 404s. That is not cross-origin and not a
   page-load request.
4. **Authored initializer, `static/js/api-docs.js`.** It is an authored-in-tree `VENDOR.md` row, and
   the `/js/` path takes `no-cache`. It is a vanilla script that runs `SwaggerUIBundle({…})` on
   `DOMContentLoaded` with these options set explicitly, not left to defaults:
   - `url: "/api/v1/openapi.json"`, with no `urls`, `spec` or `configUrl`;
   - `dom_id: "#swagger-ui"`;
   - `layout: "BaseLayout"`, so there is no top bar or URL box;
   - `queryConfigEnabled: false`;
   - `validatorUrl: "none"`;
   - `persistAuthorization: false`;
   - `withCredentials: false`;
   - `deepLinking: false`;
   - `tryItOutEnabled: false`, the default, so a visitor opts in per operation.

   `supportedSubmitMethods` keeps its default (every method), because the API's own authorization
   decides (OQ-2). The document's `servers: [{url: "/"}]` makes every "Try it out" request
   same-origin. The pasted token lives only in swagger-ui's in-memory store. Nothing is written to
   `localStorage`, `sessionStorage`, a cookie or the URL, and a reload forgets it.
5. **What enforces it:**
   - check-arch R1-R3 cover the five new files;
   - the new check-arch rule `api-docs-page` (feature-delta DESIGN, check-arch Rules) pins the
     initializer's options, forbids absolute URLs in it, keeps swagger-ui references to
     `api_docs.html`, and keeps `/api` owned by foundry-api;
   - the browser lane asserts zero cross-origin requests, zero CSP violations, no token trace after
     a call and a reload, and the query string ignored (US-AOD-05/06).
6. **The Authorize dialog's D14 copy** ("Machine token" label and the hint) is supplied by a
   swagger-ui plugin in the initializer. The plugin overrides the bearer branch of the `HttpAuth`
   component, built only with `system.React` and `getComponent`, and reproduces the stock behaviour
   with the D14 label and hint. It is coupled to swagger-ui's internal component contract, so the
   browser lane pins both strings, and a swagger-ui bump re-runs that lane before the row changes.
   Accepted by the user, 2026-10-18 (OQ-U3 (a)).

## Consequences

- Positive: the page adds one route, one template, one authored script, four vendored files and
  one of the feature's three new check-arch rules (`api-docs-page`). No crate is added and no other page changes.
- Positive: every option that once caused a swagger-ui advisory is pinned in reviewed source and
  checked by a rule, not left to a default that a future release could change.
- Negative: a ~1.6 MB third-party bundle runs on Foundry's origin. Mitigations: the pinned version,
  the advisory record, the page-only strict CSP (ADR-API-OPENAPI-004), no session read, and a
  separate path that no other page links to.
- Negative: the Authorize-copy plugin is version-coupled. If the user prefers zero coupling, the
  stock "Value" label stays and the D14 hint is shown in the page heading area instead (OQ-U3
  option b), which changes D14's placement.
- Negative: the page has no Foundry stylesheet, so it looks like stock swagger-ui with a plain
  heading. That is accepted for a tool page reached by URL only.

## Alternatives considered

- **A. Extend `base.html`** (nav, footer, theme). Rejected:
  - it loads htmx and five app scripts the page does not need;
  - it puts the page under the session layer;
  - `settings_action_result.html`'s inline `onclick` and `issue_card.html`'s inline `style` show that
    the shared layout has not been made CSP-clean, so a strict policy on a `base.html` page is a
    larger job (the site-wide follow-up).
- **B. A static `static/docs/index.html`.** Rejected. It cannot show `{version}` (D14) without
  client-side work, and a per-route CSP header on a `ServeDir` file needs a path-matching layer.
- **C. `utoipa-swagger-ui`.** Rejected. It downloads swagger-ui at build time (SC-4), lags upstream
  (5.32.6), and serves from its own router, not `/static` under R1-R3.
- **D. Redoc or Scalar.** Rejected. Redoc has no "Try it out", which D2 requires. Scalar is younger,
  and its default build fetches fonts and a proxy from its own CDN. swagger-ui is what the user
  named.
- **E. Vendor swagger-ui-standalone-preset as well.** Rejected. Its top bar is a URL box, which is
  exactly the spec-from-elsewhere path D10 forbids.
