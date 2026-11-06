# Upstream changes for the product owner — instance-integration-setup DESIGN

Status:
- The user asked for items 1–3 on 2026-11-05.
- Item 6 was approved by the user on 2026-11-05.
- Items 4, 5, 7, 8 and 8a (including the new I-28a) follow from the accepted design and need the
  product owner's acknowledgement before DISTILL.
- None is applied to discuss/user-stories.md yet.

Also: a filesystem without hard links (where `link` fails) shows I-27, not I-31. The directory
cannot hold a new key, which I-27's words already cover.

## 1. New copy I-31: the key file cannot be read

| ID | Where | Words |
|----|-------|-------|
| I-31 | the instance key file exists but cannot be read or parsed, and no operator key is set | `This Foundry cannot read its instance key file ({path}). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry.` |

- It takes the place of the I-25/I-27 instance key line.
- Generate and every credential form are withheld, as under I-26.
- Foundry never overwrites the file (ADR-IIS-002).
- **Restore-only in both deliverables.** Start over needs an instance key to seal a new secrets
  key, so it cannot be offered under I-31.
- Once the operator sets `FOUNDRY_INSTANCE_KEY` and restarts, a secrets key the new key cannot
  open shows I-26. In Deliverable B, Start over is offered there.
- KPI-IIS-2's state count goes from 16 to 17.

## 2. I-2e when nothing is sealed yet (I-2e0)

| ID | Where | Words |
|----|-------|-------|
| I-2e0 | direct mode (legacy `FOUNDRY_SECRETS_KEY`, no stored secrets key) with no saved credentials | `Credentials saved here are sealed directly with the instance key. Move to a secrets key to manage it here.` then `Move to a secrets key` |

- Choosing Move with n = 0 generates the secrets key and answers I-21 (not I-22 with "0 saved
  credentials").
- The activity line is "generated the secrets key".

## 3. I-14 with one workspace

`1 workspace is connected through the current Slack app ({old_client_id}). It keeps posting through
it until a workspace admin reconnects Slack. Save the new app?` then `Save the new app`.
The n ≥ 2 wording is unchanged.

## 4. Setting names (US-IIS-08 AC 6–7, US-IIS-06 AC 1)

- The operator setting is `FOUNDRY_INSTANCE_KEY` (previous: `FOUNDRY_INSTANCE_KEY_PREVIOUS`).
- `FOUNDRY_SECRETS_KEY` / `_PREVIOUS` are read as aliases.
- Both names set to different values refuses start, naming both settings. This is a second refusal
  beside AC 7's malformed key.
- The key file's directory is `FOUNDRY_DATA_DIR` (default `/var/lib/foundry`).
- I-27's words are unchanged.

## 5. US-IIS-01 scenario 1 applies to the new setting name

- "An instance key set by the operator, no secrets key" means `FOUNDRY_INSTANCE_KEY` or a file. An
  instance on the legacy name stays in direct mode and shows I-2e / I-2e0 (ruling 1e).
- DISTILL should set `FOUNDRY_INSTANCE_KEY` in US-IIS-01/02 scenarios, and the legacy name only in
  US-IIS-06's.

## 6. A different Slack client ID needs both secrets (US-IIS-05 AC 1–2, B) — APPROVED by the user 2026-11-05

This narrows D7's "blank secret keeps the saved value" to the same client ID only. The user
approved it on 2026-11-05.

- A blank secret field keeps the saved value only when the client ID is unchanged.
- With a different client ID, a blank secret answers I-11, because the secrets belong to the other
  app.
- The I-14 confirm is `GET`/`POST /admin/instance/integrations/slack/confirm`.
- The submitted app is held sealed in a pending row for 10 minutes, so the confirm page never
  contains a secret. Only the same instance admin can confirm it.

## 7. A saved Slack app that does not open (US-IIS-01 AC 5)

- The instance page shows I-13.
- Workspace pages and the route form read W-2 (Slack not set up), because an app that cannot be
  read is not in effect.
- ADR-IW-001's cause set is unchanged.

## 8a. From peer review: when Generate, Move and Start over are offered

- **Generate and Move are withheld while I-4 holds**, that is, while some saved credential opens
  under no key this server holds. The page shows I-4 and offers neither, as Replace is withheld
  under I-26 (US-IIS-02 AC 1/5, US-IIS-06 AC 2).
- **Start over (B, US-IIS-07 AC 5) is offered only when no running replica opens the secrets key.**
  When another server holds the right key file, Start over would discard credentials that server
  can open. I-26 stays restore-only until every live replica reports it cannot open the key.
- The gate is checked again inside the confirming POST.
- **Servers of an earlier release cannot report**, so I-28 gains a required typed confirmation,
  new copy I-28a:

  | ID | Where | Words |
  |----|-------|-------|
  | I-28a | on the I-28 confirm, required before `Start over` | `Type START OVER to confirm that no Foundry server of an earlier release is still running.` Field label: `Confirmation`. A wrong or empty value answers I-28 again and changes nothing. |

- **New scenario for DISTILL:** another live replica that opens the key withholds Start over, even
  when confirmed.
- **Direct mode also follows direct seals.** An operator who renames `FOUNDRY_SECRETS_KEY` to
  `FOUNDRY_INSTANCE_KEY` with the same value keeps I-2e and working deliveries, rather than
  dropping to I-3.

## Product owner acknowledgement (2026-11-05)

### Acknowledged and folded into discuss/

The items below are folded into `user-stories.md`, the journey YAML and
`outcome-kpis.md`.

- **1 (I-31):** Copy table, US-IIS-01 AC 5, US-IIS-08 AC 3, and the
  instance-key artifact. KPI-IIS-2 now counts 17 states.
- **2 (I-2e0):** Copy table, and US-IIS-06 AC 1–2 (Move with n = 0 answers
  I-21).
- **3 (I-14 for one workspace):** Copy table.
- **4 (setting names):** System Constraints, US-IIS-06 AC 1, US-IIS-08 AC 6–7
  and the journey YAML.
  - `FOUNDRY_INSTANCE_KEY` and `_PREVIOUS`, with the legacy `FOUNDRY_SECRETS_KEY`
    names as aliases.
  - Both names set to different values refuses start.
  - `FOUNDRY_DATA_DIR`.
- **5:** US-IIS-01 scenario 1, and a DISTILL note in US-IIS-06.
- **6 (approved):** US-IIS-05 AC 1–2 (both secrets for a new client ID, and
  `…/slack/confirm`).
- **7:** US-IIS-01 AC 5. Under I-13, the workspace pages read W-2.
- **8:** US-IIS-08 Technical Notes. The 60 s bound is unchanged.
- **8a, offer rules:**
  - Generate is withheld under I-4 (US-IIS-02 AC 5).
  - Move is withheld under I-4 (US-IIS-06 AC 2).
  - Start over requires that no live replica opens the key, re-checked in the
    POST, and is never offered under I-31 (US-IIS-07 AC 4).
  - Direct mode follows direct seals (US-IIS-06 AC 1).

### 8a, I-28a: approved by the user 2026-11-05

- Acknowledged and folded: the Copy table, US-IIS-07 AC 4, and wave-decisions
  D4.
- Item 8a is now fully acknowledged.

## 8. Timing

- "Every replica within 60 seconds" is met with a 5 s poll (`FOUNDRY_INTEGRATIONS_RELOAD_MS`).
- Acceptance shortens it, and the 60 s bound in the ACs stays as written.
