# Feature: external-integration-plugins, slice 05 (US-EIP-10, US-EIP-11) — a
# member chooses which project events Foundry emails them, and the emails stop
# the moment they should, including from the email itself (D-9, D-14, OD-8,
# OD-10, OD-11, SC-5, DDR-15, DDR-16, ADR-EIP-007).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
# Pre-requisite (D-9, C-10): notification-preferences-ui is finished first.
#
# Harness as in external-integration-plugins-01-connect-slack.feature, with
# email through the in-process recorder (the notify_recorder pattern, extended
# with headers for the EmailTransport port). Stop links never expire (DD-7,
# DESIGN UI-2): US-EIP-11's "expired" link is not a case. The one-click
# unsubscribe is the mail client's cookie-less POST to the address in the
# email's List-Unsubscribe header (RFC 8058, DDR-16).
#
# Subjects for New issue, Issue moved and Issue deleted emails are DESIGN's
# proposal UI-4, not yet locked: the scenarios judge those emails by their
# K-12 footer, which is locked. The @needs-smtp scenario proves the headers on
# the wire through the shipped SmtpProvider to a pinned mailpit (DEVOPS-4).

@external-integration-plugins @eip-05 @us-eip-10 @us-eip-11 @real-io
Feature: A member chooses which project events Foundry emails them, and stops any of them in one click

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Tomás Rivera | member         | Backend as member             |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And Identity Platform holds these issues
      | key    | title                              | lane        | filed by   |
      | AUTH-7 | Move Grafana sign-in to Keycloak   | In progress | Sam Okafor |
      | AUTH-9 | Retire the old bind account        | Todo        | Lena Park  |
    And the next issue filed in Identity Platform becomes AUTH-20
    And it is 14:02 on 21 Oct 2026
    And the instance can send email

  @us-eip-10 @walking_skeleton @driving_port @kpi @contract-shape:bounded-change
  Scenario: A member chooses an event for a project and gets the email it describes
    Given Sam has chosen New comment for issues he created or commented on in Identity Platform
    When Lena comments "The new bind DN is live on staging; can you verify the Grafana login?" on AUTH-7
    Then Sam receives within 2 minutes an email "[AUTH-7] Lena Park commented: Move Grafana sign-in to Keycloak"
    And its footer reads "You get this because you chose New comment for issues you created or commented on in Identity Platform. Stop these emails · Change what you get"
    And it carries the comment "The new bind DN is live on staging; can you verify the Grafana login?" and Open in Foundry

  @us-eip-10 @driving_port @kpi @contract-shape:bounded-change
  Scenario: Saving shows exactly what will be emailed, three steps from the board
    Given Sam has no project email choices and has followed Settings in the sidebar of the Identity Platform board
    When Sam ticks New comment for issues he created or commented on in Identity Platform and saves
    Then Sam reads "Saved. Foundry will email sam.okafor@canzan.dev about: New comment (issues you created or commented on) in Identity Platform."
    And Sam's Project emails show New comment chosen for Identity Platform

  @us-eip-10 @contract-shape:pure-function
  Scenario: A member sees only projects they can open, with nothing chosen for them
    When Sam opens Project emails in his settings
    Then Sam is offered Identity Platform and not Homelab Ops
    And no event is ticked for Identity Platform
    And Identity Platform offers New issue, Issue moved, New comment and Issue deleted, for all issues or only issues he created or commented on

  @us-eip-10 @kpi @contract-shape:bounded-change
  Scenario Outline: Each chosen event emails what happened and why
    Given Sam has chosen <event> for all issues in Identity Platform
    When <action>
    Then Sam receives within 2 minutes an email about <issue> whose footer reads "You get this because you chose <event> for all issues in Identity Platform. Stop these emails · Change what you get"

    Examples:
      | event         | action                                                       | issue   |
      | New issue     | Lena files "Rotate the bind password" in Identity Platform   | AUTH-20 |
      | Issue moved   | Lena moves AUTH-9 from Todo to Done                          | AUTH-9  |
      | New comment   | Lena comments "Retiring on Friday." on AUTH-9                | AUTH-9  |
      | Issue deleted | Lena deletes AUTH-9                                          | AUTH-9  |

  @us-eip-10 @edge @contract-shape:bounded-change
  Scenario Outline: "Issues I created or commented on" means exactly that
    Given Sam has chosen Issue moved for issues he created or commented on in Identity Platform
    And <involvement>
    When Lena moves AUTH-9 from Todo to Done
    Then Sam <receives> about AUTH-9

    Examples:
      | involvement                                                   | receives                          |
      | Sam has never touched AUTH-9                                  | receives no email                 |
      | Sam has commented "I can take this." on AUTH-9                | receives within 2 minutes an email |
      | Sam has commented "I can take this." on AUTH-9 and deleted it | receives no email                 |

  @us-eip-10 @error @contract-shape:unbounded-preservation
  Scenario Outline: A member is not emailed about their own actions
    Given Sam has chosen <event> for all issues in Identity Platform
    When <action>
    Then Sam receives no email about <issue>

    Examples:
      | event       | action                                                     | issue   |
      | New comment | Sam comments "Verified on staging." on AUTH-7              | AUTH-7  |
      | New issue   | Sam files "Rotate the bind password" in Identity Platform  | AUTH-20 |
      | Issue moved | Sam moves AUTH-7 from In progress to Done                  | AUTH-7  |

  @us-eip-10 @error @contract-shape:unbounded-preservation
  Scenario: A muted workspace sends no project email
    Given Sam has chosen New issue for all issues in Identity Platform
    And Sam has muted Canzan Labs
    When Lena files "Rotate the bind password" in Identity Platform
    Then Sam receives no email about AUTH-20
    And Sam's Project emails read "Canzan Labs is muted. These choices apply when you resubscribe."

  @us-eip-10 @error @contract-shape:unbounded-preservation
  Scenario: An instance that cannot send email says so
    Given the instance cannot send email
    When Sam opens Project emails in his settings
    Then Sam reads "This Foundry cannot send email yet. An instance operator needs to set up email delivery."
    And Sam is offered no way to save project email choices

  @us-eip-10 @error @contract-shape:unbounded-preservation
  Scenario: A choice for a project the member cannot open is refused
    When Sam submits a project email choice of New issue for all issues in Homelab Ops
    Then Sam gets the same not-found page any unknown address gets
    And Sam has no project email choices
    And Sam's own choice for Identity Platform is still accepted

  @us-eip-11 @kpi @contract-shape:bounded-change
  Scenario: A member stops one kind of email from the email itself
    Given Sam has chosen New comment and New issue for all issues in Identity Platform
    And Sam has received a New comment email from Identity Platform
    And Sam has followed Stop these emails in it while signed out
    When Sam confirms the stop
    Then the page Sam confirmed on asked "Stop New comment emails from Identity Platform?"
    And Sam reads "Stopped: New comment in Identity Platform. Your other choices are unchanged."
    And Sam's Project emails show New issue chosen and New comment not chosen for Identity Platform

  @pending @us-eip-11 @kpi @contract-shape:unbounded-preservation
  Scenario: A stopped choice sends no more emails
    Given Sam has chosen New comment for all issues in Identity Platform
    And Sam has received a New comment email from Identity Platform
    And Sam has stopped New comment emails from Identity Platform from that email
    When Lena comments "One more thing." on AUTH-7
    Then Sam receives no email about AUTH-7

  @us-eip-11 @error @contract-shape:unbounded-preservation
  Scenario: Merely opening a stop link changes nothing
    Given Sam has chosen New comment for all issues in Identity Platform
    And Sam has received a New comment email from Identity Platform
    When a link scanner opens its Stop these emails link without confirming
    Then Sam's Project emails show New comment chosen for Identity Platform

  @pending @us-eip-11 @kpi @contract-shape:bounded-change
  Scenario: A mail client's one-click unsubscribe stops the same choice
    Given Sam has chosen New comment and New issue for all issues in Identity Platform
    And Sam has received a New comment email from Identity Platform
    When Sam's mail client sends the one-click unsubscribe of that email
    Then Sam's Project emails show New issue chosen and New comment not chosen for Identity Platform

  @us-eip-11 @error @kpi @contract-shape:unbounded-preservation
  Scenario: Losing access stops the emails without any setting changing
    Given Tomás has chosen New issue for all issues in Identity Platform
    And Sam has chosen New issue for all issues in Identity Platform
    And Tomás has been removed from Backend by Lena
    When Lena files "Rotate the bind password" in Identity Platform
    Then Tomás receives no email about AUTH-20
    And Sam receives within 2 minutes an email about AUTH-20 whose footer reads "You get this because you chose New issue for all issues in Identity Platform. Stop these emails · Change what you get"

  @us-eip-11 @error @contract-shape:unbounded-preservation
  Scenario Outline: A tampered stop link changes nothing and reveals nothing
    Given Sam has chosen New comment for all issues in Identity Platform
    And Sam has received a New comment email from Identity Platform
    When someone <uses> a copy of its stop link <tampered>
    Then they get the same refusal page any invalid stop link gets
    And Sam's Project emails show New comment chosen for Identity Platform

    Examples:
      | uses                    | tampered                                         |
      | opens                   | with its signature altered                       |
      | confirms                | with its signature altered                       |
      | confirms                | naming New issue instead of New comment          |
      | confirms                | naming Homelab Ops instead of Identity Platform  |
      | sends as a one-click to | with its signature altered                       |

  @us-eip-11 @error @contract-shape:unbounded-preservation
  Scenario: Stopping project emails never stops security emails
    Given Sam has chosen New comment for all issues in Identity Platform
    And Sam has received a New comment email from Identity Platform
    And Sam has stopped New comment emails from Identity Platform from that email
    When Sam asks for a password reset
    Then Sam receives the password-reset email

  @pending @us-eip-11 @needs-smtp @adapter-integration @kpi @contract-shape:bounded-change
  Scenario: A project email sent through the relay carries its one-click stop link
    Given the instance sends email through its SMTP relay
    And Sam has chosen New comment for all issues in Identity Platform
    When Lena comments "Relay check." on AUTH-7
    Then the relay holds one email to sam.okafor@canzan.dev carrying Foundry's one-click stop link
    And its footer reads "You get this because you chose New comment for all issues in Identity Platform. Stop these emails · Change what you get"
