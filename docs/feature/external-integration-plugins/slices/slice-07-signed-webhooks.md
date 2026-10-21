# Slice 07: Project events reach any system as signed webhooks

Stories: US-EIP-13 | Estimate: 2.5 days | job_id: `job-grant-custom-integration-access` | Deliverable C

## Goal

Lena adds a webhook route from Identity Platform to
`https://n8n.canzan.dev/webhook/foundry-auth`, copies its signing secret once, and
Sven's n8n workflow receives a signed, documented JSON event for AUTH-12, with
retries that repeat the same event id and an address policy that keeps the
webhook from reaching into the cluster unless the operator allowed it.

## IN

- Webhook as a route kind: URL, signing secret shown once, rotation with a 24-hour overlap, Send test.
- Payload per D-2's webhook column (including `comment.edited`, `comment.deleted`), schema version 1, stable event id, D-3 content level, documented with the API description.
- Timestamped HMAC signature; 2xx delivered; 410 stops the route; others follow slice 03's retries and history; redirects not followed.
- Address policy (OD-13): loopback, link-local and metadata always refused; private ranges only when the operator allows; K-16.
- Account notifications never reach webhook routes; the env `WebhookProvider` unchanged (SC-7).

## OUT

- Inbound webhooks into Foundry. Per-receiver custom payload templates.

## Learning Hypothesis

- **Disproves if it fails**: that one generic payload is enough for n8n-style tools without per-receiver templates. If Sven must reshape every event in n8n, a template feature becomes the next need.
- **Confirms if it succeeds**: an n8n workflow verifies the signature and acts on AUTH-12 at the dogfood (EIP-KPI-8, webhook half).

## Acceptance Criteria

- [ ] The receiver gets one signed POST per event that verifies with the secret; a retry carries the same event id.
- [ ] The secret is shown once and never again.
- [ ] `169.254.169.254`, `localhost` and an unallowed private address show K-16.
- [ ] A 410 stops the route with the stated reason.
- [ ] A password reset sends nothing to any webhook route.

## Dependencies

- Slices 01-03; slice 06 if routes are created by API.
- OD-13, OD-14 answered.
