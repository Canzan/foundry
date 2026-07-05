# Outcome KPIs — issue-edit-dialog

| KPI | Target | Measurement |
|-----|--------|-------------|
| Editing works | Click card → edit title/desc → save → card updates in place | Browser dogfood + acceptance |
| Persistence | title + description_md persisted for the issue | Store test + acceptance |
| Tenancy safe | 0 cross-workspace edits; foreign issue → uniform 404 | Acceptance (AC-01.6) + store isolation test |
| Validation | empty/oversized title rejected in dialog, nothing persisted | Acceptance (AC-01.4/.5) |
| No-JS fallback | plain POST saves + returns to board | Acceptance (AC-01.8) |
| Mutation | ≥80% kill on the new store/service edit path | cargo-mutants feature-scoped |
| No regressions | @all lane + xtask ci green | full CI |

**North-star**: a member can fix an issue's title/description from the board in a focused dialog and see it
immediately — without a page navigation.
