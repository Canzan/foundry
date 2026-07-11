# Feature: Notification Delivery Providers — configure channels, emit once, deliver everywhere
#
# Personas:
#   Ops Olivia (olivia.okonkwo@acme.example) — SRE running Foundry for Acme; configures the active
#     providers via env and watches delivery on /metrics.
#   Dev Dan (dan.novak@acme.example) — emits one notification event and trusts delivery.
#   Maria Santos (maria.santos@acme.example) — downstream recipient (reset / invite / removed).
#
# This generalizes the shipped single EmailSender port (email.rs:19-22, only prod impl NoopEmailSender)
# into a pluggable NotificationProvider abstraction with a provider registry + config-driven selection,
# fan-out to all active providers, and per-provider best-effort failure isolation.
#
# Core invariants:
#   - A provider failure NEVER fails or blocks the originating request, and NEVER prevents another
#     provider from delivering (NFR-3).
#   - Provider secrets NEVER appear in logs, errors, metric labels, or Debug output (NFR-2).
#   - A listed-but-misconfigured or unknown provider FAILS FAST at startup; an unlisted provider is
#     inactive (NFR-1). With no providers configured, behavior equals today's no-op (NFR-5).
#
# Scope (v1 = slices 01-03): port + registry + Log provider, SMTP provider, fan-out + isolation +
# observability. Slices 04-06 (webhook, hosted API, new events) fast-follow. Recipient PREFERENCES are
# out of scope (successor feature recipient-notification-preferences).

