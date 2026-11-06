<!-- markdownlint-disable MD024 -->
# User stories: instance-integration-setup

Job: `job-route-project-events-to-chat` (docs/product/jobs.yaml). Slack and
Matrix routing work only after an instance has a secrets key, and for Slack, a
Foundry Slack app. Today both are environment settings that need a restart
(`config.rs` `slack_settings` and `secrets`; ADR-EIP-004 decision 1;
ADR-EIP-005 decision 1). With this feature an instance admin does all of it in
the browser and uses it at once.

Origin: a deployed instance (v0.18.2) with neither a secrets key nor a Slack
app shows W-2 for both Slack and Matrix on `/workspace/integrations`. Its only
user is both instance admin and workspace admin. The request arrived through
/nw-refactor. The user ruled on 2026-11-05 that it changes behaviour and so
starts in DISCUSS.

Delivery split (D17):

- **Deliverable A:** US-IIS-01, 02, 03, 06 and 08.
- **Deliverable B:** US-IIS-07, 04 and 05.
- Where a story in A depends on something B adds, the AC says "(Deliverable B)".

## System Constraints

- **Two keys; a database dump alone opens nothing** (user decision 2 as
  refined 2026-11-05; wave-decisions D1).
  - The **secrets key** seals credentials: the Slack app's secrets, and the
    Slack and Matrix connection tokens (`SecretBox` / `SealedSecret`,
    ADR-EIP-004). An instance admin creates and replaces it in the UI. Foundry
    stores it in the database, encrypted under the instance key.
  - The **instance key** encrypts the secrets key. It never lives in the
    database.
    - It comes from the operator setting `FOUNDRY_INSTANCE_KEY` when present
      (ADR-IIS-001). The legacy `FOUNDRY_SECRETS_KEY` is read as an alias.
      An instance on the legacy name, or with credentials sealed directly
      under a held instance key, stays in direct mode: I-2e / I-2e0 with Move,
      as today.
    - Otherwise it comes from `{FOUNDRY_DATA_DIR}/instance.key`, a file Foundry
      makes. `FOUNDRY_DATA_DIR` defaults to `/var/lib/foundry` (US-IIS-08, D6,
      ADR-IIS-002). Foundry never overwrites the file. An unreadable file
      shows I-31.
  - A **previous instance key** (`FOUNDRY_INSTANCE_KEY_PREVIOUS`, alias
    `FOUNDRY_SECRETS_KEY_PREVIOUS`, or a superseded Foundry-made file) is
    accepted for opening only.
  - Both the new and the legacy name set to different values refuses start,
    naming both settings. The same value under both counts as the legacy name.
  - Keys are never stored in plaintext. Integrations stay off until an instance
    key exists.
  - Every replica must use the same instance key. A replica whose instance key
    cannot open the stored secrets key says so (I-26). It never fails silently.
  - Losing the instance key means saved credentials must be re-entered or
    reconnected. The UI says so (I-26). The confirmed way to start over is
    Deliverable B (I-28).
