# DISTILL — Walking Skeleton: navigation-bar-linear-ui

## The skeleton scenario

```gherkin
@pending @us-01 @walking_skeleton @real-io
Scenario: The dashboard shows the shared sidebar with Home current
  When Ada visits "/"
  Then a persistent left sidebar is shown
  And the sidebar shows the workspace name "Acme"
  And the sidebar shows primary navigation items "Home" and "Board"
  And the "Home" navigation item is marked as the current page
  And the "Board" navigation item is not marked as current
```

(Background: workspace "Acme" with admin Ada / "Ada Lovelace", a project "Sandbox"
in "Acme", Ada signed in.)

## Why this is the thinnest end-to-end slice

The skeleton closes the whole loop through the **production composition root** — a
real signed-in HTTP GET on `/` → the real `dashboard_root` handler → the new
`app_shell.html` + `partials/sidebar.html` → rendered HTML — and asserts the *user's*
first observable outcome: "I can see the shared rail, it names my workspace, it offers
the two primary destinations, and it tells me I'm Home."

It is the **minimum** that proves the feature's core mechanism is wired, because it
forces DELIVER to stand up every load-bearing part exactly once:

- the `NavContext` value object assembled from the session (workspace_name,
  display_name, active section),
- the `app_shell.html` inheritance layer that injects the rail,
- the `sidebar.html` partial rendering brand + the Home/Board primary items,
- the server-authoritative active-state (`NavSection::Home` → `aria-current="page"` on
  Home, not Board).

Once this is GREEN, every other scenario is an increment on an already-wired rail:
presence on other pages (US-04), the Board active-state + deep-link (US-01/ADR-003),
the footer user menu (US-02), instance-admin gating (US-03), the pre-auth absence
guard, and the Quick-actions scoping guard (US-05). None of them re-stand-up the
shell; they extend it.

## Litmus test (non-technical stakeholder)

"Devon signs in, lands on the dashboard, and sees a left rail with their workspace
name and two links — Home (highlighted, because they're on Home) and Board." — yes,
that is the user-visible value, not a layer-wiring statement.

## Demo path

`FOUNDRY_ACCEPTANCE_TAGS=walking_skeleton cargo test -p foundry-acceptance --test acceptance`
(after DELIVER removes this scenario's `@pending` tag).
