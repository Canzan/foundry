# Slice 05 — Hosted email API provider (SendGrid/SES/Postmark-style HTTP)

**Goal**: send email through a hosted vendor's HTTPS API → an operator sets `NOTIFICATION_PROVIDERS=email_api`,
`EMAIL_API_URL=https://api.postmark.example/email`, `EMAIL_API_KEY=…`, `EMAIL_API_FROM=foundry@acme.example`,
and a `POST /forgot-password` for `maria.santos@acme.example` is sent via the vendor API — with the key kept
out of every log line.
**Story**: US-05.

**IN scope**
- A **Hosted email API** provider implementing the `NotificationProvider` port, active when `email_api` is
  listed and `EMAIL_API_URL` + `EMAIL_API_KEY` are configured; sends each email-shaped notification via the
  vendor's HTTPS API using the key as a credential header.
- **Secret non-leakage**: `EMAIL_API_KEY` used only to construct the request — never in logs/errors/metrics/
  `Debug` (NFR-2, ODD-8).
- Participates in **fan-out + best-effort isolation + counting**: a 2xx counts `delivered`, a non-2xx (incl.
  429) counts `failed` and is isolated (NFR-3, NFR-4); **no automatic retry in v1** (NFR-6).
- **Fail-fast** if `email_api` is listed with a missing required setting, secret-free error (NFR-1).
- Acceptance: delivered-via-api, rate-limit-isolated-no-retry, key-never-leaks-and-fail-fast; dogfooded against
  a vendor sandbox / mock.

**OUT of scope**: new event types (06); retry/backoff (v1 best-effort, ODD-7); vendor-specific analytics/
webhooks-back; per-recipient routing (successor feature).

**Learning hypothesis**: disproves "a second HTTP-shaped provider reuses the slice-04 HTTP client + secret
handling with near-zero new machinery" if hosted-vendor auth/response semantics (API-key header, 2xx/4xx/429
handling) need a different transport contract than the webhook provider, or if secret-safe handling of the API
key needs a mechanism slice 04 didn't establish.

**Seams**: the fan-out + isolation + metric machinery (slice 03); the HTTP client + secret handling (slice 04);
the `NotificationProvider` port (slice 01); config style (`main.rs:242-262`).
**Dependencies**: US-03 (fan-out) + US-04 (HTTP client + secret handling). DESIGN ODD-1 (payload), ODD-8
(secret-safe Debug), ODD-7 (no-retry stance).
**Effort**: ~1 day (an HTTP API adapter; mostly reuses the slice-04 client + secret handling).
