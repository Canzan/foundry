# Component Boundaries — navigation-bar-linear-ui

New and changed units, their responsibilities, and the dependency direction. Presentation-tier only;
the store/domain (ports-and-adapters core) is untouched.

## Dependency-inversion note

Presentation depends **inward** on session-provided identity, never the reverse. `NavContext` is
assembled from the already-resolved authenticated `SessionContext` (workspace, display name, admin
flag, CSRF token) that the auth/session layer produces. The sidebar partial depends only on the
`NavContext` value object; it has no knowledge of routing, the store, or how identity was resolved.
The store layer has zero awareness that a sidebar exists. No inward-pointing dependency is added.

## New units

| Unit | Path | Responsibility |
|---|---|---|
| `NavContext` value object + `NavSection` enum + builder | `crates/foundry-app/src/nav.rs` (new module; add `mod nav;` to `lib.rs`) | Single typed carrier of the five shared-artifact values plus the derived `board_href`. One constructor assembles it from the session; helper methods (`is_home`, `is_board`, `monogram`) keep the template dumb. See `data-models.md`. |
| `app_shell.html` | `crates/foundry-app/templates/app_shell.html` (new) | Intermediate layout: `{% extends "base.html" %}`, fills `{% block content %}` with `.app-shell` → `{% include "partials/sidebar.html" %}` + `.app-shell__content` wrapper exposing `{% block app_content %}`. Owns the content offset. |
| `partials/sidebar.html` | `crates/foundry-app/templates/partials/sidebar.html` (new) | Renders the rail from `nav.*`: brand (monogram + workspace_name), primary nav (Home, Projects), footer user menu (identity anchor, Keyboard shortcuts, Sign out form, Instance admin gated). The single documented consumer of every shared-artifacts-registry variable. |
| Sidebar CSS rules | appended to `crates/foundry-app/static/css/foundry.<newhash>.css` | `.app-shell`, `.app-shell__content` (offset), `.sidebar`, `.sidebar__brand`, `.sidebar__monogram`, `.sidebar__nav`, `.sidebar__item`, `.sidebar__item--active`, `.sidebar__user`, focus-visible rings. See ADR-004 for the re-hash procedure. |

## Changed units

| Unit | Path | Change |
|---|---|---|
| `base.html` | `crates/foundry-app/templates/base.html` | `<link>` href updated to the new hashed CSS filename (ADR-004). Structure otherwise unchanged — it remains the parent of pre-auth pages. |
| CSS immutability test | `crates/foundry-app/src/lib.rs:284` | The one hardcoded literal `foundry.4c43c2a8.css` → new hash. (This is the **only** hardcoded-hash site besides `base.html`.) |
| Authed page view structs | `crates/foundry-app/src/views.rs` | Add a `pub nav: NavContext` field to each migrated page struct; the corresponding handler builds it once. |
| Authed page templates | `crates/foundry-app/templates/*.html` | Change `{% extends "base.html" %}` → `{% extends "app_shell.html" %}` and rename `{% block content %}` → `{% block app_content %}`. Body markup unchanged. |

> **Not changed:** `crates/foundry-app/src/projects.rs:958` — its CSS assertion matches only the
> `href="/static/css/foundry.` prefix and the `.css">` suffix (hash-agnostic), so a re-hash does **not**
> break it. The task brief's claim that this line hardcodes the hash is incorrect; verified by reading
> lines 951–959. No edit required there.

## Exact templates to migrate to `app_shell.html`

Authed **full-page** app surfaces (get the shell), with their `active` section:

| Template | View struct (`views.rs`) | `active` |
|---|---|---|
| `dashboard_root.html` | `DashboardRoot` | Home |
| `board.html` | `BoardPage` | Board |
| `issue.html` | `IssuePage` | Board |
| `report.html` | `ReportPage` | Board |
| `token_list.html` | token list struct | Home |
| `token_mint_form.html` | `TokenListPage` | Home |
| `token_minted.html` | `TokenMintedPage` | Home |
| `token_revoke_confirm.html` | token revoke struct | Home |
| `member_invite_form.html` | `MemberInviteForm` | Home |
| `member_invite_sent.html` | `MemberInviteSent` | Home |
| `project_create.html` | `ProjectCreatePage` | Home |
| `instance_dashboard.html` | `InstanceDashboardPage` | Home |