- **Operator instance key format.** 32 random bytes, base64url, padded or not
  (RELEASING R-1's command makes one). A malformed value refuses start with
  `health.startup.refused` naming the setting, never its value. This is the
  existing ADR-EIP-004 decision 4 refuse rule. A missing value is not an error.
- **Deployment independence** (AGENTS.md).
  - Foundry always deploys standalone: plain docker compose, plain Kubernetes,
    or a single binary.
  - It depends on no particular hosting environment or external
    infrastructure repo, and nothing in code, defaults, tests or docs names
    one.
  - Every deploy stores keys encrypted, with or without an operator-set
    instance key.
- **Existing instances keep opening what they sealed.** Credentials sealed
  directly with an existing `FOUNDRY_SECRETS_KEY` keep opening after the
  upgrade, with no action and no gap. Moving them under a secrets key is an
  explicit, gapless step (US-IIS-06).
- **Keys are never shown.** Neither the secrets key nor the instance key is
  ever rendered, logged, returned, audited or put in debug output. Nobody
  types either one (D2). Generating is create-if-absent: two requests at once
  yield one key.
- **Matrix needs only the secrets key.** Once one exists, Matrix is ready, with
  no further instance setup.
- **Instance admins only** (user decision 4).
  - The page and its posts answer only a signed-in user with an
    `instance_admins` row (`instance_admin.rs` `require_instance_admin`).
  - Everyone else, signed out included, gets the uniform not-found page.
  - A link to the page is offered only to instance admins.
- **Slack secrets are write-only** (SC-3). The client secret and signing secret
  are never rendered (not even their length), logged, returned or audited. The
  client ID is shown.
- **No restart, every replica.**
  - A change made on the page takes effect without a restart. The serving
    replica applies it on its next request. Every other replica applies it
    within 60 seconds.
  - Deploys may run several replicas (the `deploy/k8s` example runs 3) and
    upgrade by rolling. A replica on the previous release reads only the
    environment.
  - No connection is ever marked lost, and no admin is emailed, because a
    replica has not yet caught up (ADR-EIP-004 decision 5 posture:
    `Transient(secret_unavailable)`).
  - DESIGN chooses the mechanism; it is a spike candidate (D10).
- **The environment wins for Slack.** All three `SLACK_*` set means the
  environment's app is in effect, shown read-only. A partial trio still
  refuses start. `SLACK_API_BASE_URL` and `SLACK_AUTHORIZE_URL` stay
  environment-only test seams.
- **Slack's addresses need `FOUNDRY_PUBLIC_URL`.** Unset, Foundry falls back to
  `http://localhost:{port}` (`main.rs`), which Slack cannot reach. The page
  says so (I-29).
- **The board never goes down for an integration key** (ADR-EIP-004 decision
  4). A key that cannot open degrades Slack or Matrix (I-4, I-13, I-26, I-31).
  It never refuses start, except for a malformed operator setting, or the new
  and legacy names set to different values.
- **Rotation covers everything sealed.**
  - Replacing the secrets key in the UI (US-IIS-07) re-seals every stored
    credential, the saved Slack app included. Nobody reconnects.
  - Rotating the instance key (operator, RELEASING R-5's order, with a previous
    key) re-encrypts only the secrets key. DESIGN adapts
    `foundry doctor rotate-secrets`.
- **Locked copy stays.** K-2, K-18, K-19, K-20, W-2 and W-3 keep their words.
  The integrations-wayfinding links stay: "How to set up Slack" for workspace
  admins (US-IW-02 AC 1, KPI-IW-3). New copy is I-n below, locked once
  approved.
- **No GET changes state.** Confirms are pages (GET) plus a POST (the
  ADR-ISSUE-DELETE-002 pattern). Every POST carries `_csrf`.
- **No new telemetry.** KPIs are measured by acceptance scenarios and a manual
  walk on a deployed instance.
- **Test seam.** Acceptance needs the generated secrets key, and a Foundry-made
  instance key, as planted strings for the "never shown" grep. That is a
  feature-gated test-support read under check-arch `integrations-test-seam`.
  It is absent from release builds.

## Impacted Journeys

- `journey-route-project-events-to-chat` (SSOT
  `docs/product/journeys/journey-external-integration-plugins.yaml`):
  - A new step 0, "Set up integrations for this Foundry", comes before step 1.
  - Step 1's failure mode K-2 (no Slack app) and step 7's Matrix "not set up"
    become fixable in the browser by an instance admin.
  - Step 2's W-2 and W-3 failure modes gain the I-19 link for instance admins,
    beside the existing links.
  - The feature journey is `journey-instance-integration-setup.yaml`. The SSOT
    edit was applied on 2026-11-05 (see wave-decisions.md, Upstream Changes).
- `journey-integrations-wayfinding`:
  - W-2 and W-3 gain I-19 for an instance admin. "How to set up Slack" stays
    for every workspace admin.
  - US-IW-02 AC 1 and KPI-IW-3 remain valid.

## Copy (locked once approved)

| ID | Where | Words |
|----|-------|-------|
| I-1 | instance dashboard `/admin/instance/workspaces` | `Integrations` (link to `/admin/instance/integrations`) |
| — | page title | `Integrations for this Foundry` |
| I-2 | secrets key stored | `Secrets key: set. Credentials entered here are sealed with it.` (Deliverable B adds `Replace secrets key`) |
| I-2e | direct mode, with saved credentials | `Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here.` then `Move to a secrets key` |
| I-2e0 | direct mode, nothing saved yet | `Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.` then `Move to a secrets key`. Move with n = 0 answers I-21, and the activity reads "generated the secrets key". |
| I-3 | no secrets key | `This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials.` then `Generate secrets key` |
| I-4 | some saved credentials do not open | `This Foundry cannot read some saved credentials: neither its instance key nor its previous instance key opens them. An instance operator sets the instance key they were sealed with as the previous instance key, and restarts Foundry.` |
| I-5 | no Slack app | `Slack: not set up. Create the Foundry Slack app in Slack from the manifest below, then enter its credentials.` Followed by the manifest, `Redirect URL {slack_return_url}` and `Events URL {slack_events_url}` |
| I-6 | Slack app saved here | `Slack: set up here · app {client_id} · saved by {name} · {date}`, `Client secret: set`, `Signing secret: set` (Deliverable B adds `Replace` and `Remove`) |
| I-7 | Slack app in the environment | `Slack: set in Foundry's environment, app {client_id}. To manage it here, enter the same app below; it takes over once SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from the environment.` |
| I-8 | Matrix with a secrets key | `Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page.` |
| I-9 | Matrix without a secrets key | `Matrix: needs the secrets key (above). Nothing else is set up for Matrix on this Foundry.` |
| I-10 | after a Slack save | `Saved. Workspace admins can now choose Add to Slack.` |
| I-11 | Slack save, blank field | `Enter the {field} from the Slack app's Basic Information.` |
| I-12 | Slack save, malformed field | `That is not a Slack {field}. Copy it again from the Slack app's Basic Information.` |
| I-13 | saved Slack app does not open | `Slack: saved, but this Foundry cannot read it (see the secrets key above).` |
| I-14 | Slack save with another client ID while connected (Deliverable B), on `GET /admin/instance/integrations/slack/confirm` | n ≥ 2: `{n} workspaces are connected through the current Slack app ({old_client_id}). They keep posting through it until a workspace admin reconnects Slack. Save the new app?` n = 1: `1 workspace is connected through the current Slack app ({old_client_id}). It keeps posting through it until a workspace admin reconnects Slack. Save the new app?` Then `Save the new app` |
| I-15 | Slack remove confirm (B) | `Remove the Slack app from this Foundry? {n} workspaces' Slack routes stop posting until it is set up again, and no workspace can connect Slack.` When n = 0: `Remove the Slack app from this Foundry? No workspace is connected to Slack.` |
| I-16 | after Slack remove (B) | `Removed. Slack is not set up on this Foundry.` |
| I-17 | Slack save while the environment sets the app | `Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used.` |
| I-18 | page Activity | `{date} · {name} generated the secrets key`; `… moved {n} credentials under a secrets key`; `… saved the Slack app ({client_id})`; system lines `{date} · Foundry re-sealed the secrets key under the operator's instance key`. B adds: `… replaced the secrets key ({n} credentials sealed again)`; `… started over with a new secrets key ({n} credentials discarded)`; `… replaced / removed the Slack app ({client_id})` |
| I-19 | W-2 (Slack or Matrix) or W-3, viewer is an instance admin (B) | `Set up integrations for this Foundry` (link to `/admin/instance/integrations`), shown after any existing link |
| I-20 | `/workspace/integrations/slack/setup` steps 3 and 4 (B) | `3. An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin › Integrations.` `4. No restart is needed. A workspace admin can then choose Add to Slack on the workspace integrations page.` Footnote: `An instance operator may instead set SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET in Foundry's environment and restart; the environment takes precedence.` |
| I-21 | after generating the key | `Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.` |
| I-22 | after moving | `Moved. {n} saved credentials are sealed under a new secrets key. Nobody needs to reconnect.` |
| I-23 | replace-key confirm (B) | `Replace the secrets key? Foundry seals its {n} saved credentials again under a new key. Nobody needs to reconnect.` then `Replace secrets key` |
| I-24 | after replacing the key (B) | `Replaced. {n} saved credentials are sealed under the new key.` |
| I-25 | instance key line | Operator-set: `Instance key: set by the operator.` Made by Foundry: `Instance key: made by Foundry in {path}. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected.` |
| I-26 | the instance key cannot open the stored secrets key | `This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used. An instance operator restores that instance key, or sets it as the previous instance key, and restarts Foundry.` Deliverable B appends `Start over`. |
| I-27 | no instance key, and Foundry cannot make one (also when the filesystem cannot hold a new key, e.g. no hard links) | `Foundry has no instance key and cannot make one ({path} cannot be written). An instance operator sets the instance key, or gives Foundry a writable data directory, and restarts Foundry.` |
| I-31 | the key file exists but cannot be read or parsed, and no operator key is set; replaces the instance key line | `This Foundry cannot read its instance key file ({path}). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry.` Restore-only in both deliverables. Generate and every credential form are withheld. |
| I-28 | start-over confirm (B) | `Start over with a new secrets key? The {n} saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again.` then `Start over` |
| I-28a | on the I-28 confirm, required before `Start over` (B) | `Type START OVER to confirm that no Foundry server of an earlier release is still running.` Field label: `Confirmation`. A wrong or empty value answers I-28 again and changes nothing. |
| I-29 | `FOUNDRY_PUBLIC_URL` unset, shown above I-5 | `FOUNDRY_PUBLIC_URL is not set, so the addresses below use {fallback_url}, which Slack cannot reach. An instance operator sets FOUNDRY_PUBLIC_URL to this Foundry's public address and restarts Foundry.` |
| I-30 | after the operator's instance key replaces Foundry's file | `Instance key: set by the operator. The secrets key, first sealed with {path}, is now sealed with the operator's key. Keep a backup of {path} until you are sure, then remove it.` |

`{field}` is `client ID`, `client secret` or `signing secret`. For the example
instance at `https://foundry.example.org`, `{slack_return_url}` is
`https://foundry.example.org/workspace/integrations/slack/return`
(`Integrations::slack_return_url`). `{slack_events_url}` is
`https://foundry.example.org/integrations/slack/events`, from a new
`Integrations::slack_events_url` built the same way from `slack::EVENTS_PATH`.
`slack::manifest` should use it too, so the page and the manifest have one
source. `{fallback_url}` is `http://localhost:{port}`.

### Scenario cast

- **Priya Raman** (persona-instance-operator): instance admin, and workspace
  admin of Canzan Labs.
- **Tomás Rivera**: workspace admin of Canzan Labs, not an instance admin. This
  role is inline for these scenarios.
- **Lena Park**: lead of Backend, not an admin.
- **Sam Okafor** (sam.okafor@canzan.dev): member of Backend, not a lead. He
  files the issues that prove delivery (SSOT journey step 4).
- **Marco** (persona-team-member-foil): signed in, on no Canzan Labs team, no
  admin role. Prior features give him no surname, and the persona file forbids
  inventing one.

Slack app example: client ID `6123456789012.7234567890123`, client secret
`3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8`, signing secret
`b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05`. These are planted test values.

Routes are proposals for DESIGN. Existing routes are marked "(shipped)". The
route-history port is
`GET /team/{team}/project/{project}/integrations/routes/{route_id}/history`
(shipped).

---

## US-IIS-01: See what this Foundry has and lacks for Slack and Matrix

job_id: job-route-project-events-to-chat · Deliverable A

### Problem

Priya is the instance admin of a self-hosted Foundry. Her workspace page says
Slack and Matrix are "not set up on this Foundry yet", and nothing in Foundry
says what that means or where to fix it. She has to read RELEASING.md R-1 and
R-2 to learn that the secrets key is missing and that Matrix needs nothing
else.

### Who

- Instance admin (`instance_admins` row) who runs the servers. Comfortable in a
  terminal, but resents it being the only route (persona).

### Elevator Pitch

- **Before:** the only explanation of "not set up" is RELEASING.md.
- **After:** Priya follows Instance admin › Integrations
  (`GET /admin/instance/integrations`). She sees:
  - where the instance key comes from (I-25), or why there is none (I-27);
  - the secrets key's state (I-2, I-2e, I-3 or I-26);
  - the Slack app's state, with the manifest, redirect URL and events URL
    (I-29 if they would be unreachable);
  - Matrix's state.
- **Decision enabled:** what to do first, here (generate the secrets key, or
  enter the Slack app), or what to hand the operator (the instance key, or
  `FOUNDRY_PUBLIC_URL`).

### Domain Examples

1. **Kubernetes with an operator-set instance key.** No secrets key and no app
   yet. Priya reads I-25 "set by the operator", then I-3 and I-9. No Slack form
   is offered yet.
2. **Secrets key set, no app.** Priya reads I-2, then I-5 with
   `https://foundry.example.org/workspace/integrations/slack/return`, the
   events URL and the manifest, then I-8.
3. **An instance configured before this feature.** FOUNDRY_SECRETS_KEY and
   SLACK_* are in its environment, and its connections are sealed directly
   with that key. Priya reads I-2e and I-7 naming app
   `6123456789012.7234567890123`. No secret or key appears.
4. **A single binary started without FOUNDRY_PUBLIC_URL on port 8080.** Priya
   reads I-29 naming `http://localhost:8080` above the addresses.
5. **Access.** Tomás, Lena, Sam, Marco and a signed-out visitor get the uniform
   not-found page.

### UAT Scenarios (BDD)

```gherkin
Scenario: Without a secrets key the page says integrations need one
  Given the operator set FOUNDRY_INSTANCE_KEY, and this Foundry has no secrets key and no Slack app
  When Priya opens the instance integrations page
  Then Priya reads "Instance key: set by the operator."
  And Priya reads "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials."
  And Priya is offered no Slack credentials form
  And Priya reads "Matrix: needs the secrets key (above). Nothing else is set up for Matrix on this Foundry."

Scenario: With a secrets key the page shows what to paste into Slack
  Given this Foundry at https://foundry.example.org has a secrets key and no Slack app
  When Priya opens the instance integrations page
  Then Priya sees the redirect URL "https://foundry.example.org/workspace/integrations/slack/return" and the events URL "https://foundry.example.org/integrations/slack/events"
  And the manifest shown equals the one on the Slack setup page
  And Priya reads "Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page."

Scenario: Unreachable Slack addresses are named before Priya copies them
  Given this Foundry runs on port 8080 without FOUNDRY_PUBLIC_URL and has a secrets key
  When Priya opens the instance integrations page
  Then Priya reads "FOUNDRY_PUBLIC_URL is not set, so the addresses below use http://localhost:8080, which Slack cannot reach."

Scenario: An instance configured before this feature is shown without its secrets
  Given the environment sets FOUNDRY_SECRETS_KEY, which seals Canzan Labs' Slack connection directly, and the Slack app 6123456789012.7234567890123 with client secret "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8"
  When Priya opens the instance integrations page
  Then Priya reads "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here."
  And Priya reads "Slack: set in Foundry's environment, app 6123456789012.7234567890123."
  And the page source contains neither key, nor "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8", nor the signing secret

Scenario: A missing or different instance key is named, never silent
  Given the stored secrets key was sealed with an instance key this server does not hold
  When Priya opens the instance integrations page
  Then Priya reads "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used."
  And Priya is offered neither "Generate secrets key" nor any way to save credentials

Scenario: The instance dashboard links to the integrations page
  When Priya opens the instance admin dashboard
  Then following "Integrations" lands on the instance integrations page

Scenario Outline: Only instance admins reach the page
  When <person> opens /admin/instance/integrations
  Then <person> gets <answer>
  Examples:
    | person               | answer                     |
    | Priya                | the page                   |
    | Tomás                | the uniform not-found page |
    | Lena                 | the uniform not-found page |
    | Sam                  | the uniform not-found page |
    | Marco                | the uniform not-found page |
    | a signed-out visitor | the uniform not-found page |
```

### Acceptance Criteria

1. `GET /admin/instance/integrations`: with no secrets key, the page shows I-3
   and I-9, and no Slack form. The instance key line is I-25, or I-27 when
   there is none.
2. `GET /admin/instance/integrations`: with a secrets key and no app, the page
   shows I-2, I-5 and I-8. The manifest and both URLs are equal to those on
   `GET /workspace/integrations/slack/setup` (shipped).
3. `GET /admin/instance/integrations`: with `FOUNDRY_PUBLIC_URL` unset, the page
   shows I-29 with the fallback URL, above the addresses.
4. `GET /admin/instance/integrations`: credentials sealed directly with the
   instance key show I-2e, and an environment Slack app shows I-7. No key,
   secret or secret length appears in the response.
5. `GET /admin/instance/integrations`: an instance key that cannot open the
   stored secrets key shows I-26, restore-only in Deliverable A. Generate and
   every credential form are withheld. A degraded probe over saved credentials
   shows I-4, and a saved app that does not open shows I-13. Under I-13,
   `GET /workspace/integrations` and the route form read W-2, because an
   unreadable app is not in effect. A key file that cannot be read shows I-31
   in place of the instance key line, restore-only, with Generate and every
   credential form withheld.
6. `GET /admin/instance/workspaces` (shipped) shows I-1.
7. `GET /admin/instance/integrations`: a caller without an `instance_admins`
   row, signed out included, gets the uniform not-found page. The states shown
   to an instance admin agree with what `GET /workspace/integrations`
   (shipped) offers.

### Outcome KPIs

KPI-IIS-2, KPI-IIS-G2.

### Technical Notes

- The page reads what `Integrations::slack_offer` and `matrix_offer` derive
  (`mod.rs`), plus where each setting comes from (environment, Foundry, file,
  none).
- `Integrations::slack_events_url` is new (beside `slack_return_url`), and
  `slack::manifest` should use it (DD-9).
- The check-arch `integrations-secret-seam` rule limits `open` to
  `integrations/` and `doctor`. The page's placement is DESIGN's choice.

Estimate: 1 day. Dependencies: `FOUNDRY_PUBLIC_URL` (existing setting; I-29
covers its absence).

---

## US-IIS-02: Turn on integrations by generating the secrets key

job_id: job-route-project-events-to-chat · Deliverable A

### Problem

Without a secrets key, neither Slack nor Matrix can be offered. Creating one
today means running an openssl command, editing Foundry's environment, and
restarting every server.

### Who

- Instance admin of an instance that has an instance key (operator-set, or
  made by Foundry) and no secrets key.

### Elevator Pitch

- **Before:** the secrets key exists only if an operator generates one with
  openssl, puts it in the environment and restarts.
- **After:** Priya chooses "Generate secrets key" on
  `/admin/instance/integrations` (`POST /admin/instance/integrations/secrets-key`).
  She reads I-21, and every workspace's integrations page offers the Matrix
  connect form within 60 seconds, with no restart.
- **Decision enabled:** connect Matrix now, or go on to enter the Slack app.

### Domain Examples

1. **Kubernetes, 3 replicas.** The operator set the instance key in the
   Kubernetes Secret. Priya generates the secrets key. Canzan Labs'
   `/workspace/integrations` then shows the Matrix connect form instead of
   "Matrix is not set up on this Foundry yet", on all replicas.
2. **A stolen backup.** Someone copies a database dump of the instance, which
   does not contain the instance key. Nothing in it opens: not the secrets key,
   not Canzan Labs' Matrix token.
3. **Two at once.** Priya double-clicks, and her request lands on two replicas
   at the same moment. Exactly one secrets key is stored, and both replicas use
   it.
4. **No instance key and none can be made.** The page shows I-27, and Generate
   is not offered.

### UAT Scenarios (BDD)

```gherkin
@walking_skeleton
Scenario: Generating the secrets key makes Matrix available at once
  Given this Foundry has an instance key and no secrets key
  When Priya chooses "Generate secrets key"
  Then Priya reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it."
  And within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers to connect Matrix on every replica

Scenario: Two generate requests at the same moment store one key
  Given this Foundry has two replicas, an instance key and no secrets key
  When a generate request reaches each replica at the same moment
  Then exactly one secrets key is stored
  And after Canzan Labs connects Matrix, both replicas open its token

@property
Scenario: A database dump alone opens nothing
  Given Priya generated the secrets key and Canzan Labs connected Matrix with bot token "syt_Zm91bmRyeQ_planted_token"
  When the database's contents are read without the instance key
  Then neither the secrets key nor "syt_Zm91bmRyeQ_planted_token" can be recovered from them

@property
Scenario: The secrets key is never shown
  Given Priya generated the secrets key
  Then the key appears in no page, page source, log line, activity line or API answer

Scenario: A later generate does not replace the key
  Given Priya generated the secrets key and Canzan Labs then connected Matrix
  When a second generate is posted
  Then Priya reads "Secrets key: set. Credentials entered here are sealed with it."
  And Canzan Labs' Matrix connection still opens

Scenario: Without an instance key, no secrets key is made
  Given this Foundry has no instance key and cannot make one
  When Priya opens the instance integrations page
  Then Priya reads the no-instance-key words and is not offered "Generate secrets key"

Scenario: Generating is recorded
  When Priya generates the secrets key on 5 Nov 2026
  Then the activity reads "5 Nov 2026 · Priya Raman generated the secrets key"
```

### Acceptance Criteria

1. `POST /admin/instance/integrations/secrets-key` with an instance key and no
   secrets key: Foundry generates a full-strength key (32 random bytes),
   stores it encrypted under the instance key, and answers I-21. Nobody types
   a key. Creation is create-if-absent: concurrent posts on any replicas
   store exactly one key, and every post answers the page's resulting state.
2. `GET /workspace/integrations` (shipped) offers the Matrix connect form at
   once on the serving replica, and on every replica within 60 seconds.
   `GET /team/{team}/project/{project}/integrations/routes/new?kind=matrix`
   (shipped) no longer reads W-2.
3. Observed through a direct database read in the acceptance harness (the dump
   is the attack surface, and there is no HTTP port): the stored values,
   without the instance key, yield neither the secrets key nor any credential
   sealed with it.
4. Observed through every response of `/admin/instance/integrations`,
   `/workspace/integrations`, the route form and `/api/v1/integrations`
   (shipped), plus the captured log stream and activity: the generated key,
   read through the feature-gated test seam, never appears.
5. `POST /admin/instance/integrations/secrets-key` changes nothing and answers
   the page's current state in each of these cases:
   - a secrets key exists;
   - there is no instance key (I-27 / I-31);
   - the instance is in direct mode (Move applies instead);
   - I-4 holds, meaning some saved credential opens under no key this server
     holds.

   `GET /admin/instance/integrations` offers Generate in none of those cases.
6. `GET /admin/instance/integrations` shows I-18 "generated the secrets key".
   After a process restart, the stored key is in effect and the boot key-check
   passes (`probe_failures_total{probe_name="secrets"} == 0`).

### Outcome KPIs

KPI-IIS-1, KPI-IIS-G1.

### Technical Notes

- `config.rs` `secrets` reads the key once from the environment today. How the
  stored key is held, wrapped and spread to replicas is DESIGN's choice.
  Cross-replica live reload is a spike candidate (D10).
- ADR-EIP-004 decisions 1 and 4 change, and the change needs an ADR amendment.
  OD-12's goal ("a dump alone yields nothing") is kept.
- The existing canary `integration_key_checks` should cover the stored key.

Estimate: 2 days. Dependencies: US-IIS-01, and an instance key: operator-set,
or US-IIS-08's. Acceptance sets one in the scenario settings.

---

## US-IIS-03: Enter the Slack app's credentials and Slack works at once

job_id: job-route-project-events-to-chat · Deliverable A

### Problem

To make Slack available, Priya copies three values from api.slack.com into
Foundry's environment and restarts every server (RELEASING R-2). A typo shows
up only as a server that refuses to start, or a failed Add to Slack.

### Who

- Instance admin with the Slack app's Basic Information open, on an instance
  that has a secrets key.

### Elevator Pitch

- **Before:** adding the Slack app means an environment edit and a restart.
- **After:** Priya enters Client ID, Client Secret and Signing Secret and saves
  (`POST /admin/instance/integrations/slack`). She reads I-10, and
  `/workspace/integrations` offers Add to Slack within 60 seconds, with no
  restart.
- **Decision enabled:** tell workspace admins (herself included) to connect
  Slack now.

### Domain Examples

1. **First app.** Priya saves `6123456789012.7234567890123` with its two
   secrets. Ten seconds later Canzan Labs' page shows Add to Slack on whichever
   of the 3 replicas answers.
2. **Typo.** A client ID of "…0123 copy" answers I-12 for `client ID`. The ID
   typed is kept, the secret fields are cleared, and nothing is saved.
3. **Slack verifies events.** Event Subscriptions' retry shows "Verified",
   using the saved signing secret.
4. **No secrets key.** A save posted directly answers I-3 and stores nothing.

### UAT Scenarios (BDD)

```gherkin
Scenario: An instance admin enters the Slack app and workspaces can add Slack at once
  Given this Foundry has a secrets key and no Slack app
  When Priya saves the Slack app with client ID "6123456789012.7234567890123", its client secret and its signing secret
  Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
  And the page reads "Client secret: set" and "Signing secret: set"
  And within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers Add to Slack on every replica

Scenario Outline: A blank or malformed value is refused and nothing is saved
  Given this Foundry has a secrets key and no Slack app
  When Priya saves the Slack app with <field> set to "<value>"
  Then Priya reads "<words>"
  And Slack is still not set up on this Foundry
  Examples:
    | field          | value                            | words                                                                                     |
    | client ID      |                                  | Enter the client ID from the Slack app's Basic Information.                                |
    | client ID      | 6123456789012.7234567890123 copy | That is not a Slack client ID. Copy it again from the Slack app's Basic Information.       |
    | signing secret | b7e1 d4c9                        | That is not a Slack signing secret. Copy it again from the Slack app's Basic Information.  |

@property
Scenario: The saved Slack secrets never leave Foundry
  Given Priya saved the Slack app with client secret "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8" and signing secret "b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05"
  Then neither value appears in any page, page source, log line, activity line or API answer

Scenario: Slack's install and event checks use the saved app
  Given Priya saved the Slack app 6123456789012.7234567890123
  When Slack sends its URL verification challenge signed with the saved signing secret
  Then Foundry answers the challenge
  And Add to Slack on Canzan Labs' workspace page sends Priya to Slack's approval for client ID 6123456789012.7234567890123

Scenario: Saving is recorded without the secrets
  When Priya saves the Slack app 6123456789012.7234567890123 on 5 Nov 2026
  Then the activity reads "5 Nov 2026 · Priya Raman saved the Slack app (6123456789012.7234567890123)"

Scenario: Nothing can be saved without a secrets key
  Given this Foundry has no secrets key
  When a Slack app save is posted for Priya anyway
  Then Priya reads "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials." and nothing is saved
```

### Acceptance Criteria

1. `POST /admin/instance/integrations/slack`: with valid values, the page
   answers I-10 and I-6. `GET /workspace/integrations` (shipped) offers Add to
   Slack at once on the serving replica, and on every replica within 60
   seconds.
2. `POST /admin/instance/integrations/slack`: a blank field answers I-11 and a
   malformed field answers I-12, naming the field. Nothing is saved, the client
   ID typed is kept, and the secret fields are empty.
3. Observed through every response of `/admin/instance/integrations` (GET and
   POST), `/workspace/integrations` and `/api/v1/integrations` (shipped), plus
   the captured log stream, the activity, and a direct database read: the
   planted secret values never appear, and are never stored in plaintext.
4. `POST /integrations/slack/events` (shipped) verifies with the saved signing
   secret. `GET /workspace/integrations/slack/install` (shipped) uses the saved
   client ID. `GET /workspace/integrations/slack/return` (shipped) exchanges
   the code with the saved client secret.
5. `GET /admin/instance/integrations` shows I-18 "saved the Slack app".
6. `POST /admin/instance/integrations/slack` with no secrets key answers I-3 and
   stores nothing. A non-instance-admin gets the uniform not-found page.

### Outcome KPIs

KPI-IIS-1, KPI-IIS-G1, KPI-IIS-G3.

### Technical Notes

- Shape check only, with no call to Slack on save (D7). DESIGN fixes the shapes
  from recorded Slack values.
- `Integrations::try_new` builds `SlackApp` once at boot. The harness already
  swaps `Integrations` through `IntegrationsHandle::rewire`. The cross-replica
  mechanism is DESIGN's choice (spike candidate, D10).
- `integration_audit_events` requires `workspace_id` and has a closed `action`
  list. The instance activity needs a home (DESIGN).

Estimate: 2 days. Dependencies: US-IIS-01, US-IIS-02.

---

## US-IIS-04: Workspace pages send an instance admin to the setup

job_id: job-route-project-events-to-chat · Deliverable B

### Problem

When Priya reads W-2 on her workspace page, the only link is "How to set up
Slack". That guide says to set environment variables and restart. W-2 for
Matrix and W-3 have no link, though she can now fix both in the browser.

### Who

- An instance admin who meets W-2 or W-3 on `/workspace/integrations` or on a
  new-route form.
- Secondary: a workspace admin who is not an instance admin.

### Elevator Pitch

- **Before:** W-2 links only to a guide that says "Restart Foundry". Matrix's
  W-2 and W-3 have no link.
- **After:** on `GET /workspace/integrations` or a new-route form, an instance
  admin sees W-2 or W-3 followed by the existing link, if any, and "Set up
  integrations for this Foundry", which links to
  `/admin/instance/integrations`.
- **Decision enabled:** fix it now, from where she noticed it.

### Domain Examples

1. **Priya on the workspace page of an instance with nothing set up.** W-2 for
   Slack with "How to set up Slack" and I-19. W-2 for Matrix with I-19.
2. **Tomás.** He keeps "How to set up Slack" only. The guide's step 3 now names
   Instance admin › Integrations.
3. **Lena on the route form.** W-2 with no link, as today.
4. **W-3 (environment Slack app, no secrets key).** Priya gets I-19. The page
   then offers Generate secrets key.

### UAT Scenarios (BDD)

```gherkin
Scenario: An instance admin is sent from the workspace page to the instance setup
  Given this Foundry has a secrets key and no Slack app
  When Priya opens the workspace integrations page
  Then Priya reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
  And Priya is offered "How to set up Slack"
  And following "Set up integrations for this Foundry" lands on the instance integrations page

Scenario: An instance admin is sent to the setup from a Matrix route form
  Given this Foundry has an instance key and no secrets key
  When Priya opens a new Matrix route for Identity Platform
  Then Priya reads "Matrix is not set up on this Foundry yet. An instance operator sets it up once."
  And Priya is offered "Set up integrations for this Foundry"

Scenario Outline: Each viewer is offered only links they can open
  Given this Foundry has a secrets key and no Slack app
  When <person> opens <page>
  Then <person> is offered <links>
  And following each offered link answers the page, not the not-found page
  Examples:
    | person | page                                    | links                                                          |
    | Priya  | the workspace integrations page         | "How to set up Slack" and "Set up integrations for this Foundry" |
    | Tomás  | the workspace integrations page         | "How to set up Slack" only                                     |
    | Lena   | a new Slack route for Identity Platform | no link                                                        |

Scenario: The Slack setup guide no longer asks for a restart
  When Tomás opens the Slack setup page
  Then step 3 reads "An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin › Integrations."
  And no step says "Restart Foundry"
```

### Acceptance Criteria

1. `GET /workspace/integrations` (shipped): for an instance admin, W-2 (Slack
   or Matrix) and W-3 are followed by any existing link ("How to set up Slack"
   for Slack W-2), then I-19. US-IW-02 AC 1 and KPI-IW-3 stay valid.
2. `GET /team/{team}/project/{project}/integrations/routes/new` (shipped), and
   its posts `.../routes/preview` and `.../routes`: the same rule per kind,
   with the same words for the same state.
3. `GET /workspace/integrations` and the route form: a workspace admin who is
   not an instance admin is offered "How to set up Slack" and not I-19. A lead
   who is not an admin is offered no link. Every offered link, followed
   (`GET /admin/instance/integrations`,
   `GET /workspace/integrations/slack/setup`), answers 200 to its viewer.
4. `GET /workspace/integrations/slack/setup` (shipped) shows I-20, and the
   manifest is unchanged.
5. `GET /workspace/integrations` and the route form: the words of W-2, W-3,
   K-2 and K-19 are unchanged.

### Outcome KPIs

KPI-IIS-3, KPI-IIS-G2.

### Technical Notes

- `NavContext.is_instance_admin` is already read on every page, so this adds no
  query. The link choice extends `NoConnectionCause::words`
  (`no_connection.rs`), whose locked table gains the instance-admin
  dimension.
- RELEASING.md R-1 and R-2 and `deploy/k8s/README.md` need rewording, as a
  docs change in DELIVER.

Estimate: 1 day (the route form and its preview and save, the `no_connection`
table, and the guide). Dependencies: US-IIS-01.

---

## US-IIS-05: Replace or remove the Slack app, knowing what it affects

job_id: job-route-project-events-to-chat · Deliverable B

### Problem

Slack lets the app owner regenerate the client secret or signing secret. Once
they do, Foundry's copy is stale, and events stop verifying. Priya also needs to
retire or switch the app. Today either change is a blind environment edit plus
a restart.

### Who

- Instance admin on an instance where workspaces connected Slack through the
  saved app.

### Elevator Pitch

- **Before:** changing or removing the Slack app gives connected workspaces no
  warning.
- **After:** Priya chooses Replace (`POST /admin/instance/integrations/slack`)
  or Remove (`GET`, then `POST /admin/instance/integrations/slack/remove`). She
  sees the number of connected workspaces and the effect on them before
  anything changes.
- **Decision enabled:** rotate now, switch apps, or remove Slack, knowing the
  cost.

### Domain Examples

1. **Rotated signing secret.** Priya enters only the new signing secret,
   leaving the other fields blank to keep them. There is no confirm, and events
   verify again within 60 seconds.
2. **Another app.** Canzan Labs, Ops Guild and Reading Club are connected.
   Saving `6123456789012.8888888888888` shows I-14 with n = 3.
3. **Remove.** I-15 with n = 3. A prefetch of the confirm page changes nothing.
   After she confirms, she reads I-16, and the workspace pages read W-2.
4. **Remove with no one connected.** I-15's n = 0 form.

### UAT Scenarios (BDD)

```gherkin
Scenario: Rotating a secret of the same app takes effect without a confirm
  Given Priya saved the Slack app 6123456789012.7234567890123
  When Priya replaces only its signing secret, leaving the other fields blank
  Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
  And within 60 seconds Slack's events verify with the new signing secret on every replica

Scenario: Switching to another app warns about connected workspaces
  Given Canzan Labs, Ops Guild and Reading Club are connected to Slack through app 6123456789012.7234567890123
  When Priya saves the app 6123456789012.8888888888888
  Then Priya reads "3 workspaces are connected through the current Slack app (6123456789012.7234567890123). They keep posting through it until a workspace admin reconnects Slack. Save the new app?"
  And nothing changes until Priya chooses "Save the new app"

Scenario: Removing the Slack app says what it stops before it does
  Given Canzan Labs, Ops Guild and Reading Club are connected to Slack through the saved app
  When Priya opens Remove on the instance integrations page
  Then Priya reads "Remove the Slack app from this Foundry? 3 workspaces' Slack routes stop posting until it is set up again, and no workspace can connect Slack."
  And Slack is still set up

Scenario: After removal, workspaces read "not set up" and nobody is told a connection was lost
  Given Canzan Labs is connected to Slack and Priya confirms Remove
  Then Priya reads "Removed. Slack is not set up on this Foundry."
  And Canzan Labs' workspace integrations page reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
  And Canzan Labs' Slack connection is not marked lost and no workspace admin is emailed

Scenario: Replace and remove are recorded
  When Priya replaces and later removes the Slack app 6123456789012.7234567890123
  Then the activity reads "Priya Raman replaced the Slack app (6123456789012.7234567890123)" and "Priya Raman removed the Slack app (6123456789012.7234567890123)"
```

### Acceptance Criteria

1. `POST /admin/instance/integrations/slack` (replace): the client ID is
   pre-filled.
   - With the same client ID, a blank secret field keeps the saved value. There
     is no confirm, and the change applies as in US-IIS-03 AC 1.
   - With a different client ID, both secrets are required, and a blank one
     answers I-11 (D7 narrowed, approved by the user 2026-11-05).
2. The same POST with a different client ID while Slack connections exist
   redirects to `GET /admin/instance/integrations/slack/confirm`, which shows
   I-14 with the count (the n = 1 form for one workspace).
   - The submitted app is held sealed for 10 minutes, so the confirm page holds
     no secret.
   - Only the same instance admin's `POST .../slack/confirm` saves it.
   - This applies equally when the current app is the environment's (US-IIS-06's
     deferred AC).
