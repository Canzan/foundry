# DISCUSS Decisions — integrations-wayfinding

## Key Decisions
- [D1] Keep one connection per workspace per kind, and a channel or room per
  project route. The user confirmed this on 2026-11-03: the "configure once,
  choose channel per project" request is already the model, so the gap is
  discoverability (see: user-stories.md, System Constraints).
- [D2] Offer a link only to people the target page answers, as eip-06 does:
  workspace admins for workspace pages, leads and admins for project pages.
  This keeps the guardrail that no one is offered a link that leads to a 404
  (see: outcome-kpis.md KPI-IW-G1).
- [D3] Keep K-4 word for word for a lead who is not an admin. New wording
  W-1 to W-8 goes to admins and to instance-level causes (see:
  user-stories.md, Copy).
- [D4] Route form cause precedence, worded as the workspace page already
  words them.
  - Slack: no app, then an unreadable saved connection (K-19), then no
    secrets key, then not connected.
  - Matrix: an unreadable saved connection (K-19), then no secrets key (shown
    as "not set up", since Matrix has no instance app), then not connected.
  - The K-19 positions follow each kind's offer order (US-IW-02 AC 6, refined
    in DESIGN and accepted 2026-11-03).
  - K-19 is unchanged. Found in review: `MatrixOffer` has no "Matrix enabled"
    flag (`workspace.rs:260`).
- [D5] Put the sidebar entry in the user section, after "Keyboard shortcuts"
  and before "Instance admin", following the admin-only "Instance admin"
  pattern.

## Requirements Summary
- Primary need: a workspace admin who meets a "no connection" refusal learns
  the real cause and reaches the fix in one click, and anyone on a project's
  integrations page sees that routes reuse the workspace connection.
- Walking skeleton scope: none (brownfield, three thin independent slices).
  DISTILL tags one scenario `@walking_skeleton` (the sidebar link to the
  workspace page) as the feature's end-to-end demo path; it does not add a
  slice, so this decision stands.
- Feature type: user-facing.

## Constraints Established
- No schema change, and no new telemetry.
- Each link is shown according to the access rule of the page it points to.
- The route form, preview and save show the same refusal words for the same
  state.

## Risks
- No DISCOVER or DIVERGE wave ran. The need comes from one user's direct
  reports on dev (2026-11-02, 2026-11-03), and the job was already validated
  by external-integration-plugins.

## Upstream Changes
- None to DISCOVER (none ran).
- The SSOT journey `journey-route-project-events-to-chat` step 2's failure mode
  "The workspace has no chat connection (K-4)" is refined into W-2 to W-5
  (changelog entry added).
