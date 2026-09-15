# Closing notes — card-drag-drop-feedback (DELIVER Phases 3–7)

2026-09-14/15. User choices: full closing (refactor, review, CI gate, evolution doc),
no push. Branch `card-drag-drop-feedback`: `3ee56fa` (the DELIVER wave, committed by the
concurrent `foundry` session) + `90ed631` (rustls 0.23.40 → 0.23.45 for RUSTSEC-2026-0285,
separate and revertable, same session). This session owns the closing gate; the other
session stood down read-only.

**Units, once:** the `.feature` has **35 scenario declarations** (9 of them Outlines) which
execute as **54 examples**. "35 scenarios" (commit `3ee56fa`) and "54/54" (lane runs) are
the same suite counted two ways.

## Phase 3 — Refactor (L1–L4; L5/L6 clean)

Behaviour-preserving, scoped to `board-dnd.js` and this feature's test code (no CSS,
template or shipped file touched). `board-dnd.js` 395 → 426 lines (sha256 `7e633e63` →
`bfd0f143`): named hooks (`CARD`, `LANE`, `KEY`, `BEFORE_KEY`), `keyOf`, shared
`nearestCard` / `otherCards`, extracted `keepOneMarkerIn` and `moveBody`. Step module:
merged duplicated probes and contrast helpers, `release_accepted`, split the long
`refresh_in_place` match, a `DropPlace` enum. Harness: `run_kit_at`.

Verification history (honest):

| When | Result |
|---|---|
| Refactor agent, under a saturated host (load 95–126) | fmt / clippy / release build green; `cdf` and smoke produced **no count** (Chrome sessions and Postgres containers would not start) |
| Orchestrator re-run #1 (load ~24; a concurrent session's `cargo xtask ci` + Docker survey) | `cdf` 14/54 — all 40 failures infrastructure (chromedriver session, window-size on a dead session, one WaitTimeout, one un-rendered sign-in); smoke failed on Postgres `StartupTimeout` |
| After the concurrent session stood down and finished its Docker cleanup (dangling images + build cache + anonymous volumes only; postgres ready in ~1s) — **tip `90ed631`, fresh release build, warm binary, load ~5** | **`cdf` 54/54 examples (400/400 steps), EXIT 0; `cargo xtask smoke` all gates green** |

## Phase 4 — Adversarial review

`nw-software-crafter-reviewer`: **APPROVED**, 0 blocking, zero Testing-Theater patterns,
refactor judged semantically equivalent by a static diff against the pre-refactor snapshot.
Non-blocking: document the hand-seeded fault procedure as a runbook; the recurring sqlx
`'\0'` flake; the dead `cdf_marker_before` field in `world.rs`.
**Discounted claims (unverified or wrong):** it asserted M1–M9 were re-seeded on the
post-refactor code (they were not, at that time), that post-refactor runs were green (none
had completed), and that pre-`:has()` browsers show the placeholder everywhere (ADR-003:
the `display: none` default wins — it never shows).

## Phase 5 — Mutation re-run on the final code

