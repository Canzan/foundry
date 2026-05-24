# Story: US-01 — Operator installs Foundry in under an hour
# Slice: 1 (Walking Skeleton)
# JTBD: outcome-1 (Minimize time to stand up a working issue tracker)
#
# Scope note: this feature exercises the docker-compose harness, NOT the
# in-process HTTP driver used by US-05..US-08. Driver design in driver.md
# section 4 (Postgres provisioning) and section 7 (time budget). Two
# scenarios are marked @docker-compose (slow; ~30-60s each); the third is
# marked @manual because verifying "an operator who has never seen Foundry
# completes install in under 30 minutes" requires a human-in-the-loop.

@slice1 @us-01 @walking_skeleton @driving_port @docker-compose
Feature: An operator stands up a healthy Foundry instance from a fresh machine
  An operator with Docker installed can run `docker compose up -d`, see the
  containers become healthy, and discover the admin bootstrap URL from logs
  — all without manual database initialisation or hidden steps.

  Background:
    Given an empty working directory with a Foundry checkout and a default `.env`
    And no Foundry containers or volumes exist on this machine

  @real-io @adapter-integration
  Scenario: A fresh-machine install becomes healthy and prints the bootstrap URL
    When the operator starts the Foundry stack with `docker compose up -d`
    Then within 300 seconds the foundry container reports healthy on `/healthz`
    And the postgres container reports healthy
    And the foundry container logs contain exactly one line beginning with `[BOOTSTRAP]`
    And that line contains a URL with a token query parameter

  @real-io
  Scenario: Re-running `docker compose up` after the admin is claimed prints no second bootstrap URL
    Given the operator has already claimed admin from a prior install
    When the operator runs `docker compose up -d` a second time
    Then the foundry container reports healthy on `/healthz` within 60 seconds
    And the foundry container logs contain zero new lines beginning with `[BOOTSTRAP]`

  @nfr-port-01 @real-io
  Scenario: The compose file uses no host-bind volumes for the app container
    When the operator inspects the foundry service definition in `docker-compose.yml`
    Then the foundry service declares zero host-bind mounts under `volumes`
    And the only persistent volume is a named volume backing postgres

  @manual @us-01 @demo
  Scenario: An evaluating operator reaches the admin claim form within 30 minutes
    # Manual scenario — verifies the JTBD outcome-1 "hour to demo" promise.
    # Reason for manual classification: requires a fresh human, a fresh VM, a
    # timer, and a yes/no judgement on the bootstrap URL being "discoverable".
    # Automation cost (full headless VM + timed human-substitute) is
    # disproportionate to the value for an MVP that has automated coverage of
    # the underlying capability via the @real-io scenarios above.
    Given an operator who has never used Foundry before, on a fresh Ubuntu VM
    When the operator follows the README quickstart without external help
    Then the operator reaches the bootstrap claim form within 30 minutes
    And the operator does not need to consult any documentation outside the README
