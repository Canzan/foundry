# Feature: external-integration-plugins, slice 01 (US-EIP-01, US-EIP-02,
# US-EIP-05) — a signed-out visitor is sent to sign in from every integration
# page, exactly as from any page that needs a session (the shipped
# `redirect_to("/sign-in")`); the uniform not-found of SC-6 is for people who
# are signed in without the right (the authority outlines of the 01 and 03
# files). The two public doors, Slack's events address and the stop link, are
# covered by the 03 broken-connection and 05 project-emails files.
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED). Numbered
# 115 in the feature delta's scenario list (added after the review of the
# lifted run; the list's numbers 1-114 are stable).
#
# Harness as in external-integration-plugins-01-connect-slack.feature.

@external-integration-plugins @eip-01 @us-eip-01 @us-eip-02 @us-eip-05 @real-io
Feature: A signed-out visitor is sent to sign in from every integration page

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up

  @error @contract-shape:unbounded-preservation
  Scenario Outline: A signed-out visitor is sent to sign in from every integration page
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    When a signed-out visitor opens <door>
    Then the visitor is sent to sign in, as from any page that needs a session
    And nothing changed for Canzan Labs

    Examples:
      | door                                        |
      | the workspace integrations page             |
      | the Slack setup page                        |
      | Add to Slack                                |
      | the integrations page on Identity Platform  |
      | the history of the route to #auth-dev       |