**9/9 killed on the final code at `90ed631`, no survivors** — every named scenario of every
fault reddened on its own oracle (no timeout or harness error, no repeated run). Two seeds
changed shape because the code moved: M6 removes the dragged-card skip from `slotFor` only
(the skip now lives in the shared `otherCards`, also used by `slotMidline` — seeding there
would be a stronger fault than the recorded one); M3 calls `preventDefault()` on `drop` only
when a session exists (the refactor removed `drop`'s own early return). M8 keeps the
network-error revert. Every restore `cp` + `cmp`; final `cdf` 54/54 examples, 400/400 steps.
Details and verbatim assertions: `mutation/mutation-report.md` § "Post-refactor re-run
(final code, tip 90ed631)". The pre-refactor 9/9 (per-slice) remains recorded above it.

**Survivor record, precisely** (commit `3ee56fa`'s message says "M6 and M7 survived their
first gate", which omits one): at the per-slice gates **M6 survived outright** (no named
scenario reddened), while **M2 and M7 were killed but each left one named scenario green**
(the swallow outline under M2; the remote-empty scenario under M7). All three gaps were
closed by strengthening the scenario that should have caught them — never by weakening an
assertion — and the post-refactor re-run reddens every named scenario of all nine faults.

## Phase 6 — Integrity

`des-verify-integrity docs/feature/card-drag-drop-feedback/deliver/` → **"All 8 steps have
complete DES traces"** (final run, after all closing work). Every step has PREPARE,
RED_ACCEPTANCE and GREEN EXECUTED, RED_UNIT SKIPPED (NOT_APPLICABLE — no JS unit runner,
DB6) and COMMIT SKIPPED (APPROVED_SKIP — no-commit mode). 04-02 additionally keeps its honest
first GREEN `FAIL` (the sqlx `'\0'` flake under a too-narrow brief) followed by the corrective
GREEN `PASS` entries.

## Phase 3.5 — CI gate attempts

**Attempt #1 (tip `90ed631`, pre-built + warm binary, load ~5 at start; 04:58–05:04Z):
FAILED — a pre-existing intermittent test, not this feature.** fmt, clippy, check-arch and
`cargo build --release` passed; `cargo test --workspace (excl. foundry-acceptance)
--release` stopped on `foundry-store/tests/grant_super_admin_idempotent.rs` →
`grant_records_operator_as_super_admin` panicking at `:46:10` with `connect pool:
PoolTimedOut` (its sibling test in the same binary passed; every other workspace test
passed). Evidence it is not the branch: `git diff aa8a6f6 90ed631 -- crates/foundry-store`
is empty; the test file last changed 2026-06-12 (`ecbade2`); run alone three times it went
FAILED (same `PoolTimedOut`, 10.9s) → ok → ok (1.1s each). The browser lane never ran on
this attempt. Follow-up (out of scope): that test's pool acquisition is flaky.

**Attempt #2 (tip `90ed631`, warm binary, load ~7 at start; 05:07–05:22Z): FAILED — on a
pre-existing intermittent `kb` test, NOT this feature.** _(Corrected 2026-09-15: this entry
first read "this one is REAL" and blamed the refactor, from a one-run-per-arm A/B. A
multi-run re-test — below — overturned that.)_ fmt, clippy, check-arch, build, the workspace tests (the
`grant_super_admin` flake passed) and `cargo deny check` (RUSTSEC-2026-0285 cleared by
`90ed631`) all passed; the full acceptance lane (all tags) ran **824/825**. The one failure,
`keyboard-shortcut-bindings.feature:429` "Shortcuts keep working after the page content is
swapped", failed its Given with `submitting the new-issue form left the modal open …
WaitTimeout`. It **reproduces**: `kb` alone at the tip → **36/38** (also "Mei files an issue
entirely from the keyboard", same error).

**A/B — first read, then overturned.** One `kb` run with only `board-dnd.js` swapped back to
the pre-refactor version (`7e633e63`) went 38/38, which the orchestrator wrongly took as proof
the refactor was the cause. The troubleshooter's multi-run re-test (same binary, `kb` alone,
warm, one lane at a time):

| `board-dnd.js` | runs | reds (incl. the orchestrator's runs) |
|---|---|---|
| committed `bfd0f143` | 38/38 · **37/38** · 38/38 | 3 of 5 |
| pre-refactor `7e633e63` | 38/38 · **36/38** (both scenarios, same WaitTimeout) | 1 of 3 |

The pre-refactor file fails identically, so no hunk controls it — the bisect was correctly
not run, and **no change to `board-dnd.js` is warranted**. (The orchestrator's first red run
was also confounded: it began with a 4-minute rebuild after the rustls bump.)

**Root cause — confirmed (user-approved test-only bugfix, 2026-09-15).** A temporary
keydown/focusin/htmx-swap recorder in the two steps reproduced the failure (run 2 of a
`kb`-only loop) and printed, for "Mei files an issue entirely from the keyboard":
`c@BODY SWAP>modal-root FOCUS>INPUT[title] Shift@BODY S@BODY e@BODY … c@BODY … SWAP>modal-root
… Enter@BODY SWAP>modal-root FOCUS>INPUT[title]`, and the same shape for AC-X.5. Focus *did*
reach the title field, yet every key landed on `BODY`: the step read `active_element()`
inside the beat before htmx's settle-time `[autofocus]` focus, got `<body>`, and WebDriver's
send-keys on that reference pulled focus back to the body. The typed title's `c`s re-opened
the modal and `Enter` opened the selected card's edit modal (`titleValue: "Seeded issue 4"`),
so the form never submitted. Product behaviour is correct; the steps raced it. A caveat worth
keeping: the first recorder, installed between "field exists" and typing, added a WebDriver
round trip into exactly that beat and masked the race (0/8 reds); moved ahead of the `c`
press, it reproduced within two runs.

**Fix:** `wait_for_title_field_focus` (the wait AC-06.3's step already carried, now shared)
runs before `active_element()` in both steps; the diagnostic was removed (file restored to
HEAD by `cp` + `cmp`, then the fix applied). Committed alone as **`868090f`** (user's choice:
two local commits — the step fix, then these closing docs; no push). This modifies a shipped step module, so KPI 8's
"unmodified" holds for `3ee56fa`/`90ed631` but not for the follow-up — recorded, user-approved.

Verification of the fix (same tree, one lane at a time, warm binary):

| Check | Result |
|---|---|
| `cargo fmt --all --check`; `cargo clippy -p foundry-acceptance --all-targets -- -D warnings` | both EXIT 0 |
| `kb` ×10 with the fix (load rising 4.4 → 12.2) | **10/10 runs 38/38**, 0 reds. At the pre-fix rate (≈1 red run in 2–3) ten straight greens has ≈1% odds by chance |
| Fault: `autofocus` removed from `new_issue_modal.html` (restored `cp` + `cmp`, `== HEAD`) | **34/38, EXIT 101.** Red: AC-03.1 "Pressing the create key opens the new-issue modal on the board" (its own oracle: `BODY` focused); AC-03.2, AC-06.3 and AC-X.5 through the shared wait ("never took focus"). The wait does not mask an unfocused modal |

**Attempt #3 (tip `90ed631` + the uncommitted `kb` step fix, pre-built + warm binary, load
8.5 at start → 95.5 at end; 12:34–12:41Z): FAILED — environment, before the browser lane
ran.** fmt, clippy, check-arch and the release build passed; `cargo test --workspace (excl.
foundry-acceptance)` stopped on `foundry-services/tests/write_use_cases.rs:75`
(`create_issue_files_with_next_key_and_backlog_state`): `connect pool: Protocol("unexpected
response from SSLRequest: 0x48")` — the testcontainers-mapped port answered with a non-Postgres
byte (`'H'`), the same wire-protocol-garbage family as the sqlx `'\0'` flake; its 6 siblings in
that binary passed. Not the branch: `git diff --stat aa8a6f6 HEAD -- crates/foundry-services`
is empty (file last changed `87282d3`, 2026-09-07). The host was saturated by processes
outside this session (a growing set of `pi` processes at ~40% CPU each), so no result from this
attempt says anything about the tree.

**Attempt #4 (tip `868090f`, pre-built + warm binary; the user had stopped `pi-companion`, 0
`pi` processes at start, load 9.4; 13:46–14:00Z): GREEN.** `xtask ci :: all gates green`,
EXIT 0. fmt, clippy, check-arch, `cargo build --release`, the workspace tests, `cargo deny
check`, and the full acceptance lane (all tags, including `@docker-compose`) at **825/825
scenarios, 5757/5757 steps**. Nothing was re-run to get there: it is the first attempt on a
quiet host after the `kb` fix.

## Phase 7 — finalize

Done:
- The gate markers are filled with attempt #4's result, in `feature-delta.md` (Quality
  Gates, DoD rows 1 and 9, Commit Record), the evolution doc, and `kpi-contracts.yaml`
  `final_gate`.
- The evolution doc, the DELIVER sections, and the SSOT updates to `brief.md` and
  `kpi-contracts.yaml` are written.
- `CONTEXT.md` is rewritten as the handoff.
- The closing docs land in a local commit after `868090f`, with no push.
- `.nwave/des/deliver-session.json` is removed, as the user approved. The `des-task-active`
  files are left for the user.
