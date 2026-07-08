# Story Map: navigation-bar-linear-ui

## User: Devon Park — workspace member on the "Acme" workspace
## Goal: Navigate the Foundry app — always know the current location and reach any primary surface in one click, from any authenticated page.

## Backbone

The single user activity is **"Navigate the app"**, decomposed into the sub-activities a member performs while moving through authed pages.

| Orient (know where I am) | Move (reach a surface) | Act on my account | Stay unobstructed (pre-auth) |
|--------------------------|------------------------|-------------------|------------------------------|
| See a persistent rail on every authed page | Click Home → dashboard | Open user menu (name + workspace) | Sign-in / bootstrap pages show no rail |
| See workspace identity (brand + Acme) | Click Projects → board | Sign out (CSRF POST) | Focused single-task view preserved |
| See current section highlighted | Active state follows the route | Open Keyboard shortcuts | |
| Read my signed-in name in the footer | | Instance admin (admins only) | |
| Accessible landmark + aria-current | | Preserve dashboard Quick actions (invites/tokens) | |

---

### Walking Skeleton (thinnest end-to-end slice, touches all activities)

Sidebar added to `base.html`, rendered on **dashboard + board** only, with:
- Workspace identity block (brand + `workspace_name`).
- Two primary nav items: **Home** (`/`) and **Projects** (board), with **active-state highlighting** driven by `active_section`.
- Content offset to the right of the rail.
- Rail **absent** on pre-auth pages (signin/forgot excluded).

This connects Orient → Move → (chrome-free pre-auth) end to end. It deliberately excludes the user menu, admin gating, and remaining pages — those are release slices. Maps to story **US-01**.

> Note: the skeleton must NOT regress pre-auth pages, so "rail absent on excluded pages" is part of the skeleton even though it is an absence, not a feature.

### Release 1 — Outcome: "Account actions live in one predictable place"
- User/account menu in the pinned footer: shows `display_name` + `workspace_name`.
- **Sign out** via the existing CSRF `POST /sign-out` form.
- **Keyboard shortcuts** link (`/keyboard-help`).
- Stories: **US-02** (user menu + sign-out + shortcuts).
- KPI targeted: account actions reachable in ≤1 click from any authed page (see outcome-kpis KPI-2).

### Release 2 — Outcome: "The right people (and only them) see admin entry"
- **Instance admin** item in the user menu, gated by `is_instance_admin` (absent for non-admins).
- Preserve dashboard Quick actions links to Invites/Tokens (Decision #5 guard).
- Stories: **US-03** (instance-admin gating), **US-05** (scoping guard / regression protection).
- KPI targeted: correct admin-visibility (100% of non-admin renders omit the item).

### Release 3 — Outcome: "Every authenticated page shares the same nav"
- Extend rail presence + context plumbing (`display_name`, `workspace_name`, `is_instance_admin`, `csrf`, `active_section`) to the remaining authed pages: issue detail, report, token pages, invite pages.
- Stories: **US-04** (extend to remaining authed pages).
- KPI targeted: consistent nav present on 100% of authed pages; 0% of pre-auth pages.

### Release 4 — Outcome: "It reads as Linear-quality"
- Visual polish: quiet neutral rail surface, subtle right border, rounded hover, accent-tinted active state, tight-but-generous spacing, focus states.
- Accessibility finish: `<nav>` landmark, `aria-current="page"`, keyboard focus order.
- Stories: **US-06** (Linear visual + accessibility polish).
- KPI targeted: active-state correctness + accessibility checks pass.

## Priority Rationale

Priority follows outcome impact and dependency order, not feature grouping:

1. **Walking skeleton first (US-01)** — validates the riskiest assumption: that the shared layout can carry a rail on authed pages while leaving pre-auth pages chrome-free, and that `active_section` can be threaded per route. If this end-to-end plumbing does not work, nothing else matters.
2. **User menu (US-02)** next — highest everyday value after orientation; sign-out is currently only on the dashboard, so putting it in the always-present rail removes a real friction. Depends on US-01 (footer anchor exists).
3. **Admin gating + scoping guard (US-03, US-05)** — lower reach (admins only) but must land before broad rollout so non-admins never see a 403 trap, and so the Invites/Tokens links are provably preserved. Depends on US-02 (menu exists).
4. **Extend to remaining pages (US-04)** — mechanical but touches the most templates and the shared-context risk; sequenced after behavior is proven on dashboard+board so the pattern is settled before fan-out.
5. **Visual/accessibility polish (US-06)** — last because it is refinement over working behavior; doing it earlier risks polishing a structure that still changes.