**Active-section rule (deterministic, satisfies "exactly one" — AC-03.3):**
`active = Board` **iff** the route belongs to the board family `/team/{slug}/project/{slug}` and its
descendants (board, report, issue detail). **Every other authed surface** (dashboard, tokens,
invites, project-create, instance admin) resolves to `active = Home` — Home is the app's default/hub
section from which those management surfaces are reached. This guarantees never-zero / never-two with
a two-variant enum, and it is set explicitly by the handler (not path-sniffed in the template), so it
is unit-testable.

### Confirm-before-wrapping (classification edge cases)

| Template | View struct | Recommendation |
|---|---|---|
| `new_issue_modal_page.html` | `NewIssueModalPage` | Full-page **non-htmx fallback** for the board's "New issue" modal (the htmx path swaps a partial into `#modal-root`, not this template). If it renders as a standalone full page, wrap it with the shell, `active = Board`. Confirm it is never returned as an innerHTML fragment before wrapping — a fragment carrying the shell would inject a second sidebar. |

## Templates that MUST stay on `base.html` (chrome-free, structural exclusion)

Pre-auth / utility full pages — **no `nav` field, no shell, output unchanged (NFR-3):**
`signin.html`, `forgot.html`, `forgot_sent.html`, `bootstrap_dashboard.html`, `bootstrap_claim.html`,
`bootstrap_invite.html`, `invite_accept.html`, `invalid_page.html`, `payload_too_large.html`,
`events_signin_required.html`.

Fragments / partials (never extend the shell — they are htmx swap targets, not pages):
`error_fragment.html`, `instance_provisioned.html` (`InstanceProvisionedFragment`),
`instance_grant_confirmed.html` (`InstanceGrantConfirmedFragment`), and everything under
`templates/partials/`.

## Handler structs to add `nav` to

In `crates/foundry-app/src/views.rs`, add `pub nav: NavContext` to: `DashboardRoot`, `BoardPage`,
`IssuePage`, `ReportPage`, `ProjectCreatePage`, `MemberInviteForm`, `MemberInviteSent`,
`TokenListPage`, `TokenMintedPage`, `InstanceDashboardPage`, plus the token-list and
token-revoke-confirm structs (and, if wrapped, `NewIssueModalPage`). Each owning handler constructs
`NavContext::for_page(&session, active, board_target)` once and moves it into the struct. Pre-auth
structs (`SigninPage`, `ForgotPage`, `ForgotSentPage`, `InviteAcceptPage`, `PayloadTooLarge`,
`EventsSigninRequired`, `BootstrapDashboard`, `BootstrapClaim`, `BootstrapInvite`, `InvalidPage`) are
**not** touched.

## Architectural enforcement (Principle 11)

Rust + Askama already enforce most boundaries at **compile time** — the strongest possible guard:

1. **Chrome-scope invariant:** a page extending `app_shell.html` without a `nav` field is a *build
   error* (Askama type-checks template field references against the struct). No page can render the
   sidebar with missing identity. This is the enforceable rule; no extra tooling needed.
2. **Exactly-one-active + admin-gating + CSS-immutability:** guarded by unit tests in `views.rs` /
   `lib.rs` and the DISTILL acceptance suite (`scraper` DOM assertions across the authed page set).
3. **Recommended addition (cheap, language-appropriate):** a small `#[test]` (or extend the existing
   `xtask check-assets` from Feature B) that parses `base.html`'s `<link href>` and asserts the
   referenced `foundry.<hash>.css` exists on disk — this reds CI if a re-hash forgets the rename
   (ADR-004 self-correcting guard).
</content>
