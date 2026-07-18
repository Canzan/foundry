# Evolution — new-issue-dialog-description (the field the edit dialog already had)

**Finalized**: 2026-07-18
**Commits**: DELIVER `ec52a7a` → `3c91c57` (8 commits; `8d3213f` is the pre-feature baseline) — 7 DES-monitored
5-phase TDD steps across 3 thin slices, plus one out-of-band refactor (`3c91c57`, mutation-coverage extraction)
and one re-executed step (`03-01`, first BLOCKED on a security control then completed after a user-approved
scope expansion). Trunk-based; repo legacy multi-file convention; DES-monitored (5-phase contract, exempt at
finalize). Feature dir PRESERVED. **Not pushed** (per the standing trunk pattern — push deferred to the user).
**Wave coverage**: full DISCUSS → DESIGN → DISTILL → DELIVER. DESIGN resolved four ODDs into two feature-local
ADRs and one design review. `des-verify-integrity` exit 0; all 7 steps carry complete 5-phase traces
(PREPARE → RED_ACCEPTANCE → RED_UNIT → GREEN → COMMIT); RED_UNIT is `SKIPPED`/`NOT_APPLICABLE` on 4 of 7 (the
edges and cross-feature coherence are HTTP/acceptance-level, no new unit logic — recorded, not glossed).
**Scope**: the new-issue dialog (opened by `c` or the board "New issue" button) collected **Title only**; the
shipped *edit* dialog collected **Title + Description**. The only way to describe an issue was to create it,
find the card, and reopen it in a second dialog. This feature adds the Description field to create — threaded
through every layer, kept in web/API rule-parity, and bounded. ZERO new routes, ZERO endpoints, ZERO
migrations (latest remains `0014`); `description_md` and its `CHECK(length ≤ 262144)` already shipped.

## Milestone — capture the work in one pass. The dialog `c` opens now takes what the work *is*.

The user's request was *"when pressing `c` or 'New issue', show the Description field like edit."* It read like a
template one-liner. It was not — and the value of this archive is the four things execution found that reading
the request could not.

## The four findings, none visible from the request

**1. The gap was full-stack, not template-deep.** The description was absent at every layer of the create path
— `new_issue_modal.html` → `CreateIssueForm` → `services::create_issue` → `insert_issue_with_outbox` — while
the edit path had it at each corresponding layer. "Show the field" was unreachable without threading the value
through the store insert, the service (and its shared JSON-API caller), the form, both view-models, and the
partial. The precedent was already on the record: `board-new-issue`'s "near-zero backend change" estimate was
**revised at DELIVER** when a view-model field turned out to be required. Scoped as full-stack from the start
here; the store change touched **3** real call-sites (verified by re-grep + the compiler, not the 7 the design
note guessed — four of the named files referenced the function only in comments).

**2. "Description is unbounded" was false — the bound was at the database.** DISCUSS asserted the shipped edit
path stored descriptions unbounded. Reading the schema corrected it: `issues.description_md … CHECK (length ≤
262144)` has existed since `0001_init.sql`. The real defect was not "no bound" but a **bad error surface**: an
over-long edit description reached the DB CHECK and surfaced as an **HTTP 500**, not a clean refusal. US-03
became "add the *application* bound at the DB's threshold, converting the 500 into a 422/fragment" — value
262144 to match the DB exactly, so nothing that saves today becomes refused.

**3. "The modal is destroyed and input is lost" was also false — the opposite is true.** A mid-design claim
that a validation error swaps into `#modal-root` and destroys the typed description was checked against the
vendored htmx (**2.0.4**, default `responseHandling` `{code:"[45]..",swap:false}`, no override, no
`response-targets`, `bad_request_fragment` = 400): htmx **does not swap the 400 at all**. The modal stays open
and the typed input is **preserved** — for free. So AC-01.6 needed zero code. The real, *pre-existing*,
app-wide defect is that validation errors are **invisible** in the browser (every htmx form, not just this one)
— which makes `board-new-issue`'s D3 "the error swaps in, visible" false, and which the acceptance suite never
caught because every error step asserts the HTTP body, never the DOM. **Deferred to its own `/nw:root-why`
bugfix** by user decision; this feature ships errors that behave identically to every other form. The DISTILL
error scenarios assert the HTTP response only — a scenario asserting a visible message would be red for an
unrelated reason; one asserting input-survival over HTTP would be green over nothing.

