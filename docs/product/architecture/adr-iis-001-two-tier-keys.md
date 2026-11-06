# ADR-IIS-001: An instance key that never lives in the database wraps a stored secrets key; the secrets key seals credentials

## Status

Proposed (instance-integration-setup DESIGN, 2026-11-05). The options were chosen by the user on
2026-11-05 (`docs/feature/instance-integration-setup/design/options.md`, decisions 1a, 1d, 1e, 4).
Supersedes ADR-EIP-004 decision 1 and the boot part of decision 4. Amends decisions 3, 5 and 6.
Decision 2 (sealed format, key ids, row-bound AAD) is unchanged and reused.

## Context

- D1: an instance admin creates and replaces the **secrets key** in the UI. Foundry stores it
  encrypted under an **instance key** that never lives in the database. A dump alone opens nothing
  (OD-12 kept).
- D5: an existing `FOUNDRY_SECRETS_KEY` becomes the instance key. Credentials sealed directly with
  it, or with `_PREVIOUS`, keep opening indefinitely. Moving them is an explicit button, never
  automatic at first start, because previous-release replicas know only the environment key.
- D2/D3: keys are generated (32 random bytes), never typed or shown.
- D4: Replace re-seals everything all-or-nothing. Nobody reconnects.
- D10: replicas catch up within 60 s. Meanwhile no connection is marked lost and no email is sent.
- Today a fresh instance with only `FOUNDRY_SECRETS_KEY` offers Matrix at once. The existing suites
  rely on this (KPI-IIS-G3).

## Decision

1. **Settings.**
   - `FOUNDRY_INSTANCE_KEY` holds 32 random bytes, base64url, padded or not.
     `FOUNDRY_INSTANCE_KEY_PREVIOUS` is a comma-separated list, accepted for opening only.
   - The legacy `FOUNDRY_SECRETS_KEY` / `FOUNDRY_SECRETS_KEY_PREVIOUS` are read as aliases.
   - Both names set to different values refuses start (`health.startup.refused`,
     `probe = "secrets"`), naming both settings.
   - A malformed value refuses start naming the setting actually used, never its value.
   - A missing value is not an error. The key file (ADR-IIS-002) applies.
2. **Two tiers, one cipher.**
   - The secrets key is wrapped with `SecretBox` under the instance key. Slot:
     `integration_secrets_key|1|wrapped_key`.
   - The value wrapped is the key's base64url text, the operator-key format. `open` returns UTF-8
     text, so the raw bytes would not round-trip. A unit test round-trips generate, wrap, unwrap and
     parse.
   - It is stored in the single-row table `integration_secrets_key`. Columns: `id = 1`,
     `key_id`, `wrapped_key`, `wrapped_by_key_id`, `wrapped_by_source`
     (`operator | file`), `rewrapped_from_path`, `created_by`, `created_at`.
   - A second current key is non-representable.
