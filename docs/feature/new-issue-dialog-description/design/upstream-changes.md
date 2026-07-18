# Upstream Changes — new-issue-dialog-description (DESIGN)

DESIGN verification corrected two DISCUSS premises and discovered one pre-existing defect. Recorded here per
the back-propagation contract. The corrected DISCUSS artifacts were updated in place (same-session drafts);
this file is the provenance.

## Correction 1 — `description_md` is bounded at the DB, not unbounded

- **DISCUSS said** (`requirements.md` F2, `user-stories.md` US-03): "`description_md` is **unbounded** on the
  shipped edit path."
- **Verified**: `crates/foundry-store/migrations/0001_init.sql:70` —
  `description_md TEXT NOT NULL DEFAULT '' CHECK (length(description_md) <= 262144)`. Bounded at the **DB**
  (262144 chars). What is missing is an **application** bound.
- **Effect on design**: US-03 is reframed from "add a missing bound" to "add an app bound at the DB's
  threshold, converting today's DB-CHECK **500** into a clean **422**." Value set to 262144 (ADR-002), not the
  65536 first proposed in DISCUSS.
- **DISCUSS fix applied**: `user-stories.md` US-03 and `discuss/wave-decisions.md` D2 updated to say
  "bounded only at the DB (262144), surfaces as 500"; the ODD-1 proposal changed 16384 → 262144.

## Correction 2 — validation errors do not destroy the modal; they are invisible

- **DISCUSS implied** (AC-01.6 rationale, and my mid-session claim): a validation error swaps into
  `#modal-root` and would destroy/lose the typed description.
- **Verified**: vendored htmx is **2.0.4**; default `responseHandling` is `{code:"[45]..",swap:false}` with
  **no app override**, no `response-targets` extension, no `htmx:responseError` handler; `bad_request_fragment`
  returns **400** (asserted at `feature_board_new_issue.rs:292`). htmx therefore does **not swap** the 400 —
  the modal stays open, typed input is **preserved**, and the error body is discarded.
- **Effect on design**: **AC-01.6 is satisfied with zero code.** No echo-back handler, no slice 04. The
  `{{ description }}` template binding is retained as defensive only.
- **DISCUSS fix applied**: `acceptance-criteria.md` AC-01.6 annotated "satisfied by htmx non-swap; no build
  work"; `user-stories.md` US-01 AC-01.6 clarified.

## Discovered — app-wide invisible validation errors (DEFERRED, not this feature)

`board-new-issue` D3 claims "the error swaps INTO the modal — visible." Under the shipped htmx config that is
**false**: no htmx form's 4xx validation body is shown in the browser (new-issue, edit dialog, and the future
too-long-description case). The acceptance suite never caught it because every relevant step asserts the HTTP
**response body**, never the resulting **DOM** — the same green-over-DOM pattern as UI-4 (the runner green over
undefined steps) and the blur-on-arrival scenario.

- **Scope call** (user, 2026-07-17): **out of scope** for "add a description field." Filed as its own
  `/nw:root-why` bugfix. This feature ships errors that behave identically to every other form — no regression.
- **When it is fixed**, the `description_too_long` 422/400 (ADR-002) becomes visible in-browser for free; the
  JSON API already shows it today.

## No documents modified outside this feature

No `board-new-issue`, `issue-edit-dialog`, or `issue-change-history` document was edited. The `board-new-issue`
D3 correction is recorded here and will be carried by the deferred bugfix, not by rewriting shipped history.