Feature: Notification delivery providers — select channels, emit once, deliver everywhere

  Background:
    Given Foundry is deployed for the "Acme" organization
    And Acme runs an SMTP relay, a chat webhook, and a hosted email vendor

  # -------------------------------------------------- Capability 1: Configuration (operator, startup)

  Scenario: A password-reset notification is delivered through the selected log provider
    Given Olivia has set NOTIFICATION_PROVIDERS to "log" and started Foundry
    When a user submits a password-reset request for "maria.santos@acme.example"
    Then the reset notification is delivered through the log provider
    And Olivia sees one structured log line naming the event and recipient
    And the request returns its normal response

  Scenario: With no providers configured, delivery is a silent no-op
    Given Olivia has left NOTIFICATION_PROVIDERS unset
    When a user submits a password-reset request
    Then the request returns its normal response
    And no notification is delivered and no error is raised

  Scenario: An unknown provider name fails fast at startup
    Given Olivia has set NOTIFICATION_PROVIDERS to "logg"
    When Foundry starts
    Then startup aborts with an error naming the unknown provider and the known ones
    And the process exits non-zero

  Scenario: A provider missing a required setting fails fast without leaking a secret
    Given Olivia has listed "smtp" but not set SMTP_HOST
    When Foundry starts
    Then startup aborts naming the smtp provider and the missing SMTP_HOST
    And no secret value appears in the error
    And the process exits non-zero

  # ------------------------------------------------------- Capability 2: SMTP delivery (operator)

  Scenario: A reset email is delivered through the configured SMTP relay
    Given Olivia has configured the smtp provider against "smtp.acme.internal:587"
    When a user submits a password-reset request for "maria.santos@acme.example"
    Then a reset email is delivered from "foundry@acme.example" via the relay
    And the delivery is counted as provider "smtp", outcome "delivered"

  Scenario: With smtp inactive, no email is attempted
    Given "smtp" is not in NOTIFICATION_PROVIDERS
    When any existing notification fires
    Then no SMTP connection is attempted
    And behavior is identical to before this feature

  # ----------------------------------------------- Capability 3: Fan-out and isolation (v1 gate)

  Scenario: One notification fans out to all active providers
    Given Olivia has set NOTIFICATION_PROVIDERS to "log,smtp" and both are reachable
    When a bootstrap workspace invite is issued for "newadmin@acme.example"
    Then the invite notification is delivered through both the log and smtp providers
    And the delivery metric records one delivered outcome for each provider

  Scenario: One provider failing does not affect the others or the request
    Given NOTIFICATION_PROVIDERS is "log,smtp" and the smtp relay is unreachable
    When a user submits a password-reset request for "maria.santos@acme.example"
    Then the log provider still delivers the notification
    And the request returns its normal response
    And the metric records provider "smtp" outcome "failed" and provider "log" outcome "delivered"

  Scenario: A slow provider does not stall the originating request
    Given NOTIFICATION_PROVIDERS is "log,smtp" and the smtp relay hangs on connect
    When a user submits a password-reset request
    Then the request returns its normal response without waiting on the slow provider
    And the smtp delivery is counted as a failure

  Scenario: Every existing notification fans out through the abstraction
    Given NOTIFICATION_PROVIDERS is "log,smtp"
    When a member invite, a bootstrap invite, and a password reset each fire
    Then each is delivered through both active providers
    And each delivery is counted per provider and event

  Scenario: Per-provider delivery is visible on /metrics
    Given NOTIFICATION_PROVIDERS is "log,smtp" and several notifications have fired
    When Olivia scrapes /metrics
    Then foundry_notification_deliveries_total is present with provider, event, and outcome labels
    And the counts reflect the delivered and failed outcomes per provider

  # ------------------------------------------------ Capability 4: Webhook delivery (slice 04)

  Scenario: A notification is posted to the configured webhook
    Given Olivia has activated the webhook provider with a valid WEBHOOK_URL
    When a member invite is issued for "sam.okafor@acme.example"
    Then a JSON payload describing the event is POSTed to the webhook URL
    And the delivery is counted as provider "webhook", outcome "delivered"

  Scenario: A failing webhook receiver is isolated
    Given the webhook provider is active and the receiver returns HTTP 500
    When a notification fires
    Then the delivery is counted as provider "webhook", outcome "failed"
    And the originating request and other providers are unaffected

  # --------------------------------------------- Capability 5: Hosted email API (slice 05)

  Scenario: A reset email is delivered through the hosted email API
    Given Olivia has configured the email_api provider against a hosted vendor endpoint
    When a user submits a password-reset request for "maria.santos@acme.example"
    Then the reset email is sent via the vendor API from "foundry@acme.example"
    And the delivery is counted as provider "email_api", outcome "delivered"

  Scenario: A vendor rate-limit response is isolated and not retried in v1
    Given the email_api provider is active and the vendor returns HTTP 429
    When a notification fires
    Then the delivery is counted as provider "email_api", outcome "failed"
    And the request and other providers are unaffected
    And no automatic retry is attempted

  # ------------------------------------------------ Capability 6: New event types (slice 06)

  Scenario: Removing a member notifies that person through configured channels
    Given NOTIFICATION_PROVIDERS is "log,smtp" and an admin removes "maria.santos@acme.example" from "Northwind"
    When the remove-member action completes
    Then a member_removed notification is delivered to Maria through both providers
    And each delivery is counted with event "member_removed"

  Scenario: Changing a password notifies the account owner
    Given "maria.santos@acme.example" changes her password with at least one provider active
    When the password change completes
    Then a password_changed notification is delivered to her through the active providers
    And each delivery is counted with event "password_changed"

  # ------------------------------------------------------------ Security / operability properties

  @property
  Scenario: A provider failure never fails or blocks the originating request
    Given any active provider set with one provider guaranteed to fail
    When any notification is emitted
    Then the originating request returns its normal response
    And every other active provider still delivers
    And no request failure is attributable to delivery

  @property
  Scenario: Provider secrets never appear in any observable output
    Given the smtp, webhook, and email_api providers are all active and configured with secrets
    When a full deliver cycle runs across every provider
    Then no SMTP password, webhook signing secret, or API key value appears in any log line
    And none appears in any error, metric label, or Debug output

  @property
  Scenario: Fan-out is complete and counted for every active provider
    Given N active providers and one emitted notification
    When delivery runs
    Then exactly N delivery attempts occur
    And exactly N counter increments are recorded, split by outcome

  @property
  Scenario: Config validation fails fast and unlisted providers stay inactive
    Given a provider listed with a missing required setting, an unknown provider name, and an unlisted provider
    When Foundry starts
    Then startup aborts non-zero with a provider-named, secret-free error for the misconfigured and unknown ones
    And the unlisted provider is never constructed

  @property
  Scenario: Delivery metric labels stay within their bounded domains
    Given every notification the catalog can emit is delivered through every provider kind
    When /metrics is scraped
    Then the provider, event, and outcome labels never exceed their bounded sets
    And a cardinality check fails closed if an unbounded label is introduced
