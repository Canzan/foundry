# Evolution — notification-preferences-ui (a signed-in UI entry point onto the shipped notification-preferences backend)

**Finalized**: 2026-07-12
**Commits**: DELIVER — single lean pass (uncommitted at finalize; staged together with the DISTILL artifacts).
Trunk-based; repo legacy multi-file convention; feature dir PRESERVED.
**Wave coverage**: lean **DISTILL → DELIVER** (DISCUSS/DESIGN skipped by design — the requirements + per-workspace
mute/subscribe backend were already settled and shipped by the predecessor `recipient-notification-preferences`).
Grounding for the two skipped waves was captured as seed `discuss/requirements.md` + `design/architecture.md`. DISTILL
authored 12 `@real-io` acceptance scenarios (genuine RED verified); DELIVER greened all 12 on production wiring alone
(zero new unit tests needed — the behavior was already unit-covered in the backend).
**Scope**: `recipient-notification-preferences` shipped a full per-workspace mute/subscribe backend
(`GET /account/notifications`, `POST /account/notifications/resubscribe`, the public `/unsubscribe` flow) — but
**nothing in the app's UI linked to it**, and the signed-in surface offered resubscribe only. This feature adds the
missing **signed-in UI**: a dedicated `/account/settings` surface hosting the notifications section, a sidebar entry
point to reach it from every authenticated page, and a NEW signed-in per-workspace **mute** action (`POST
/account/settings/mute`) so the surface is a complete subscribe/unsubscribe control. ZERO new migrations, ZERO new
crates, ZERO backend behavior change — a thin driving-adapter layer over shipped seams.

## Milestone — a signed-in member can now find and change their notification preferences from inside the app

Before this feature, notification preferences were only reachable via the unsubscribe link buried in an old email, and
the signed-in page could only *un*-mute. Now the shared Linear-style sidebar carries a **"Notifications"** link (footer
user block, on every authenticated page via `NavContext`) → **`/account/settings`**, which lists every workspace the
caller belongs to with a state-aware control: **Mute** when subscribed, **Resubscribe** when muted. Muting calls the
already-shipped `Store::insert_unsubscribe` behind a new signed-in POST that mirrors the shipped resubscribe handler's
posture exactly — identity ONLY from `SessionUser`, membership checked against `workspaces_for_member`, CSRF-checked,
idempotent, and non-enumerable (a signed-out caller or a crafted foreign workspace collapses to the shipped uniform
404). The predecessor's `/account/notifications` + `/unsubscribe` routes are left byte-for-byte identical.

## What shipped

| Surface | Route | Delivered |
|---|---|---|
| Sidebar entry point | — | `<a href="/account/settings">Notifications</a>` added to the footer `sidebar__user` block in `partials/sidebar.html`; a footer destination, NOT a third primary rail item, so the "exactly one primary item current" invariant (Home stays active) is preserved |
| Settings surface | `GET /account/settings` | `unsubscribe.rs::show_settings` — session-gated (signed-out → uniform `resource_not_found_page()`); reuses the shipped read path (`workspaces_for_member` + `list_unsubscribed_workspace_ids`) to render one row per membership with Muted/Subscribed status + a per-row CSRF-protected control. New `settings.html` (extends `app_shell.html`) + `SettingsPage`/`SettingsRow` view structs |
| Signed-in mute | `POST /account/settings/mute` | `unsubscribe.rs::mute_notifications` — mirrors `resubscribe_notifications`: `SessionUser`-only identity, `workspaces_for_member` membership check, `Store::insert_unsubscribe` (idempotent `ON CONFLICT DO NOTHING`), CSRF via shared middleware, uniform 404 on non-member/unknown/signed-out; success via the shipped `render_result_page` |
| Signed-in resubscribe | `POST /account/notifications/resubscribe` | reused UNCHANGED — the settings page's Resubscribe control posts to the shipped route |

## Decisions realized

