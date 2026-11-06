# DISCUSS Decisions: instance-integration-setup

## Key Decisions

Fixed by the user (2026-11-05):

- [D0a] This is a new feature, starting in DISCUSS. It arrived through
  /nw-refactor, but it changes behaviour.
- [D1] Two keys (decision 2 as twice revised on 2026-11-05).
  - The **secrets key**, which seals credentials, is created and replaced by an
    instance admin in the UI. Foundry stores it in the database, encrypted
    under an **instance key** that never lives in the database.
  - A database dump alone opens nothing. ADR-EIP-004 / OD-12's goal is kept.
  - The instance key is an operator setting when present. A standalone deploy
    without one still never stores keys in plaintext (D6).
  - Every replica shares the same instance key. Losing it means saved
    credentials are reconnected or re-entered, and the UI says so (I-26, I-28).
  - This replaces the original "secrets key is env-only" decision, and the
    interim "key stored unprotected, accepted risk" note, which was dropped.
- [D0b] Matrix needs only the secrets key, so it is fully enabled from the UI.
- [D0c] Visibility is instance admins only (`require_instance_admin`), with the
  uniform 404 for everyone else.
- [D0d] Deployment independence (AGENTS.md). Foundry deploys standalone
  (compose, plain Kubernetes, single binary). Nothing in code, defaults, tests
  or docs names a particular hosting environment.

Approved by the user 2026-11-05 ("Accept all, review, then DESIGN"):

- [D2] **Approved by the user 2026-11-05.** Foundry generates the secrets key on
  a button press. Nobody types a key, which rules out weak hand-typed keys.
  Generated keys are 32 random bytes.
  - Refined (peer review, 2026-11-05): generating is atomic create-if-absent.
    Concurrent requests on any replicas store exactly one key (US-IIS-02 AC 1).
- [D3] **Approved by the user 2026-11-05.** No key is ever shown again: not the
  secrets key, the instance key, or the Foundry-made key file's contents. The
  page says "set". Backup is of the instance key (operator setting or file),
  never of a displayed value.
  - Refined (peer review): acceptance reads generated keys only through a
    feature-gated test seam (`integrations-test-seam`), absent from release
    builds, for the planted-string grep.
- [D4] **Approved by the user 2026-11-05.** Replacing the secrets key in the UI
  generates a new key and re-seals every stored credential all-or-nothing. No
  one reconnects, and the old key is discarded once all replicas use the new
  one. DESIGN may add a grace window like `FOUNDRY_SECRETS_KEY_PREVIOUS`.
  - When the instance key is lost, "Start over" (with its stated cost, I-28) is
    the only UI way forward. Restoring the instance key is the other.
  - Refined (peer review):
    - Replace is withheld while I-26 holds (the secrets key does not open).
    - In Deliverable A, I-26 is restore-only. Start over ships with US-IIS-07
      in Deliverable B.
  - Refined (approved by the user 2026-11-05, DESIGN upstream item 8a):
    - Start over is offered only when no live replica opens the secrets key,
      re-checked in the confirming POST.
    - The I-28 confirm also requires typing `START OVER` (I-28a).
