# Feature: external-integration-plugins, slice 02 (US-EIP-04) — a team lead
# decides how much a message reveals and sees it before saving: three content
# levels (D-3, OD-6), the preview (DDR-25: a pure render, calls no port), the
# audience notice K-5, the shared-channel tick K-6, and the never-sent and
# never-pings rules applied to every message (D-3, DDR-12, R5, EIP-KPI-5).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# "Without notifying anyone" is judged on what Slack received (the double's
# oracle, support::slack_double::notifies_anyone): no Slack control sequence
# (<!channel>, <!here>, <!everyone>, <!subteam^…>, <@U…>, <#C…>) is live in any
# mrkdwn text, and every user-supplied string travels in plain_text objects
# with parse "none" and link_names false (DDR-12). The property over arbitrary
# text is DELIVER's layer-1 proptest in foundry-integrations (encode_slack);
# these rows are its acceptance-level key examples.

@external-integration-plugins @eip-02 @us-eip-04 @real-io
Feature: A team lead decides how much a message reveals, and sees it before saving

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And Identity Platform holds these issues
      | key     | title                              | lane        | filed by   |
      | AUTH-7  | Move Grafana sign-in to Keycloak   | In progress | Sam Okafor |
      | AUTH-12 | Rotate LLDAP bind password         | Backlog     | Sam Okafor |
    And the next issue filed in Identity Platform becomes AUTH-13
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack

  @pending @contract-shape:pure-function
  Scenario Outline: Lena sees exactly what the channel will receive before saving
    Given Lena is creating a Slack route from Identity Platform to #general with <level>
    When Lena previews the route
    Then the preview reads <lines>
    And Slack was asked nothing by the preview

    Examples:
      | level        | lines                                                                                     |
      | Link only    | "AUTH-12 · Identity Platform · New issue" with Open in Foundry, and no title, actor or lane |
      | Summary      | "AUTH-12 Rotate LLDAP bind password" / "New issue in Identity Platform by Sam Okafor · Backlog" with Open in Foundry |

  @pending @contract-shape:pure-function
  Scenario: A new route reveals a summary unless the lead chooses otherwise
    When Lena chooses #auth-dev as the destination of a new Slack route on Identity Platform
    Then the route form offers Link only, Summary and Summary with comment text, with Summary chosen
    And Lena reads "Everyone who can read #auth-dev will see what this route sends. Foundry cannot check who that is."

  @pending @kpi @contract-shape:bounded-change
  Scenario Outline: Comment text is sent only when chosen
    Given Lena has routed New comment on Identity Platform to #auth-dev with <level>
    When Sam comments "The bind DN changed; staging is green." on AUTH-7
    Then #auth-dev shows AUTH-7's comment message <shows> "The bind DN changed; staging is green."

    Examples:
      | level                      | shows       |
      | Link only                  | without     |
      | Summary                    | without     |
      | Summary with comment text  | with        |

  @pending @edge @contract-shape:bounded-change
  Scenario Outline: Comment text is cut at 300 characters
    Given Lena has routed New comment on Identity Platform to #auth-dev with Summary with comment text
    When Lena posts a <length>-character comment on AUTH-7
    Then #auth-dev shows <excerpt>

    Examples:
      | length | excerpt                                                     |
      | 299    | the whole comment                                           |
      | 300    | the whole comment                                           |
      | 301    | the comment's first 300 characters followed by "…"          |
      | 450    | the comment's first 300 characters followed by "…"          |

  @pending @error @kpi @contract-shape:bounded-change
  Scenario Outline: Text from people can never ping a channel
    Given Lena has routed every chat event on Identity Platform to #auth-dev with Summary with comment text
    When <action>
    Then #auth-dev shows that text without notifying anyone in the channel

    Examples:
      | action                                                                         |
      | Sam comments "@channel the LDAP bind is broken <!here>" on AUTH-7              |
      | Sam comments "<!everyone> <@U0LENA> look at <#C0AUTH> now" on AUTH-7           |
      | Sam comments "<!subteam^S0BACKEND\|@backend> *bold* _it_ <https://evil.example\|click>" on AUTH-7 |
      | Sam files "<!channel> LDAP outage" in Identity Platform                       |

  @pending @error @kpi @contract-shape:unbounded-preservation
  Scenario: Descriptions, attachments and email addresses are never sent
    Given Lena has routed every chat event on Identity Platform to #general with Link only
    And Lena has routed every chat event on Identity Platform to #auth-dev with Summary
    And Lena has routed every chat event on Identity Platform to #auth-releases with Summary with comment text
    And Sam has filed "Rotate the vault bind secret" in Identity Platform with the description "bind DN cn=admin,dc=canzan… password in vault path secret/ldap" and the attachment "ldap-bind.env"
    And Sam has moved AUTH-13 from Backlog to Todo
    When Sam comments "Done; see the attachment" on AUTH-13
    Then no message in any channel contains any part of AUTH-13's description, the name "ldap-bind.env", any email address, or any identifier other than AUTH-13

  @pending @error @contract-shape:unbounded-preservation
  Scenario: A channel shared with another organisation asks for confirmation
    When Lena chooses #ext-auth-vendor as the destination of a new Slack route on Identity Platform
    Then Lena reads "#ext-auth-vendor is shared with another organisation. Tick to confirm that is intended."

  @pending @error @contract-shape:bounded-change
  Scenario Outline: A route to a shared channel is saved only once its sharing is confirmed
    Given Lena is creating a Slack route from Identity Platform to #ext-auth-vendor
    And Lena <ticked> the shared-channel confirmation
    When Lena saves the route
    Then Identity Platform <has> a route to #ext-auth-vendor

    Examples:
      | ticked     | has          |
      | has ticked | has          |
      | left blank | still has no |

  # One row per content level, because each level builds a different message
  # body (Link only has no summary block, Summary with comment text adds the
  # excerpt), so a credential could slip into any one of them alone.
  @pending @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: No message carries a Foundry or Slack credential
    Given Lena has routed every chat event on Identity Platform to #auth-dev with <level>
    When Sam comments "Rotation done; the new bind secret is in place." on AUTH-7
    Then #auth-dev shows AUTH-7's comment message
    And no message Slack received carries any Slack credential or the secrets key

    Examples:
      | level                     |
      | Link only                 |
      | Summary                   |
      | Summary with comment text |
