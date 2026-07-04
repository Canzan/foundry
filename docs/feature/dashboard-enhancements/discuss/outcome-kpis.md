# Outcome KPIs — dashboard-enhancements

| KPI | Target | Measurement |
|-----|--------|-------------|
| Dashboard test coverage | Base query + render covered; ≥1 store test + ≥1 acceptance scenario green | `cargo test -p foundry-store`; `@all` acceptance lane |
| Mutation kill rate (store scope) | ≥ 80% on the new/covered dashboard store queries | `cargo mutants` feature-scoped (per repo bar) |
| Role-correct nav | 0 instances of the instance-admin link rendered to a non-super-admin | AC-03.2 acceptance scenario (assert body absence) |
| No regressions | `@all` scenarios remain green; `cargo xtask ci` green | full CI lane |
| Inline-style debt | 0 `<style>` blocks in `dashboard_root.html` after slice 04 | grep + AC-04.1 |
| Visual equivalence (refactor) | Dashboard renders identically pre/post style promotion | browser diff (claude-in-chrome screenshot) |

**North-star**: a signed-in user lands oriented (name + workspace), reaches only the tools their role
grants, and can sign out — with the surface protected against silent regression.
