# Shared Artifacts Registry: navigation-bar-linear-ui

Every `${variable}` the shared sidebar depends on, its single source of truth, and its consumers. The sidebar lives in the shared layout (`crates/foundry-app/templates/base.html`), so these values must be present in the template context of **every authenticated page** that extends it.

## Registry

```yaml
shared_artifacts:
  display_name:
    source_of_truth: "auth/session context (server) -> injected into each authed page's template context"
    consumers:
      - "sidebar footer account anchor ({{ display_name }})"
      - "dashboard_root.html welcome line (existing: 'Welcome back, {{ display_name }}')"
    owner: "foundry-app web tier (session/auth layer)"
    integration_risk: "HIGH - board.html, issue.html, report.html, token_*.html, member_invite_*.html do not all currently pass display_name; adding the rail to base.html requires it everywhere."
    validation: "Render each authed page; footer name equals the session user's display name."

  workspace_name:
    source_of_truth: "auth/session context (server) -> injected into each authed page's template context"
    consumers:
      - "sidebar workspace identity block (top of rail)"
      - "sidebar footer account anchor"
      - "dashboard_root.html workspace line (existing: 'Workspace: {{ workspace_name }}')"
    owner: "foundry-app web tier (session/auth layer)"
    integration_risk: "HIGH - same as display_name; must be threaded into every authed page context."
    validation: "Identity block and footer show the same workspace_name as the active session."

  is_instance_admin:
    source_of_truth: "auth/session context (server) -> injected into each authed page's template context"
    consumers:
      - "sidebar user menu 'Instance admin' item ({% if is_instance_admin %})"
      - "dashboard_root.html Quick actions 'Instance admin' item (existing gate)"
    owner: "foundry-app web tier (authorization layer)"
    integration_risk: "MEDIUM - flag already exists on the dashboard; reuse verbatim. Item must be ABSENT (not merely hidden) for non-admins to avoid 403-on-click."
    validation: "Admin session shows the item; non-admin session omits it from rendered HTML."

  csrf:
    source_of_truth: "auth/session context (server) -> per-request CSRF token in template context"
    consumers:
      - "sidebar footer sign-out form (hidden input _csrf = {{ csrf }})"
      - "dashboard_root.html sign-out form (existing: POST /sign-out, hidden _csrf = {{ csrf }})"
    owner: "foundry-app web tier (CSRF middleware)"
    integration_risk: "HIGH - a sign-out form without a valid _csrf token is rejected. The rail's sign-out MUST reuse the existing POST /sign-out endpoint and token field name (_csrf)."
    validation: "Submitting the rail sign-out form ends the session (same behavior as the existing dashboard form)."

  active_section:
    source_of_truth: "request handler / route (server) sets the current section: 'home' | 'projects'"
    consumers:
      - "sidebar nav item highlighting (accent-tinted class + aria-current=\"page\")"
    owner: "foundry-app web tier (routing layer)"
    integration_risk: "MEDIUM - new value introduced by this feature; every authed route must declare its section, or the layout must derive it from the request path. Exactly one item may be active."
    validation: "Home active for '/'; Projects active for board/report/issue routes; never two at once; never zero on an authed page."
```

## Route / destination artifacts (stable URLs the rail links to)

| Link | URL | Source | Notes |
|------|-----|--------|-------|
| Home / Dashboard | `/` | existing dashboard route | Primary nav item. |
| Projects / Board | *(see open question)* | — | No dedicated projects-index route exists today; boards live at `/team/{slug}/project/{slug}`. DESIGN must decide the "Projects" target (new index route, or deep-link). |
| Keyboard shortcuts | `/keyboard-help` | existing route | User menu. |
| Instance admin | `/admin/instance/workspaces` | existing route (admin-gated) | User menu, `is_instance_admin` only. |
| Sign out | `POST /sign-out` (hidden `_csrf`) | existing endpoint | User menu; reuse existing form verbatim. |
| Invite a member (NOT promoted) | `/workspace/invites` | existing route | Stays in dashboard Quick actions (Decision #5). |
| Machine tokens (NOT promoted) | `/admin/tokens` | existing route | Stays in dashboard Quick actions (Decision #5). |

## Consistency checks (horizontal integration)

- If the CSS/rail is added to `base.html`, does every authed page still receive `display_name`, `workspace_name`, `is_instance_admin`, `csrf`? — **Open risk; the 21 templates that extend base.html do not all pass these today.**
- Does the rail's sign-out reference the same endpoint and token field (`/sign-out`, `_csrf`) as the existing dashboard form? — **Must, to avoid divergence.**
- Do two surfaces render the same identity from different sources? — No: `display_name`/`workspace_name` have a single session source; the dashboard's existing lines and the rail read the same values.
- Are Invites/Tokens links duplicated or deleted? — Neither promoted to rail nor deleted from dashboard (Decision #5).
