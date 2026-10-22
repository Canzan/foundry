# RED classification — external-integration-plugins

Gate output of the pre-DELIVER *fail-for-the-right-reason* check (ADR-025
scaffolded RED, rigor profile `adr-025-scaffolded-red`). DELIVER reads this file
at the start of each step to confirm the RED it un-pends is genuine.

## Acceptance (`@external-integration-plugins`, eleven feature files) — final (revision cycle 3, 2026-10-22)

**Preflight** (2026-10-22): no other foundry session running (another
project's acceptance run, canzan-lift, was active; its containers use their own
ports); foreign listeners on 55xxx/56xxx checked (none); `foundry` rebuilt and
warmed (`CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false cargo build --release
-p foundry-app --bin foundry`, then `target/release/foundry --help` three times,
`real 0.00`-`0.01`); `@pending` stripped from the working copies of all eleven
files (byte-exact copies kept in the scratchpad), then one bounded invocation per
lane with `FOUNDRY_EIP_DISTILL_CLASSIFICATION=1` (so the Slack double serves its
hand-written answers instead of refusing to start):
`FOUNDRY_ACCEPTANCE_TAGS=<lane> timeout 1800 cargo test -p foundry-acceptance
--release --test acceptance` for `external-integration-plugins`,
`needs-matrix`, `needs-smtp`, `eip-replica`, `eip-metrics`, `eip-restart`.
The files were restored by an EXIT trap and compared with `cmp` (clean).

| Lane | Rows | Result |
|---|---|---|
| `external-integration-plugins` (default selection) | 170 | 170 RED |
| `needs-matrix` (pinned Synapse started) | 32 | 32 RED |
| `needs-smtp` (pinned mailpit started, STARTTLS required, test certificate) | 1 | 1 RED |
| `eip-replica` | 6 | 6 RED |
| `eip-metrics` (subprocess, warm binary) | 8 | 8 RED |
| `eip-restart` (`@slow`, subprocess) | 1 | 1 RED |

**Final: 218 rows (130 declarations, outline rows expanded) = 218 RED
(MISSING_FUNCTIONALITY), 0 GREEN_ALREADY, 0 BROKEN**; 0 parsing errors, 0 hook
errors, 0 undefined or ambiguous steps (the lane runs `fail_on_skipped`). Rows by
file: 01 connect-slack 19, 01 route-new-issues 21, 01 signed-out 5, 02 content
21, 02 events 17, 03 broken-connection 16, 03 history-and-retries 19, 03
operations 14, 03 route-lifecycle 26, 04 matrix 32, 05 project-emails 28.

**Verdict: PASS.** Zero BROKEN, zero false GREEN.

