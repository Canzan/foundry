# Feature: instance-integration-setup, Deliverable A (US-IIS-01, 02, 03, 06,
# 08) — an instance admin turns on Matrix and Slack from the browser, with no
# restart, on any standalone deploy; no key or secret is ever shown or stored
# in plaintext, and instances configured before this feature keep working.
#
# Copy I-n is locked in docs/feature/instance-integration-setup/discuss/
# user-stories.md. Deliverable B is in this file too, at the end: US-IIS-04
# (I-19: the workspace pages and the route form send an instance admin to the
# setup; I-20: what the Slack setup guide tells her to do), and US-IIS-05's
# replace - the saved app's own secrets rotated and another app entered whole
# (I-11, the I-18 "replaced" line). The rest of US-IIS-05 (the I-14 confirm
# while workspaces are connected, Remove, the environment-app rules) and
# US-IIS-07 (Start over) are not here yet; where A shows a state B extends
# (I-26), A's scenario checks the restore-only form.
#
# Harness (DEVOPS D8, brief.md "For Acceptance Designer"): the production
# router in process (InProcHarness), real Postgres, Slack as the in-process
# double. The operator's settings (FOUNDRY_INSTANCE_KEY, the legacy
# FOUNDRY_SECRETS_KEY, FOUNDRY_DATA_DIR in a temp directory per scenario,
# FOUNDRY_INTEGRATIONS_RELOAD_MS=200, FOUNDRY_PUBLIC_URL, a Slack app given as
# settings) reach the app through the scenario lookup; "restarts" boots it
# again through the same path. @iis-replica: two replicas on one database
# (MultiReplicaHarness, a shared MockClock advanced while waiting, each
# replica asked directly). @iis-subprocess: the prebuilt foundry binary (key
# file, servers starting together, refused starts, alert inputs on reload).
#
# Scenario names: the temp data directory reads as /var/lib/foundry, and an
# in-process app's own fallback address as http://localhost:8080. A Slack
# connection stands for "a saved credential" where the stories say Matrix:
# connecting Matrix needs the Synapse container, which this lane does not run
# (DEVOPS D8; see distill/upstream-issues.md).
#
# DISTILL 2026-11-06 scaffolded every scenario RED as @pending (ADR-025);
# docs/feature/instance-integration-setup/distill/red-classification.md records
# why each fails today. DELIVER un-pends them story by story.

