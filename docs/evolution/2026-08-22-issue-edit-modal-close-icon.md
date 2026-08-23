# Evolution — issue-edit-modal-close-icon (a visible way out of the issue edit dialog)

**Finalized**: 2026-08-22
**Commits**: DELIVER ran in **NO-COMMIT mode** (shared-dirty tree with the parallel
`instance-admin-project-rename` session); the entire delta — source, tests, and all wave docs —
was later committed by that parallel session inside `39890ad`
`refactor(instance-admin-project-rename): L1 retire stale RED-scaffold commentary`. Content
verified correct and now pushed; the misattributed message is recorded here as the honest
history pointer. 4 DES-monitored TDD steps (2 phases); DES integrity exit 0; roadmap review
APPROVED (0 blockers); adversarial review APPROVED (0 defects); mutation gate **vacuously
satisfied** (only Rust production delta is test literals — the behavioral surface is
template/JS/CSS, held by the browser lane). Re-verified at HEAD today: **10/10 scenarios
(88/88 steps), real headless Chrome**. Feature dir PRESERVED. Finalize commit **not pushed**.
**Scope**: a conventional close (×) button in the top right of the issue edit dialog
(`issue_edit_modal.html`) so a pointer user can leave without saving. Strictly additive:
Esc, Save, and "Open full page" byte-unchanged. No route, no migration, no dependency,
no config, no pipeline edit.

## Business context

The operator (same persona as `job-instance-project-rename`) opens issue cards as often to
*read* as to edit — but the edit dialog's only labeled button is Save, and its only no-save
exit is the unadvertised Esc key. With a mouse there was no way out at all
(`job-dismiss-edit-dialog`, `docs/product/jobs.yaml`). This feature adds the × every other
tool's dialog has: one click, top right, back to the board, nothing saved.

## Key decisions (D-01–D-16, condensed)

- **D-01/D-02** — edit dialog only (new-issue dialog is the recorded fast-follow); closing
  discards silently — exact Esc parity, no unsaved-changes confirmation.
- **D-03/D-12** — a native `<button type="button" aria-label="Close"
  data-action="close-modal">` with a text × glyph, in `.modal-header`, OUTSIDE the form:
  accidental submit structurally impossible; Enter and Space activation asserted, not assumed.
- **D-04/D-10 — the declarative close-trigger contract** (adr-modal-close-001): ONE
  mechanism (`keyboard.js::closeModal()` emptying `#modal-root`), two triggers (Esc via
  `closeTopLayer()`, the × via one document-delegated `click` listener matched with
  `closest('[data-action="close-modal"]')`). No second `Escape` listener (BR-4 held);
  survives htmx swaps by construction; every future dialog close affordance is a
  template-only attribute, never new JS.
- **D-11 — focus restore DEFERRED**: Esc parity is the spine; restoring focus on the ×-path
  only would make the two triggers diverge. If ever added, it goes inside `closeModal()` so
  BOTH triggers get it — recorded as a separate fast-follow.
- **D-13** — `.modal-header` flex row; new `.modal-close` class (28px target,
  `:focus-visible` outline); benign to the sibling dialog.
- **D-14 — same-commit CSS re-hash executed**: `foundry.7c858984.css` → `foundry.8ce38566.css`
  with `base.html` href, `VENDOR.md` row, and the three `lib.rs` cache-test literals rotated
  in the same changeset — split, the immutable-cache tests go red.
- **D-15** — no new probe surface: the shipped `data-kb-ready` marker means the delegated
  listeners are live; every scenario waits on it before touching the ×.

## Steps completed (4/4, execution-log.json — NO-COMMIT mode, verification-green per step)

| Step | What landed | RED → GREEN |
|---|---|---|
| 01-01 | Step module wiring + rendered × (template, CSS, full D-14 re-hash) | RED at "no close control in the rendered header" → scenario 1 green |
| 01-02 | The one delegated click listener → existing `closeModal()` | RED "dialog still open after click" → scenarios 1/2/6 green; kb regression lane 39/39 |
| 02-01 | No-save oracle (store read-back) + Tab/Enter/Space activation proofs | Scenarios 3/4/5 green — dirty form saves nothing, both keys close |
| 02-02 | Regression/error guards: Esc, Save, full-page link, 4xx-error escape | 7/8/10 green-on-wire as classified (REGRESSION_GUARD); 9 green via the same click path from the error state |