Two edits landed after this run and were not re-run, because no lifted row
reaches them (every row of both scenarios stops in its Background at "Priya has
connected Canzan Labs to Slack"): the 03-broken-connection outline's third Then
now reads "the workspace integrations page reads "Connected to Canzan Labs"",
and scenario 125 has a comment. Both were checked by the step-coverage script
(every expanded step matches exactly one definition).

**Fixture-gate control** (cycle 3 B7): scenario 13 alone un-pended, run with
`FOUNDRY_ACCEPTANCE_TAGS=eip-restart` and without
`FOUNDRY_EIP_DISTILL_CLASSIFICATION`: it fails at the Slack double, "no recorded
Slack fixtures under tests/fixtures/slack ([oauth.v2.access, auth.test,
conversations.list, conversations.info, chat.postMessage, apps.uninstall,
events]); record them (DEVOPS H-3) before un-pending slice 01". File restored.

### Where each row stops (first failing step, final run)

| First failing step | Rows | Why it is the right reason |
|---|---|---|
| "Priya has connected Canzan Labs to Slack" (Background or Given): `GET /workspace/integrations/slack/install` answers 404 | 138 | The Slack install (DESIGN Driving Ports) does not exist |
| "Priya has connected Canzan Labs to Matrix …" | 25 | `POST /workspace/integrations/matrix` answers 404 (the pinned Synapse, its rooms and accounts were created; the Background passed) |
| "Sam/Tomás has chosen …" | 24 | `POST /account/settings/project-emails` answers 404 |
| a When whose universe snapshot cannot read a page (`the universe cannot be read: /workspace/integrations answered 404`): the authority rows, Connect Matrix rows, the settings rows | 20 | The integration pages do not exist; the strict universe refuses to compare vacuous images |
| "Priya holds a Slack return address with …" (used, older than 10 minutes, another session) | 3 | Needs the install redirect |
| "Foundry refuses to start, naming …" (malformed key, partial Slack settings, short lease) | 3 | Today's binary ignores those settings and keeps running (exit `None`); every other setting is supplied |
| "Canzan Labs has no Slack (or Slack or Matrix) connection" | 3 | The page does not exist |
| "the operator's delivery counters all read zero" | 1 | The series `{kind=slack, event=issue.created, outcome=delivered}` is not registered |
| "Sam has no project email choices and has followed Settings in the sidebar …" | 1 | The board has no Settings link: notification-preferences-ui is unfinished (D-9, C-10) |

### Defects found and fixed during classification (all runs)

1. **BROKEN (harness)**: the Synapse container generated its config with
   `--generate-config`, which listens on localhost only, and the readiness poll had
   no client timeout. Fixed: the image's `/start.py generate` then `/start.py`,
   2 s / 15 s client timeouts.
2. **BROKEN (harness)**: `/start.py generate` needs `/data`. Fixed: `mkdir -p /data`.
3. **Wrong reason**: the refused-start probe omitted `MACHINE_TOKEN_PUBLIC_KEYS`.
   Fixed: every setting `FoundrySubprocess` gives.
4. **GREEN_ALREADY**: "A choice for a project the member cannot open is refused"
   passed today. Fixed: the control "Sam's own choice for Identity Platform is still
   accepted".
5. **Wrong reason**: the sidebar `When` read the board from the scenario's latest
   answer after the universe snapshot replaced it. Fixed (`EipState::in_front`;
   also the confirm-delete and confirm-stop `When`s).
6. **Wrong expectation**: signed-out rows asserted the uniform not-found page; the
   shipped app sends a signed-out visitor to sign in. Moved into scenario 115.
7. **Leak**: the mailpit container; now kept in a run-wide slot and removed at the
   end of `main` beside the homeserver.
8. **check-arch**: the Slack double mounted `/api/…` routes, which the shipped
   `api-openapi-operations` rule reserves for foundry-api. Moved to `/slack-web/`.
9. **Would be BROKEN (cycle 3, found before the run)**: the driver's wait parser
   knew days, hours and seconds but not minutes, so "2 minutes pass" and "1 minute
   passes" would have panicked in DELIVER. Fixed.
10. **Fail-closed checks added (cycle 3)**: the universe now refuses a 200 page
    without its seams; a declared change must hold; an armed homeserver answer
    that was never given fails the scenario; the history's no-raw-text Then
    first requires the delivery line.

### Not verified at DISTILL

- The universe-delta Thens end to end and every Then after a row's first failing
  step (first exercised in DELIVER). The oracle itself is proven by
  `universe_oracle_tests` (4 tests: an unreadable page, a page without its
  seams, a declared change that did not happen, an undeclared change; 4 passed
  on 2026-10-22).
- The `@needs-matrix` rows past Connect Matrix need the U-D1 seam and the fault
  proxy's TLS.

## Gold-test stubs

Each lifted with `-- --include-ignored` on 2026-10-22 (no Docker needed: the bodies panic). `eip-core-unit` (`cargo test -p foundry-integrations` without the flag) runs 0 tests until DELIVER and is not counted:

| File | Stubs | Result |
|---|---|---|
| `crates/foundry-integrations/tests/core_properties.rs` (scaffold crate) | 19 | 19 RED (the marker panic) |
| `crates/foundry-auth/tests/eip_secret_box.rs` | 12 | 12 RED (the marker panic) |
| `crates/foundry-store/tests/eip_store.rs` | 14 | 14 RED |
| `crates/foundry-app/tests/eip_contract_shapes.rs` | 13 | 13 RED |
| `xtask/src/check_arch.rs` `external_integration_plugins_check_arch_tests` | 19 | 19 RED |
| `crates/foundry-acceptance/tests/eip_kpi4_bench.rs` | 1 | 1 RED |

**78 stubs, 78 RED, 0 BROKEN.** They are scaffolds with an unconditional marker: RED because they always panic, not because they probe behaviour. Marker: `Not yet implemented -- RED scaffold
(external-integration-plugins NN-NN: …)`; ignore reason `pending:
external-integration-plugins NN-NN …`.

## Committed state

Every scenario carries `@pending` (130 of 130) and every stub is `#[ignore]`d,
so every committed lane selects none of them: the hand-off is green by
construction. `cargo clippy -D warnings` on the touched crates' targets, `cargo fmt --check`,
`cargo xtask check-arch` (PASSED) and `cargo deny check bans` (ok) are clean.
