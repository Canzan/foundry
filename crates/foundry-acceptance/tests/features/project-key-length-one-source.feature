# The sentence that refuses a project key must have exactly one home: the
# refusal the domain owns. A second copy anywhere in the web, services, api or
# store code drifts from it, so the architecture gate a contributor runs must
# refuse the tree that carries one. Law L7, architectural test T1.
#
# Observed from outside, through the gate's own command over a throwaway copy of
# the tree: the copy with a planted second sentence must be refused and named,
# the untouched copy must be accepted.

@project-key-length @one-source @real-io
Feature: The sentence that refuses a project key has one home

  Scenario Outline: A second copy of the refusal sentence is refused and named
    Given a throwaway copy of the codebase carries a second copy of the project key refusal sentence in the <area> code
    When a contributor runs the architecture gate over that copy
    Then the gate refuses the copy and names the file that holds the second sentence

    Examples:
      | area     |
      | web      |
      | services |
      | store    |

  Scenario: The codebase as it stands is accepted
    Given a throwaway copy of the codebase with no second sentence planted in it
    When a contributor runs the architecture gate over that copy
    Then the gate accepts the copy