3. `GET /admin/instance/integrations/slack/remove` shows I-15 and changes
   nothing. `POST` of the same path removes the app and answers I-16.
4. After removal: `GET /workspace/integrations` (shipped) reads W-2, and
   `POST /integrations/slack/events` (shipped) answers 401. Connection states
   on `GET /workspace/integrations` are not lost, and the mail double receives
   no email. Pending deliveries on the route history wait under the 24-hour
   give-up.
5. `GET /admin/instance/integrations` shows I-18 "replaced" and "removed".
6. `GET /admin/instance/integrations` with the environment's Slack app in effect
   offers neither Replace nor Remove. `POST /admin/instance/integrations/slack/remove`
   then changes nothing and answers the page.

### Outcome KPIs

KPI-IIS-4.

### Technical Notes

- Bot tokens stay valid whatever app Foundry holds. Disconnect forgets the
  token whatever `apps.uninstall` answers (ADR-EIP-005 decision 7).
- Events from an old app fail verification under a new signing secret.
  Uninstall detection for those workspaces then falls back to refusals.

Estimate: 1.5 days. Dependencies: US-IIS-03.

---

## US-IIS-06: Upgrade an instance whose key and Slack app are in the environment, without a gap

job_id: job-route-project-events-to-chat · Deliverable A (AC 6 is Deliverable B)