Post-delivery gate: fresh 10/10 scenarios (88/88 steps) at HEAD, 2026-08-22, real Chrome.

## Issues and lessons

1. **A pre-existing VENDOR.md hash drift was caught and fixed by the review gate.** The
   D-14 re-hash procedure walked straight into a `static/VENDOR.md` row that had already
   drifted from the shipped asset before this feature started; the adversarial review gate
   surfaced it and the fix rode the same changeset. A recorded hash nobody re-derives is a
   hash nobody can trust — the re-hash procedure is now the audit.
2. **The suite's first focused-control keypress exposed a latent harness gap.** The shipped
   send-keys dispatch silently blurred the focused element, so Enter-on-the-focused-button
   could never be tested honestly. Fixed in `browser_harness.rs` with W3C key actions
   (`press_key()`, plus the missing `"Space"` chord);
   `steps/keyboard_shortcut_bindings.rs` now delegates to it, and the kb lane's 39/39 green
   proved the semantics were preserved, not merely relocated.
3. **Vacuous-oracle hardening (Always-Green-Theater hole closed).** The globally-bound
   "Then the dialog closes" step polls the *new-issue* selector — vacuous for this dialog.
   Step 02-02 hardened the module's `When Mei clicks the close control` to carry the bounded
   `#modal-root`-empty proof itself, so a scenario cannot pass while the edit dialog is
   still open. Shared step phrases are a reuse win only when their oracle actually binds to
   the surface under test.
4. **chromedriver/Chrome host skew, again.** Host chromedriver 152 vs Chrome 151 meant xtask
   preflight 3 would refuse; delivery lane runs used a matched Chrome-for-Testing
   chromedriver 151.0.7922.138 from the session scratchpad. Now fixed on the host: 151/151,
   preflight green. Same lesson as the sibling feature — the probe-and-refuse preflight is
   the guard, keep it loud.
5. **NO-COMMIT delivery in a shared-dirty tree, and the sweep.** The working tree was shared
   with the parallel `instance-admin-project-rename` session, so the user chose NO-COMMIT
   mode: every step's COMMIT phase logged APPROVED_SKIP with verification green. The
   parallel session's cleanup commit `39890ad` then swept this feature's entire delta in
   under *its* message. The content is correct and pushed; the cost is a misattributed
   history entry, pointed at honestly here. Two features in one working tree means someone's
   `git add` will eventually eat the other's delta — prefer worktrees, or commit-per-step.
6. **Fast-follows recorded** (both made cheap by adr-modal-close-001): a close icon on
   `new_issue_modal.html` is now a template-only change (zero new JS); focus-restore, if
   wanted, goes inside `closeModal()` so Esc and the × stay in lockstep.

## Measured KPI baselines (acceptance-verified, no runtime telemetry — DEVOPS ruling)

- **KPI-1** (visible no-save exits in the edit dialog): 1, from 0 — scenario 1 asserts the
  control renders with accessible name "Close".
- **KPI-2** (pointer actions to leave without saving): 1 click, from impossible — scenario 2.
- **KPI-3 guardrail** (keyboard cost to close): Esc still closes in exactly 1 keypress —
  scenario 7 plus the shipped `@layered` BR-4 scenario.
- **KPI-4** (save requests issued by a dismissal): exactly 0 — scenario 3's store read-back
  shows GEN-1's title/description/state unchanged.

## Permanent artifacts

- `docs/product/architecture/adr-modal-close-001-declarative-close-trigger.md`
- `docs/product/architecture/brief.md` — dialog-close subsection
- `docs/product/jobs.yaml` — `job-dismiss-edit-dialog`
- `docs/product/outcomes/registry.yaml` — OUT-6 (invariant: the declarative close-trigger contract)
- `CHANGELOG.md` — entry under [Unreleased]
- `docs/feature/issue-edit-modal-close-icon/` — lean single-file wave history
  (`feature-delta.md`, all 5 waves) plus `deliver/{roadmap.json,execution-log.json}`

## Open / deferred

- Close icon on `new_issue_modal.html` — natural fast-follow, template-only now (D-01).
- Focus-restore to the triggering card — deferred (D-11); belongs inside `closeModal()`
  for both triggers if ever taken up.
- Backdrop-click dismiss and any unsaved-changes confirmation — explicitly out of scope (DISCUSS).
- One flake observed, not ours: `foundry-services` provision_workspace unit test under
  workspace-parallel execution; green in isolation and on re-run.
