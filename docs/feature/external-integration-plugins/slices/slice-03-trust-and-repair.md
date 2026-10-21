# Slice 03: The team can trust the channel when Slack misbehaves

Stories: US-EIP-05, US-EIP-06, US-EIP-07 | Estimate: 4 days | job_id: `job-route-project-events-to-chat` | Deliverable A (last slice)

## Goal

A missing message has an answer on the route's history page. Rate limits and
outages fix themselves within 24 hours, in order. When a Slack admin removes the
Foundry app, Priya hears about it within 5 minutes and one reconnect resumes
everything. Lena can pause, edit and delete routes, and Priya can disconnect,
each with a counted confirm.

## IN

- Per-route history (last 100 attempts, 30 days), plain reasons, attempt counts.
- Retries with backoff for up to 24 h, honouring "retry after"; permanent refusals not retried; per-issue order kept per route (OD-7).
- Connection-level versus route-level refusals: K-10, "Paused: connection lost", "Needs attention: {reason}"; one admin email (new suppressible account event, D-15).
- Reconnect (same Slack workspace resumes and drains up to 24 h; a different one changes nothing).
- Pause and resume (skips, D-8), edit, delete (GET dialog plus POST confirm), disconnect (counts routes and projects, withdraws access at Slack).
- Project delete pauses, restore resumes (D-8). Audit for every change (D-16).

## OUT

- Matrix's own refusals (slice 04, reusing this slice's model).
- Alerting outside email (the operator's Grafana gets the bounded counters, SC-9).

## Learning Hypothesis

- **Disproves if it fails**: that Slack's refusals can be classified into connection-level and route-level reliably from its answers alone. If not, every failure must surface at both levels and the admin email becomes noisy.
- **Confirms if it succeeds**: a forced uninstall at the dogfood is noticed and fixed by Priya in under 10 minutes with no event older than 24 h lost.

## Acceptance Criteria

- [ ] History lists each attempt; a 30 s "retry after" is honoured; a 2-hour outage delivers afterwards in order; 24 h gives up visibly.
- [ ] Uninstall pauses all routes on the first refusal and emails each workspace admin within 5 minutes; reconnect resumes and delivers missed events.
- [ ] An archived channel affects only its route.
- [ ] Pause skips; delete and disconnect confirm with counts; disconnect withdraws access.
- [ ] A deleted project sends nothing; a restore resumes.

## Dependencies

- Slices 01, 02. The Slack double must script `ratelimited`, `token_revoked`, `account_inactive`, `channel_not_found`, `is_archived`, `not_in_channel` and outages (SC-10).
- OD-7 answered.