### Problem

An instance configured before this feature has `FOUNDRY_SECRETS_KEY` sealing
its connections directly, and possibly SLACK_* in its environment. After the
upgrade, those connections must keep opening with no action. Moving to a
UI-managed secrets key and a saved Slack app must not open a window where
credentials cannot be opened or Slack is "not set up".

### Who

- Instance admin of an instance whose environment holds FOUNDRY_SECRETS_KEY and
  possibly SLACK_*. The operator, often the same person, edits the environment
  and restarts or rolls.

### Elevator Pitch

- **Before:** the upgrade would either break existing connections or keep the
  instance on environment settings forever.
- **After:** after the upgrade everything keeps working untouched (I-2e). When
  Priya is ready, she chooses "Move to a secrets key"
  (`POST /admin/instance/integrations/secrets-key/move`) and reads I-22. She
  saves the same Slack app (`POST /admin/instance/integrations/slack`, I-17).
  The operator can then drop SLACK_* with no gap.
- **Decision enabled:** when to move, and when to remove SLACK_* from the
  environment. Both are safe at any time.

### Domain Examples

1. **Upgrade.** Example Org's Foundry at https://foundry.example.org upgrades
   with 3 replicas rolling. Canzan Labs' Slack connection, sealed with
   FOUNDRY_SECRETS_KEY, keeps posting to #auth-dev throughout. The page reads
   I-2e.
