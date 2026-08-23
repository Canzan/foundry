# Slice 03 — Refused renames explain themselves inline

Story: US-IAPR-03 | Estimate: 1 day | job_id: `job-instance-project-rename`

## Goal

Invalid renames (empty, over-long, duplicate-in-team) are refused with 422 and
a bare error fragment that lands in the submitting row's `[data-error-slot]`
via `form-errors.js` — the operator sees the reason in place and resubmits
without a reload; the persisted name never changes on a refusal.

## IN

- Validation rules (D4):
  - trimmed name non-empty → "Project name must not be empty"
  - ≤256 characters (mirrors `issues.title` precedent; `projects.name` has no
    DB CHECK) → "Project name must be at most 256 characters"
  - within the same team, refuse when the trimmed new name case-insensitively
    equals another project's name OR `slugify(new name)` equals another
    project's stored slug (excluding the project itself) → "Project name must
    be unique within the team"
- 422 + bare error fragment (`error_fragment.html` idiom); form stays mounted.
- `<div data-error-slot></div>` inside each row's rename form (the
  `form-errors.js` opt-in — without it htmx discards the 4xx silently, the
  exact defect that script exists to fix).
- `@needs-browser` fantoccini scenario proving the message renders in the DOM
  (the HTTP lane is byte-blind to the swap).

## OUT

- Create-path duplicate residual (D7, recorded, not fixed here).
- Any new DB CHECK constraint on `projects.name` (handler-level only; DESIGN
  may propose one separately).
- Race-free uniqueness (check-then-write acceptable at homelab scale; noted).

## Learning Hypothesis

The established error-slot contract composes onto a repeated (per-row) form
without target ambiguity — each row's form resolves its own slot.

## Acceptance Criteria

- [ ] Empty/whitespace, 300-character, and duplicate ("Sandbox", "sandbox")
      submissions each return 422 with the exact message above.
- [ ] The message appears inside the submitting row's `[data-error-slot]`
      without a page reload; the form remains resubmittable (browser lane).
- [ ] After every refusal, the name is unchanged on dashboard, board, and
      report.

## Dependencies

Slice 02 (rename form + write port).
