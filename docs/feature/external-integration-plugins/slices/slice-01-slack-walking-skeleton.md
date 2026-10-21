# Slice 01: One project's new issues reach one Slack channel, end to end

Stories: US-EIP-01, US-EIP-02 | Estimate: 3 days | job_id: `job-route-project-events-to-chat` | Walking skeleton | Deliverable A

## Goal

Priya connects Canzan Labs' Slack with Add to Slack on `/workspace/integrations`.
Lena routes Identity Platform's New issue events to `#auth-dev`, sends a test,
and saves. When Sam files AUTH-12, a Foundry message reaches `#auth-dev` within
10 seconds, and filing the issue is exactly as fast as before whatever Slack does.

## IN

- The workspace integrations page, Add to Slack, Slack's approval and return, K-1, K-2, K-3, the connection stored with its secret write-only (SC-3, OD-12).
- The project integrations page for Manage users: New route → Slack → channel picker (public channels, and private ones the app is in) → Send test (K-7) → save.
- One event, `issue.created`, at the Summary content level (K-8), with Open in Foundry.
- Delivery after commit, durable across a restart (SC-1, SC-2): an undelivered message is retried at a fixed interval for up to 24 h, so nothing is silently lost (OD-7); the route row's status ("Delivering", "Retrying: {reason}") and last delivery time.
- Task T-1, first: record the no-routes smoke-lane p95 of issue create and move (100 sequential calls, release build) as EIP-KPI-4's baseline.
- Uniform 404 for non-admins (connections) and non-Manage users (routes); audit records for connect and route create.

## OUT

- Other events and lane filters (slice 02); content levels and preview (slice 02).
- Delivery history page, backoff and "retry after", per-issue ordering, connection-lost handling, pause, edit, delete, disconnect (slice 03).
- Matrix, email, API, webhooks, OAuth (slices 04-08).

## Learning Hypothesis

- **Disproves if it fails**: that a self-hosted instance can run a Slack app install end to end with only operator settings (OD-3), and that events can be delivered durably after commit without changing the action's latency, given the outbox is fire-and-forget today (R3). If either fails, the delivery model for every later slice changes before more kinds are built on it.
- **Confirms if it succeeds**: Lena gets AUTH-12 in Slack within 10 s, and the smoke-lane p95 for issue create with Slack unreachable stays within 10% of the baseline (EIP-KPI-4).

## Acceptance Criteria

- [ ] Priya connects Slack and reads K-1; cancel or forged return reads K-3; an instance without the Slack app reads K-2.
- [ ] Lena's test message lands in `#auth-dev`; AUTH-12's message lands within 10 s with key, title, actor, lane and Open in Foundry.
- [ ] Filing an issue with Slack unreachable succeeds in its usual time; the route row reads "Retrying"; the message arrives once Slack answers, also after a Foundry restart in between; after 24 h undelivered the route says so (US-EIP-02 scenarios 3, 4).
- [ ] Homelab Ops issues never reach the AUTH route.
- [ ] No Slack token in any page, log or answer.

## Dependencies

- Shipped: outbox `IssueCreated`, `TeamAccess`, uniform 404, CSRF, the board's create paths (web and API).
- OD-3, OD-6, OD-7, OD-12 answered (recommended answers assumed).
- A Slack HTTP double for CI (SC-10); a real Slack workspace for the dev dogfood.
- Measure the issue-create p95 baseline on the smoke lane before the slice starts.
