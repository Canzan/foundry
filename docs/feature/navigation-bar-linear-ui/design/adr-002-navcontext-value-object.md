# ADR-002: Shared `NavContext` value object vs threading individual fields

## Status
Accepted

## Context
The sidebar depends on five values on **every** authed page: `display_name`, `workspace_name`,
`is_instance_admin`, `csrf`, and `active_section` (shared-artifacts registry). Several authed
handlers do not currently pass all of these — `BoardPage`, for example, lacks
`display_name`/`workspace_name`/`csrf` today. This is the feature's highest integration risk
(registry marks it HIGH). Askama is compile-time typed, so any field the partial references must
exist on the page struct or the build fails.

Question: **how do the five values reach the sidebar partial on 12+ pages without five-field
boilerplate scattered across every struct and handler?**

## Decision
Define a single value object `NavContext` (fields: `workspace_name`, `display_name`,
`is_instance_admin`, `csrf`, `active: NavSection`, `board_href`) and an enum
`NavSection { Home, Board }` in a new module `crates/foundry-app/src/nav.rs`. Each authed page struct
embeds **one** field `pub nav: NavContext`. A single constructor
`NavContext::for_page(&session, active, board_href)` assembles it from the authenticated session, so a
handler builds it **once** and moves it into the view struct. `sidebar.html` renders only from
`nav.*` (with helper methods `is_home()`, `is_board()`, `monogram()` keeping the template logic-free).
`active` is set explicitly by the handler for its route (server-authoritative), guaranteeing the
"exactly one current" invariant (FR-4, AC-03.3) by construction.

## Alternatives considered

1. **Thread five individual fields into every page struct and handler.**
   *Rejected:* five repeated fields × 12 structs = wide, error-prone surface; each new authed page
   must re-add all five; a partial referencing a mis-named field fails compilation per page. It also
   scatters the "which section is active" decision as a bare string, inviting typos (`"projcts"`) that
   the type system cannot catch. `active` as `&str` also cannot enforce "exactly one / never zero."

2. **A global/request-extension singleton the template reads implicitly (e.g. an axum extension
   pulled by a template filter).**
   *Rejected:* Askama templates bind to the page struct's fields, not ambient request state; wiring an
   implicit global fights the engine's typed model, hides the dependency, and makes the "exactly one
   active" property untestable in a plain `#[derive(Template)]` unit render. Adds machinery for no gain.

3. **`NavSection` as a stringly-typed `active_section: String`.**
   *Rejected:* loses compile-time exhaustiveness and the natural home for `is_home()`/`is_board()`;
   permits invalid values. The enum is nearly free and makes the invariant a type property.

## Consequences
- **Positive:** the five-field fan-out collapses to one embedded field + one constructor call; adding
  a future authed page is "embed `nav`, build it once." The single source for every registry variable
  is documented and typed.
- **Positive:** `active` as `NavSection` makes "exactly one item current" a structural guarantee (one
  enum value) rather than a runtime hope; unit-testable without a browser.
- **Positive (Earned Trust):** a page that embeds `nav` but a partial field that doesn't exist, or a
  page that extends the shell without `nav`, is a **compile error** — the missing-context failure mode
  cannot ship silently.
- **Negative:** introduces a new module and a value object the crafter must bind to the real
  `SessionContext` field names during GREEN; a small, well-bounded amount of new code.
- **Negative:** `board_href` requires one cheap first-project lookup per page (see ADR-003); acceptable
  and reuses an existing query family.
</content>
