# Evolution: integrations-wayfinding (reach the integrations pages from the UI, and every "no connection" refusal names its real cause)

**Finalized**: 2026-11-05
**Commits**:
- All waves and DELIVER: `70c758a` (one commit at wave end, after `cargo xtask smoke`; the
  DES steps logged COMMIT as an approved skip under that rule).
- Release: `a36c207` (`chore(release): v0.18.2`), tag `v0.18.2`.

**Shipped**: v0.18.2, live on dev and prod on 2026-11-05 (footer `a36c207`).

Sources: `docs/feature/integrations-wayfinding/` (`discuss/`, `design/`, `devops/`, `distill/`,
`deliver/`), ADR-IW-001 and ADR-IW-002 in `docs/product/architecture/`.

## Why

On dev (v0.18.1) the only workspace admin opened a new Slack route and read K-4, "This workspace
has no chat connection yet. Ask a workspace admin to connect Slack or Matrix." The real cause was
that the instance had no Slack app, and the page offered no link. Nothing in the navigation led
to `/workspace/integrations`. The user also asked to configure a connection once and pick a
channel per project; that is already the model (one connection per workspace and kind, a channel
or room per project route), so the gap was finding it, not building it.

## What shipped

- Sidebar: Integrations for workspace admins on every app page, after Keyboard shortcuts.
- Route form: the refusal names its cause per kind, in a fixed order, and gives an admin the
  fixing link. A lead who is not an admin still reads K-4. Open, preview, save, Send test and
  edit all give the same answer for the route's own kind.
- Project integrations page: a line per workspace connection (connected W-6, lost, unreadable
  K-19), or W-7 with a connect link for an admin and K-4 for a lead when none exists.
- Bugs fixed on the way: a Matrix preview, Send test or edit rendered the Slack form; saving a
  Slack route with no connection said "cannot post to that channel"; a Matrix Send test after
  disconnect answered not found.
- No schema change, no new env var, no telemetry.

## Key decisions

- One pure cause function over the instance offers the workspace page already uses
  (`slack_offer`, `matrix_offer`) plus "has a connection", and one function from cause and role
  to words and link (`integrations/no_connection.rs`, ADR-IW-001). Workspace page and route form
  share the wording constants.
- The admin fact rides the identity query the nav already made (`Store::nav_identity`,
  ADR-IW-002) and the access check `managed_project` already made: no added query per page.
- A lost connection is still a connection: the form behaves as before (routes save paused).
- Links are offered only to people the target page answers (the eip-06 guardrail, KPI-IW-G1).

## Work completed

| Step | What | Result |
|---|---|---|
| 01-01 | Nav identity carries the admin fact; sidebar entry | RED → GREEN |
| 01-02 | Cause and words module; route form doors | RED → GREEN |
| 02-01 | Preview, save, Send test, edit apply the kind; Matrix refusal | RED → GREEN |
| 03-01 | Project page connection lines | RED → GREEN |

Gates: roadmap review (revised once, then approved); refactor L1-L6; adversarial review (1 high,
3 medium, 3 low, all fixed); mutation 100% effective (`no_connection.rs` 9/9; services and store
11/11, one survivor killed by acceptance when applied by hand); `des-verify-integrity` all 4 steps
traced; `xtask smoke` and full Docker `xtask ci` green (2072 scenarios).

Acceptance: integrations-wayfinding 54/54, external-integration-plugins 181/181, needs-matrix
38/38, navigation-bar 33/33.

## Issues and lessons

- An outline placeholder with a space (`<key change>`) is never substituted by cucumber-rs. Use
  one word.
- Excluding sidebar links from the "offered" count weakened eip-06's "not offered" check; the
  negative check must count every link, sidebar included (`all_link_hrefs`).
- The adversarial review found the doors DESIGN listed were not all the doors: Send test and edit
  had the same missing `of_kind` as preview. Enumerate every handler that renders a form.
- cargo-mutants in the `foundry-test` VM needs `TMPDIR` on disk (the 12 GB `/tmp` tmpfs fills
  up), and its `--test-package foundry-acceptance` baseline fails in the scratch copy, so
  acceptance-only kills are checked by hand.
- The eip lane needs the `foundry` binary built first, or restore scenarios fail with
  `CARGO_BIN_EXE_foundry` unset.

## Open

- KPI-IW-M is met: on 2026-11-05 the user confirmed on dev (v0.18.2) that a new Slack route on
  ABC reads "Slack is not set up on this Foundry yet…" with "How to set up Slack".
- On the project page, a lost or unreadable connection offers an admin no link (W-6 puts the link
  on the connected line only).
- Send test with no usable connection answers 422 (was 200), matching save.

## Artifacts

No files migrated: the journey, ADRs and KPI contracts were written to the SSOT
(`docs/product/journeys/journey-external-integration-plugins.yaml`, which the feature's journey
extends; `docs/product/architecture/adr-iw-00{1,2}-*.md`;
`docs/product/kpi-contracts.yaml`, `docs/product/outcomes/registry.yaml` OUT-36 and OUT-37) during
their waves. The feature directory is kept as delivery history.