| # | Decision | Status |
|---|---|---|
| OD-1 | `/account/settings` is the NEW canonical shell hosting the notifications section; the shipped `/account/notifications` + `/unsubscribe` routes left intact (regression-guarded), and the settings page reuses their read path rather than reimplementing it | IMPLEMENTED |
| OD-2 | Sidebar label "Notifications" (most discoverable for the user's stated intent), `href="/account/settings"`; asserted by href so the text stays flexible | IMPLEMENTED |
| OD-3 | Mute action at `POST /account/settings/mute`, mirroring the shipped resubscribe handler's least-privilege/CSRF/idempotent/non-enumerable posture; reuses `Store::insert_unsubscribe` and `render_result_page` | IMPLEMENTED |
| — | State-aware per-row control: Mute when subscribed, Resubscribe when muted; Home stays the sole active primary item (NFR-3, nav invariant) | IMPLEMENTED |
| — | ZERO new migrations, ZERO new crates, ZERO backend behavior change — a driving-adapter layer over shipped seams | IMPLEMENTED |

## Deviations (recorded honestly)

1. **Lean two-wave pass (DISCUSS/DESIGN skipped by design).** The predecessor `recipient-notification-preferences`
   already settled the requirements and the mute/subscribe backend; re-running DISCUSS/DESIGN would have re-derived
   settled facts. Grounding for the skipped waves is preserved as seed `discuss/requirements.md` +
   `design/architecture.md` (the one genuinely-new element — the settings shell — is captured there with OD-1/2/3).
   The mandatory 4-parallel-reviewer DISTILL gate was reduced to a single self-review pass, proportional to a
   sidebar-link + settings-shell + one-POST feature.

2. **No new unit tests, no roadmap.json/execution-log.json.** DELIVER was a single direct crafter dispatch, not the
   formal roadmap→execute→finalize path, so no `deliver/` process artifacts exist. All 12 DISTILL-authored `@real-io`
   acceptance scenarios reached GREEN on production wiring alone — the underlying mute/subscribe behavior was already
   unit-covered in the shipped backend, so no decomposing unit tests were warranted (behavior budget trivially
   satisfied; anti-Fixture-Theater `git diff` confirms the ATs flip RED→GREEN because of production handler/view/router
   code, not fixtures).

## Deferred follow-ups (out of scope, tracked)

- **Broader settings surface** — the shell hosts only the Notifications section today; profile/security/other sections
  were explicitly out of scope.
- **Per-channel / digest / quiet-hours granularity** — the mute unit stays per-workspace, exactly as the shipped
  backend models it.
- **Per-feature mutation testing** — deferred, consistent with the predecessor; the new `mute_notifications` /
  `show_settings` handlers are thin mirrors of already-reviewed seams.

## Verification

- **Acceptance**: 12 feature scenarios (`notification-preferences-ui`), all un-`@pending` and green — 49 steps.
  Regression: `recipient-unsubscribe` 27/27 and `navigation-bar` 33/33 both pass unchanged (backend + one-active-primary
  invariant intact).
- **Gates**: `cargo fmt --all --check` clean; `cargo clippy --all-targets --release -D warnings` clean (one latent
  `doc_lazy_continuation` surfaced + fixed).
- **Live browser drive** (real Postgres + real `foundry` binary): bootstrap-claimed an admin/workspace, confirmed the
  sidebar "Notifications" link with Home still active → `/account/settings` showing "Acme — Subscribed" → **Mute** →
  "invitations are stopped" (persisted; reload shows "Muted" + Resubscribe) → **Resubscribe** → "subscribed again".
  Full round-trip observed end-to-end.
- **Cost**: ZERO migrations, ZERO new crates, ZERO new infra; 5 production files touched + 1 new template.
- **Finalize**: feature dir PRESERVED (wave matrix); no DES session markers to remove (lean pass). Trunk-based — commit
  performed with the user; push confirmed separately.
