# Feature: token-mutations-metric-export
#
# JTBD: outcome-1 (operators stand up Foundry and confirm it's healthy
#       within an hour — an empty Grafana panel is a deploy-time
#       correctness failure). This feature closes the last register-at-0
#       gap left by slice-8: the per-principal revoke-storm guardrail
#       counter `foundry_token_mutations_total{principal,outcome}`
#       (emitted by `RateLimiter::check` on every revoke decision) was
#       ALREADY wired and served on `/metrics`, but it was never
#       registered/described at startup. On a fresh instance — before any
#       revoke has happened — the metric was ABSENT from the scrape, so
#       the "token mutations" Grafana panel showed "no-data" until the
#       first revoke. This is the same "panel never shows no-data"
#       contract slice-8 enforced for its five metrics (ADR-018 / D4);
#       slice-8 hardened five OTHER metrics and explicitly deferred this
#       one.
#
# The metric (contract PRESERVED — labels unchanged):
#   | Metric                          | Type    | Labels              |
#   | foundry_token_mutations_total   | counter | principal, outcome  |
#
# Cardinality tradeoff (documented, NOT changed here):
#   The `principal` label is per-UUID (unbounded cardinality) —
#   intentional for per-principal abuse attribution (rate-guardrail.md
#   §Metric / OD-TMA-1b). It is bounded in practice by the count of
#   ACTIVE principals plus the shipped per-principal bucket eviction
#   (ADR-005 idle + LRU). A bounded-aggregate variant (drop `principal`,
#   keep `outcome`) is a deferred follow-up IF dashboard cardinality
#   becomes a concern; the shipped {principal,outcome} contract is not
#   broken now. The startup baseline therefore registers at 0 under a
#   sentinel `system` principal (the per-principal label makes a concrete
#   zero series awkward otherwise) — empirically the mechanism that makes
#   the family appear at zero on a fresh scrape.
#
# Inheritance (slice 6 / slice 8 — metrics harness, reused VERBATIM):
#   - `metrics_exporter_prometheus` recorder + `/metrics` sidecar are
#     already wired; the real recorder lives ONLY in the subprocess (the
#     in-process harness skips `install_recorder()`), so every assertion
#     scrapes the subprocess `/metrics` — the operator's observable port.
#   - `support/metrics_scrape.rs` parser + `poll_until_sample` +
#     `sum_for` / `contains_metric_line` / `label_keys_for` — reused.
#   - Reused step phrases (registered by slice-6
#     `handler_instrumentation.rs`): `the operator's foundry instance is
#     running`, `the operator scrapes the metrics endpoint immediately`,
#     `the scrape returns HTTP 200`, `the scrape body contains the line
#     "..."`, `the scrape body's "..." samples carry only the label keys
#     "..."`.
#
# Layer / PBT mode declaration (per nw-test-design-mandates Mandate 9):
#   - Layer 3+ (real subprocess + real HTTP scrape + real Postgres +
#     real machine-token auth + real DELETE revoke). Example-only — PBT
#     for the bucket arithmetic lives at unit level in
#     `crates/foundry-app/src/rate_limit.rs` (the existing proptest).
#
# ROBUST METRIC ASSERTIONS (slice-6/8 hard-won lesson): the counter is
# incremented at a real event chokepoint (a revoke decision) and the
# suite runs scenarios concurrently, so a one-shot exact scrape flakes.
# Both arms use bounded polls:
#   - register-at-0: assert the LINE is present immediately (HTTP 200 +
#     contains-the-line), never a racy one-shot.
#   - tick-on-mutation: the counter is monotonic — bounded poll
#     "eventually reaches at least 1" via poll_until_sample (>=), immune
#     to scrape-vs-emit timing drift.

@token-mutations-metric @metrics @nfr-tma-sec-07 @observability
Feature: The per-principal revoke guardrail counter foundry_token_mutations_total is registered at zero on a fresh instance so the Grafana panel never shows no-data, and ticks with its {principal,outcome} labels on a real revoke
  An operator who stands up Foundry and opens the bundled Grafana
  dashboard sees the "token mutations" panel resolve to real data from
  the first scrape: a flat-zero baseline before any revoke (so an empty
  panel always means "no mutations yet", never "metric never wired"),
  then a per-principal tick the moment a management bearer revokes a
  token. The {principal,outcome} contract is the shipped per-principal
  abuse-attribution signal and is preserved unchanged.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-3

  @real-io @startup-register
  Scenario: The token-mutations counter is scrapable at zero on a fresh instance so the panel never shows no-data
    # register-at-0 contract (mirrors slice-8 ADR-018 / D4). On a fresh
    # instance no revoke has happened, yet the metric family must be
    # present from the first scrape so Grafana never shows no-data —
    # asserted by HTTP 200 + contains-the-line. This is the arm that is
    # genuinely RED on current code: the live emission in
    # `RateLimiter::check` only mints the series on the first revoke, so
    # before any revoke the family is ABSENT from `/metrics`. The startup
    # registration makes it present at zero.
    Given the operator's foundry instance is running
    When the operator scrapes the metrics endpoint immediately
    Then the scrape returns HTTP 200
    And the scrape body contains the line "foundry_token_mutations_total"
    And the scrape body's "foundry_token_mutations_total" samples carry only the label keys "outcome,principal"

  @real-io @tick-on-mutation
  Scenario: A real token revoke ticks the per-principal mutation counter with its {principal,outcome} labels
    # The live-emission half: a management bearer revokes a token over
    # the API; the revoke decision flows through `RateLimiter::check`,
    # which increments foundry_token_mutations_total for the calling
    # principal. The counter is monotonic, so the assertion is "eventually
    # reaches at least 1" via bounded-poll (immune to scrape-vs-emit
    # timing drift). The {principal,outcome} labels are the shipped
    # per-principal attribution contract — preserved unchanged.
    Given the operator's foundry instance is running
    And a management bearer for "devansh@acme.com" with a revocable token in the "Backend" team's "Auth v2" project exists
    When the management bearer revokes that token over the API
    Then the scrape body's "foundry_token_mutations_total" sample is eventually at least 1 within 10 seconds
    When the operator scrapes the metrics endpoint
    Then the scrape body's "foundry_token_mutations_total" samples carry only the label keys "outcome,principal"
