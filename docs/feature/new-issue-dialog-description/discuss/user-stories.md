# User Stories — new-issue-dialog-description

## Jobs-to-be-Done

**J1 (primary)** — When I press `c` or click **New issue** to capture work I just thought of, I want to write
down *what the work actually is* — not just its name — so I can offload the whole thought and stop holding
the detail in my head.

| Dimension | Statement |
|-----------|-----------|
| Functional | Record the title AND the context/detail of a piece of work in one pass. |
| Emotional | Relief of a complete hand-off — nothing left to remember and come back for. |
| Social | Teammates see an issue that explains itself; I'm not the person who files bare one-liners. |

**Four forces (J1)**

| Force | Evidence |
|-------|----------|
| **Push** | The dialog physically cannot take a description (`new_issue_modal.html` renders Title only), so every issue is born context-free. |
| **Pull** | The *edit* dialog already offers Description (`issue_edit_modal.html:7`) — the capability visibly exists one click away. |
| **Anxiety** | None material. Description is optional; a title-only create must keep working exactly as today. |
| **Habit** | Today's workaround is create → reopen the card → edit → paste the description. A two-step ritual to do one thing. |

**J2 (secondary)** — When I create issues from a script or integration, I want the API to accept the same
fields the UI does, so my automation isn't a second-class citizen that files issues a human then has to fix up.

Push: `create_issue_handler` is documented as taking "the SAME path the browser handler uses" (rule-parity,
NFR-WEB-API-CON-02) — a UI-only description would silently break that stated invariant.

**Opportunity scores**

| Job | Importance | Satisfaction | Gap | Rank |
|-----|-----------|--------------|-----|------|
| J1 — describe work at capture time | High | Low (impossible in-dialog) | **High** | 1 |
| J2 — API accepts what the UI accepts | Medium | Low (field absent) | Medium | 2 |

---

## US-01 — Write a description while filing the issue

**As a** workspace member filing an issue from the board (P1)
**I want** the new-issue dialog to offer a Description field beside Title
**so that** I can capture the detail of the work at the moment I think of it, in one pass.

**job_id**: J1

### Elevator Pitch
Before: pressing `c` opens a dialog with Title only — to add any detail I must create the issue, find the
card, open the edit dialog, and type it there.
After: press `c` → dialog shows **Title** *and* **Description** → type both → **Create** → the card lands in
Backlog with the description already persisted.
Decision enabled: I decide the thought is fully captured and move on — instead of deciding to come back later.

### Acceptance Criteria
- AC-01.1: `GET …/issues/new` renders a dialog containing a `description` textarea beside the title input,
  matching the shipped edit dialog's field (`<label>Description <textarea name="description">`).
- AC-01.2: Submitting title + description issues an htmx `POST …/issues` carrying `_csrf`, `title`, and
  `description`; the description is persisted to `issues.description_md`.
- AC-01.3: The created card lands in **Backlog** via the shipped OOB swap and the modal closes — no
  full-page navigation, unchanged from today.
- AC-01.4: Reopening the new issue's **edit** dialog shows the description exactly as typed (round-trip).
- AC-01.5: **Description is optional** — submitting a title with an empty description creates the issue with
  an empty `description_md`, exactly as today. No new required-field friction.
- AC-01.6: Empty title still returns the shipped "Title is required" error, and a typed description is
  **not** lost. *(DESIGN-verified: satisfied with zero code — htmx 2.0.4 does not swap the 400 response, so
  the modal stays open with input intact. The error message is not shown in-browser; that is a pre-existing
  app-wide defect, deferred. See `design/upstream-changes.md`.)*
- AC-01.7: **No-JS fallback preserved on BOTH surfaces** — `new_issue_modal.html` (htmx) and
  `new_issue_modal_page.html` (the full-page fallback) each carry the field; with htmx disabled the plain
  POST still creates the issue with its description.
- AC-01.8: Tenancy unchanged — a foreign team/project still yields the uniform 404; no cross-workspace write.

---

## US-02 — Create an issue with a description over the API

**As an** integration/automation author using a machine token (P2)
**I want** `POST /api/v1/teams/{team}/projects/{project}/issues` to accept a description
**so that** my automation files complete issues, not stubs a human has to finish.

**job_id**: J2

### Elevator Pitch
Before: the API create endpoint accepts `title` only — scripted issues are born context-free even though the
column exists.
After: `POST /api/v1/teams/acme/projects/gen/issues` with `{"title":"…","description":"…"}` → `201 Created`
and a subsequent `GET …/issues/{n}` echoes the persisted description.
Decision enabled: I decide my integration is complete — no follow-up PATCH, no human clean-up pass.

### Acceptance Criteria
- AC-02.1: `CreateIssueRequest` accepts an **optional** `description`; a request omitting it behaves exactly
  as today (empty `description_md`, `201 Created`) — existing API clients are unaffected.
- AC-02.2: The persisted description is the one the service normalized, and equals a subsequent read
  (NFR-WEB-API-CON-02 — the returned representation must equal a subsequent read).
- AC-02.3: Web and API go through the **same** `services::create_issue` — identical validation, authz, and
  outbox behavior (rule-parity preserved, per the invariant asserted in `create_issue_handler`'s docs).
- AC-02.4: An over-long description is rejected by the service as `Validation` → **422** with the same copy
  the UI shows (rule-parity, mirroring how `title_required` behaves today).

---

## US-03 — Oversized descriptions are refused, not silently stored

**As a** workspace member (P1) / operator (P3)
**I want** a description length bound enforced on both create and edit
**so that** a paste-bomb is refused with a clear message instead of being written to the database.

**job_id**: J1 (integrity precondition)

### Elevator Pitch
Before: `description_md` is bounded only at the **database** (`CHECK length ≤ 262144`); the service has no
guard, so an over-long edit description hits the DB CHECK and surfaces as an **HTTP 500**, not a clean
refusal. *(DESIGN-corrected: DISCUSS first believed this unbounded — see `design/upstream-changes.md`.)*
After: an over-long description in either path → **Save**/**Create** → a clean `description_too_long`
validation error (422 on the API), nothing persisted, at the **same 262144 threshold the DB already enforces**.
Decision enabled: I get a clear "too long" instead of an opaque server error.

### Acceptance Criteria
- AC-03.1: A `DESCRIPTION_MAX_LEN` bound (**262144**, matching the DB CHECK — ADR-002) is enforced in the
  shared service validation, applied to **both** `create_issue` and `edit_issue_details` — one rule, both
  paths. Over-long now returns a validation error instead of a DB-CHECK 500.
- AC-03.2: An over-long description on **create** renders the error inside the open modal; no issue is
  created (mirrors the `title_required` fragment behavior).
- AC-03.3: An over-long description on **edit** renders the error inside the open dialog; the issue's
  existing title and description are **unchanged** (nothing partially written).
- AC-03.4: A description exactly at the bound is accepted (boundary is inclusive and tested).
- AC-03.5: Counting matches the title rule — `chars().count()`, not bytes — so multi-byte text isn't
  penalized (consistent with `TITLE_MAX_LEN` enforcement at `issues.rs:60`).
