# Slice 05: Each member gets the project emails they chose, and can stop them

Stories: US-EIP-10, US-EIP-11 | Estimate: 3.5 days | job_id: `job-choose-my-project-emails` | Deliverable B

## Goal

Sam opens Project emails on `/account/settings`, chooses New comment on issues he
created or commented on in Identity Platform, and gets an email when Lena
comments on AUTH-7, saying why he got it. One click in that email stops that one
choice. When Tomás leaves Backend, his Identity Platform emails stop with nothing
changed in his settings.

## IN

- A Project emails section on `/account/settings`: the member's visible projects, the four v1 events, scope "All issues" or "Only issues I created or commented on", nothing on by default (OD-10), saved confirmation.
- One email per event (OD-8), never about the member's own action (OD-11), with the K-12 footer and D-3 content.
- Signed per-choice stop link (GET confirm, POST stop) and one-click unsubscribe headers.
- Access checked at send time (SC-5); the shipped workspace mute suppresses; security mail untouched (D-14); K-13 without an email transport.
- Asynchronous, retried delivery (OD-7) through the configured email transport.

## OUT

- Digests (F-4). Admin-configured group-address routes (Alternatives Considered).
- Any change to the five account notifications (SC-7).

## Learning Hypothesis

- **Disproves if it fails**: that "issues I created or commented on" is a useful default scope with only author and comment data (no watch or assignment exists). If members ask for "issues I care about" immediately, a watch concept is the real need.
- **Confirms if it succeeds**: Sam keeps at least one choice on for 30 days without stopping it (dogfood report).

## Acceptance Criteria

- [ ] Sam's choice produces "[AUTH-7] Lena Park commented: Move Grafana sign-in to Keycloak" within 2 minutes, footer K-12.
- [ ] No email about Sam's own comment; none while Canzan Labs is muted; none to Tomás after removal.
- [ ] The stop link stops one choice; the mail client's one-click does the same; a tampered link gets the uniform refusal.
- [ ] Sam sees only projects he can open; K-13 without email transport.
- [ ] A password reset still arrives after every project email is stopped.

## Dependencies

- **notification-preferences-ui finished** (D-9): its 12 scenarios un-pended and green, its CHANGELOG entry written.
- Slices 01-03 (delivery engine, history model). The shipped unsubscribe token design (recipient-notification-preferences ADR-001).
- OD-8, OD-10, OD-11 answered.
