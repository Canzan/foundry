# Journey (Visual): Navigate the Foundry app via a shared Linear-style sidebar

**Feature**: `navigation-bar-linear-ui`
**Persona**: Devon Park — workspace member on the "Acme" workspace, moves many times a day between the dashboard and a project board.
**Goal**: Always know where they are and reach any primary surface in one click, from any authenticated page.

## Emotional arc

Confidence-building / orientation.

| Phase | State | What drives it |
|-------|-------|----------------|
| Start | Slightly disoriented — "which page am I on, how do I get back?" (today nav is ad-hoc per page) | Scattered per-page links; board only offers "Change report"/"New issue" |
| Middle | Oriented and in control — a persistent rail is always there, current section is highlighted | Sidebar present on every authed page, active item tinted |
| End | Confident and fast — one click to Home or a project board, account actions always in the same corner | Consistent surface, muscle memory, `aria-current` matches the page |

The arc must never regress: pre-auth pages (signin, bootstrap) stay chrome-free so a signed-out user is never shown a nav they cannot use.

## Happy path (ASCII flow)

```
[Devon opens Foundry] ─▶ [Dashboard "/"] ─▶ [Clicks "Projects" in rail] ─▶ [Project board] ─▶ [Opens user menu] ─▶ [Sign out]
     Feels: returning        Sees: rail +          Sees: rail persists,          Sees: same rail,        Sees: workspace +      Feels: done,
     wants orientation       "Home" active         "Projects" now active         board content right     name, menu opens       safely signed out
     Artifacts:              display_name,          active_section=projects       active_section=          csrf, is_instance_     (pre-auth page,
     workspace_name          active_section=home                                  projects                 admin gated            no rail)
```

## Shared left sidebar — target layout (Linear-style)

Persistent rail ~220–260px on the left; page content offset to its right. Quiet neutral surface, subtle right border. Present on authenticated app pages only.

```
+------------------------------+---------------------------------------------+
|  [A] Acme                     |                                             |
|      workspace                |   {% block content %}  page content here    |
|                               |                                             |
|  ── navigation ──             |   e.g. Dashboard "Your projects",           |
|  [#] Home            ◀ active |        a project board, an issue,           |
|  [▦] Projects                 |        token list, invite form, report      |
|                               |                                             |
|                               |                                             |
|  (rail footer, pinned)        |                                             |
|  ┌──────────────────────────┐ |                                             |
|  │ [D] Devon Park           │ |                                             |
|  │     Acme            ▾    │ |   <- user/account menu anchor               |
|  └──────────────────────────┘ |                                             |
+------------------------------+---------------------------------------------+
```

Active item state (accent-tinted background + higher-contrast label + `aria-current="page"`):

```
  [#] Home                 (idle: quiet label, transparent bg)
  [▦] Projects  ◀ ACTIVE   (tinted bg, stronger label, aria-current="page")
```

User/account menu (opens from the pinned footer anchor):

```
  ┌──────────────────────────┐
  │ Devon Park               │   <- {{ display_name }}
  │ Acme                     │   <- {{ workspace_name }}
  ├──────────────────────────┤
  │ Keyboard shortcuts       │   -> /keyboard-help
  │ Instance admin           │   -> /admin/instance/workspaces   (only if {{ is_instance_admin }})
  ├──────────────────────────┤
  │ Sign out                 │   -> POST /sign-out  (hidden _csrf = {{ csrf }})
  └──────────────────────────┘
```

## Pre-auth / utility pages — NO rail (chrome-free)

```
+--------------------------------------------------+
|                                                  |
|            Sign in to Foundry                    |   signin.html, forgot.html, forgot_sent.html,
|            [ email ......... ]                    |   bootstrap_*.html, invite_accept.html,
|            [ Sign in ]                            |   invalid_page.html, payload_too_large.html,
|                                                  |   events_signin_required.html
+--------------------------------------------------+
       (no sidebar — user has no workspace session yet)
```

## Integration checkpoints (per step)

1. **Rail renders on an authed page** → the page's template context MUST carry `display_name`, `workspace_name`, `is_instance_admin`, `csrf`, and `active_section`. Today `board.html` / `issue.html` / token / invite / report contexts do NOT all provide these — this is the primary integration risk (see registry).
2. **Active state** → `active_section` from the request handler must match the current route family (`home` for `/`, `projects` for board/report/issue).
3. **Sign out** → reuse the existing CSRF-protected `POST /sign-out` form with hidden `_csrf` = `{{ csrf }}`; do not invent a new endpoint.
4. **Instance admin gating** → reuse existing `is_instance_admin` flag; item is absent (not merely hidden) for non-admins.
5. **Scoping guard (Decision #5)** → dashboard "Quick actions" links to `/workspace/invites` and `/admin/tokens` MUST remain; they are intentionally NOT promoted to the rail and must not be deleted.

## Deferred (out of lightweight scope — not requirements)

- Responsive/mobile collapsed drawer for the rail.
- Promoting Invites / Machine tokens into the global nav or user menu later.
- Per-team / multi-workspace switcher inside the workspace identity block.