- [D5] **Approved by the user 2026-11-05.** Existing instances: an existing
  `FOUNDRY_SECRETS_KEY` becomes the instance key (DESIGN names the setting).
  - Credentials sealed directly with it, or with `_PREVIOUS`, keep opening
    after the upgrade indefinitely, with no action.
  - Moving them under a UI-managed secrets key is an explicit button ("Move to
    a secrets key"), pressed after the roll. It is not automatic at first
    start, because a previous-release replica knows only the environment key.
  - An operator-set instance key always wins over a Foundry-made file.
  - Refined (peer review):
    - The legacy `FOUNDRY_SECRETS_KEY` is read as the instance key under
      whatever name DESIGN picks.
    - The operator key's format is 32 random bytes, base64url, padded or not. A
      malformed value refuses start with `health.startup.refused` naming the
      setting (US-IIS-08 AC 7).
- [D6] **Approved by the user 2026-11-05.** Zero-config fallback.
  - With no operator instance key, Foundry creates a random instance key file
    in its data directory (outside the database) on first start, readable only
    by its user, and reuses it on later starts.
  - The page names the file and says to back it up apart from the database,
    and that every server must share it (I-25).
  - With an unwritable directory the board still starts and the page says why
    (I-27).
  - A server whose key file differs says so (I-26) and never fails silently.
  - Refined (peer review):
    - The file is created create-if-absent, so servers starting together on a
      shared volume make exactly one.
    - When an operator key later replaces the file, the file's key is held as
      the previous instance key, so nothing is lost. The secrets key is
      re-sealed under the operator's key, and the page shows I-30.
    - I-4 names the remedy: the key that sealed the credentials is set as the
      previous instance key. Its scope is saved credentials.
- [D7] **Approved by the user 2026-11-05.** Slack values are validated on shape
  only (blank, whitespace, Slack's published format). There is no call to
  Slack on save. Slack proves them itself: Event Subscriptions "Verified" and
  the first Add to Slack.
  - Secrets are never echoed, not even their length, and the page shows
    "set".
  - On Replace, a blank secret field keeps the saved value, and the client ID
    is pre-filled.
- [D8] **Approved by the user 2026-11-05.** Where it lives:
  - A new page, `/admin/instance/integrations` ("Integrations for this
    Foundry"), linked from the instance dashboard (I-1).
  - For an instance admin, W-2 (Slack or Matrix) and W-3 on the workspace
    page and the route form are followed by "Set up integrations for this
    Foundry" (I-19), replacing "How to set up Slack".
  - A workspace admin who is not an instance admin keeps "How to set up Slack".
    That guide is reworded to say an instance admin enters the credentials, and
    no restart is needed (I-20).
  - Refined (peer review): I-19 is shown beside "How to set up Slack", not
    instead of it. integrations-wayfinding US-IW-02 AC 1 and KPI-IW-3 stay
    valid.
  - Refined (peer review): with `FOUNDRY_PUBLIC_URL` unset, the page shows I-29
    before the Slack addresses. `Integrations::slack_events_url` (new) is the
    events address's single source, beside `slack_return_url`.
- [D9] **Approved by the user 2026-11-05.** Environment precedence for Slack.
  - All three `SLACK_*` set means the environment's app is in effect, shown
    read-only (I-7).
  - Saving an app while the environment's is in effect is allowed and kept
    (I-17), so the operator can remove SLACK_* later with no gap.
  - A partial trio still refuses start.
  - No automatic import of SLACK_* into the database.
- [D10] **Approved by the user 2026-11-05.** Live effect.
  - Every change takes effect without a restart: on the serving replica at
    once, on every replica within 60 s.
  - While replicas catch up, no connection is marked lost and no admin is
    emailed (the existing `Transient(secret_unavailable)` posture).
  - The mechanism is DESIGN's.
  - Refined (peer review): cross-replica live reload is flagged as a DESIGN
    spike candidate. Today `Integrations` is built once at boot and swapped
    only in the harness.
- [D11] **Approved by the user 2026-11-05.** Slack app changes.
  - Same client ID (secret rotation): applies with no confirm. Existing
    connections are untouched.
  - Different client ID while workspaces are connected: a confirm with the
    count (I-14). Existing tokens keep posting until each workspace reconnects.
  - Remove: a confirm with the count (I-15). After removal, Slack reads W-2,
    events answer 401, connections are not marked lost and no email is sent,
    and deliveries wait under the 24 h give-up.
- [D12] **Approved by the user 2026-11-05.** Audit.
  - Every key and Slack app change is recorded with the actor, date and client
    ID (never a value), shown as "Activity" on the instance page (I-18).
  - It is not copied into each workspace's activity.
  - Storage is DESIGN's (`integration_audit_events` needs a workspace).
- [D13] **Approved by the user 2026-11-05.** `SLACK_API_BASE_URL` and
  `SLACK_AUTHORIZE_URL` stay environment-only test seams and are never shown.
- [D14] **Approved by the user 2026-11-05.** Locked copy (K-2, K-18, K-19, K-20,
  W-2, W-3) is unchanged.
  - W-3 ("an instance operator needs to set the secrets key") and Matrix's W-2
    remain true but now point at a UI action. For an instance admin, I-19
    bridges this.
  - Rewording them is left for a later copy pass.
- [D15] The walking skeleton is US-IIS-01 → 02 → 03, with an operator-set
  instance key. DISTILL tags US-IIS-02 scenario 1 `@walking_skeleton`.
  - Brownfield, but a new page, newly stored encrypted state and live
    multi-replica reconfiguration are new integration points (unlike
    integrations-wayfinding's "none").
- [D16] **Approved by the user 2026-11-05.** No new job.
  - Every story traces to `job-route-project-events-to-chat`, since instance
    setup is its prerequisite (step 1's K-2).
  - Alternative: add `job-instance-integration-setup` to jobs.yaml.
- [D17] **Approved by the user 2026-11-05.** Scope split. The feature is about
  12.5 days across 8 stories. That shows two oversized signals: more than 2
  weeks of effort, and two separable outcomes (turn on, and change safely).
  - Deliverable A: US-IIS-01, 02, 03, 06 and 08 (about 8.5 days).
  - Deliverable B: US-IIS-07, 04 and 05 (about 4 days).
  - Both stay in one feature directory.
  - Refined (peer review): A is self-contained.
    - I-26 is restore-only in A.
    - US-IIS-06's I-14 check (its AC 6) moves to B with US-IIS-05 AC 2.
  - Re-estimated: US-IIS-04 is 1 d, and US-IIS-08 is 2 d (adds the
    create-if-absent file, the operator-key takeover and the malformed-key
    refusal).
  - Totals: A = 1 + 2 + 2 + 2 + 2 = 9 d; B = 2 + 1 + 1.5 = 4.5 d; all = 13.5 d.

Review record: peer review needs_revision → fixed → approved, 2026-11-05.

Refinements from DISTILL upstream issues (2026-11-06, no new user decision):

- Issue 1: the planted-string checks no longer name `/api/v1/integrations`,
  which does not exist and is not in scope. The checks cover pages, page
  source, logs, activity and the database (US-IIS-02 AC 4, US-IIS-03 AC 3, the
  scenarios, KPI-IIS-G1).
- Issue 2: I-22 gains a one-credential form.
- Issue 6: brief.md's "held in the session" line for a pending Slack app is
  Deliverable B and is already superseded by ADR-IIS-003 r4 (a fenced
  `instance_slack_app_pending` table). No discuss/ change, and brief.md is left
  as is here.

## Requirements Summary

- Primary need: an instance admin turns on Matrix (one button) and Slack (three
  values) from `/admin/instance/integrations`, with no restart. On every
  deploy, keys stay encrypted with a key outside the database, and existing
  connections keep working.
- Walking skeleton scope: US-IIS-01, 02 and 03 (D15).
- Feature type: user-facing.
- Stories:
  - **US-IIS-01:** see the instance's integration state.
  - **US-IIS-02:** generate the secrets key.
  - **US-IIS-03:** enter the Slack app.
  - **US-IIS-04:** instance-admin links from W-2 and W-3.
  - **US-IIS-05:** replace or remove the Slack app.
  - **US-IIS-06:** upgrade and move without a gap.
  - **US-IIS-07:** replace the secrets key, or start over.
  - **US-IIS-08:** standalone instance key file.
- Scope Assessment: **borderline oversized.** 8 stories, about 3 modules
  (foundry-app instance admin and integrations, foundry-auth `SecretBox`, the
  store), about 13.5 days (A 9, B 4.5). Split approved by the user 2026-11-05
  (D17).
- Research depth: lightweight. No DIVERGE or DISCOVER artifacts exist. **Risk:**
  the need comes from the user's direct requests (2026-11-05), not interviews.
  The job was validated earlier by external-integration-plugins.

## Constraints Established

- A database dump alone opens no key or credential. The instance key never
  lives in the database.
- Keys are generated, never typed or shown. Slack secrets are write-only.
- Instance admins only, with the uniform 404 otherwise. Links are offered only
  to people the target answers.
- No restart, and every replica within 60 s. No connection is marked lost and
  no email is sent because of a key or app change.
- Environment `SLACK_*` win when set. An operator instance key wins over a
  Foundry-made file.
- Credentials sealed with an existing `FOUNDRY_SECRETS_KEY` keep opening after
  the upgrade.
- Foundry deploys standalone. Nothing names a particular hosting environment.
- The board never goes down for an integration key: degrade, never refuse
  start, except for a malformed operator instance key (existing refuse rule).
- Key creation (the secrets key and the key file) is create-if-absent.
- GET never changes state. Every POST carries `_csrf`.

## Upstream Changes

- **ADR-EIP-004 (secrets at rest), needs an amendment in DESIGN.**
  - Decision 1: the key now has two tiers, an instance key from a setting or
    file, and a stored, encrypted secrets key made in the UI.
  - Decision 3: new `seal` callers (the instance page) and a new credential
    kind with no workspace (the Slack app).
  - Decision 4: boot with a stored secrets key, plus the fallback file.
  - Decision 5: rotation in the UI, and `doctor rotate-secrets` scope.
  - OD-12's goal is preserved.
- **ADR-EIP-005 decision 1 and OD-3, need an amendment.** The Slack app is
  either environment settings validated at boot (unchanged, and winning) or
  saved by an instance admin and applied live.
- **ADR-IW-001 / `no_connection.rs`.** `NoConnectionCause::words` gains the
  viewer's instance-admin fact. I-19 is added beside existing links, so
  US-IW-02 AC 1 and KPI-IW-3 are unchanged.
- **Dependency on `FOUNDRY_PUBLIC_URL`** (existing, `main.rs`). Its absence is
  shown as I-29.
- **`slack.rs`:** add `Integrations::slack_events_url` and have
  `slack::manifest` use it (DESIGN).
- **SSOT journey `journey-route-project-events-to-chat`** (applied 2026-11-05
  to `docs/product/journeys/journey-external-integration-plugins.yaml`, with
  a changelog entry and a `slack_client_id` shared artifact). Insert step 0 "Set
  up integrations for this Foundry" before step 1. Step 1's failure mode K-2
  and step 7's Matrix "not set up" gain "fixable by an instance admin at
  /admin/instance/integrations", and a changelog entry dated 2026-11-05 is
  added.
- **`integration_audit_events`** cannot hold instance-level events
  (`workspace_id NOT NULL`, closed `action` CHECK). DESIGN decides.
- **Docs for DELIVER.**
  - RELEASING.md R-1, R-2 and R-5 (key creation, app setup and rotation now
    happen in the UI, plus the instance key).
  - `deploy/k8s/README.md` and `foundry-secret.example.yaml` (instance key, or a
    shared data volume).
  - `slack_setup.html` steps 3 and 4 (I-20).
- **None to DISCOVER or DIVERGE** (none ran).

## External Notes (optional, outside this repo)

- DEVOPS, optional: an operator's external configuration (for example, an
  infrastructure-as-code tool writing into the deployment's secret) may
  generate the instance key so nobody sees or types it. Any such change lives
  outside this repo. Foundry must not depend on it (D0d). Without it, D6's
  fallback applies.
