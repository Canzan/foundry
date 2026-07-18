# Outcome KPIs — new-issue-dialog-description

| KPI | Target | Measurement |
|-----|--------|-------------|
| Field present | `c` / New issue → dialog shows Title **and** Description on both surfaces | Browser dogfood (`@needs-browser` lane) + acceptance AC-01.1/.7 |
| One-pass capture | 0 of 0 — filing a described issue requires **no** second dialog (today: 1 extra open+save) | Browser dogfood: create with description → card shows → no edit needed |
| Persistence + round-trip | description_md persisted; edit dialog shows it byte-identical | Store test + acceptance AC-01.2/.4 |
| Optionality preserved | Title-only create behaves byte-identically to today | Acceptance AC-01.5 + existing create scenarios stay green unchanged |
| Input never lost | A typed description survives a title-validation re-render | Acceptance AC-01.6 |
| No-JS fallback | Plain POST on `new_issue_modal_page.html` creates **with** the description | Acceptance AC-01.7 (asserted separately from the htmx path) |
| Web/API parity | API create accepts description; omitting it is unaffected; same 422 copy as the UI | Acceptance AC-02.1-.4 + `cli.sh` dogfood |
| Bound enforced | Over-long refused on **both** dialogs, nothing persisted; at-bound accepted | Acceptance AC-03.1-.5 |
| Tenancy safe | 0 cross-workspace writes; foreign project → uniform 404 | Acceptance AC-01.8 + store isolation test |
| Change-history coherence | Creating with a description emits **0** timeline events; first edit reports it as `old_value` | Acceptance (cross-feature scenarios) |
| Mutation | ≥80% kill on the new validation + create-description path | cargo-mutants, feature-scoped (rebuild the `foundry` bin, `--test-package foundry-app`) |
| No regressions | default lane + `@needs-browser` lane + `cargo xtask ci` green | full CI locally (no PR — trunk-based) |

**North-star**: a member presses `c`, types what the work *is* as well as what it's called, hits Create — and
the thought is fully captured in one pass, with nothing left to come back for.

## Counter-metric (guard against a green-over-nothing outcome)

**A passing suite must be able to fail.** Per the standing lesson from UI-4 (the runner that was green over
undefined steps) and the blur-on-arrival scenario that ran 6/6 green while destroying a human's typing:

- The AC-01.6 "description survives a title error" scenario MUST be shown red against an implementation that
  drops the field — otherwise it asserts nothing.
- The AC-01.7 no-JS scenario MUST be shown red against a fallback template that lacks the textarea — a green
  htmx lane is not evidence about the fallback surface.
- The AC-03.4 at-bound scenario MUST be shown red against an off-by-one (exclusive) bound.

Each of these is a falsification check to run during DELIVER, not a box to tick.
