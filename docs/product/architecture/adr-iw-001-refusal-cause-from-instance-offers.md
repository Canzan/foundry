# ADR-IW-001: The route form's "no connection" cause is derived from the instance offers the workspace page already uses

Status: Proposed (integrations-wayfinding DESIGN, 2026-11-03)

## Context

The route form shows K-4 whenever it has no usable bot of its kind
(`RouteDoor::no_connection`, `route_form.rs`). It ignores the reason. The
workspace page already tells the reasons apart through
`Integrations::slack_offer` and `Integrations::matrix_offer` (`mod.rs`), which
return `NotSetUp | NoSecretsKey | Unavailable | Ready` for Slack and
`NoSecretsKey | Unavailable | Ready` for Matrix. DISCUSS (US-IW-02) requires
the form to name the same cause as the workspace page, per kind and per role.

## Decision

Add one pure function in `foundry-app/src/integrations` that maps the kind's
offer and whether a connection exists to a closed cause:

```
NoConnectionCause = NotSetUp(kind) | NoSecretsKey(kind) | Unreadable(kind) | NotConnected(kind)
cause(kind, offer, has_connection) -> Option<NoConnectionCause>   // None = usable
```

The precedence is each offer's own order:
- Slack: `NotSetUp`, then `Unavailable` (K-19), then `NoSecretsKey`, then
  `NotConnected`.
- Matrix: `Unavailable` (K-19), then `NoSecretsKey`, then `NotConnected`. A second pure function maps
`(cause, viewer_is_workspace_admin)` to the words and an optional link, using
copy W-2 to W-5. Matrix `NoSecretsKey` uses W-2, which is also the workspace
page's `matrix_not_set_up` wording.

"Has a connection" means `usable_connection` returned a row and this
Foundry can build the kind's bot from it (`bot.is_some()`); any connection
that is not disconnected counts, lost included. With that definition the
unreadable case is judged before "not connected", so after the save guard the
no-bot `cannot_post` arm and `matrix_setup.rs:162` are unreachable.

`RouteDoor` holds the cause instead of the `no_connection` bool. Every door
that renders the route form applies the route's kind first: open, preview,
save, Send test and edit (the last two added at the final review gate).

- **Save** checks `door.cause()` right after `of_kind` and re-renders the form
  when it is `Some`. Today Slack falls through to `cannot_post` and Matrix
  answers K-4 at `matrix_setup.rs:162`; both are replaced.
- **Preview** applies `of_kind`, which it lacks today: a Matrix preview renders
  the Slack form.

Form, preview and save then show the same words (US-IW-02 AC 7).

## Alternatives considered

- **Compute the cause in the template from separate booleans** (the
  workspace page's style). Rejected: it is a second precedence, written in
  Askama, that can drift from the workspace page's, and it cannot be unit
  tested as one table.
- **Move the cause into `foundry-services`.** Rejected: whether the instance
  has a Slack app and a secrets key is configuration held by `foundry-app`'s
  `Integrations`. Services sees only stored connections.

## Consequences

- One 5-cell × 2-role table is unit testable without a database (KPI-IW-1,
  KPI-IW-2).
- The workspace page and the route form agree, because both read the same
  `slack_offer` and `matrix_offer`.
- K-4 stays word for word for a lead who is not an admin. It is now reached
  only through the cause function. `matrix_setup.rs:162`'s direct use is
  removed.
- With no Slack app, a saved but unreadable Slack connection shows W-2, not
  K-19, matching the workspace page (US-IW-02 AC 6, refined in
  upstream-changes.md).
