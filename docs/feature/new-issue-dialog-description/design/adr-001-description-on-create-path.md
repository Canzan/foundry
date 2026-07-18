# ADR-001 — Thread `description` through the shared create path (web + API)

**Status**: Accepted (2026-07-17) · **Resolves**: ODD-2, ODD-4 · **Story**: US-01, US-02

## Context

The new-issue dialog collects Title only; the edit dialog collects Title + Description. The gap is present at
every layer of the create path (template → `CreateIssueForm` → `services::create_issue` →
`insert_issue_with_outbox`), while the edit path has the field at each corresponding layer. The
`issues.description_md` column exists and the edit path writes it, so **no migration** is required. The create
service is **shared** with the JSON API (`create_issue_handler` documents "the SAME path the browser handler
uses — identical validation/authz/outbox", NFR-WEB-API-CON-02), so the field must be threaded through the
service, which necessarily reaches the API surface.

## Decision

Add `description` as an **optional** field at each create-path layer, mirroring the shipped edit-path shapes:

| Layer | Change | Mirrors |
|-------|--------|---------|
| `partials/new_issue_modal.html` | `<textarea name="description">{{ description }}</textarea>` | `issue_edit_modal.html:7` |
| `views::NewIssueModal`, `views::NewIssueModalPage` | `+ description: String` (both include the one partial) | `IssueEditModal.description` |
| `CreateIssueForm` | `#[serde(default)] description: String` | `EditIssueForm` (`issues.rs:269`) |
| `services::create_issue` + facade | `+ description: &str` param | `edit_issue_details` |
| `insert_issue_with_outbox` | `+ description: &str` → `INSERT … description_md` | `update_issue_details` |
| `CreateIssueRequest` (API) | `#[serde(default)] description: String` | `EditIssueForm` serde |
| `create_issue_handler` | forward `request.description` | its existing `title` handling |

**Serde shape (ODD-2)**: `#[serde(default)] description: String`. Absent field, JSON `null`-absent, and empty
string all normalize to `""`. Existing API clients that omit `description` are byte-compatible (still `201`,
`description_md = ""`).

**Echo-back (ODD-4)**: the template binds `{{ description }}`, but since htmx 2.0.4 does not swap the 400
validation response (see architecture §Error behavior), the typed input is preserved by the browser anyway.
The binding is defensive, not load-bearing — no separate "re-render the modal with the submitted description"
handler is built. This is why ODD-4 resolves to "no dedicated echo path needed."

**Response contracts unchanged**: the web happy path returns the same OOB Backlog card (the card does not show
a description); the API returns the same `IssueJson { key, number, title, state }`. AC-02.2's read-back
equality is served by the persisted value, not by widening the create response.

## Alternatives considered

- **Web-only (API unchanged)** — rejected by the user (2026-07-17): the shared service would gain the param
  regardless, so leaving the API unable to send it knowingly breaks the stated rule-parity invariant to save
  one struct field.
- **Widen the API create response to echo `description`** — rejected: an api-contract change with no
  acceptance need; read-back equality already covers it.
- **A separate create-with-description store fn** — rejected: `insert_issue_with_outbox` is the one insert
  seam; a parallel fn would fork the outbox/number-allocation logic. Add a param instead.

## Consequences

- Every existing `create_issue` / `insert_issue_with_outbox` call-site must pass `""` and stay behaviorally
  identical — enforced by store tests asserting empty-description creates are byte-identical to today.
- One new INSERT column binding; no schema change; latest migration stays `0014`.
- The API and web now accept identical create inputs, restoring the parity the code comments already assert.
