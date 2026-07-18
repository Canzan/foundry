# Walking Skeleton — new-issue-dialog-description

The create pipe ships end-to-end (`c` → modal → CSRF'd POST → `create_issue` → `insert_issue_with_outbox` →
OOB Backlog card) and `description_md` is a live column the edit path already writes. There is **no new
end-to-end thread to establish** — only one field to thread through the existing pipe. So the "skeleton" here
is the thinnest slice that proves the field survives the whole round-trip: **create with a description →
it's in the store → it comes back in the edit dialog.**

## First failing test (DELIVER entry)

**S2 — "Filing an issue with a description persists it and returns the Backlog card"** — drives the whole
vertical (template field → form struct → service param → store INSERT), with **S3** (round-trip to edit)
proving persistence from the read side.

RED → GREEN (slice 01):
1. **RED store test**: `insert_issue_with_outbox` persists a supplied description → fails to compile (no
   param), then fails assertion until the INSERT includes `description_md`.
2. **RED S1/S2**: modal has no `description` textarea; POST drops the field → S2 store assertion red.
3. **GREEN, bottom-up**:
   - `insert_issue_with_outbox(…, description: &str)` → include `description_md` in the existing tx INSERT.
     Update all **7 call-sites** to pass `""` (list in `design/wave-decisions.md` §DELIVER note); compiler
     enforces completeness.
   - `services::create_issue(…, description: &str)` + facade → forward to the store.
   - `CreateIssueForm { …, #[serde(default)] description: String }`; `submit_create` forwards it.
   - `NewIssueModal` + `NewIssueModalPage` each gain `description: String`; `new_issue_modal.html` gains the
     `<textarea name="description">{{ description }}</textarea>`; the GET renderers pass `String::new()`.
4. **GREEN** S3 (round-trip), S4 (optional ⇒ ""), S5 (empty title still 400 + no row), S6 (no-JS fallback),
   S7 (foreign refused), S15/S16 (change-history coherence).
5. `cargo fmt --all --check` + `cargo clippy --all-targets --release -- -D warnings`; commit. Then **DOGFOOD**
   the live press-`c`→type→Create→reopen flow + the input-survival check (dogfood checklist).

## Slice sequence

1. **Slice 01** (skeleton above) — web create field, store→service→web→templates. S1–S7, S15, S16.
2. **Slice 02** — API parity. `CreateIssueRequest { …, #[serde(default)] description }`; `create_issue_handler`
   forwards it. S8, S9. (Depends on slice 01's service param.)
3. **Slice 03** — the shared bound. `const DESCRIPTION_MAX_LEN = 262144` + one validation applied in
   `create_issue` AND `edit_issue_details`; `description_too_long` → 400 (web) / 422 (API). S10–S14.
   Converts the edit path's current DB-CHECK **500** into a clean refusal.

## Lane safety

All scenarios `@pending` → excluded by `filter_run` from every lane (default, `@all`, browser), so `@all`
stays green until DELIVER un-@pends each slice as it lands. `fail_on_skipped()` remains on — an un-@pended
scenario with no matching step definition FAILS the lane rather than passing silently (the acceptance-runner
lesson from UI-4). Full `@all` at finalize.

## Falsification checks (a passing scenario must be able to fail)

Per the standing "a green can be an artefact of the instrument" lesson, each of these MUST be shown RED before
being accepted GREEN:
- **S2** red against a `submit_create` that drops `form.description` (the field must actually reach the store).
- **S6** red against a `new_issue_modal_page.html` that lacks the textarea (a green htmx path says nothing
  about the fallback surface).
- **S12/S13** red against an off-by-one (exclusive) bound and against a byte-count (rather than char-count)
  implementation.
- **S11** red against a validate-after-write ordering (the issue must be untouched on refusal — no partial
  write).
