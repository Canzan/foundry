# DESIGN Decisions — instance-integration-setup

Read before designing (see options.md for the checklist):
- ✓ brief.md integration sections; ADR-EIP-004, -005, ADR-IW-001, -002
- ✓ the SSOT journey's step 0; discuss/ (D0a–D17); AGENTS.md Deployment independence
- ✓ integrations-wayfinding design; the code listed in options.md
- ⊘ discover/, diverge/ (neither ran)

Scope: application. Mode: propose. Every recommended option was accepted by the user on
2026-11-05, with the rulings below.

## Key Decisions

- [D1] **Two tiers, one cipher.**
  - The instance key, from `FOUNDRY_INSTANCE_KEY` (legacy `FOUNDRY_SECRETS_KEY` read as an alias)
    or a Foundry-made file, wraps a stored secrets key.
  - The secrets key seals credentials in the shipped `SecretBox` format.
  - The credential box also opens with every held instance key, so legacy direct seals keep
    opening.
  - The two names set to different values, or a malformed value, refuse start.
  - (adr-iis-001-two-tier-keys.md)
- [D2] **Ruling 1e.**
  - With no stored secrets key, the legacy `FOUNDRY_SECRETS_KEY` means **direct mode**, exactly
    today's behaviour: Matrix ready, new credentials sealed directly.
  - The page shows I-2e (I-2e0 when n = 0) and Move. Move with n = 0 just generates.
  - With the new name or a file and no stored key, the page shows I-3 and Generate.
  - Generate and Move are one use-case. (ADR-IIS-001)
- [D3] **Key file.**
  - `FOUNDRY_DATA_DIR`, default `/var/lib/foundry`; file `instance.key`, mode 0600, base64url.
  - Created by temp file, fsync, `link(2)`, then read-back.
  - Never created when a file-wrapped secrets key already exists (I-26 instead), and never
    overwritten.
  - An unreadable file shows new copy I-31. An operator key replacing the file: the file is held as
    the previous key, and the secrets key is re-wrapped at boot by compare-and-set (I-30).
  - (adr-iis-002-instance-key-file.md)
- [D4] **Slack app.**
  - Stored in the single-row `instance_slack_app`, secrets sealed. The environment's trio wins.
  - A saved app that does not open is not in effect (I-13; W-2 on workspace pages).
  - The I-14 switch is held sealed in a fenced pending row until confirmed.
  - A different client ID needs both secrets; with the same client ID a blank secret keeps the
    saved value. **Approved by the user 2026-11-05** (narrows discuss D7).
  - Remove is a no-op while the environment's app is in effect.
  - (adr-iis-003-slack-app-saved-by-instance-admin.md)
- [D5] **Live effect and atomicity.**
  - One advisory lock serialises instance key and app writes. Sealing writes take it shared and
    fence on the sealing key id, so Replace needs no grace window.
  - `integration_config_version` is bumped in the same transaction.
  - A reload task polls it every 5 s (`FOUNDRY_INTEGRATIONS_RELOAD_MS`). An unknown key id triggers
    an immediate reload. A held key whose id is unchanged is kept.
  - No separate spike: the walking skeleton runs on two replicas.
  - (adr-iis-004-live-reconfiguration-across-replicas.md)
- [D6] **Activity** is kept in the new `instance_integration_audit_events`
  (adr-iis-005-instance-integration-activity.md).
- [D7] **ADRs.**
  - ADR-IIS-001 to -005 are new.
  - Supersession notes are appended to ADR-EIP-004 (decision 1 and boot of 4; amends 3, 5 and 6)
    and ADR-EIP-005 (decision 1 / OD-3 in part).
  - An amendment note is appended to ADR-IW-001 (the I-19 dimension).
- [D8] **Smaller decisions.**
  - `IntegrationSettings` records whether `FOUNDRY_PUBLIC_URL` fell back (I-29), and
    `Integrations::slack_events_url` feeds the page and `slack::manifest`.
  - A `test-support` reader exposes the generated key under `integrations-test-seam`.
  - One shared boot function serves `main.rs`, the harness and reload.
  - Start over (B) moves unopenable connections to `disconnected`: no `lost` transition, no email.
  - I-26 on reload reuses `probe_failures_total{probe_name="secrets"}` and the degraded line.

## Peer review (2026-11-05, iteration 1: needs_revision, then fixed)

| Severity | Finding | Resolution |
|---|---|---|
| critical | Start over on a replica with a different key file could discard credentials that peers open | Gated on no live replica opening the key; `integration_replica_states` (ADR-IIS-004 r8, ADR-IIS-001 r7) |
| high | Mode keyed on the setting name stops a renamed legacy instance | `Direct` also follows direct seals; `runs_worker` follows any openable credential (ADR-IIS-001 r4) |
| high | Unopenable direct seals with no stored key had no state | I-4 in any mode; Generate and Move withheld under I-4 (r4, r5) |
| high | Reload read order could miss a commit | Version and state in one `REPEATABLE READ` snapshot, with a gold test (ADR-IIS-004 r4) |
| high | Pending I-14 app in the session sits outside the fence and the re-seal | `instance_slack_app_pending` table, in every sealed-value set (ADR-IIS-003 r4) |
| high | Fence placement and enforcement | In the store transaction; `integrations-seal-fence` rule; previous-release writes explained (ADR-IIS-004 r2) |
| high | D7 narrowed, upstream items unacknowledged | D7 narrowing approved by the user 2026-11-05. The remaining items await product-owner acknowledgement (upstream-changes.md) |
| medium | Minting, reload re-minting, byte round-trip, planted keys | Fixed in ADR-IIS-002 r3 and ADR-IIS-001 r2 and Enforcement |
| medium | No stale-replica signal | Replica state rows (`applied_version`); no new metric |
| low | ADR-IIS-002 enforcement, C4 canaries, 5 s vs 60 s | Fixed |

