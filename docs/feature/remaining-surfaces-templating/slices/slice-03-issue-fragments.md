# Slice 03 — Issue-create-error + state-change fragments

- **Story**: US-R03
- **Job**: htmx-web-1
- **Type**: move-only refactor
- **Effort**: ≤1 day (two tiny fragments)
- **Learning hypothesis**: Tiny mutating-response fragments (`data-hx-fragment`,
  `data-state`) move to templates with their markers + error copy byte-stable.

## Surfaces

| Site | Kind | Today | Target |
|------|------|-------|--------|
| `issues.rs::bad_request_fragment` | fragment | inline `<div class="error" data-hx-fragment="issue-create-error">` + "Title is required" | error-fragment template; marker + copy byte-stable |
| `issues.rs` state-change response | fragment | inline `<span class="state" data-state="{state}">{state}</span>` | tiny state-fragment template; `data-state` byte-stable |

## Done when
- [ ] `bad_request_fragment` renders from a template; `data-hx-fragment="issue-create-error"` + "Title is required" byte-stable.
- [ ] State-change `<span>` renders from a template; `class="state" data-state` byte-stable.
- [ ] Both remain bare fragments (no `base.html`).
- [ ] `cargo test -p foundry-acceptance` passing count does not drop; no scenario edited.
- [ ] No inline HTML `format!()` left in these two sites.

## Notes
- May reuse the shared error-fragment partial from Slice 01 (US-R01).
- US-R03 has 2 UAT scenarios (two tiny moves); a 3rd ("invalid issue state") can be
  promoted from Domain Example 3 if a reviewer requires 3 — trivial, not a blocker.