3. **The credential box.**
   - `current` is the secrets key. `previous` holds every instance key this server holds (the
     operator key, the operator's previous keys and a superseded key file).
   - Legacy direct seals therefore open with no action. A sealed value's key id says which tier
     sealed it. "Sealed directly" (I-2e) is a key-id comparison, not a stored flag.
4. **Key mode** (one closed type, derived at boot and on every reload):

   | Mode | When | Page |
   |---|---|---|
   | `NoInstanceKey(Unwritable \| Unreadable)` | no operator key, and the file cannot be made or read | I-27 / I-31 |
   | `Direct` | no stored secrets key, and either the legacy name is in use or some stored value is sealed directly with a held instance key | I-2e (I-2e0 when n = 0) + Move |
   | `AwaitingSecretsKey` | no stored secrets key, the new name or a file, and no direct seals | I-3 + Generate |
   | `Sealed` | stored secrets key unwrapped | I-2 |
   | `Unopenable` | stored secrets key, no held instance key unwraps it | I-26 (restore-only in A) |

   - `Direct` is today's behaviour exactly: Matrix ready, new credentials sealed under the instance
     key (ruling 1e).
   - Direct seals also select `Direct`. Renaming the setting with the same value therefore never
     stops a legacy instance.
   - Both names set to the same value count as the legacy name.
   - Precedence: `NoInstanceKey` is judged first. Then a stored key (`Sealed` or `Unopenable`). Then
     `Direct` or `AwaitingSecretsKey`.
   - Sealing is possible only in `Direct` and `Sealed`, and the Matrix and Slack offers key off that
     fact.
   - `runs_worker` holds whenever this server holds any key that opens a stored credential, so
     openable deliveries never stop.
   - In any mode, stored values whose key id this server does not hold show I-4 (the degraded probe,
     unchanged).
5. **Generate and Move are one use-case** (`POST …/secrets-key`, `POST …/secrets-key/move`).
   - Create-if-absent the secrets key.
   - Re-seal every value whose key id is an instance-key id.
   - Write the canary.
   - Record the activity line (I-18 "generated" when n = 0, else "moved n").
   - Bump the config version.
   - One transaction under the instance-integrations lock (ADR-IIS-004). Move with n = 0 is
     Generate.
   - Generate and Move are withheld while I-4 holds (some stored value opens under no held key),
     as Replace is under I-26. The remedy is I-4's: restore the previous instance key.
   - **Roll window.** A previous-release replica knows only the environment key. After Generate or
     Move it cannot open anything sealed under the secrets key.
     - Its deliveries are `Transient(secret_unavailable)`: never lost, never emailed, bounded by
       the 24 h give-up.
     - It logs the degraded line, and `probe_failures_total{probe_name="secrets"}` rises on its
       next boot probe.
     - D5 already says to press Move after the roll; the same advice applies to Generate on an
       instance that is mid-upgrade.
     - DEVOPS alerts on `probe_failures_total{probe_name="secrets"}` and the `health.degraded
       probe=secrets` line.
6. **Replace (B).**
   - New key: re-seal every sealed value (connections and the saved Slack app) and write the new
     canary.
   - Overwrite the row (the old key is gone in the same transaction), record the activity line and
     bump the version.
   - No grace window: the seal fence (ADR-IIS-004) guarantees nothing is sealed under the old key
     after commit.
   - Withheld under `Unopenable`.
7. **Start over (B)** turns EIP-004 decision 6 into a confirmed action.
   - It is offered and accepted only when this server is `Unopenable` **and** no live replica opens
     the stored secrets key.
   - Live means a row in `integration_replica_states` (ADR-IIS-004 rule 8) that beat within the
     last 30 s and reports it opens.
   - Otherwise the page keeps I-26 restore-only. A replica with a different key file must never
     discard credentials its peers can open.
   - Previous-release replicas never write `integration_replica_states`, so the gate cannot see
     them. I-28 therefore carries a required typed confirmation that no server of an earlier Foundry
     release is running (copy I-28a, upstream-changes.md). A POST without it changes nothing and
     answers I-28 again.
   - The gate (a fresh beat by the injected `Clock`, and "opens") is evaluated on the `GET` for
     display. It is evaluated **again inside the POST's transaction**, after taking the
     instance-integrations lock. A replica that began opening the key between the two refuses
     the POST, which changes nothing and answers the page.
   - Gold test: a replica state row flips to "opens" between the GET and the POST.

   Under those conditions it:
   - generates a new key;
   - moves every connection whose credential does not open to `disconnected`, so there is no
     `lost` transition and no email;
   - deletes a saved Slack app that does not open;
   - audits per workspace and on the instance.
8. **Instance key rotation** (operator) keeps EIP-004 decision 5's order. `foundry doctor
   rotate-secrets` now:
   - re-wraps the secrets key row under the current instance key;
   - re-seals any remaining direct seals;
   - writes the canaries and bumps the version, in one transaction under the lock.
9. **The boot probe** (wire, probe, use) is one shared function used by `main.rs`, the acceptance
   harness and the reload task (ADR-IIS-004).
   - It resolves the mode.
   - It opens the canaries of the held keys.
   - It compares the key ids of every stored sealed value (connections and the saved Slack app)
     with them.
   - The degraded posture of EIP-004 decision 4 is unchanged.

## Alternatives considered

- **Keep `FOUNDRY_SECRETS_KEY` as the only name** (1a-A). Rejected: the UI's "secrets key" would
  be a different key from the setting with that name, so operators would set the wrong one. It
  also loses the signal that keeps a legacy instance in direct mode.
- **New name only, legacy name refused** (1a-C). Rejected: violates D5.
- **Any instance key with no stored secrets key means I-3** (1e-B). Rejected: it regresses existing
  instances and suites (KPI-IIS-G3), and old and new replicas disagree during the roll.
- **Generate the secrets key automatically at first start** (1e-C). Rejected by D5.
- **Envelope per row, a KMS, or Vault.** Rejected for the reasons in ADR-EIP-004. One wrapped data
  key is the smallest envelope that meets D1.

## Consequences

- Positive: a dump alone yields neither the secrets key nor any credential. No new crate.
- Positive: existing instances keep working untouched through the roll.
- Positive: rotation of the secrets key is a button. Rotating the instance key re-wraps one row.
- Negative: two keys to explain. The page explains both (I-25, I-26, I-30, I-31).
- Negative: the plaintext secrets key lives in each replica's memory, as `FOUNDRY_SECRETS_KEY`
  does today.
- Enforcement:
  - check-arch `integrations-secret-seam` is extended: unwrapping the secrets key, and `open`,
    stay inside `crates/foundry-app/src/integrations/` and `doctor`.
  - A new rule, `integrations-instance-tables`: SQL naming any of the following appears only in
    `foundry-store/src/integrations/instance.rs`:
    - `integration_secrets_key`;
    - `instance_slack_app`;
    - `instance_slack_app_pending`;
    - `integration_config_version`;
    - `integration_replica_states`;
    - `instance_integration_audit_events`.
  - A new rule, `integrations-seal-fence`: SQL that writes a sealed column appears only in the
    fenced store functions:
    - the connection save and reconnect (`sealed_credential`);
    - the Slack app save (`instance_slack_app`);
    - the pending-app insert (`instance_slack_app_pending`);
    - the lock-holding Generate, Move, Replace, Start over, re-wrap and re-seal functions
      (`wrapped_key` and every sealed column).

    A gold test plants an unfenced sealed write, including one into `instance_slack_app_pending`.
  - Each rule gets an injected-violation gold test.
  - Acceptance greps every page, log, activity line and API answer for three planted values:
    - the generated secrets key, read through the `integrations-test-seam` reader;
    - the operator instance key, from the scenario settings;
    - the Foundry-made file key, read from the scenario's temp `FOUNDRY_DATA_DIR`.

    This covers D3 and US-IIS-08.
