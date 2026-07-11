# Feature: notification-delivery-providers — generalize the single hard-wired
# email sender into a config-selected registry of NotificationProviders behind a
# concurrent, best-effort fan-out dispatcher, with per-provider delivery
# observability and no-secret-leakage.
#
# Source SSOT for docs/feature/notification-delivery-providers/distill/test-scenarios.md.
# Requirements SSOT: ../discuss/ (US-01..06, AC-01..06, the 5 @property criteria).
# Design SSOT: ../design/ (architecture.md + adr-001..007; the "Handoff to DISTILL"
# list this file pins). Structured port NotificationProvider{kind, deliver, probe};
# Notification{event, recipient, subject, body}; closed NotificationEvent catalog
# (password_reset, workspace_invite, member_invite, member_removed, password_changed);
# registry from NOTIFICATION_PROVIDERS + per-provider SMTP_*/WEBHOOK_*/EMAIL_API_*;
# concurrent JoinSet fan-out with per-provider timeout; infallible notify(); binary
# outcome {delivered,failed}; foundry_notification_deliveries_total{provider,event,outcome}
# on the /metrics sidecar; SecretString + no-Debug port (adr-006).
#
# Driving ports (Mandate 1 — every scenario enters through one, never an internal fn):
#   1. Operator config at the composition root — NOTIFICATION_PROVIDERS + per-provider
#      settings, loaded by build_notifier() (fail-fast on unknown/misconfigured).
#   2. A real shipped app flow — POST /forgot-password (signin.rs:235), the bootstrap
#      + member invites (bootstrap.rs:258, member_invites.rs:189), remove-member,
#      and password-change — each emitting ONE notification through notify().
#   3. The /metrics sidecar + the recording provider double — the observable side.
#
# HARNESS BOUNDARY (distill/acceptance-review.md): the app + Postgres are REAL
# (in-process axum harness + testcontainers, @real-io), mirroring how FakeEmailSender
# is wired today. External transports are IN-PROCESS TEST DOUBLES: a recording log
# provider, a local webhook receiver, and a fake SMTP / hosted-API recorder — NO real
# third-party SMTP/SendGrid calls. The webhook probe() is asserted to make NO POST
# (host-reachability only, N-ODD-3); the webhook happy path asserts a real POST to the
# local receiver. Fan-out is await-bounded so the synchronous recorder assertions hold.
#
# EVERY scenario is @pending: DELIVER removes the tag per-slice as it authors the port,
# registry, dispatcher, adapters, and metric seam and turns each GREEN (Outside-In).
# @pending is excluded from every lane (acceptance.rs filter_run), so this file keeps
# the @all lane green until DELIVER unskips slice-by-slice. Slice order = US-01 (walking
# skeleton) → US-02 (SMTP) → US-03 (fan-out + observability, the v1 gate) → US-04
# (webhook) → US-05 (hosted email API) → US-06 (new events).

