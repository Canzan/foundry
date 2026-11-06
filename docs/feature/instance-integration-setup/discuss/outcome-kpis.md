# Outcome KPIs: instance-integration-setup

## Objective

An instance admin turns on Slack and Matrix from the browser, on any deploy,
with no restart. No key or secret is ever shown or stored in plaintext, and no
existing connection breaks on the way.

The instance has no product analytics (System Constraints). Each KPI is
measured by acceptance scenarios on every gate run, and once by a manual walk
on a deployed instance after release. This follows the integrations-wayfinding
precedent and the persona's rule: a store query, an acceptance assertion, or
her own report.

## Outcome KPIs

| ID | Who | Does what | By how much | Baseline | Measured by | Type |
|----|-----|-----------|-------------|----------|-------------|------|
| KPI-IIS-1 | Instance admin with an instance key | Turns on Matrix (secrets key) and Slack (app) so that a workspace page offers Connect Matrix and Add to Slack | 0 restarts and 0 environment edits. Every replica shows the change within 60 s of the save. | 2 environment edits (key, SLACK_*) and 1 restart or roll per change | Acceptance with two app processes on one database: poll `/workspace/integrations` on the second | Leading |
| KPI-IIS-2 | Instance admin | Reads why integrations are off, and the next step, on one page | 100% of the 17 instance states show their I-n, 0 unexplained. Instance key: operator (I-25), file (I-25), replaced file (I-30), none (I-27), file unreadable (I-31). Secrets key: set (I-2), direct mode (I-2e, or I-2e0 when nothing is saved), none (I-3), does not open (I-26), saved credentials unreadable (I-4). Slack: none (I-5), saved (I-6), environment (I-7), saved but unreadable (I-13), public URL unset (I-29). Matrix: ready (I-8), needs key (I-9). | 0%: only W-2's "not set up" | Acceptance outline over the states | Leading |
| KPI-IIS-3 | Instance admin who meets W-2 or W-3 | Reaches the page that fixes it | ≤ 1 click from the workspace page and the route form. "How to set up Slack" stays (KPI-IW-3 unchanged). | No path. The guide says to restart. | Acceptance outline: I-19 present for an instance admin, and following it answers 200 | Leading |
| KPI-IIS-4 | Workspaces connected to Slack or Matrix | Are not broken unannounced by an instance-level change (replace key, switch or remove app, start over) | 100% of such changes show the affected count before applying. 0 connections marked lost and 0 admin emails caused by a key or app change. | Blind environment edits | Acceptance (confirm pages, connection state, mail double) | Guardrail-leading |
| KPI-IIS-5 | Instance configured before this feature | Keeps delivering through the upgrade and the move under a secrets key | 100% of pre-existing credentials open after the upgrade. 0 refused deliveries or events attributable to the upgrade or the move. | n/a (new path) | Acceptance: seal under the old key, upgrade, move, deliver | Leading |
| KPI-IIS-6 | Admin of a standalone deploy (single binary or compose) | Reaches "Matrix ready" with zero key settings | 1 click (Generate) after first start. The page names the key file to back up. | Integrations off until an operator reads RELEASING | Acceptance (subprocess with a temp data directory) | Leading |
| KPI-IIS-G1 | Everyone | Never sees or recovers a key or secret | 0 occurrences of the planted secrets key, instance key, client secret or signing secret in pages, page source, logs or activity (no integrations API exists or is in scope). A database dump alone opens 0 credentials. | EIP-KPI-5 (credentials) | Planted-string grep: generated keys are read through the feature-gated test seam (`integrations-test-seam`), plus an open attempt on a database dump without the instance key | Guardrail |
| KPI-IIS-G2 | Non-instance-admins | Are never offered a link they cannot open | 0 offered links that answer 404 (I-1, I-19). `/admin/instance/integrations` gives the uniform 404 to all others. | KPI-IW-G1 | Outline over Priya, Tomás, Lena, Sam, Marco and signed out | Guardrail |
| KPI-IIS-G3 | Existing behaviour | Environment-configured Slack and key keep working | external-integration-plugins and integrations-wayfinding acceptance 100% pass | 100% | `cargo xtask ci` | Guardrail |
| KPI-IIS-M | The user, after release | On a deployed instance with an instance key and nothing else, turns on Matrix and Slack from the browser | Reads I-21 and I-10. Connect Matrix and Add to Slack appear with no restart. | W-2 for both | Manual walk | Manual |

## Metric Hierarchy

- **North Star:** KPI-IIS-1, integrations turned on with zero restarts and zero
  environment edits.
- **Leading indicators:** KPI-IIS-2, KPI-IIS-3, KPI-IIS-6.
- **Guardrails:** KPI-IIS-G1 (no plaintext, no echo), G2 (no 404 links), G3,
  KPI-IIS-4 and KPI-IIS-5 (nothing breaks).

## Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| 1, 4, 5 | acceptance harness (two processes, one database; mail double; Slack double) | assertions | every gate run | DISTILL |
| 2, 3, G2 | acceptance outlines | assertions | every gate run | DISTILL |
| 6 | subprocess lane with a temp data directory | assertions | every gate run | DISTILL |
| G1 | page, log, activity and API capture; database read without the instance key | planted-string grep and an open attempt | every gate run | DISTILL |
| G3 | existing suites | `cargo xtask ci` | every gate run | DELIVER |
| M | the user's walk | report | once after release | user |

DEVOPS: no new metrics required. Consider reusing `foundry_integration_secrets_degraded`
for I-26, so an operator is alerted when a replica holds a different instance
key (DESIGN or DEVOPS).

## Hypothesis

We believe that an instance integrations page with a generated secrets key
(encrypted under an instance key that never lives in the database) and a Slack
app form, applied live, will let instance admins turn on integrations without
touching the environment. We will know this is true when an instance admin goes
from W-2 to Connect Matrix and Add to Slack with 0 restarts (KPI-IIS-1) and 0
keys exposed (KPI-IIS-G1).