**4. The 262144 bound was unreachable over the web — a 64 KB security cap sat below it.** Slice 03's crafter
**BLOCKED rather than fabricate a green**: the CSRF middleware (`csrf.rs:124`) buffers *every* form body at a
64 KB cap (`to_bytes(body, 64*1024)`, chosen when "sign-in forms are tiny"), so a 262144-char description never
reaches `submit_create` — the request is refused before the handler. The DB-matching bound was physically
unsubmittable via the web (the JSON API, on axum's ~2 MB default, could carry it — a pre-existing web/API
parity gap this feature surfaced). Brought to the user; **user-approved raising the cap**: the CSRF buffer went
to 2 MiB with **explicit per-route `DefaultBodyLimit`** on the issue create/edit POSTs (the `attachments.rs`
precedent), scoped to the issue write path. A security-sensitive change, flagged here for the security review.

## What shipped

- **Store** (`insert_issue_with_outbox`): a `description: &str` param; the existing in-tx INSERT widened to
  carry `description_md`. No new query, no migration. Every prior caller passes `""` (byte-identical).
- **Service** (`create_issue` + `edit_issue_details` + facade): the description param, and a shared
  `validate_description` helper — `chars().count() > DESCRIPTION_MAX_LEN (262144)`, code `description_too_long`,
  message "Description is too long" — applied on **both** paths (edit's guard runs BEFORE the read-old→UPDATE
  tx, so a refusal leaves the issue untouched). One rule, both paths; the edit path's DB-CHECK 500 is now a
  clean refusal.
- **Web** (`CreateIssueForm`, `submit_create`, `NewIssueModal` + `NewIssueModalPage`, `new_issue_modal.html`,
  `keyboard.rs` renderers): the optional `description` field threaded through; the shared partial gains the
  `<textarea>` (so the no-JS full-page fallback inherits it — verified, zero extra template edit); the
  handlers route `description_too_long` to its own fragment while keeping the `title_required` fallback
  byte-identical.
- **API** (`CreateIssueRequest` + `create_issue_handler`): an optional `#[serde(default)] description`, forwarded
  to the same shared service (rule-parity, NFR-WEB-API-CON-02). Response body unchanged; read-back equality
  serves the contract. The 422 for over-long comes free from the existing `Validation → 422` mapping — a dead
  edit avoided.
- **CSRF / body limits** (`csrf.rs`, `lib.rs`): the 64 KB buffer raised to 2 MiB + per-route `DefaultBodyLimit`
  on the issue create/edit POSTs, so the DB-matching bound is reachable through the web.
- **Tests**: 16 acceptance scenarios (real Postgres) across web create, API parity, the bound on both paths,
  and two issue-change-history coherence invariants (a created description emits no timeline event; its first
  edit reports the created value as `old_value`) + 3 store-integration cases + 4 fast `validate_description`
  unit tests.

## The instrument told the truth twice

**Falsification was demonstrated, not asserted.** Each new guard was shown red against its obvious mutant before
being accepted green: S2 against a handler that drops the field; S6 against a fallback template lacking the
textarea; S11 against a validate-*after*-write ordering (the issue must be untouched on refusal); S12/S13
against an exclusive bound and a byte-count.

**Mutation testing caught a real coverage gap — and it was the documented `@real-io` trap.** A feature-scoped
`cargo-mutants` on the two bound checks reported **5 of 6 mutants MISSED**, including the `>`→`>=` exclusive-bound
mutant the falsification claimed dead. The 13-second baseline was the tell: cargo-mutants ran only the fast unit
tests, not the acceptance suite (Postgres testcontainers, too slow), so the acceptance-only-covered bound
"falsely survived" — exactly the trap in `[[cargo-mutants-realio-subprocess]]`. The fix was also the code
reviewer's refactor suggestion: **extract the pure `validate_description` and unit-test it directly**. The
re-run: **4/4 caught, 0 missed**. The gate found the gap; closing it deduplicated the guard and gave the core
new logic real, fast mutation coverage.

## Open items (carried, not blockers)

- **Invisible in-browser validation errors** (app-wide, pre-existing) — the `description_too_long` and
  `title_required` fragments are correct at the HTTP layer and invisible in the browser under htmx 2.0.4's
  no-swap-on-4xx. Deferred to its own `/nw:root-why` bugfix. When fixed, this feature's error copy becomes
  visible for free; the JSON API already shows it.
- **Security review of the CSRF body-cap raise** — the 64 KB → 2 MiB buffer is global (matching axum's existing
  default, so no *new* DoS surface) with per-route limits scoping the *intent* to the issue endpoints. A comment
  overstates "scoped" (the buffer ceiling is global); worth a one-line clarification for the reviewer.
- **Web/API body-limit parity** — the API path allowed ~2 MB before this feature while the web capped at 64 KB;
  now aligned at 2 MiB on the issue endpoints. Other form routes keep axum's default.
- **Full `cargo xtask ci` before push** — smoke (the commit gate) is green throughout; the full acceptance
  lane + release build is the pre-push gate and runs when the user pushes.
