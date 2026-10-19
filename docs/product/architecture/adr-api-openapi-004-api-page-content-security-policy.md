# ADR-API-OPENAPI-004: The API page carries its own strict Content-Security-Policy, set by a route-scoped layer, as the seed of a site-wide policy

## Status

Accepted (2026-10-18). Proposed by the api-openapi-document DESIGN wave. The user accepted the
policy as designed, with `'unsafe-eval'` not granted.

Decision 4 is still contingent. OQ-U4 was not asked. It is settled by slice 03 step 0 (the static
grep) and step 1 (the browser probe), and goes to the user only if `eval` turns out to be needed.

## Context

Foundry sends no `Content-Security-Policy` today, from any crate or from
`deploy/k8s/foundry-ingress.yaml` (DISCUSS Prior Wave Consultation). OQ-4 was answered: the API
page gets its own restrictive policy now, and a site-wide policy is a follow-up. The required
outcome (US-AOD-05 AC 4):

- the browser refuses any script, style, image, font, connection or frame from another origin;
- another site cannot frame the page.

What swagger-ui 5.33.3 needs:

- its own script and stylesheet from `'self'`;
- `data:` images (icons in its CSS);
- `fetch` to `'self'` for the document and for "Try it out";
- **inline styles**. React writes `style` attributes, and the bundle injects a `<style>` element
  (swagger-ui issue #10655, open). A hash is fragile across versions.

Older releases needed `'unsafe-eval'` (issues #5817 and #7540, traced to regenerator-runtime and
swagger-js). Whether 5.33.3 still needs it is unverified.

tower-http's `set-header` feature **is** enabled for foundry-app (`crates/foundry-app/Cargo.toml:52`,
added by feature unification, though the workspace line `Cargo.toml:36` lacks it), but nothing uses
it today. (Corrected 2026-10-19 after DEVOPS; this ADR first said the feature was not enabled.) The
house idiom for per-path headers is an `axum::middleware::from_fn` layer (`static_cache_control`,
`foundry-app/src/lib.rs:305`).

## Decision

1. **The header, exactly**, on every `GET /docs/api` response:

   ```
   Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'
   ```

   With `default-src 'none'`, fonts, frames, workers, media and manifests are refused from every
   origin. swagger-ui's CSS is **expected** to name font families but load no font file. That is
   confirmed at ADR-API-OPENAPI-003 slice 03 step 0 (the `@font-face` / `url(` grep) and enforced
   afterwards by rule `api-docs-page` (vi). If step 0 finds a font, this header changes under the
   slice-03 stop rule.

   Companion headers on the same response:
   - `X-Frame-Options: DENY`, for browsers without `frame-ancestors`;
   - `X-Content-Type-Options: nosniff`;
   - `Referrer-Policy: no-referrer`;
   - `Cache-Control: no-cache`, because the heading carries the release version.
2. **Where.** The constant is `API_DOCS_CSP` in foundry-app. It is applied by
   `middleware::from_fn(api_docs_headers)` layered on the `/docs/api` route **only**. No other
   response changes: not `/static`, not `/api/v1`, no other page (D12, SC-3). The headers apply to
   the HTML document. Subresources need none, because a CSP governs the document that loads them.
3. **The site-wide seam.** The layer is a named function holding a named constant. The site-wide
   follow-up moves the same layer to the base router with a site policy, starting from this one. It
   must first make the shared templates clean:
   - `partials/settings_action_result.html:5`'s inline `onclick`;
   - `partials/issue_card.html`'s inline `style`;
   - any htmx inline-style use (htmx 2 injects an indicator `<style>` unless
     `htmx.config.includeIndicatorStyles` is false).

   This feature records the inventory and does not do the job.
4. **`'unsafe-eval'` is not granted by design.** Slice 03's first step serves the page with this
   policy, opens it in the browser lane with a `securitypolicyviolation` recorder, renders the
   document, and runs one "Try it out" call.
   - Zero violations: the policy above ships unchanged.
   - An `eval` violation: the slice stops and the user decides (OQ-U4) between adding
     `'unsafe-eval'` to this page's `script-src` only (residual recorded in `VENDOR.md` and here) or
     declining slice 03.

   An inline `<style>` is already allowed by `'unsafe-inline'` in `style-src`. **`script-src` never
   gains `'unsafe-inline'`.**

## Consequences

- Positive: an injected script from another origin, a framing attack (the class of CVE-2021-46708)
  and a fetch to an attacker's spec or config URL are each refused by the browser, even if D10's
  configuration were bypassed. The policy is defence in depth over the initializer's options.
- Positive: the browser lane can assert the policy's effect directly: zero violation events and zero
  non-same-origin resource entries, plus a framing probe.
- Negative: `style-src 'unsafe-inline'` leaves CSS injection open on this page. The document is the
  instance's own fixed bytes, and swagger-ui sanitises rendered Markdown, so an injection needs a
  swagger-ui flaw first. Accepted, and recorded.
- Negative: `form-action 'none'` blocks any HTML form submission. swagger-ui sends requests with
  `fetch`, not forms, so it is unaffected.
- Negative: two layers (CSP and initializer) say the same thing, so a future swagger-ui feature
  that needs a new origin fails closed in the browser. That is the intended direction.

## Alternatives considered

- **A. No page policy; rely on D10's options and the acceptance assertion.** Rejected (OQ-4
  answered). The page runs a large third-party bundle next to the session cookie's origin. It is
  the page that most needs a policy.
- **B. A policy in the ingress (`deploy/k8s/foundry-ingress.yaml`).** Rejected:
  - it applies only on Kubernetes deployments, not Compose or local;
  - it moves an application rule into deployment config;
  - per-path matching there is a second routing table.
- **C. Use tower-http's `SetResponseHeaderLayer`** (the `set-header` feature is already enabled
  for foundry-app). Viable. Rejected on consistency alone: every per-path header in foundry-app
  is set by an `axum::middleware::from_fn` layer, the house idiom, and this one should not be the
  first exception. Either way the policy is set at the route.
- **D. Hashes or nonces for swagger-ui's inline style.** Rejected. The injected CSS changes with
  each release (fragile hash), and a nonce needs the bundle's cooperation, which it does not offer.
- **E. Site-wide now.** Rejected: out of scope (DISCUSS Out of Scope). The shared templates are not
  CSP-clean (decision 3).