2. **Move.** After the roll, Priya moves. She reads I-22 with n = 2 (Slack and
   Matrix connections), and nobody reconnects. FOUNDRY_SECRETS_KEY now serves
   as the instance key that wraps the new secrets key.
3. **Slack app.** Priya saves `6123456789012.7234567890123` and reads I-17. The
   operator removes SLACK_* and restarts, and the page reads I-6.
4. **Move pressed during the roll.** A replica still on the previous release
   cannot open the re-sealed credentials until it is replaced. Its deliveries
   wait (`Transient(secret_unavailable)`). No connection is marked lost, and no
   email is sent.

### UAT Scenarios (BDD)

```gherkin
Scenario: After the upgrade, connections sealed with the environment key keep working
  Given Canzan Labs' Slack connection was sealed with FOUNDRY_SECRETS_KEY before the upgrade
  When Foundry is upgraded and Sam files "Rotate LLDAP bind password" in Identity Platform
  Then the issue reaches #auth-dev
  And the instance integrations page reads "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here."

Scenario: Moving to a secrets key re-seals everything and nobody reconnects
  Given Canzan Labs' Slack and Matrix connections are sealed directly with FOUNDRY_SECRETS_KEY
  When Priya chooses "Move to a secrets key"
  Then Priya reads "Moved. 2 saved credentials are sealed under a new secrets key. Nobody needs to reconnect."
  And Sam's next new issue in Identity Platform reaches #auth-dev and #auth:canzan.dev

Scenario: While the environment sets the Slack app, a saved app is kept but not used
  Given the environment sets the Slack app 6123456789012.7234567890123
  When Priya saves the same app on the instance integrations page
  Then Priya reads "Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used."

Scenario: The saved app takes over when the environment's is removed
  Given Priya saved the same Slack app as the environment's and Canzan Labs has a Slack route to #auth-dev
  When the operator removes the three Slack settings and restarts Foundry
  Then the page reads "Slack: set up here · app 6123456789012.7234567890123"
  And Sam's next new issue in Identity Platform reaches #auth-dev
```