@notification-delivery @driving_port
Feature: An operator selects delivery providers and every notification fans out to all of them
  An operator chooses which channels carry Foundry's notifications by listing them in
  configuration; each notification a member action triggers is delivered, best-effort and
  isolated, to every active channel, and each delivery is observable per provider and event —
  with no configured secret ever appearing in a log line, error, metric label, or debug output.

  Background:
    Given Foundry is serving workspace "Acme" with member "maria.santos@acme.example"

  # ── Slice 01 — US-01 walking skeleton: configure → emit → deliver → observe at N=1 ──

  @us-01 @walking_skeleton @real-io
  Scenario: A password reset is delivered through the operator's chosen log provider
    Given the operator has activated providers "log"
    When a member requests a password reset for "maria.santos@acme.example"
    Then the notification is delivered through the "log" provider
    And the delivery is recorded for provider "log", event "password_reset", outcome "delivered"
    And the request returns its normal response

  @pending @us-01 @real-io
  Scenario: With no providers configured, delivery is a silent no-op
    Given the operator has activated no providers
    When a member requests a password reset for "maria.santos@acme.example"
    Then the request returns its normal response
    And no notification is delivered
    And no error is raised

  @pending @us-01 @config @error @property @real-io
  Scenario: An unknown provider name fails fast at startup
    Given the operator has listed an unknown provider "logg"
    When Foundry starts up
    Then startup is refused and the process exits non-zero
    And the startup error names the unknown provider "logg" and the known providers
    And the startup error contains no secret value

  @pending @us-01 @security @real-io
  Scenario: A delivered log line carries no reset token or secret
    Given the operator has activated providers "log"
    When a member requests a password reset for "maria.santos@acme.example"
    Then the notification is delivered through the "log" provider
    And no reset token appears in the delivery log line

  # ── Slice 02 — US-02: real email behind the port through an SMTP relay ──────────────

  @pending @us-02 @real-io
  Scenario: A reset email is delivered through the configured SMTP relay
    Given the operator has activated providers "smtp"
    When a member requests a password reset for "maria.santos@acme.example"
    Then the notification is delivered through the "smtp" provider
    And the delivery is recorded for provider "smtp", event "password_reset", outcome "delivered"

  @pending @us-02 @error @real-io
  Scenario: A temporarily unreachable relay does not fail the request
    Given the operator has activated providers "smtp"
    And the "smtp" provider's endpoint is unreachable
    When a member requests a password reset for "maria.santos@acme.example"
    Then the request returns its normal response
    And the delivery is recorded for provider "smtp", event "password_reset", outcome "failed"

  @pending @us-02 @config @error @property @real-io
  Scenario: An SMTP provider missing a required setting fails fast at startup
    Given the operator has listed provider "smtp" without required setting "SMTP_HOST"
    When Foundry starts up
    Then startup is refused and the process exits non-zero
    And the startup error names provider "smtp" and the missing setting "SMTP_HOST"
    And the startup error contains no secret value

  @pending @us-02 @real-io
  Scenario: With SMTP inactive, no email is attempted and existing behavior is unchanged
    Given the operator has activated providers "log"
    When a member requests a password reset for "maria.santos@acme.example"
    Then no delivery is attempted through the "smtp" provider
    And the existing notification behavior is unchanged

  @pending @us-02 @security @real-io
  Scenario: The SMTP password never leaks across a delivery cycle
    Given the operator has activated providers "smtp"
    When a member requests a password reset for "maria.santos@acme.example"
    Then the "SMTP_PASSWORD" value never appears in any log, error, metric label, or debug output

  # ── Slice 03 — US-03 (v1 GATE): fan-out, best-effort isolation, per-provider metrics ─

  @pending @us-03 @property @real-io
  Scenario: One notification fans out to every active provider
    Given the operator has activated providers "log,smtp"
    When a bootstrap workspace invite is issued for "newadmin@acme.example"
    Then the notification is delivered through the "log" provider
    And the notification is delivered through the "smtp" provider
    And the delivery is recorded for provider "log", event "workspace_invite", outcome "delivered"
    And the delivery is recorded for provider "smtp", event "workspace_invite", outcome "delivered"

  @pending @us-03 @error @property @real-io
  Scenario: One failing provider affects neither the others nor the request
    Given the operator has activated providers "log,smtp"
    And the "smtp" provider's endpoint is unreachable
    When a member requests a password reset for "maria.santos@acme.example"
    Then the notification is delivered through the "log" provider
    And the request returns its normal response
    And the delivery is recorded for provider "smtp", event "password_reset", outcome "failed"
    And the delivery is recorded for provider "log", event "password_reset", outcome "delivered"

  @pending @us-03 @error @real-io
  Scenario: A slow provider does not stall the originating request
    Given the operator has activated providers "log,smtp"
    And the "smtp" provider's endpoint hangs on connect
    When a member requests a password reset for "maria.santos@acme.example"
    Then the request returns its normal response without waiting on the slow provider
    And the delivery is recorded for provider "smtp", event "password_reset", outcome "failed"

  @pending @us-03 @real-io
  Scenario: Every existing notification fans out through the abstraction
    Given the operator has activated providers "log,smtp"
    When a password reset, a bootstrap invite, and a member invite each fire
    Then each notification is delivered through the "log" provider
    And each notification is delivered through the "smtp" provider
    And each delivery is recorded per provider and event

  @pending @us-03 @real-io
  Scenario: The delivery metric is registered at zero on first scrape
    Given the operator has activated providers "log,smtp"
    When Foundry starts up
    Then the delivery metric is present on the metrics endpoint with every series at zero

  @pending @us-03 @property @real-io
  Scenario: The delivery metric labels stay bounded
    Given the operator has activated providers "log,smtp"
    When a member requests a password reset for "maria.santos@acme.example"
    Then the delivery metric labels stay within their bounded sets
    And a cardinality check fails closed on an unbounded label value

  # ── Slice 04 — US-04: webhook / generic HTTP POST provider ──────────────────────────

  @pending @us-04 @real-io
  Scenario: A notification is posted to the configured webhook
    Given the operator has activated providers "webhook"
    When a member invite is issued for "sam.okafor@acme.example"
    Then a JSON payload describing the event is posted to the webhook endpoint
    And the delivery is recorded for provider "webhook", event "member_invite", outcome "delivered"

  @pending @us-04 @probe @real-io
  Scenario: The webhook health probe makes no POST to the receiver
    Given the operator has activated providers "webhook"
    When Foundry starts up
    Then the webhook probe made no post to the receiver

  @pending @us-04 @security @real-io
  Scenario: A signed webhook payload carries a signature without leaking the secret
    Given the operator has activated providers "webhook"
    And the "webhook" provider is configured with a signing secret
    When a member invite is issued for "sam.okafor@acme.example"
    Then the delivery carries a signature header derived from the secret
    And the "WEBHOOK_SIGNING_SECRET" value never appears in any log, error, metric label, or debug output

  @pending @us-04 @error @real-io
  Scenario: A rejecting webhook receiver is isolated
    Given the operator has activated providers "log,webhook"
    And the "webhook" endpoint rejects the delivery
    When a member invite is issued for "sam.okafor@acme.example"
    Then the delivery is recorded for provider "webhook", event "member_invite", outcome "failed"
    And the request returns its normal response
    And the other active providers still deliver

  @pending @us-04 @config @error @property @real-io
  Scenario: A webhook provider missing its URL fails fast at startup
    Given the operator has listed provider "webhook" without required setting "WEBHOOK_URL"
    When Foundry starts up
    Then startup is refused and the process exits non-zero
    And the startup error names provider "webhook" and the missing setting "WEBHOOK_URL"

  # ── Slice 05 — US-05: hosted email vendor API provider ──────────────────────────────

  @pending @us-05 @real-io
  Scenario: A reset email is delivered through the hosted email API
    Given the operator has activated providers "email_api"
    When a member requests a password reset for "maria.santos@acme.example"
    Then the notification is delivered through the "email_api" provider
    And the delivery is recorded for provider "email_api", event "password_reset", outcome "delivered"

  @pending @us-05 @error @real-io
  Scenario: A vendor rate-limit response is isolated and not retried in v1
    Given the operator has activated providers "log,email_api"
    And the "email_api" endpoint rejects the delivery
    When a member requests a password reset for "maria.santos@acme.example"
    Then the delivery is recorded for provider "email_api", event "password_reset", outcome "failed"
    And the request returns its normal response
    And the other active providers still deliver
    And no automatic retry is attempted

  @pending @us-05 @config @security @error @property @real-io
  Scenario: A hosted email API missing its key fails fast without leaking it
    Given the operator has listed provider "email_api" without required setting "EMAIL_API_KEY"
    When Foundry starts up
    Then startup is refused and the process exits non-zero
    And the startup error names provider "email_api" and the missing setting "EMAIL_API_KEY"
    And the "EMAIL_API_KEY" value never appears in any log, error, metric label, or debug output

  # ── Slice 06 — US-06: two new catalog events route through the same abstraction ──────

  @pending @us-06 @real-io
  Scenario: Removing a member notifies that person through configured channels
    Given the operator has activated providers "log,smtp"
    When an admin removes member "maria.santos@acme.example" from "Acme"
    Then the notification is delivered through the "log" provider
    And the notification is delivered through the "smtp" provider
    And the delivery is recorded for provider "log", event "member_removed", outcome "delivered"

  @pending @us-06 @real-io
  Scenario: Changing a password notifies the account owner
    Given the operator has activated providers "log"
    When member "maria.santos@acme.example" changes their password
    Then the notification is delivered through the "log" provider
    And the delivery is recorded for provider "log", event "password_changed", outcome "delivered"

  @pending @us-06 @error @property @real-io
  Scenario: A new event flows through fan-out and isolation like the existing ones
    Given the operator has activated providers "log,smtp"
    And the "smtp" provider's endpoint is unreachable
    When an admin removes member "maria.santos@acme.example" from "Acme"
    Then the notification is delivered through the "log" provider
    And the request returns its normal response
    And the delivery is recorded for provider "smtp", event "member_removed", outcome "failed"

  @pending @us-06 @property @real-io
  Scenario: The event label set stays bounded as the catalog grows
    Given the operator has activated providers "log"
    When member "maria.santos@acme.example" changes their password
    Then the delivery metric labels stay within their bounded sets
    And a cardinality check fails closed on an unbounded label value
