# Slice 04: The same routes speak Matrix through Foundry's bot

Stories: US-EIP-08, US-EIP-09 | Estimate: 3.5 days | job_id: `job-route-project-events-to-chat` | Deliverable B

## Goal

Priya connects `@foundry:canzan.dev` on `matrix.canzan.dev` from
`/workspace/integrations`. Lena invites the bot to `#auth:canzan.dev` in Element,
routes Identity Platform there with the events and content level she already uses
in Slack, and AUTH-12 appears as a formatted bot notice. Encrypted rooms and a
missing invite are explained before anything is saved, and a kicked bot flags
its route.

## IN

- Matrix connection: homeserver address plus bot access token, checked before saving, write-only token, bot user shown (OD-4).
- Matrix routes by room alias or id: invite check, join, room name and member count, K-14, K-15 (OD-5).
- Formatted and plain-text bodies carrying K-8 lines; notices; `@room` and pills inert.
- Every slice 02-03 behaviour for Matrix: events, filters, levels, preview, history, retries, connection lost (logged-out bot), route-level refusals (kicked, banned, room became encrypted), lifecycle.

## OUT

- Encrypted rooms (F-3). Inbound commands (F-7).

## Learning Hypothesis

- **Disproves if it fails**: that the plugin contract from slices 01-03 admits a second kind with no change to events, content rules, history or lifecycle. If Matrix needs changes there, the contract is Slack-shaped and webhooks (slice 07) will need them too.
- **Confirms if it succeeds**: Lena runs AUTH in Element for a week (dogfood report) and the slice adds no new route-form concepts beyond the room field.

## Acceptance Criteria

- [ ] Connect with a valid token shows "Connected as @foundry:canzan.dev on matrix.canzan.dev"; a refused token or unreachable homeserver saves nothing.
- [ ] A not-invited room shows K-14; an encrypted room shows K-15; neither is saved.
- [ ] AUTH-12 arrives as a formatted notice within 10 s; `@room` in a comment notifies nobody.
- [ ] A kicked bot puts the route in Needs attention; a logged-out bot pauses Matrix routes until a new token.

## Dependencies

- Slices 01-03. A pinned homeserver container for acceptance (SC-10).
- OD-4, OD-5 answered; OD-13 for a homeserver on a private address.