@instance-integration-setup @real-io
Feature: An instance admin turns on Slack and Matrix for this Foundry in the browser

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Tomás Rivera | admin          |                               |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Marco        | member         |                               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And it is 14:02 on 21 Oct 2026
    And Priya Raman is this Foundry's instance admin

  # ------------------------------------------------------------ walking skeleton

  # US-IIS-01 -> 02 -> 03 -> 05 with an operator-set instance key (DISCUSS D15):
  # the one journey through this Foundry's setup, extended by each story that
  # adds to it. Slack regenerates a secret the day after, and the rotation is
  # the last leg.
  @walking_skeleton @driving_port @iis-replica @us-iis-01 @us-iis-02 @us-iis-03 @us-iis-05 @kpi-iis-1 @kpi-iis-2 @kpi-iis-4
  Scenario: An instance admin turns on Matrix and Slack and rotates a Slack secret, and every replica follows without a restart
    Given this Foundry runs as two replicas with its instance key set by the operator
    When Priya opens the instance integrations page
    Then Priya reads "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials."
    When Priya chooses "Generate secrets key" on the instance integrations page
    Then Priya reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it."
    And within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers to connect Matrix on every replica
    When Priya saves the Slack app with client ID "6123456789012.7234567890123", client secret "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8" and signing secret "b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers Add to Slack on every replica
    When Priya saves the Slack app with client ID "6123456789012.7234567890123", client secret "" and signing secret "0f1e2d3c4b5a69788796a5b4c3d2e1f0"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And within 60 seconds, without a restart, every replica verifies Slack's events signed with the signing secret now in effect

  # ------------------------------------------------------------------ US-IIS-01

  @us-iis-01 @driving_port @kpi-iis-2
  Scenario: Without a secrets key the page says integrations need one
    Given this Foundry starts with its instance key set by the operator
    When Priya opens the instance integrations page
    Then Priya reads "Instance key: set by the operator."
    And Priya reads "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials."
    And Priya is offered "Generate secrets key"
    And Priya is offered no way to enter the Slack app
    And Priya reads "Matrix: needs the secrets key (above). Nothing else is set up for Matrix on this Foundry."

  @us-iis-01 @kpi-iis-2
  Scenario: With a secrets key the page shows what to paste into Slack
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Priya opens the instance integrations page
    Then Priya reads "Secrets key: set. Credentials entered here are sealed with it."
    And Priya reads "Slack: not set up. Create the Foundry Slack app in Slack from the manifest below, then enter its credentials."
    And Priya sees the redirect URL "https://foundry.example.org/workspace/integrations/slack/return" and the events URL "https://foundry.example.org/integrations/slack/events"
    And the manifest shown equals the one on the Slack setup page
    And Priya reads "Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page."
    And Priya is not offered "Generate secrets key"

  @us-iis-01 @kpi-iis-2 @error
  Scenario: Unreachable Slack addresses are named before Priya copies them
    Given this Foundry starts with its instance key set by the operator
    And the operator has not given this Foundry its public address
    And Priya has generated the secrets key
    When Priya opens the instance integrations page
    Then Priya reads "FOUNDRY_PUBLIC_URL is not set, so the addresses below use http://localhost:8080, which Slack cannot reach. An instance operator sets FOUNDRY_PUBLIC_URL to this Foundry's public address and restarts Foundry."
    And the warning about the unreachable address comes before the Slack addresses

  @us-iis-01 @us-iis-06 @kpi-iis-2 @kpi-iis-g1
  Scenario: An instance configured before this feature is shown without its secrets
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya opens the instance integrations page
    Then Priya reads "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here."
    And Priya is offered "Move to a secrets key"
    And Priya reads "Slack: set in Foundry's environment, app 6123456789012.7234567890123. To manage it here, enter the same app below; it takes over once SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from the environment."
    And the page shows no key and no Slack secret

  @us-iis-01 @us-iis-06 @kpi-iis-2
  Scenario: An instance on the legacy setting with nothing saved yet can move at once, and Matrix is ready
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    When Priya opens the instance integrations page
    Then Priya reads "Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here."
    And Priya is offered "Move to a secrets key"
    And Priya is not offered "Generate secrets key"
    And Priya reads "Matrix: ready. Workspace admins connect their own Matrix bot on their workspace's integrations page."

  # Deliverable A: I-26 is restore-only (no Replace, no Start over).
  @us-iis-01 @us-iis-08 @kpi-iis-2 @error
  Scenario: A missing or different instance key is named, never silent
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And this Foundry has since restarted with a different instance key set by the operator
    When Priya opens the instance integrations page
    Then Priya reads "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used. An instance operator restores that instance key, or sets it as the previous instance key, and restarts Foundry."
    And Priya is not offered "Generate secrets key"
    And Priya is not offered "Replace secrets key"
    And Priya is not offered "Start over"
    And Priya is offered no way to enter the Slack app

  @us-iis-01 @us-iis-02 @us-iis-06 @kpi-iis-2 @error
  Scenario Outline: Saved credentials no held key opens are named, and generate and move change nothing
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And this Foundry has since restarted with a different instance key set by the operator
    When Priya posts a <door> request anyway
    Then Priya reads "This Foundry cannot read some saved credentials: neither its instance key nor its previous instance key opens them. An instance operator sets the instance key they were sealed with as the previous instance key, and restarts Foundry."
    And Priya is not offered "Generate secrets key"
    And Priya is not offered "Move to a secrets key"
    And this Foundry still has no secrets key
    And the instance integrations page reads "This Foundry cannot read some saved credentials: neither its instance key nor its previous instance key opens them."

    Examples:
      | door     |
      | generate |
      | move     |

  @us-iis-01 @kpi-iis-2 @error
  Scenario: A saved Slack app this Foundry cannot read is not in effect anywhere
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Priya has saved the Slack app 6123456789012.7234567890123
    And this Foundry has since restarted with a different instance key set by the operator
    When Priya opens the instance integrations page
    Then Priya reads "Slack: saved, but this Foundry cannot read it (see the secrets key above)."
    And the workspace integrations page reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."

  @us-iis-01 @us-iis-08 @kpi-iis-2 @error
  Scenario: Without a writable data directory the board serves and the page says why
    Given this Foundry starts with no instance key setting and a data directory it cannot write
    When Priya opens the instance integrations page
    Then Priya reads "Foundry has no instance key and cannot make one (/var/lib/foundry/instance.key cannot be written). An instance operator sets the instance key, or gives Foundry a writable data directory, and restarts Foundry."
    And Priya is not offered "Generate secrets key"
    And Priya is offered no way to enter the Slack app
    And Canzan Labs' workspace integrations page offers neither Add to Slack nor to connect Matrix
    And Priya still opens Identity Platform's board
    And this Foundry still answers that it is ready

  @us-iis-01 @us-iis-08 @kpi-iis-2 @error
  Scenario: A key file Foundry cannot read is named, kept, and never replaced
    Given this Foundry starts with no instance key setting and an instance key file it cannot read
    When Priya opens the instance integrations page
    Then Priya reads "This Foundry cannot read its instance key file (/var/lib/foundry/instance.key). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry."
    And Priya is not offered "Generate secrets key"
    And Priya is not offered "Start over"
    And Priya is offered no way to enter the Slack app
    And the instance key file is unchanged

  @us-iis-01 @kpi-iis-g2
  Scenario: The instance admin dashboard links to the integrations page
    When Priya follows "Integrations" on the instance admin dashboard
    Then Priya is on the instance integrations page

  @us-iis-01 @us-iis-02 @us-iis-03 @kpi-iis-g2 @error
  Scenario Outline: Only an instance admin reaches the page or changes anything on it
    Given this Foundry starts with its instance key set by the operator
    When <person> <door>
    Then <person> <answer>
    And this Foundry still has no secrets key

    # The Priya row keeps the others discriminating: the page exists, and
    # only the people below are refused it.
    Examples:
      | person               | door                                 | answer                                                 |
      | Priya                | opens the instance integrations page | is on the instance integrations page                   |
      | Tomás                | opens the instance integrations page | gets the same not-found page any unknown address gets |
      | Lena                 | opens the instance integrations page | gets the same not-found page any unknown address gets |
      | Sam                  | opens the instance integrations page | gets the same not-found page any unknown address gets |
      | Marco                | opens the instance integrations page | gets the same not-found page any unknown address gets |
      | a signed-out visitor | opens the instance integrations page | gets the same not-found page any unknown address gets |
      | Tomás                | posts a generate request             | gets the same not-found page any unknown address gets |
      | Tomás                | posts a Slack app save request       | gets the same not-found page any unknown address gets |
      | a signed-out visitor | posts a generate request             | gets the same not-found page any unknown address gets |

  # ------------------------------------------------------------------ US-IIS-02

  @iis-replica @us-iis-02 @kpi-iis-1 @error
  Scenario: Two generate requests at the same moment store one key that both replicas use
    Given this Foundry runs as two replicas with its instance key set by the operator
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    When a generate request reaches each replica at the same moment
    Then exactly one secrets key is stored, and both requests were answered with it set
    When Priya connects Canzan Labs to Slack with this Foundry's app
    Then within 60 seconds Canzan Labs' Slack connection opens on every replica

  @us-iis-02 @us-iis-03 @kpi-iis-g1
  Scenario: A database dump alone opens nothing
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When the database's contents are read without the instance key
    Then neither the secrets key, the instance key, the Slack app's secrets nor Canzan Labs' Slack token can be recovered from them

  @us-iis-02 @error
  Scenario: A later generate does not replace the key
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya posts a generate request again
    Then Priya reads "Secrets key: set. Credentials entered here are sealed with it."
    And Canzan Labs' saved Slack connection still opens
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman generated the secrets key" exactly once

  @us-iis-02 @us-iis-08 @error
  Scenario Outline: A generate request changes nothing where generating does not apply
    Given this Foundry starts with <setting>
    When Priya posts a generate request anyway
    Then Priya reads "<words>"
    And Priya is not offered "Generate secrets key"
    And this Foundry still has no secrets key

    Examples:
      | setting                                                          | words                                                                                                          |
      | no instance key setting and a data directory it cannot write    | Foundry has no instance key and cannot make one (/var/lib/foundry/instance.key cannot be written).             |
      | no instance key setting and an instance key file it cannot read | This Foundry cannot read its instance key file (/var/lib/foundry/instance.key).                                |
      | the legacy FOUNDRY_SECRETS_KEY setting                           | Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.    |

  @us-iis-02 @kpi-iis-1
  Scenario: Generating is recorded
    Given this Foundry starts with its instance key set by the operator
    When Priya chooses "Generate secrets key" on the instance integrations page
    Then the instance integrations activity reads "21 Oct 2026 · Priya Raman generated the secrets key"
    And Canzan Labs' workspace integrations page offers to connect Matrix
    And a new Matrix route for Identity Platform no longer reads "Matrix is not set up on this Foundry yet. An instance operator sets it up once."

  @iis-subprocess @us-iis-02 @kpi-iis-1
  Scenario: After a restart the stored secrets key is in effect and the key check passes
    Given Foundry runs as the operator's standalone process with its instance key set by the operator
    And Priya has generated the secrets key
    When the operator's standalone process restarts
    Then the instance integrations page reads "Secrets key: set. Credentials entered here are sealed with it."
    And the operator's health signals report the secrets key as healthy

  # ------------------------------------------------------------------ US-IIS-03

  @us-iis-03 @driving_port @kpi-iis-1 @kpi-iis-g1
  Scenario: An instance admin enters the Slack app and Canzan Labs can add Slack at once
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Priya saves the Slack app with client ID "6123456789012.7234567890123", client secret "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8" and signing secret "b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And Priya reads "Slack: set up here · app 6123456789012.7234567890123 · saved by Priya Raman · 21 Oct 2026"
    And Priya reads "Client secret: set"
    And Priya reads "Signing secret: set"
    And the page shows no key and no Slack secret
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman saved the Slack app (6123456789012.7234567890123)"
    And Canzan Labs' workspace integrations page offers Add to Slack

  @us-iis-03 @error
  Scenario Outline: A blank or malformed value is refused and nothing is saved
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Priya saves the Slack app with client ID "<client_id>", client secret "<client_secret>" and signing secret "<signing_secret>"
    Then Priya reads "<words>"
    And the form keeps the client ID "<client_id>" and leaves both secrets empty
    And Slack is still not set up on this Foundry

    Examples:
      | client_id                        | client_secret                    | signing_secret                   | words                                                                                     |
      |                                  | 3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8 | b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05 | Enter the client ID from the Slack app's Basic Information.                               |
      | 6123456789012.7234567890123 copy | 3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8 | b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05 | That is not a Slack client ID. Copy it again from the Slack app's Basic Information.      |
      | 6123456789012.7234567890123      |                                  | b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05 | Enter the client secret from the Slack app's Basic Information.                           |
      | 6123456789012.7234567890123      | 3f9c0a7e                         | b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05 | That is not a Slack client secret. Copy it again from the Slack app's Basic Information.  |
      | 6123456789012.7234567890123      | 3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8 |                                  | Enter the signing secret from the Slack app's Basic Information.                          |
      | 6123456789012.7234567890123      | 3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8 | b7e1 d4c9                        | That is not a Slack signing secret. Copy it again from the Slack app's Basic Information. |

  @us-iis-03 @error
  Scenario: Nothing can be saved without a secrets key
    Given this Foundry starts with its instance key set by the operator
    When Priya posts a Slack app save request anyway
    Then Priya reads "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials."
    And Slack is still not set up on this Foundry

  @us-iis-03 @kpi-iis-1
  Scenario: Slack's event check and Add to Slack use the saved app
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Slack sends its URL verification challenge signed with the saved signing secret
    Then Slack's confirmation of Foundry's events address succeeds
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.7234567890123

  @us-iis-03 @kpi-iis-1
  Scenario: Add to Slack through the saved app exchanges Slack's code with the saved client secret
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya connects Canzan Labs to Slack with this Foundry's app
    Then the workspace integrations page reads "Connected to Canzan Labs"

  @us-iis-03 @error
  Scenario: An event signed with a secret Foundry was never given is refused
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Slack sends its URL verification challenge signed with another signing secret
    Then Foundry refuses as unauthorised the message

  # ------------------------------------------------------------------ US-IIS-06

  @us-iis-06 @kpi-iis-5
  Scenario: After the upgrade, a connection sealed with the environment key keeps delivering
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And the next issue filed in Identity Platform becomes AUTH-12
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Lena has routed New issue on Identity Platform to #auth-dev
    And this Foundry has since restarted with the same settings
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth-dev shows within 10 seconds a Foundry message "AUTH-12 Rotate LLDAP bind password" / "New issue in Identity Platform by Sam Okafor · Backlog" with Open in Foundry
    And the instance integrations page reads "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here."

  @us-iis-06 @kpi-iis-5
  Scenario: Renaming the setting to FOUNDRY_INSTANCE_KEY with the same value keeps everything working
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And the next issue filed in Identity Platform becomes AUTH-12
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Lena has routed New issue on Identity Platform to #auth-dev
    And this Foundry has since restarted with the same key under FOUNDRY_INSTANCE_KEY
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth-dev shows within 10 seconds a Foundry message "AUTH-12 Rotate LLDAP bind password" / "New issue in Identity Platform by Sam Okafor · Backlog" with Open in Foundry
    And the instance integrations page reads "Saved credentials are sealed directly with the instance key. Move them under a secrets key to manage it here."

  @us-iis-06 @kpi-iis-5 @kpi-iis-4
  Scenario: Moving to a secrets key re-seals everything and nobody reconnects
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And the next issue filed in Identity Platform becomes AUTH-12
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Lena has routed New issue on Identity Platform to #auth-dev
    When Priya chooses "Move to a secrets key" on the instance integrations page
    Then Priya reads "Moved. 2 saved credentials are sealed under a new secrets key. Nobody needs to reconnect."
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman moved 2 credentials under a secrets key"
    And Canzan Labs' Slack connection is not marked lost and no workspace admin is emailed
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth-dev shows within 10 seconds a Foundry message "AUTH-12 Rotate LLDAP bind password" / "New issue in Identity Platform by Sam Okafor · Backlog" with Open in Foundry

  @iis-replica @us-iis-06 @kpi-iis-5 @kpi-iis-4
  Scenario: After a move on one replica, every replica opens the moved credentials and keeps delivering
    Given this Foundry runs as two replicas with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And the next issue filed in Identity Platform becomes AUTH-12
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Lena has routed New issue on Identity Platform to #auth-dev
    When Priya chooses "Move to a secrets key" on the instance integrations page
    Then Priya reads "Moved. 1 saved credential is sealed under a new secrets key. Nobody needs to reconnect."
    And within 60 seconds Canzan Labs' Slack connection opens on every replica
    And Canzan Labs' Slack connection is not marked lost and no workspace admin is emailed
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then within 60 seconds every replica's history of the route to #auth-dev lists AUTH-12 as delivered

  @us-iis-06 @error
  Scenario: Moving with nothing saved yet just generates the secrets key
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    When Priya chooses "Move to a secrets key" on the instance integrations page
    Then Priya reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it."
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman generated the secrets key"

  @us-iis-06 @kpi-iis-5
  Scenario: While the environment sets the Slack app, a saved app is kept but not used
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    When Priya saves the Slack app with client ID "6123456789012.8888888888888", client secret "0a1b2c3d4e5f60718293a4b5c6d7e8f9" and signing secret "9f8e7d6c5b4a39281706f5e4d3c2b1a0"
    Then Priya reads "Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used."
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.7234567890123

  @us-iis-06 @kpi-iis-5
  Scenario: The saved app takes over when the environment's is removed
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And the next issue filed in Identity Platform becomes AUTH-12
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Lena has routed New issue on Identity Platform to #auth-dev
    And this Foundry has since restarted without the Slack app in its environment
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth-dev shows within 10 seconds a Foundry message "AUTH-12 Rotate LLDAP bind password" / "New issue in Identity Platform by Sam Okafor · Backlog" with Open in Foundry
    And the instance integrations page reads "Slack: set up here · app 6123456789012.7234567890123"
    When Slack sends its URL verification challenge signed with the saved signing secret
    Then Slack's confirmation of Foundry's events address succeeds

  # ------------------------------------------------------------------ US-IIS-08

  @iis-subprocess @us-iis-08 @kpi-iis-6 @kpi-iis-2
  Scenario: A standalone Foundry makes its own instance key, says what to back up, and turns on Matrix in one click
    Given Foundry runs as the operator's standalone process with no instance key setting and a writable data directory
    When Priya opens the instance integrations page
    Then Priya reads "Instance key: made by Foundry in /var/lib/foundry/instance.key. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected."
    And the instance key file is readable only by Foundry's user
    When Priya chooses "Generate secrets key" on the instance integrations page
    Then Canzan Labs' workspace integrations page offers to connect Matrix

  @iis-subprocess @us-iis-08 @kpi-iis-6 @error
  Scenario: Servers starting together on one data directory make one key file and share it
    Given two Foundry servers share one data directory and neither has an instance key setting
    When both servers start at the same moment
    Then exactly one instance key file exists in the shared data directory
    When Priya generates the secrets key on the first server
    Then within 60 seconds the second server's instance integrations page reads "Secrets key: set. Credentials entered here are sealed with it."

  @us-iis-08 @kpi-iis-6
  Scenario: The key file is reused, not remade, on restart
    Given this Foundry starts with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When this Foundry restarts with the same settings
    Then the instance key file is unchanged
    And the instance integrations page reads "Slack: set up here · app 6123456789012.7234567890123"
    And Canzan Labs' workspace integrations page offers Add to Slack

  @us-iis-08 @kpi-iis-2 @error
  Scenario: A server with a different key file says so and still serves
    Given this Foundry starts with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And this Foundry has since restarted with a different instance key file
    When Priya opens the instance integrations page
    Then Priya reads "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server."
    And this Foundry still answers that it is ready
    And Canzan Labs' Slack connection is not marked lost and no workspace admin is emailed

  @us-iis-08 @kpi-iis-2 @kpi-iis-g1
  Scenario: Moving from Foundry's key file to an operator key loses nothing
    Given this Foundry starts with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When this Foundry restarts with a different instance key set by the operator
    Then the instance integrations page reads "Instance key: set by the operator. The secrets key, first sealed with /var/lib/foundry/instance.key, is now sealed with the operator's key. Keep a backup of /var/lib/foundry/instance.key until you are sure, then remove it."
    And the instance integrations activity reads "21 Oct 2026 · Foundry re-sealed the secrets key under the operator's instance key"
    And Canzan Labs' saved Slack connection still opens

  @us-iis-08 @kpi-iis-2 @error
  Scenario Outline: Empty or matching key settings are read as the operator meant
    Given this Foundry starts with <setting>
    When Priya opens the instance integrations page
    Then Priya reads "<words>"

    Examples:
      | setting                                                              | words                                                                                                    |
      | FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY both set to empty values | Instance key: made by Foundry in /var/lib/foundry/instance.key.                                          |
      | FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY set to the same key      | Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here. |

  @iis-subprocess @us-iis-08 @kpi-iis-g1 @error
  Scenario Outline: Foundry refuses to start on a malformed or conflicting instance key, naming only the setting
    When Foundry is started with <setting>
    Then Foundry refuses to start with a "health.startup.refused" line naming <names>, never the value

    Examples:
      | setting                                                           | names                                        |
      | an instance key of 20 characters                                  | FOUNDRY_INSTANCE_KEY                         |
      | FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY set to different keys | FOUNDRY_INSTANCE_KEY and FOUNDRY_SECRETS_KEY |

  # ---------------------------------------------- keys and secrets never leave

  # The replaced secret is swept with the rest: a rotation hands Foundry one
  # more secret, and the page, the log and the database must keep it as they
  # keep the first one.
  @iis-subprocess @us-iis-02 @us-iis-03 @us-iis-05 @us-iis-08 @kpi-iis-g1
  Scenario: No key or secret appears in any page, log line or activity line, or in plaintext in the database
    Given Foundry runs as the operator's standalone process with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has rotated the Slack app's signing secret
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When everything Foundry shows, logs and stores is searched for the keys and the Slack secrets
    Then none of them appears in any page, page source, log line or activity line, and none is stored in plaintext

  # ------------------------------------------------------------------ US-IIS-04

  # An instance admin who meets "not set up" or "no secrets key" on a workspace
  # page or a route form is offered the setup she can reach, after the links
  # that ship today; everyone else is offered only what opens for them. The
  # words of those sentences are the shipped ones, byte for byte.
  #
  # "The links <who> is offered" reads every link the page itself carries, in
  # reading order, the sidebar excluded: an offer into the instance setup is a
  # page link like any other, and a page that offers one to someone it refuses
  # has to show up here. Every one of them is then followed as that viewer.

  @us-iis-04 @driving_port @kpi-iis-3 @kpi-iis-g2
  Scenario: An instance admin is sent from the workspace integrations page to this Foundry's setup
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Priya opens the workspace integrations page
    Then Priya reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And the links Priya is offered read "How to set up Slack" then "Set up integrations for this Foundry"
    And every link Priya is offered answers with its page
    And following "Set up integrations for this Foundry" lands Priya on the instance integrations page

  # Without a secrets key the page says so for Slack (its app is in Foundry's
  # environment) and for Matrix, and offers the setup beside each sentence.
  @us-iis-04 @kpi-iis-3
  Scenario: An instance admin is offered the setup beside every integration the page says is missing
    Given this Foundry starts with its instance key set by the operator
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    When Priya opens the workspace integrations page
    Then Priya reads "Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key."
    And Priya reads "Matrix is not set up on this Foundry yet. An instance operator sets it up once."
    And the links Priya is offered read "Set up integrations for this Foundry" then "Set up integrations for this Foundry"
    And every link Priya is offered answers with its page

  @us-iis-04 @kpi-iis-g2
  Scenario: A workspace admin who is not an instance admin keeps the guide and is offered nothing she cannot open
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Tomás opens the workspace integrations page
    Then Tomás reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And the links Tomás is offered read "How to set up Slack"
    And every link Tomás is offered answers with its page

  # The same reader on the other door: a workspace admin who is not an
  # instance admin reads the route form, its preview and its save, and the
  # setup is not hers to be offered on any of the three.
  @us-iis-04 @kpi-iis-g2
  Scenario Outline: A workspace admin who is not an instance admin keeps the guide on a route form, its preview and its save
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Tomás <door> a new Slack route for Identity Platform
    Then Tomás reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And the links Tomás is offered read "How to set up Slack"
    And every link Tomás is offered answers with its page

    Examples:
      | door     |
      | opens    |
      | previews |
      | saves    |

  # The form, its preview and its save are three doors on one state: they read
  # the same sentence and offer the same links, in the same order.
  @us-iis-04 @kpi-iis-3
  Scenario Outline: An instance admin is sent to the setup from a Slack route form, its preview and its save
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Priya <door> a new Slack route for Identity Platform
    Then Priya reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And the links Priya is offered read "How to set up Slack" then "Set up integrations for this Foundry"
    And every link Priya is offered answers with its page

    Examples:
      | door     |
      | opens    |
      | previews |
      | saves    |

  @us-iis-04 @kpi-iis-3
  Scenario Outline: An instance admin is sent to the setup from a Matrix route form, its preview and its save
    Given this Foundry starts with its instance key set by the operator
    When Priya <door> a new Matrix route for Identity Platform
    Then Priya reads "Matrix is not set up on this Foundry yet. An instance operator sets it up once."
    And the links Priya is offered read "Set up integrations for this Foundry"
    And every link Priya is offered answers with its page
    And following "Set up integrations for this Foundry" lands Priya on the instance integrations page

    Examples:
      | door     |
      | opens    |
      | previews |
      | saves    |

  @us-iis-04 @kpi-iis-3
  Scenario: An instance admin is sent to the setup from a form that says Slack has no secrets key
    Given this Foundry starts with its instance key set by the operator
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    When Priya opens a new Slack route for Identity Platform
    Then Priya reads "Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key."
    And the links Priya is offered read "Set up integrations for this Foundry"
    And every link Priya is offered answers with its page

  # Lena leads Backend, so she opens Identity Platform's route form; she is not
  # a workspace admin, so the Slack guide is not hers to open.
  @us-iis-04 @kpi-iis-g2
  Scenario: An instance admin who is not a workspace admin is offered the setup alone
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Lena Park is this Foundry's instance admin
    When Lena opens a new Slack route for Identity Platform
    Then Lena reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And the links Lena is offered read "Set up integrations for this Foundry"
    And every link Lena is offered answers with its page
    And following "Set up integrations for this Foundry" lands Lena on the instance integrations page

  @us-iis-04 @kpi-iis-g2 @error
  Scenario: A lead who is neither admin is offered no way out of a form that says Slack is not set up
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Lena opens a new Slack route for Identity Platform
    Then Lena reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."
    And the links Lena is offered read nothing at all

  # The sentences that have no browser fix keep their words and gain no offer:
  # a saved connection this Foundry cannot read, a workspace nobody connected,
  # and the project page's own lines.
  @us-iis-04 @error
  Scenario: A connection this Foundry cannot read still names the key, and offers no setup
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And this Foundry has since restarted with a different instance key set by the operator
    When Priya opens a new Slack route for Identity Platform
    Then Priya reads "Slack is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key."
    And the links Priya is offered read nothing at all

  @us-iis-04
  Scenario: A workspace nobody connected still sends an instance admin to the workspace page, and nowhere else
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya opens a new Slack route for Identity Platform
    Then Priya reads "This workspace has no Slack connection yet."
    And the links Priya is offered read "Connect Slack in workspace integrations"
    And every link Priya is offered answers with its page

  @us-iis-04
  Scenario: A project page reads to an instance admin exactly as it ships
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    When Priya opens Identity Platform's integrations page
    Then Priya reads "This workspace has no chat connection yet."
    And the links Priya is offered read "Connect Slack or Matrix in workspace integrations" then "New route"
    And every link Priya is offered answers with its page

  # The guide the page sends a workspace admin to tells her who sets Slack up
  # and that nothing restarts: an instance admin enters the app in the browser,
  # and the operator's settings survive only as a footnote after the steps, as
  # the alternative that wins where it is used. Tomás is a workspace admin and
  # not this Foundry's instance admin, so this is the reader the offer sends.
  # The second absent phrase is the sentence this guide is replacing: a guide
  # that gained the new sentences but kept the old instruction would still send
  # her to an operator.
  @us-iis-04 @kpi-iis-3
  Scenario: The Slack setup guide names the browser form and asks for no restart
    Given this Foundry starts with its instance key set by the operator
    When Tomás opens the Slack setup page
    Then Tomás reads these sentences in this order
      | sentence                                                                                                                                                       |
      | An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin › Integrations.                                             |
      | No restart is needed. A workspace admin can then choose Add to Slack on the workspace integrations page.                                                        |
      | An instance operator may instead set SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET in Foundry's environment and restart; the environment takes precedence. |
    And nothing Tomás reads there says "Restart Foundry"
    And nothing Tomás reads there says "SLACK_SIGNING_SECRET on this Foundry"

  # ------------------------------------------------------ alert inputs on reload

  # DEVOPS D5 / task 8: the reload path, not only boot, updates the two alert
  # inputs. Two operator processes, not MultiReplicaHarness: metrics live in one
  # process-wide registry, so only a process shows a per-server /metrics.
  @iis-subprocess @us-iis-08 @us-iis-01 @kpi-iis-1 @error
  Scenario: A server that cannot open the new secrets key says so and raises the alarm, without a restart
    Given Foundry runs as two servers on one database, the second with a different instance key set by the operator
    When Priya generates the secrets key on the first server
    Then within 60 seconds the second server's instance integrations page reads "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server."
    And the second server's health signals report the secrets key as degraded

  # ------------------------------------------------------------------ US-IIS-05

  # Replacing the Slack app in the browser, with no workspace connected to
  # Slack: the saved app's own secrets rotated (a blank field keeps the secret
  # Foundry already holds, so Add to Slack keeps working), and another app,
  # which is only accepted with both secrets. "0f1e…" is the secret Slack
  # regenerated: the app was entered with "b7e1…", so a rotation to it is
  # something a reader - and this file - can tell apart from nothing happening.

  @us-iis-05 @driving_port @kpi-iis-4
  Scenario: The replace form offers the saved app's client ID and asks for no secret Priya keeps
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya opens the instance integrations page
    Then Priya reads "Slack: set up here · app 6123456789012.7234567890123"
    And the Slack app form offers the client ID "6123456789012.7234567890123" with both secret fields empty
    And the page shows no key and no Slack secret

  @us-iis-05 @driving_port @kpi-iis-1 @kpi-iis-4
  Scenario: Rotating the signing secret, with the client secret left blank, is saved at once
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya saves the Slack app with client ID "6123456789012.7234567890123", client secret "" and signing secret "0f1e2d3c4b5a69788796a5b4c3d2e1f0"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And Priya is on the instance integrations page
    And Priya reads "Slack: set up here · app 6123456789012.7234567890123"
    And Priya reads "Client secret: set"
    And Priya reads "Signing secret: set"
    And the page shows no key and no Slack secret
    And the Slack app form offers the client ID "6123456789012.7234567890123" with both secret fields empty
    And the instance integrations activity reads these lines in this order
      | line                                                                           |
      | 21 Oct 2026 · Priya Raman replaced the Slack app (6123456789012.7234567890123) |
      | 21 Oct 2026 · Priya Raman saved the Slack app (6123456789012.7234567890123)    |
      | 21 Oct 2026 · Priya Raman generated the secrets key                            |

  @us-iis-05 @kpi-iis-1 @kpi-iis-4
  Scenario: After the rotation Slack's events verify with the signing secret now in effect
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has rotated the Slack app's signing secret
    When Slack sends its URL verification challenge signed with the rotated signing secret
    Then Slack's confirmation of Foundry's events address succeeds

  @us-iis-05 @kpi-iis-1 @kpi-iis-4 @error
  Scenario: The signing secret the rotation replaced no longer verifies
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has rotated the Slack app's signing secret
    When Slack sends its URL verification challenge signed with the signing secret it replaced
    Then Foundry refuses as unauthorised the message

  @us-iis-05 @kpi-iis-1 @kpi-iis-4
  Scenario: Add to Slack still exchanges Slack's code with the client secret the rotation kept
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has rotated the Slack app's signing secret
    When Priya connects Canzan Labs to Slack with this Foundry's app
    Then the workspace integrations page reads "Connected to Canzan Labs"

  @us-iis-05 @error @kpi-iis-4
  Scenario Outline: Another Slack app needs both secrets, and a blank one changes nothing
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya saves the Slack app with client ID "6123456789012.8888888888888", client secret "<client_secret>" and signing secret "<signing_secret>"
    Then Priya reads "Enter the <field> from the Slack app's Basic Information."
    And the form keeps the client ID "6123456789012.8888888888888" and leaves both secrets empty
    And the Slack app 6123456789012.7234567890123 is still in effect, with its secrets and its activity unchanged

    Examples:
      | client_secret                    | signing_secret                   | field          |
      |                                  | 9f8e7d6c5b4a39281706f5e4d3c2b1a0 | client secret  |
      | 0a1b2c3d4e5f60718293a4b5c6d7e8f9 |                                  | signing secret |

  @us-iis-05 @driving_port @kpi-iis-4
  Scenario: Another Slack app entered with both secrets takes over at once while no workspace is connected
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya saves the Slack app with client ID "6123456789012.8888888888888", client secret "0a1b2c3d4e5f60718293a4b5c6d7e8f9" and signing secret "9f8e7d6c5b4a39281706f5e4d3c2b1a0"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And Priya is on the instance integrations page
    And Priya reads "Slack: set up here · app 6123456789012.8888888888888"
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.8888888888888

  # Switching apps while Canzan Labs is connected through the current one: the
  # submission waits on its own page until the admin who made it confirms it,
  # and it is held for ten minutes only. The scenario above - the same
  # submission with nobody connected - is what tells the count apart from a
  # confirm that always appears, and the first scenario below - a secret of the
  # app in effect replaced while that workspace is connected - is what keeps a
  # rotation out of the confirm. The app in the environment is the current app
  # in the last two scenarios (US-IIS-06 AC 6), so the same wait applies to it.

  @us-iis-05 @driving_port @kpi-iis-1 @kpi-iis-4
  Scenario: A rotation of the current app's own secret is saved at once while a workspace is connected
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya saves the Slack app with client ID "6123456789012.7234567890123", client secret "" and signing secret "0f1e2d3c4b5a69788796a5b4c3d2e1f0"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And Priya is on the instance integrations page
    And Priya reads "Slack: set up here · app 6123456789012.7234567890123"
    And no Slack app is waiting for Priya's confirmation

  @us-iis-05 @driving_port @kpi-iis-4
  Scenario: Another Slack app waits while a workspace is connected through the current one
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya saves the Slack app with client ID "6123456789012.8888888888888", client secret "0a1b2c3d4e5f60718293a4b5c6d7e8f9" and signing secret "9f8e7d6c5b4a39281706f5e4d3c2b1a0"
    Then Priya is on the Slack confirm page
    And Priya reads "1 workspace is connected through the current Slack app (6123456789012.7234567890123). It keeps posting through it until a workspace admin reconnects Slack. Save the new app?"
    And Priya is offered "Save the new app"
    And the page shows no key and no Slack secret
    And the Slack app 6123456789012.7234567890123 is still in effect, with its secrets and its activity unchanged

  @us-iis-05 @driving_port @kpi-iis-1 @kpi-iis-4
  Scenario: The admin who submitted the other app saves it from the confirm page
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has submitted the Slack app 6123456789012.8888888888888 with both of its secrets
    And Priya has opened the confirm page twice without choosing anything
    And 2 minutes have passed
    When Priya chooses "Save the new app" on the confirm page
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And Priya is on the instance integrations page
    And Priya reads "Slack: set up here · app 6123456789012.8888888888888"
    And the page shows no key and no Slack secret
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.8888888888888
    And the instance integrations activity reads these lines in this order
      | line                                                                           |
      | 21 Oct 2026 · Priya Raman replaced the Slack app (6123456789012.8888888888888) |
      | 21 Oct 2026 · Priya Raman saved the Slack app (6123456789012.7234567890123)    |
      | 21 Oct 2026 · Priya Raman generated the secrets key                            |

  @us-iis-05 @error @kpi-iis-4
  Scenario: Another instance admin's confirm saves nothing
    Given this Foundry starts with its instance key set by the operator
    And Tomás Rivera is this Foundry's instance admin
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has submitted the Slack app 6123456789012.8888888888888 with both of its secrets
    When Tomás posts a Slack confirm request
    Then Tomás's answer is this Foundry's integrations page
    And the Slack app 6123456789012.7234567890123 is still in effect, with its secrets and its activity unchanged

  @us-iis-05 @kpi-iis-4
  Scenario: After another instance admin's confirm the submitting admin still saves the new app
    Given this Foundry starts with its instance key set by the operator
    And Tomás Rivera is this Foundry's instance admin
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has submitted the Slack app 6123456789012.8888888888888 with both of its secrets
    And Tomás has posted a Slack confirm request
    When Priya chooses "Save the new app" on the confirm page
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And Priya reads "Slack: set up here · app 6123456789012.8888888888888"

  @us-iis-05 @error @kpi-iis-4
  Scenario: A confirm eleven minutes after the submission saves nothing
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has submitted the Slack app 6123456789012.8888888888888 with both of its secrets
    And 11 minutes have passed
    When Priya posts a Slack confirm request
    Then Priya's answer is this Foundry's integrations page
    And the Slack app 6123456789012.7234567890123 is still in effect, with its secrets and its activity unchanged

  @us-iis-05 @error @kpi-iis-4
  Scenario: A second confirm does not save the new app twice
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has submitted the Slack app 6123456789012.8888888888888 with both of its secrets
    And Priya has saved the new app from the confirm page
    When Priya posts a Slack confirm request again
    Then Priya's answer is this Foundry's integrations page
    And the instance integrations page reads "Slack: set up here · app 6123456789012.8888888888888"
    And the instance integrations activity reads these lines in this order
      | line                                                                           |
      | 21 Oct 2026 · Priya Raman replaced the Slack app (6123456789012.8888888888888) |
      | 21 Oct 2026 · Priya Raman saved the Slack app (6123456789012.7234567890123)    |
      | 21 Oct 2026 · Priya Raman generated the secrets key                            |

  @us-iis-05 @error @kpi-iis-4
  Scenario: A confirm with nothing submitted changes nothing
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya posts a Slack confirm request anyway
    Then Priya's answer is this Foundry's integrations page
    And the Slack app 6123456789012.7234567890123 is still in effect, with its secrets and its activity unchanged

  @us-iis-05 @us-iis-06 @driving_port @kpi-iis-4 @kpi-iis-5
  Scenario: The app Foundry's environment sets also waits for the confirm
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya saves the Slack app with client ID "6123456789012.8888888888888", client secret "0a1b2c3d4e5f60718293a4b5c6d7e8f9" and signing secret "9f8e7d6c5b4a39281706f5e4d3c2b1a0"
    Then Priya is on the Slack confirm page
    And Priya reads "1 workspace is connected through the current Slack app (6123456789012.7234567890123). It keeps posting through it until a workspace admin reconnects Slack. Save the new app?"
    And Priya is offered "Save the new app"
    And the page shows no key and no Slack secret
    And the instance integrations activity reads these lines in this order
      | line                                                |
      | 21 Oct 2026 · Priya Raman generated the secrets key |

  @us-iis-05 @us-iis-06 @kpi-iis-4 @kpi-iis-5
  Scenario: The confirm stores the other app while the environment's stays in effect
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    And Priya has submitted the Slack app 6123456789012.8888888888888 with both of its secrets
    When Priya chooses "Save the new app" on the confirm page
    Then Priya reads "Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used."
    And Priya is on the instance integrations page
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman saved the Slack app (6123456789012.8888888888888)"
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.7234567890123
