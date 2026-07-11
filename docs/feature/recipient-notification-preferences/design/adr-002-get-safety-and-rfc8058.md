# ADR-002: GET-safety / one-click stance + RFC 8058 (ODD-2)

## Status
Accepted — 2026-07-11 (Morgan, DESIGN wave). Feature-local.

## Context
Email clients and security scanners **prefetch** links (NFR-2, R2). A destructive GET would let a prefetch
silently unsubscribe a recipient. NFR-5 requires CSRF on both state-changing POSTs. RFC 8058
(`List-Unsubscribe-Post: List-Unsubscribe=One-Click`) offers a mail-client-initiated one-click POST — but
that POST is **CSRF-exempt by construction** (no browser, no cookie, no `_csrf`), which is in direct tension
with NFR-5. The shipped invite flow already solved prefetch-safety + non-enumerability with a
GET(show)→POST(consume) pair under the CSRF middleware (`invites_accept.rs`, `csrf.rs:137`,
`ensure_csrf_cookie` `csrf.rs:54`).

## Decision
v1 uses **in-body link → `GET /unsubscribe` (non-destructive confirm page) → CSRF `POST /unsubscribe`**.
- `GET` decodes `t` (base64url of `email_lower|workspace_id`) + verifies `sig`; on failure → the uniform
  refusal (ADR-004/`invites_accept.rs:332-339`); on success renders a **state-aware** confirm page
  (Unsubscribe or Resubscribe per current state, ADR-006) and mints the CSRF cookie. GET **never** mutates.
- `POST` is CSRF-checked by the shipped middleware; it re-verifies the token and writes/clears the row
  (idempotent, BR-8). A bad token → uniform refusal, no state change.
- **Do NOT emit `List-Unsubscribe-Post` one-click headers** in v1. **May** emit a plain `List-Unsubscribe:
  <https-url>` (GET form) later — a low-risk deferred enhancement.

Reconciling "one-click" with "prefetch-safe + CSRF": v1 deliberately chooses **not** to offer a silent
one-click POST, because a CSRF-exempt endpoint safe only because the token authorizes it is a distinct
security design we will not rush past NFR-5. The in-body link is one action to open; the confirm is one
click; both are prefetch-safe and CSRF-guarded.

## Alternatives Considered
- **RFC 8058 one-click POST now** — rejected for v1: requires a CSRF-exempt, token-as-authorization POST
  endpoint carved out of the shipped CSRF middleware (conflicts with NFR-5); the security review of
  token-as-CSRF-defense is a separate ADR. Deferred, not refused forever.
- **Destructive GET (one-click, no confirm)** — rejected: a scanner/mail-client prefetch would silently
  unsubscribe people (NFR-2, R2). Non-starter.
- **Plaintext email in the query string** (`?e=sam@...&w=...`) — rejected: it lands the recipient's address
  in access logs. Base64url-obfuscate as an opaque `t` (obfuscation for log hygiene, not confidentiality —
  the real control is that it is the recipient's own address in their own inbox).
- **Emit `List-Unsubscribe: <url>` in v1** — deferred (not rejected): needs an optional
  `Notification.list_unsubscribe_url` set as a header by email-shaped providers; keeps the v1 `Notification`
  delta to one field (`workspace_id`). Recommended as a fast-follow.

## Consequences
- **Positive**: prefetch-safe (NFR-2) AND CSRF-guarded (NFR-5) with zero new middleware; reuses the shipped
  confirm-page pattern and CSRF cookie issuance; the confirm page doubles as the account-less resubscribe
  surface (ADR-006).
- **Negative / accepted**: no inbox-native one-click button in v1 (recipients open the link + confirm);
  `List-Unsubscribe` header + RFC 8058 one-click are explicit fast-follows.
