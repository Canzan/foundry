# Slice 02: Routes carry the right events and reveal only what was chosen

Stories: US-EIP-03, US-EIP-04 | Estimate: 3 days | job_id: `job-route-project-events-to-chat` | Deliverable A

## Goal

Lena splits Identity Platform across channels: new work and comments to
`#auth-dev`, moves into Done to `#auth-releases`, and only links to `#general`.
She sees each message in a preview before saving, a channel shared with another
organisation needs her explicit tick, and nothing anyone types can ping a channel.

## IN

- The v1 chat events of D-2: New issue, Issue moved (optional destination-lane filter), New comment, Issue deleted. No message for an in-lane reorder.
- Lane filters bound to the lane's identity; a deleted filtered lane puts the route in Needs attention.
- The burst rule (D-11): more than 5 same-kind events from one action become one message.
- Content levels Link only, Summary (default), Summary with comment text (300 characters); the never-sent and never-pings rules (D-3); the preview; K-5 on every route form; K-6 for shared channels.

## OUT

- History, retries, connection loss, lifecycle (slice 03). Matrix rendering (slice 04).
- `issue.edited`, assignment, membership events (follow-ups F-1, F-2).

## Learning Hypothesis

- **Disproves if it fails**: that the content rules can be stated once and enforced per kind by a property test over generated text (no ping, no description, no email address). If Slack's escaping cannot be made total, slice 04 and 07 inherit a hole and the default level must drop to Link only (OD-6).
- **Confirms if it succeeds**: Lena runs two channels from one project for a week without muting either (dogfood report).

## Acceptance Criteria

- [ ] Moves into Done reach `#auth-releases` only; reorders post nothing; a lane rename keeps the filter; a deleted lane asks to be fixed.
- [ ] A lane deleted with 7 cards posts one message.
- [ ] The preview equals the message sent; Link only shows no title; comment text is cut at 300 characters.
- [ ] `@channel`, `<!here>` and `<@U…>` in user text notify nobody; a description never appears.
- [ ] A shared channel cannot be saved without the K-6 tick.

## Dependencies

- Slice 01. The previous lane of a move must be known at delivery (US-EIP-03 Technical Notes).
- OD-6 answered.