### Acceptance Criteria

1. Process start after the upgrade with FOUNDRY_SECRETS_KEY set (read as an
   alias of `FOUNDRY_INSTANCE_KEY`) and no secrets key stored:
   - every credential sealed with it, or with `FOUNDRY_SECRETS_KEY_PREVIOUS`,
     opens;
   - the route history
     (`GET /team/{team}/project/{project}/integrations/routes/{route_id}/history`)
     shows deliveries continuing;
   - `GET /admin/instance/integrations` shows I-2e, or I-2e0 when nothing is
     saved yet.
   - Renaming the setting to `FOUNDRY_INSTANCE_KEY` with the same value keeps
     I-2e and working deliveries.
2. `POST /admin/instance/integrations/secrets-key/move`:
   - generates a secrets key (create-if-absent, as US-IIS-02 AC 1) and stores
     it encrypted under the instance key;
   - re-seals every directly-sealed credential, all-or-nothing, and answers
     I-22. A failure changes nothing.
   - Activity I-18 reads "moved n credentials".
   - With n = 0, it answers I-21, and the activity reads "generated the
     secrets key".
   - Move is withheld while I-4 holds: not offered, and a post changes
     nothing.
3. During and after the move, connection states on `GET /workspace/integrations`
   are never lost, and the mail double receives no email. Every new-release
   replica's route history shows deliveries within 60 seconds.
