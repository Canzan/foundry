# Data Models — navigation-bar-linear-ui

The feature introduces **one in-memory presentation value object** and **one enum**. No persisted
data, no schema, no migration. These live in a new module `crates/foundry-app/src/nav.rs`.

## `NavSection` — which primary item is current

```rust
/// The active primary sidebar item. Exactly one value is chosen per authenticated
/// page (FR-4 / AC-03.3: never zero, never two). Two variants only — the rail has
/// exactly two primary destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    /// Dashboard `/` AND every non-board authed surface (tokens, invites,
    /// project-create, instance admin). Home is the app's default/hub section.
    Home,
    /// The board route family `/team/{slug}/project/{slug}` and its descendants
    /// (board, change report, issue detail).
    Board,
}
```

**Why two variants and not a third "neither":** the shared-artifacts registry fixes
`active_section` to `home | projects`, and FR-4 forbids a zero-active state. Mapping every non-board
authed surface to `Home` keeps the invariant total with two variants and is semantically honest —
those surfaces are reached from the Home/dashboard hub.

## `NavContext` — the shared identity + nav carrier

```rust
/// Everything the shared sidebar needs, assembled ONCE per authenticated page from
/// the session context, so handlers embed one field instead of threading five.
/// Single documented source for every shared-artifacts-registry variable.
#[derive(Debug, Clone)]
pub struct NavContext {
    pub workspace_name: String,     // brand + footer identity
    pub display_name: String,       // footer identity
    pub is_instance_admin: bool,    // gates the Instance-admin menu item (FR-6)
    pub csrf: String,               // hidden _csrf for the sign-out form (BR-3)
    pub active: NavSection,         // drives active class + aria-current
    pub board_href: String,         // resolved Board deep-link target (ADR-003)
}
```

### Template-facing helper methods (keep the partial dumb, avoid importing the enum path)

```rust
impl NavContext {
    /// Uppercased first character of the workspace name, for the brand monogram.
    pub fn monogram(&self) -> String { /* first grapheme, uppercased; "?" if empty */ }
    pub fn is_home(&self)  -> bool { self.active == NavSection::Home }
    pub fn is_board(&self) -> bool { self.active == NavSection::Board }
}
```

`sidebar.html` then reads `{{ nav.workspace_name }}`, `{{ nav.monogram() }}`, `{{ nav.display_name }}`,
`{{ nav.csrf }}`, `{{ nav.board_href }}`, `{% if nav.is_board() %}`, `{% if nav.is_instance_admin %}`
— no `NavSection::` path in the template.

## Constructor signature

```rust
impl NavContext {
    /// `session` is the already-resolved authenticated session context (workspace
    /// name, display name, admin flag, per-request CSRF token). `active` is chosen
    /// by the handler for its route. `board_href` is the deep-link target.
    pub fn for_page(session: &SessionContext, active: NavSection, board_href: String) -> Self;
}

/// Resolve the Board primary-nav target (ADR-003 deep-link).
/// Returns the first/default project's board `/team/{team_slug}/project/{project_slug}`;
/// if the workspace has no projects, returns `/` (the dashboard, whose empty-state
/// hosts the "create your first project" affordance).
pub fn resolve_board_href(session: &SessionContext /* or first-project lookup */) -> String;
```

> Field names in `SessionContext` are as the auth/session layer already exposes them; the crafter
> binds `NavContext` fields to those during GREEN. `NavContext` is a *presentation* projection of
> session identity — it does not re-fetch or re-authorize anything.

## How `active` is derived per route

`active` is set **explicitly by each handler**, not path-sniffed in the template (server-authoritative
→ unit-testable, no fragile string matching):

| Route family | Handler passes | Result |
|---|---|---|
| `/` (dashboard) | `NavSection::Home` | Home current |
| `/team/{slug}/project/{slug}` (board) | `NavSection::Board` | Projects current |
| `.../project/{slug}/report` | `NavSection::Board` | Projects current |
| issue detail (`/team/.../issues/...`) | `NavSection::Board` | Projects current |
| `/admin/tokens`, token mint/revoke | `NavSection::Home` | Home current |
| `/workspace/invites`, invite sent | `NavSection::Home` | Home current |
| `/team/{slug}/projects/new` (project create) | `NavSection::Home` | Home current |
| `/admin/instance/workspaces` (instance dashboard) | `NavSection::Home` | Home current |

## Shared-artifacts-registry variable → source mapping

| Registry variable | Source of truth | Carried by | Rendered at |
|---|---|---|---|
| `display_name` | auth/session context | `NavContext.display_name` | sidebar footer identity anchor; existing dashboard welcome line reads the same session value |
| `workspace_name` | auth/session context | `NavContext.workspace_name` | sidebar brand (top) + footer anchor; existing dashboard workspace line reads the same value |
| `is_instance_admin` | authorization layer (existing flag) | `NavContext.is_instance_admin` | `{% if nav.is_instance_admin %}` around the Instance-admin item → absent from HTML for non-admins (FR-6) |
| `csrf` | CSRF middleware (per-request token) | `NavContext.csrf` | hidden `_csrf` in the footer sign-out form (`POST /sign-out`) |
| `active_section` (`home`\|`projects`) | request handler / route | `NavContext.active: NavSection` | active class + `aria-current="page"` on exactly one primary item |
| Board link target (registry open question) | first-project lookup (ADR-003) | `NavContext.board_href` | `href` on the Projects item |

No variable has two divergent sources: the dashboard's existing identity lines and the rail both read
the single session-provided values; the rail's sign-out targets the same `POST /sign-out` + `_csrf`
field as the existing dashboard form.
</content>