### Iteration 2 (needs_revision, then fixed)

| Item | Resolution |
|---|---|
| B1 Start over gate blind to previous-release replicas | Typed I-28a confirmation; gate re-evaluated inside the locked POST, with a gold test (ADR-IIS-001 r7) |
| B2 Temp file permissions | `O_CREAT\|O_EXCL` 0600 at open, leftover temp files removed at boot, mode asserted by the probe (ADR-IIS-002 r2) |
| B3 Pending insert unfenced | Shared lock and fence, in the seal-fence list, gold test (ADR-IIS-003 r4, ADR-IIS-004 r2) |
| B4 Table rule incomplete | `instance_slack_app_pending` and `integration_replica_states` added to `integrations-instance-tables` |
| B5 Remove with the environment app | No-op rule and gold test (ADR-IIS-003 r5) |
| N1 Wall time | Injected `Clock` everywhere (ADR-IIS-004 r8) |
| N2 Snapshot scope | Enumerated (ADR-IIS-004 r4) |
| N3 Lock ordering | Advisory lock before row locks (ADR-IIS-004 r2) |
| N4 Generate roll window | Documented, with alerting (ADR-IIS-001 r5) |
| N5 Pending expiry | Retention tick sweep |
| N6 `link` EPERM | I-27 (ADR-IIS-002 r2) |
| N7 Harness | brief.md "For Acceptance Designer" |
| N8 AC homes | ADR-IIS-003 r3 (US-IIS-03 AC 2) and r3a (US-IIS-01 AC 2) |

## Architecture Summary

- Pattern: the existing modular monolith with ports and adapters. No new crate or dependency.
- Paradigm: OOP Rust with `nw-software-crafter`, as before. Decisions are pure functions; the
  shell is the key file adapter, store transactions, the reload task and handlers.
- Components:
  - `foundry-app/integrations`: `key_file.rs`, key mode and keyring, `reload.rs`,
    `handlers/instance.rs`, the shared boot function.
  - `foundry-store/integrations/instance.rs` and migration 0033.
  - `foundry-auth/secret_box.rs`: names, aliases and generation.
  - `admin_cli.rs`: `rotate-secrets` re-wraps.
  - Templates: the instance page and confirms, plus the I-1 link on the instance dashboard.
- C4 (L1 and L2): see brief.md, "Instance integration setup".

## Technology Stack

- Unchanged: Rust, axum, askama, sqlx/Postgres 16, and `ring` AES-256-GCM (Apache-2.0 AND ISC).
- New: none. The filesystem uses `std` (link, fsync).

## Constraints Established

- The instance key never enters the database, a log or a page. The secrets key and Slack secrets
  are stored only sealed.
- Every instance key or app write holds `foundry.instance-integrations`, bumps the version and
  writes its activity line in one transaction.
- Every credential seal fences on the current sealing key id.
- A key file is never overwritten, and none is minted when a file-wrapped key already exists.
- Start is refused only for a malformed operator key or the two names set to different values.
  Every other key problem degrades, and the board serves.
- Only `key_file.rs` touches `FOUNDRY_DATA_DIR`, and only `foundry-store/integrations/instance.rs`
  names the four new tables. Both are check-arch rules with gold tests.
- Deployment independence: defaults and docs name no environment. The k8s example says, in general
  terms, to set the operator key or give every replica one shared data volume.

## For DEVOPS (platform-architect)

- The image creates `/var/lib/foundry` owned by `nonroot` and declares a volume.
- Compose mounts a named volume there.
- The k8s example puts `FOUNDRY_INSTANCE_KEY` in the Secret (a per-pod `emptyDir` gives each pod its
  own key).
- Document `FOUNDRY_INSTANCE_KEY`, `_PREVIOUS`, `FOUNDRY_DATA_DIR` and
  `FOUNDRY_INTEGRATIONS_RELOAD_MS`, and that `FOUNDRY_SECRETS_KEY` is a legacy alias.
- RELEASING R-1, R-2 and R-5 are reworded.
- No new metric. Alerting on `probe_failures_total{probe_name="secrets"}` covers I-26.
- External integrations: there is no new Slack call. The ADR-EIP-005 contract-test recommendation
  for the Slack Web API stands.

## Upstream Changes

See upstream-changes.md:
- new copy I-31, I-2e0 and I-14 (n = 1);
- a different client ID needs both secrets;
- I-31 is restore-only;
- a saved app that does not open reads W-2 on workspace pages;
- new route `…/slack/confirm`;
- two new settings.
