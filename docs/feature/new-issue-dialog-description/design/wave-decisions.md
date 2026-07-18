# DESIGN Decisions — new-issue-dialog-description

## ODD resolutions

| # | Question | Resolution |
|---|----------|-----------|
| ODD-1 | `DESCRIPTION_MAX_LEN` value | **262144**, matching the DB CHECK (`0001_init.sql:70`). Pure "same rule, better error" — no shipped-behavior tightening. `chars().count()`. (ADR-002) |
| ODD-2 | Serde shape for the field | `#[serde(default)] description: String` on both `CreateIssueForm` and `CreateIssueRequest`; absent/null/"" all normalize to empty. Existing clients byte-compatible. (ADR-001) |
| ODD-3 | Error code + copy | `ServiceError::Validation { code: "description_too_long", message: "Description is too long" }`. Web → `bad_request_fragment` (400); API → 422. (ADR-002) |
| ODD-4 | Does the modal need a dedicated echo-back path? | **No.** htmx 2.0.4 doesn't swap the 400, so typed input is already preserved. The `{{ description }}` binding is defensive only. (ADR-001, architecture §Error behavior) |

## Key decisions

- **[D1] Optional `description` threaded through the shared create path** (web + API). Keeps rule-parity
  (NFR-WEB-API-CON-02). No migration; no new component; mirrors the edit path at every layer. (ADR-001)
- **[D2] One `DESCRIPTION_MAX_LEN = 262144` on create AND edit**, matching the DB CHECK. Converts today's
  edit-path 500 (DB rejection) into a clean 422/validation error at the identical threshold. (ADR-002)
- **[D3] Narrow scope — no error-visibility work.** Verification proved typed input is already preserved and
  that browser-invisible validation errors are an **app-wide, pre-existing** defect (not introduced here).
  Deferred to its own `/nw:root-why` bugfix. (upstream-changes.md §Discovered; user decision 2026-07-17)
- **[D4] Response contracts unchanged.** Web returns the same OOB card (card shows no description); API returns
  the same `{key,number,title,state}`. Read-back equality serves AC-02.2. (ADR-001)
- **[D5] Paradigm unchanged.** Established ports-and-adapters layering; `@nw-software-crafter` implements. No
  `CLAUDE.md` paradigm section added.

## Existing-system analysis (performed before design)

- Create path read end-to-end: `submit_create` (`app/issues.rs:70`) → `services::create_issue`
  (`services/issues.rs:49`) → `insert_issue_with_outbox` (`store/lib.rs:1359`). Gap confirmed at each layer.
- Edit path read as the mirror: `edit_issue_details` (`services/issues.rs:174`), `IssueEditModal`,
  `issue_edit_modal.html:7`.
- DB schema read: `issues.description_md … CHECK (length ≤ 262144)` (`0001_init.sql:70`) — corrected the
  DISCUSS "unbounded" premise.
- htmx behavior read: vendored 2.0.4, default `responseHandling` (4xx not swapped), no override, no
  response-targets ext, `bad_request_fragment` = 400 asserted at `feature_board_new_issue.rs:292` — corrected
  the "modal destroyed / input lost" premise.
- Cross-feature: `issue-change-history` ODD-5 ("start empty", `insert_issue_with_outbox` = REUSE) confirms a
  created description emits no timeline event.

## Reuse vs new

**100% reuse of shape; zero net-new components.** Every change is a field or a param added to an existing
struct/fn/template, or a const beside an existing one. The one genuinely new line of logic is the description
length check — itself a copy of the title check.

## DELIVER note — `insert_issue_with_outbox` call-sites (verified at design review)

Adding a `description: &str` param to `insert_issue_with_outbox` (`store/lib.rs:1359`) is a compile-forcing
change; every existing caller must pass `""` to stay byte-identical. Confirmed call-sites (2026-07-17 review):

1. `crates/foundry-store/src/lib.rs` — the fn definition
2. `crates/foundry-services/src/issues.rs:69` — `create_issue` (this is the one that gets a REAL value, slice 01)
3. `crates/foundry-services/tests/write_use_cases.rs`
4. `crates/foundry-services/tests/board_use_case.rs`
5. `crates/foundry-acceptance/src/steps/keyboard_shortcut_bindings.rs`
6. `crates/foundry-acceptance/src/steps/feature_mwt_slice_01_coexist.rs`
7. `crates/foundry-acceptance/src/steps/feature_a_programmatic.rs`

Only #2 threads a real description; the rest pass `""`. The compiler enforces completeness, so this is a
mechanical MEDIUM-risk task, not a hidden one. DELIVER should re-grep before editing in case new callers land.

## Handoff

**To**: DEVOPS (KPIs only — no new infra/observability; `foundry_token_mutations_total`-style exporters not in
scope) and DISTILL. **Deliverables**: this `design/` set + the corrected DISCUSS artifacts. The invisible-error
defect is filed for a separate bugfix and must NOT be smuggled into DISTILL scenarios for this feature.

## Design review

Peer-reviewed by `nw-solution-architect-reviewer` (2026-07-17): **approved**, 0 blockers / 0 majors. All 8
load-bearing claims re-verified against source. Only residual: the 7-call-site mechanical change above.
