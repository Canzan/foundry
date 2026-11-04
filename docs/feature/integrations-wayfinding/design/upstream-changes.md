# Upstream changes for the product owner — integrations-wayfinding DESIGN

Status: all three were accepted by the user on 2026-11-03 and applied to
discuss/user-stories.md.

## US-IW-01 AC 3: drop the "current page" marking for W-1

Original (discuss/user-stories.md): "On `/workspace/integrations` and the pages
below it, W-1 is marked as the current page (`aria-current="page"`, active
style), as Home and Board are."

Change: W-1 is not marked as the current page. Home stays current on those
pages, as it does on every other page reached from the user section.

Rationale:
- In navigation-bar-linear-ui, `aria-current` belongs to the two primary items
  only, and exactly one of them is current on every page (FR-4: never zero,
  never two).
- Settings, Keyboard shortcuts and Instance admin, the user-section items W-1
  sits beside, are never marked.
- Marking W-1 would either leave no primary item current or mark two items
  (ADR-IW-002).

Effect on KPIs: none. KPI-IW-3 counts pages that offer W-1, not pages that
mark it.

## US-IW-02 AC 5: the "Slack or Matrix" phrase rule covers the new wording only

Original: "The message names only the form's kind. A Slack form never says
'Slack or Matrix'."

Change: W-2 to W-4 name only the form's kind. K-4, shown to a lead who is not
an admin (W-5), keeps its locked wording, which says "Slack or Matrix"
(DISCUSS D3). The AC reads: "W-2 to W-4 name only the form's kind."

## US-IW-02 AC 6: K-19's precedence follows each kind's offer order

Original: "K-19 (saved connection unreadable) is unchanged and still takes
precedence when a connection exists."

Change: with no Slack app on the instance, W-2 shows even when a Slack
connection is saved and unreadable. The workspace page does the same
(`slack_offer` checks `NotSetUp` before `Unavailable`, `mod.rs:251`), and the
two pages must agree (KPI-IW-2). Matrix keeps K-19 first (`matrix_offer`).
The AC reads: "When a connection exists, K-19 takes precedence over W-3 and
W-4, and over Matrix's W-2; Slack's W-2 (no Slack app) takes precedence over
K-19."