4. `POST /admin/instance/integrations/slack` while all SLACK_* are set saves the
   app sealed and answers I-17. `GET /workspace/integrations/slack/install`
   (shipped) still uses the environment's client ID.
5. Process start with SLACK_* removed: `GET /workspace/integrations` offers
   Slack, `POST /integrations/slack/events` verifies with the saved signing
   secret, and a partial SLACK_* trio still refuses start.
6. (Deliverable B, with US-IIS-05 AC 2.) `POST /admin/instance/integrations/slack`
   with a client ID different from the environment's, while connections exist,
   answers I-14 first. Until B ships, Deliverable A saves the app as in AC 4,
   and the environment's app stays in effect.

### Outcome KPIs

KPI-IIS-5.

### Technical Notes

- The move is explicit, not automatic at first start (D5, approved by the user
  2026-11-05). This avoids re-sealing while previous-release replicas, which
  know only the environment key, are still serving.
- I-14 has an n = 1 form ("1 workspace is …"; see the Copy table).
- DISTILL sets `FOUNDRY_INSTANCE_KEY` in US-IIS-01 and 02 scenarios, and the
  legacy name only in this story's scenarios.

Estimate: 2 days. Dependencies: US-IIS-02, US-IIS-03. The I-14 part comes with
US-IIS-05 (Deliverable B).

---

## US-IIS-07: Replace the secrets key, or start over when it cannot be opened

job_id: job-route-project-events-to-chat · Deliverable B

### Problem

When Priya doubts the secrets key, she needs a new one without making every
workspace reconnect. When the instance key is lost and no backup exists, the
restore-only I-26 from Deliverable A leaves her stuck. She needs a confirmed way
forward that states the cost.

### Who

- Instance admin of an instance with a stored secrets key.

### Elevator Pitch

- **Before:** a secrets key cannot be replaced, and a lost instance key can only
  be restored.
- **After:** Priya chooses "Replace secrets key" (`GET`, then
  `POST /admin/instance/integrations/secrets-key/replace`) and reads I-23 then
  I-24. Nobody reconnects. Under I-26, the page instead offers "Start over"
  (`GET`, then `POST /admin/instance/integrations/secrets-key/start-over`) with
  its exact cost (I-28).
- **Decision enabled:** rotate the moment she doubts the key, or choose between
  restoring the instance key and starting over, knowing what each costs.

### Domain Examples

1. **Rotation.** Canzan Labs has Slack and Matrix connections, and the Slack
   app is saved: 3 credentials. Priya reads I-23 with n = 3, confirms, and
   reads I-24. Sam's next issue posts to #auth-dev and #auth:canzan.dev.
2. **Failure.** Re-sealing one credential fails, so nothing changes and the old
   key stays.
3. **Lost instance key, backup exists.** A docker compose volume was
   recreated, and `/var/lib/foundry/instance.key` is new. Priya reads I-26 with
   no Replace offered. She restores the file and restarts.
4. **No backup.** Priya chooses Start over and reads I-28 with n = 3. After she
   confirms, Matrix is ready. Canzan Labs' workspace page asks to reconnect
   Slack and re-enter the Matrix token, and the Slack app must be entered
   again.

### UAT Scenarios (BDD)

```gherkin
Scenario: Replacing the key re-seals everything and nobody reconnects
  Given this Foundry holds 3 saved credentials for Canzan Labs and the Slack app
  When Priya confirms "Replace secrets key"
  Then Priya reads "Replaced. 3 saved credentials are sealed under the new key."
  And Sam's next new issue in Identity Platform reaches #auth-dev and #auth:canzan.dev
  And no connection is marked lost and no workspace admin is emailed

Scenario: The confirm page changes nothing
  When Priya opens "Replace secrets key"
  Then Priya reads "Replace the secrets key? Foundry seals its 3 saved credentials again under a new key. Nobody needs to reconnect."
  And the key is unchanged

Scenario: A failed replacement changes nothing
  Given re-sealing one credential fails
  When Priya confirms "Replace secrets key"
  Then the old key stays in effect and every credential still opens

Scenario: Replace is withheld while the secrets key cannot be opened
  Given the stored secrets key cannot be opened with this server's instance key
  When Priya opens the instance integrations page
  Then Priya is not offered "Replace secrets key"
  And Priya is offered "Start over"

Scenario: Starting over states its cost first, then leaves workspaces told to reconnect
  Given the stored secrets key cannot be opened with this server's instance key
  When Priya chooses "Start over"
  Then Priya reads "Start over with a new secrets key? The 3 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again."
  And after Priya confirms, Canzan Labs' workspace integrations page offers to connect Matrix and does not read Slack as connected
```

### Acceptance Criteria

1. `GET /admin/instance/integrations/secrets-key/replace` shows I-23 with the
   count and changes nothing.
2. `POST /admin/instance/integrations/secrets-key/replace` generates a new key,
   stores it encrypted under the instance key, re-seals every credential and
   the key check all-or-nothing, and answers I-24. Activity I-18 reads
   "replaced". A failure changes nothing.
3. Observed on `GET /workspace/integrations` and the route history: within 60
   seconds every replica opens every credential. In between, no connection
   reads lost, and the mail double receives no email.
