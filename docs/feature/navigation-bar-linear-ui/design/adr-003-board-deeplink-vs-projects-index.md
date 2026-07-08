# ADR-003: "Projects" nav target — deep-link to default board vs new projects-index route

## Status
Accepted

## Context
The rail's second primary item is "Projects" (FR-3), but **no dedicated projects-index route exists
today** — boards live at `/team/{slug}/project/{slug}`, and the dashboard hosts a "Your projects"
list. The shared-artifacts registry flags this as the feature's open question because it affects the
Projects `href`, the Board active-state route family (FR-4), and whether new routing/handlers are in
scope. The feature is deliberately scope-contained (presentation only, no new crate/route/migration).

## Decision
The "Projects" item **deep-links to the workspace's first/default project board**
(`/team/{slug}/project/{slug}`), resolved server-side into `NavContext.board_href` by
`resolve_board_href(&session)`. When the workspace has **zero** projects, `board_href` falls back to
`/` (the dashboard), whose existing empty-state already hosts the "create your first project"
affordance. The Board active state matches the `/team/{slug}/project/{slug}` route family (board,
report, issue detail). **No new projects-index route is created now** — it is recorded as deferred.

## Alternatives considered

1. **Create a new `/projects` index route + handler + template.**
   *Rejected for this pass:* adds routing, a handler, a new template, and its own tests — outside the
   feature's "presentation-tier, no new route" containment. It also invents a surface the product has
   not asked for; the dashboard already lists projects. Reasonable as a *future* enhancement (kept in
   Deferred), not justified by the current ACs, which only require "Projects navigates to the projects
   surface" (AC-04.2, deliberately vague).

2. **Point "Projects" at the dashboard `/` and rely on its "Your projects" section.**
   *Rejected:* makes Home and Projects resolve to the same URL, breaking the two-destination model and
   the active-state distinction (a user on `/` could not tell which item is "current"). Fails FR-4's
   "exactly one, unambiguous."

3. **Link to the last-visited board (per-session memory).**
   *Rejected:* requires persisting per-user navigation state — new data and a new source of truth,
   contradicting the no-migration/no-new-state scope. Deep-linking to the *first* project is
   stateless and predictable.

## Consequences
- **Positive:** zero new routes/handlers/templates/migrations; honors scope containment. Stateless and
  deterministic. Satisfies AC-04.2 and the FR-4 route-family active state.
- **Positive:** the empty-workspace case degrades gracefully to the existing create-project affordance
  — no dead link, no 404.
- **Negative:** `resolve_board_href` needs one cheap "first project for workspace" read per authed
  page render. This reuses the same query family the dashboard already issues for "Your projects"; no
  new schema, negligible cost. If profiling ever flags it, it can be memoized on the session.
- **Negative:** "first/default project" ordering must be defined (e.g. lowest id / creation order) —
  a small deterministic rule the crafter pins; the ACs do not constrain *which* board, only that
  Projects reaches a board.
- **Deferred:** a real `/projects` index remains a clean future addition; the Board item's `href`
  would simply change to it, with no change to `NavContext` shape or the active-state model.
</content>
