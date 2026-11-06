# ADR-IIS-003: An instance admin saves the Foundry Slack app; the environment's app still wins when all three settings are set

## Status

Proposed (instance-integration-setup DESIGN, 2026-11-05). Option 2-A was chosen by the user on
2026-11-05 (`docs/feature/instance-integration-setup/design/options.md`). Supersedes ADR-EIP-005
decision 1 / OD-3 in part: the environment trio is no longer the only source.

## Context

- D7: shape-only validation with no call to Slack on save. Secrets are never echoed. On Replace, a
  blank secret field keeps the saved value.
- D9:
  - all three `SLACK_*` set means the environment's app is in effect, shown read-only (I-7);
  - saving while it is in effect is allowed and kept (I-17);
  - a partial trio still refuses start;
  - there is no import.
- D11: same client ID applies with no confirm. A different client ID while workspaces are
  connected is confirmed with the count (I-14). Remove is confirmed (I-15). After removal, events
  answer 401, nothing is marked lost and no email is sent.
- D13: `SLACK_API_BASE_URL` and `SLACK_AUTHORIZE_URL` stay environment-only test seams.
- SC-3: secrets are never rendered, logged, returned or audited.

## Decision

1. **Storage.**
   - Single-row table `instance_slack_app`. Columns: `id = 1`, `client_id`,
     `sealed_client_secret`, `sealed_signing_secret`, `saved_by`, `saved_at`.
   - Each secret is sealed with the credential box. Slot: `instance_slack_app|1|{column}`.
   - Saving needs a sealing mode (`Direct` or `Sealed`, ADR-IIS-001). Otherwise the POST answers
     I-3 and stores nothing.
2. **The app in effect.**
   - The environment if all three are set, else the saved app if both secrets open, else none.
   - A saved app that does not open is not in effect. The instance page shows I-13, and workspace
     pages read W-2 (`SlackOffer::NotSetUp`), so ADR-IW-001's closed cause set is unchanged.
   - The test-seam URLs apply to whichever app is in effect.
3. **Shapes** (DELIVER confirms them against recorded Slack values):
   - client ID: digits, a dot, digits;
   - client secret: 32 lowercase hex;
   - signing secret: 32 lowercase hex.

   Leading and trailing whitespace is trimmed. Any other character answers I-12 for that field. A
   blank required field answers I-11.

   Validation is a pure function, table-tested. On refusal (US-IIS-03 AC 2):
   - nothing is written, and neither the lock nor the store is reached;
   - the form re-renders with the typed client ID kept and both secret fields empty;
   - the message names the first failing field in form order.
3a. **One source for Slack's addresses** (US-IIS-01 AC 2, DD-9).
   - `Integrations::slack_events_url` is built from `slack::EVENTS_PATH` exactly as
     `slack_return_url` is built from `RETURN_PATH`.
   - `slack::manifest` takes both URLs from `Integrations` instead of composing them, and the
     instance page and `/workspace/integrations/slack/setup` both render that one manifest.
   - A unit test asserts the instance page's manifest and URLs equal the setup page's for the same
     `Integrations`.
4. **Replace (B).**
   - Same client ID: a blank secret keeps the saved value, and the save applies at once.
   - A different client ID needs both secrets (blank answers I-11) because they belong to the other
     app. With Slack connections present it answers I-14 and stores nothing.
   - The submitted app is held in the single-row `instance_slack_app_pending` table. Columns:
     `client_id`, the two sealed secrets, `requested_by` and `expires_at` (10 minutes).
   - It is a sealed value like any other. Every sealed-value set includes it: the seal fence,
     Replace's re-seal, the boot probe and Start over's discard.
   - It is not kept in the session table, which would sit outside the fence and the re-seal.
   - Writing the pending row is a sealed write: it takes the instance-integrations lock and fences
     on the sealing key id (ADR-IIS-004 rule 2). It is in the `integrations-seal-fence` list, with a
     gold test.
   - The confirming `POST` (`…/slack/confirm`, "Save the new app") applies it only for the same
     instance admin before it expires (judged by the injected `Clock`), then deletes it. A new
     submission overwrites it.
   - The worker's retention tick deletes expired pending rows.
   - Secrets are never written into the confirm page.
   - This applies equally when the current app is the environment's (US-IIS-06 AC 6).
5. **Remove (B).** `GET …/slack/remove` shows I-15 and changes nothing. The `POST` deletes the row,
   records the activity line and bumps the version. Connections keep their state. Deliveries meet
   no app and fail transiently under the 24 h give-up.

   **With the environment's app in effect** (US-IIS-05 AC 6):
   - the page offers neither Replace nor Remove;
   - `GET` and `POST …/slack/remove` change nothing: no row deleted, no activity, no version bump;
   - both answer the page.

   The decision is a pure function of the app in effect, and the POST re-checks it inside its
   locked transaction. A gold test posts Remove with all three `SLACK_*` set and a saved row
   present, then asserts the row, the activity and the version are unchanged.
6. **Events and install** read the app in effect per request through `IntegrationsHandle::current()`.
   - Events signed with an old signing secret answer 401.
   - An install started under the old app and returning after a switch fails at Slack's exchange
     and shows K-3.
7. **Every change** runs under the instance-integrations lock, records I-18 (client ID only) and
   bumps the config version, in one transaction (ADR-IIS-004).

## Alternatives considered

- **Generic `instance_settings` key/value table** (2-B). Rejected: "all or none" and "ID plain,
  secrets sealed" become conventions instead of constraints.
- **A connection row with a NULL workspace** (2-C). Rejected: `workspace_id NOT NULL`, one per
  kind, and connection states do not describe an app.
- **Hold the pending app sealed in the session.** Rejected at review: the session table sits
  outside the seal fence and Replace's re-seal, so ciphertext under a discarded key would persist.
- **Confirm by re-entering the secrets on the I-14 page.** Rejected: it doubles the paste of two
  secrets for a rare action. The pending row keeps them write-only.

## Consequences

- Positive: Slack is turned on in the browser with no restart, and the environment path still
  works untouched (KPI-IIS-G3).
- Negative: two sources of truth for the app. The page always names the one in effect (I-6, I-7,
  I-17).
- External integration: no new Slack API call. The ADR-EIP-005 contract-test recommendation stands.
