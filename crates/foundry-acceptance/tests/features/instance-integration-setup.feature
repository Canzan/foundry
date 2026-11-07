# Feature: instance-integration-setup, Deliverable A (US-IIS-01, 02, 03, 06,
# 08) — an instance admin turns on Matrix and Slack from the browser, with no
# restart, on any standalone deploy; no key or secret is ever shown or stored
# in plaintext, and instances configured before this feature keep working.
#
# Copy I-n is locked in docs/feature/instance-integration-setup/discuss/
# user-stories.md. Deliverable B (US-IIS-04, 05, 07: I-19, I-20, Replace,
# Remove, the I-14 confirm, Start over) is not here; where A shows a state B
# extends (I-26), A's scenario checks the restore-only form.
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

  # US-IIS-01 -> 02 -> 03 with an operator-set instance key (DISCUSS D15).
  @walking_skeleton @driving_port @iis-replica @us-iis-01 @us-iis-02 @us-iis-03 @kpi-iis-1 @kpi-iis-2
  Scenario: An instance admin turns on Matrix and then Slack, and every replica offers them without a restart
    Given this Foundry runs as two replicas with its instance key set by the operator
    When Priya opens the instance integrations page
    Then Priya reads "This Foundry has no secrets key yet. Foundry needs one to store Slack and Matrix credentials."
    When Priya chooses "Generate secrets key" on the instance integrations page
    Then Priya reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it."
    And within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers to connect Matrix on every replica
    When Priya saves the Slack app with client ID "6123456789012.7234567890123", client secret "3f9c0a7e5b2d4c18a6e0f7b9d2c4e1a8" and signing secret "b7e1d4c9a2f05e3b8c6d1a7f4e9b2c05"
    Then Priya reads "Saved. Workspace admins can now choose Add to Slack."
    And within 60 seconds, without a restart, Canzan Labs' workspace integrations page offers Add to Slack on every replica

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

  @pending @us-iis-01 @us-iis-02 @us-iis-06 @kpi-iis-2 @error
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

  @pending @us-iis-01 @kpi-iis-2 @error
  Scenario: A saved Slack app this Foundry cannot read is not in effect anywhere
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Priya has saved the Slack app 6123456789012.7234567890123
    And this Foundry has since restarted with a different instance key set by the operator
    When Priya opens the instance integrations page
    Then Priya reads "Slack: saved, but this Foundry cannot read it (see the secrets key above)."
    And the workspace integrations page reads "Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once."

  @pending @us-iis-01 @us-iis-08 @kpi-iis-2 @error
  Scenario: Without a writable data directory the board serves and the page says why
    Given this Foundry starts with no instance key setting and a data directory it cannot write
    When Priya opens the instance integrations page
    Then Priya reads "Foundry has no instance key and cannot make one (/var/lib/foundry/instance.key cannot be written). An instance operator sets the instance key, or gives Foundry a writable data directory, and restarts Foundry."
    And Priya is not offered "Generate secrets key"
    And Priya is offered no way to enter the Slack app
    And Canzan Labs' workspace integrations page offers neither Add to Slack nor to connect Matrix
    And Priya still opens Identity Platform's board
    And this Foundry still answers that it is ready

  @pending @us-iis-01 @us-iis-08 @kpi-iis-2 @error
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

  @pending @us-iis-02 @us-iis-03 @kpi-iis-g1
  Scenario: A database dump alone opens nothing
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When the database's contents are read without the instance key
    Then neither the secrets key, the instance key, the Slack app's secrets nor Canzan Labs' Slack token can be recovered from them

  @pending @us-iis-02 @error
  Scenario: A later generate does not replace the key
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When Priya posts a generate request again
    Then Priya reads "Secrets key: set. Credentials entered here are sealed with it."
    And Canzan Labs' saved Slack connection still opens
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman generated the secrets key" exactly once

  @pending @us-iis-02 @us-iis-08 @error
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

  @pending @iis-subprocess @us-iis-02 @kpi-iis-1
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

  @pending @us-iis-03 @kpi-iis-1
  Scenario: Slack's event check and Add to Slack use the saved app
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Slack sends its URL verification challenge signed with the saved signing secret
    Then Slack's confirmation of Foundry's events address succeeds
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.7234567890123

  @pending @us-iis-03 @kpi-iis-1
  Scenario: Add to Slack through the saved app exchanges Slack's code with the saved client secret
    Given this Foundry starts with its instance key set by the operator
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When Priya connects Canzan Labs to Slack with this Foundry's app
    Then the workspace integrations page reads "Connected to Canzan Labs"

  @pending @us-iis-03 @error
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

  @pending @us-iis-06 @kpi-iis-5 @kpi-iis-4
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

  @pending @iis-replica @us-iis-06 @kpi-iis-5 @kpi-iis-4
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

  @pending @us-iis-06 @error
  Scenario: Moving with nothing saved yet just generates the secrets key
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    When Priya chooses "Move to a secrets key" on the instance integrations page
    Then Priya reads "Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it."
    And the instance integrations activity reads "21 Oct 2026 · Priya Raman generated the secrets key"

  @pending @us-iis-06 @kpi-iis-5
  Scenario: While the environment sets the Slack app, a saved app is kept but not used
    Given this Foundry starts with the legacy FOUNDRY_SECRETS_KEY setting
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    When Priya saves the Slack app with client ID "6123456789012.8888888888888", client secret "0a1b2c3d4e5f60718293a4b5c6d7e8f9" and signing secret "9f8e7d6c5b4a39281706f5e4d3c2b1a0"
    Then Priya reads "Saved. The app in Foundry's environment stays in effect until SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET are removed from it and Foundry restarts; then this one is used."
    And Add to Slack on Canzan Labs' workspace integrations page sends Priya to Slack's approval for client ID 6123456789012.7234567890123

  @pending @us-iis-06 @kpi-iis-5
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

  @pending @iis-subprocess @us-iis-08 @kpi-iis-6 @kpi-iis-2
  Scenario: A standalone Foundry makes its own instance key, says what to back up, and turns on Matrix in one click
    Given Foundry runs as the operator's standalone process with no instance key setting and a writable data directory
    When Priya opens the instance integrations page
    Then Priya reads "Instance key: made by Foundry in /var/lib/foundry/instance.key. Back up this file apart from the database. Every Foundry server must use the same file. Without it, saved connections must be reconnected."
    And the instance key file is readable only by Foundry's user
    When Priya chooses "Generate secrets key" on the instance integrations page
    Then Canzan Labs' workspace integrations page offers to connect Matrix

  @pending @iis-subprocess @us-iis-08 @kpi-iis-6 @error
  Scenario: Servers starting together on one data directory make one key file and share it
    Given two Foundry servers share one data directory and neither has an instance key setting
    When both servers start at the same moment
    Then exactly one instance key file exists in the shared data directory
    When Priya generates the secrets key on the first server
    Then within 60 seconds the second server's instance integrations page reads "Secrets key: set. Credentials entered here are sealed with it."

  @pending @us-iis-08 @kpi-iis-6
  Scenario: The key file is reused, not remade, on restart
    Given this Foundry starts with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    When this Foundry restarts with the same settings
    Then the instance key file is unchanged
    And the instance integrations page reads "Slack: set up here · app 6123456789012.7234567890123"
    And Canzan Labs' workspace integrations page offers Add to Slack

  @pending @us-iis-08 @kpi-iis-2 @error
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

  @pending @us-iis-08 @kpi-iis-2 @kpi-iis-g1
  Scenario: Moving from Foundry's key file to an operator key loses nothing
    Given this Foundry starts with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Foundry's environment sets the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When this Foundry restarts with a different instance key set by the operator
    Then the instance integrations page reads "Instance key: set by the operator. The secrets key, first sealed with /var/lib/foundry/instance.key, is now sealed with the operator's key. Keep a backup of /var/lib/foundry/instance.key until you are sure, then remove it."
    And the instance integrations activity reads "21 Oct 2026 · Foundry re-sealed the secrets key under the operator's instance key"
    And Canzan Labs' saved Slack connection still opens

  @pending @us-iis-08 @kpi-iis-2 @error
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

  @pending @iis-subprocess @us-iis-02 @us-iis-03 @us-iis-08 @kpi-iis-g1
  Scenario: No key or secret appears in any page, log line or activity line, or in plaintext in the database
    Given Foundry runs as the operator's standalone process with no instance key setting and a writable data directory
    And Priya has generated the secrets key
    And Priya has saved the Slack app 6123456789012.7234567890123
    And Priya has connected Canzan Labs to Slack with this Foundry's app
    When everything Foundry shows, logs and stores is searched for the keys and the Slack secrets
    Then none of them appears in any page, page source, log line or activity line, and none is stored in plaintext

  # ------------------------------------------------------ alert inputs on reload

  # DEVOPS D5 / task 8: the reload path, not only boot, updates the two alert
  # inputs. Two operator processes, not MultiReplicaHarness: metrics live in one
  # process-wide registry, so only a process shows a per-server /metrics.
  @pending @iis-subprocess @us-iis-08 @us-iis-01 @kpi-iis-1 @error
  Scenario: A server that cannot open the new secrets key says so and raises the alarm, without a restart
    Given Foundry runs as two servers on one database, the second with a different instance key set by the operator
    When Priya generates the secrets key on the first server
    Then within 60 seconds the second server's instance integrations page reads "This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server."
    And the second server's health signals report the secrets key as degraded