4. `GET /admin/instance/integrations` under I-26 offers Start over and not
   Replace.
   - Start over is offered only when no running replica opens the secrets key.
     While another live server holds the right instance key, I-26 stays
     restore-only. The gate is checked again inside the confirming POST.
   - Under I-31, Start over is never offered.
   - The I-28 confirm also requires typing `START OVER` (I-28a, approved by the
     user 2026-11-05). A wrong or empty value answers I-28 again and changes
     nothing. `GET`/`POST /admin/instance/integrations/secrets-key/replace` under
   I-26 changes nothing and answers the page.
5. `GET /admin/instance/integrations/secrets-key/start-over` is answered only
   under I-26, shows I-28, and changes nothing. The `POST` generates a new key,
   discards the credentials that cannot be opened, and records I-18 "started
   over".
6. After a start-over, `GET /workspace/integrations` (shipped) shows no
   discarded connection as connected, and tells its workspace admin to
   reconnect or re-enter it. Rotating the instance key itself (CLI
   `foundry doctor rotate-secrets`, adapted by DESIGN) leaves everything
   opening.

### Outcome KPIs

KPI-IIS-4, KPI-IIS-G1.

### Technical Notes

- The old secrets key is discarded once all replicas use the new one (D4,
  approved by the user 2026-11-05). DESIGN decides any grace window.
- "Start over" turns ADR-EIP-004 decision 6 ("losing the key loses the
  credentials, not the data") into an explicit, confirmed action.

Estimate: 2 days. Dependencies: US-IIS-02, US-IIS-08 (I-26's states).

---

## US-IIS-08: A standalone Foundry makes its own instance key

job_id: job-route-project-events-to-chat · Deliverable A

### Problem

Someone running Foundry from plain docker compose or a single binary sets no
instance key. Integrations must still work, and keys must still never be stored
in plaintext in the database. Today that deploy simply has no integrations
until the operator learns about the key.

### Who

- Instance admin of a standalone deploy with no operator-set instance key.
  Often the same person runs the server.

### Elevator Pitch

- **Before:** with no key setting, integrations are off and nothing says how to
  turn them on.
- **After:** on first start, Foundry makes an instance key in a file in its
  data directory. `GET /admin/instance/integrations` shows I-25 with the file's
  path and what to back up, and "Generate secrets key" works.
- **Decision enabled:** back up the file now, or switch to an operator-set
  instance key, losing nothing. With several servers, give them all the same
  file.

### Domain Examples

1. **Single binary.** First start with data directory `/var/lib/foundry` creates
   `/var/lib/foundry/instance.key`. Priya reads I-25 and generates the secrets
   key.
2. **Two compose replicas starting together on one shared volume.** Exactly one
   file is made, and both use it.
3. **Read-only filesystem.** The data directory cannot be written. Foundry
   starts and serves the board, and the page shows I-27.
4. **Operator key added later.** The operator sets an instance key that differs
   from the file's. The file's key is held as the previous instance key, so
   nothing is lost. Foundry re-seals the secrets key under the operator's key,
   and the page shows I-30.
5. **Typo in the operator key.** A 20-character value refuses start with
   `health.startup.refused` naming the setting. The old pod keeps serving.

### UAT Scenarios (BDD)

```gherkin
Scenario: A standalone Foundry makes its own instance key and says what to back up
  Given Foundry starts for the first time with data directory /var/lib/foundry and no instance key set
  When Priya opens the instance integrations page
  Then Priya reads "Instance key: made by Foundry in /var/lib/foundry/instance.key. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected."
  And Priya is offered "Generate secrets key"

Scenario: Replicas starting together on a shared volume make one key file
  Given two Foundry servers share /var/lib/foundry and neither has an instance key
  When both start at the same moment
  Then exactly one /var/lib/foundry/instance.key exists and both servers open the secrets key Priya generates

Scenario: The key file is reused, not remade, on restart
  Given Foundry made /var/lib/foundry/instance.key and Priya generated the secrets key
  When Foundry restarts
  Then Canzan Labs' Matrix connection still opens

Scenario: Without a writable data directory, the board serves and the page says why
  Given no instance key is set and /var/lib/foundry cannot be written
  When Priya opens the instance integrations page
  Then Priya reads "Foundry has no instance key and cannot make one (/var/lib/foundry/instance.key cannot be written)."

Scenario: A server with a different key file says so
  Given the secrets key was stored by a server whose instance key file differs from this one's
  When Priya opens the instance integrations page on this server
  Then Priya reads "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server."

Scenario: Moving from Foundry's key file to an operator key loses nothing
  Given Foundry made /var/lib/foundry/instance.key, Priya generated the secrets key, and Canzan Labs connected Matrix
  When the operator sets a different instance key and restarts Foundry
  Then Priya reads "Instance key: set by the operator. The secrets key, first sealed with /var/lib/foundry/instance.key, is now sealed with the operator's key."
  And Canzan Labs' Matrix connection still opens

Scenario: A malformed operator instance key stops start and names the setting
  Given the operator sets the instance key to a 20-character value
  When Foundry starts
  Then it refuses to start with a "health.startup.refused" line naming the instance key setting and not its value
```

### Acceptance Criteria

1. Process start with no instance key setting and a writable data directory:
   - Foundry creates a random instance key file, readable only by Foundry's
     user, create-if-absent, so servers starting together on a shared volume
     make exactly one;
   - it reuses the file on later starts;
   - `GET /admin/instance/integrations` shows I-25 with the path.
2. Observed through a direct database read in the acceptance harness: no
   instance key, secrets key or credential is ever stored in plaintext (as
   US-IIS-02 AC 3).
3. Process start with no writable data directory: the process starts,
   `GET /readyz` answers 200, `GET /admin/instance/integrations` shows I-27,
   and `GET /workspace/integrations` offers no integration. If the file exists
   but cannot be read or parsed, the page shows I-31 instead, and Foundry never
   overwrites the file.
4. `GET /admin/instance/integrations` on a server whose instance key cannot open
   the stored secrets key shows I-26. `GET /readyz` answers 200. Connection
   states on `GET /workspace/integrations` do not read lost.
5. Process start with an operator instance key that differs from the
   Foundry-made file that sealed the secrets key:
   - the file's key is read as the previous instance key, and everything
     opens;
   - the secrets key is re-sealed under the operator's key;
   - `GET /admin/instance/integrations` shows I-30 and I-18 "Foundry re-sealed
     the secrets key under the operator's instance key".
6. Process start with `FOUNDRY_INSTANCE_KEY` set, and no file in use: the
   operator key is used, and `GET /admin/instance/integrations` shows I-25
   "set by the operator".
7. Process start refuses with `health.startup.refused` in two cases:
   - a malformed operator instance key (not 32 bytes after base64url
     decoding), naming the setting actually used and never its value;
   - `FOUNDRY_INSTANCE_KEY` and `FOUNDRY_SECRETS_KEY` set to different values,
     naming both settings.

### Outcome KPIs

KPI-IIS-6, KPI-IIS-G1.

### Technical Notes

- ADR-IIS-002 settles the setting: `FOUNDRY_DATA_DIR` (default
  `/var/lib/foundry`), file `instance.key`, mode 0600. Container volume
  defaults are DEVOPS's choice.
- Replicas pick up changes on a 5 s poll (`FOUNDRY_INTEGRATIONS_RELOAD_MS`,
  ADR-IIS-004). Acceptance shortens it, and the 60 s bound stays as written.
- When the re-seal under the operator's key happens (at boot, or on first
  page view) is DESIGN's choice. Re-sealing one stored key is cheap, and every
  replica holds both keys.
- The docker compose and Kubernetes examples should document the volume and
  that every replica shares it, or should set the instance key instead.

Estimate: 2 days. Dependencies: US-IIS-02.
