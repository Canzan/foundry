# CONTEXT

## Current Task

card-pointer-drag is delivered and pushed to `main` (`acaea7e`, no PR). Slice 03 is edge auto-scroll (`7e51169`) and interruptions (`2850ec2`); then harness strengthening (`7045cd3`), refactor (`fc7ce42`) and finalize. Full `cargo xtask ci` with the browser lane is 880/880 on `fc7ce42`. Record: `docs/evolution/2026-10-03-card-pointer-drag.md`.

## Key Decisions

- The kill-rate gate passed at 34/39 = 87.2%, after the edge-hold harness was changed to hold truly still and to release inside an edge zone.
- `.nwave/des-config.json` (untracked) now uses `tdd_phases` [RED, GREEN, COMMIT], because the installed `des-log-phase` rejects the legacy phase names.
- The slice-02 real-device dogfood PASSED, as the user reported on 2026-10-02.

## Next Steps

- The user owes device evidence: the slice-02 tallies and OS/browser versions (table in `deliver/slice-02-delivery-notes.md`), and the slice-03 real-device checklist (`slice-03-delivery-notes.md`).
- keycloak-sso: decide OD-10 (revoking the provision role) and do OD-11 (un-pend the 23 base `keycloak-sso.feature` scenarios; the SSO flow has no acceptance coverage).
- Stale text: `card-pointer-drag.feature` line 7 still says every scenario is `@pending`, and the untracked `spike/probe-log.txt` (647 KB) needs a keep-or-delete decision.
