// iis-slack-replace-remove: the pending Slack app's provenance.
//
// Claim: the saved Slack app only ever becomes an app an instance admin
// submitted and, where that submission named a different client ID while
// workspaces were connected, that the SAME instance admin confirmed before
// its expiry. Every instance write serialises on one advisory exclusive lock
// (ADR-IIS-004 r1), which this model represents as one atomic action per
// write; the claim is quantified over the ORDERINGS of those writes.
//
// Also claimed: a secrets-key Move re-seals the pending row with every other
// sealed value (ADR-IIS-003 r4), so a held pending row is always sealed under
// the current sealing key.
module m1_confirm_ignores_requester {

  pure val NoApp = ""
  pure val NoAdmin = ""
  pure val Apps = Set("A1", "A2")
  pure val Admins = Set("priya", "tomas")
  pure val Ttl = 2
  pure val Horizon = 4
  pure val MaxConnected = 2

  // instance_slack_app.client_id, "" when the row is absent.
  var saved: str
  // The environment's app (SLACK_CLIENT_ID and friends), "" when its trio is unset.
  var envApp: str
  // instance_slack_app_pending, "" when the row is absent.
  var pending: str
  var pendingBy: str
  var pendingExpires: int
  var pendingSealedUnder: int
  // The stored secrets key's id, which the seal fence compares against.
  var sealingKey: int
  // The injected clock.
  var now: int
  // Workspaces holding a live Slack connection.
  var connected: int

  // Ghosts: what the write that produced the current `saved` row actually read.
  var savedBy: str
  var savedVia: str
  var confirmReadBy: str
  var confirmReadExpires: int
  var confirmReadAt: int
  var directRotate: bool
  var directConnected: int

  // The app in effect: the environment's when its trio is set, else the saved app.
  val current = if (envApp != NoApp) envApp else saved

  action init = {
    nondet e = oneOf(Apps.union(Set(NoApp)))
    nondet c = oneOf(0.to(MaxConnected))
    all {
      envApp' = e,
      connected' = c,
      saved' = NoApp,
      pending' = NoApp,
      pendingBy' = NoAdmin,
      pendingExpires' = 0,
      pendingSealedUnder' = 0,
      sealingKey' = 0,
      now' = 0,
      savedBy' = NoAdmin,
      savedVia' = "none",
      confirmReadBy' = NoAdmin,
      confirmReadExpires' = 0,
      confirmReadAt' = 0,
      directRotate' = false,
      directConnected' = 0,
    }
  }

  // POST /admin/instance/integrations/slack by admin `a` naming app `app`.
  // A Rotate (the app in effect) and a different app with nothing connected
  // save at once; a different app with workspaces connected is held pending.
  action submit = {
    nondet a = oneOf(Admins)
    nondet app = oneOf(Apps)
    any {
      // Rotate, or a new app with no connected workspace: saved at once.
      all {
        or { app == current, connected == 0 },
        saved' = app,
        savedBy' = a,
        savedVia' = "direct",
        directRotate' = (app == current),
        directConnected' = connected,
        pending' = pending,
        pendingBy' = pendingBy,
        pendingExpires' = pendingExpires,
        pendingSealedUnder' = pendingSealedUnder,
        confirmReadBy' = confirmReadBy,
        confirmReadExpires' = confirmReadExpires,
        confirmReadAt' = confirmReadAt,
        envApp' = envApp, connected' = connected, sealingKey' = sealingKey, now' = now,
      },
      // A different app with workspaces connected: held sealed, nothing saved.
      all {
        app != current,
        connected > 0,
        pending' = app,
        pendingBy' = a,
        pendingExpires' = now + Ttl,
        pendingSealedUnder' = sealingKey,
        saved' = saved, savedBy' = savedBy, savedVia' = savedVia,
        directRotate' = directRotate, directConnected' = directConnected,
        confirmReadBy' = confirmReadBy,
        confirmReadExpires' = confirmReadExpires,
        confirmReadAt' = confirmReadAt,
        envApp' = envApp, connected' = connected, sealingKey' = sealingKey, now' = now,
      },
    }
  }

  // POST .../slack/confirm by admin `a`: applies only for the requester that
  // holds the row and only before its expiry, and deletes it.
  action confirm = {
    nondet a = oneOf(Admins)
    any {
      all {
        pending != NoApp,
        now < pendingExpires,
        pendingSealedUnder == sealingKey,
        saved' = pending,
        savedBy' = a,
        savedVia' = "confirm",
        confirmReadBy' = pendingBy,
        confirmReadExpires' = pendingExpires,
        confirmReadAt' = now,
        pending' = NoApp,
        pendingBy' = NoAdmin,
        pendingExpires' = 0,
        pendingSealedUnder' = 0,
        directRotate' = directRotate, directConnected' = directConnected,
        envApp' = envApp, connected' = connected, sealingKey' = sealingKey, now' = now,
      },
      // Another admin, an expired row, no row: nothing changes.
      all {
        or { pending == NoApp, a != pendingBy, now >= pendingExpires },
        saved' = saved, savedBy' = savedBy, savedVia' = savedVia,
        confirmReadBy' = confirmReadBy, confirmReadExpires' = confirmReadExpires,
        confirmReadAt' = confirmReadAt,
        pending' = pending, pendingBy' = pendingBy,
        pendingExpires' = pendingExpires, pendingSealedUnder' = pendingSealedUnder,
        directRotate' = directRotate, directConnected' = directConnected,
        envApp' = envApp, connected' = connected, sealingKey' = sealingKey, now' = now,
      },
    }
  }

  // POST .../slack/remove: withheld while the environment's app is in effect.
  action remove = all {
    envApp == NoApp,
    saved' = NoApp,
    savedBy' = NoAdmin,
    savedVia' = "none",
    directRotate' = false,
    directConnected' = 0,
    confirmReadBy' = NoAdmin, confirmReadExpires' = 0, confirmReadAt' = 0,
    pending' = pending, pendingBy' = pendingBy,
    pendingExpires' = pendingExpires, pendingSealedUnder' = pendingSealedUnder,
    envApp' = envApp, connected' = connected, sealingKey' = sealingKey, now' = now,
  }

  action tick = all {
    now < Horizon,
    now' = now + 1,
    saved' = saved, savedBy' = savedBy, savedVia' = savedVia,
    directRotate' = directRotate, directConnected' = directConnected,
    confirmReadBy' = confirmReadBy, confirmReadExpires' = confirmReadExpires,
    confirmReadAt' = confirmReadAt,
    pending' = pending, pendingBy' = pendingBy,
    pendingExpires' = pendingExpires, pendingSealedUnder' = pendingSealedUnder,
    envApp' = envApp, connected' = connected, sealingKey' = sealingKey,
  }

  // The worker's retention sweep deletes an expired pending row.
  action sweep = all {
    pending != NoApp,
    now >= pendingExpires,
    pending' = NoApp,
    pendingBy' = NoAdmin,
    pendingExpires' = 0,
    pendingSealedUnder' = 0,
    saved' = saved, savedBy' = savedBy, savedVia' = savedVia,
    directRotate' = directRotate, directConnected' = directConnected,
    confirmReadBy' = confirmReadBy, confirmReadExpires' = confirmReadExpires,
    confirmReadAt' = confirmReadAt,
    envApp' = envApp, connected' = connected, sealingKey' = sealingKey, now' = now,
  }

  // Generate or Move stores a new secrets key and re-seals every sealed
  // value, the pending row included (ADR-IIS-003 r4).
  action move = all {
    sealingKey' = sealingKey + 1,
    pendingSealedUnder' = if (pending == NoApp) 0 else sealingKey + 1,
    pending' = pending, pendingBy' = pendingBy, pendingExpires' = pendingExpires,
    saved' = saved, savedBy' = savedBy, savedVia' = savedVia,
    directRotate' = directRotate, directConnected' = directConnected,
    confirmReadBy' = confirmReadBy, confirmReadExpires' = confirmReadExpires,
    confirmReadAt' = confirmReadAt,
    envApp' = envApp, connected' = connected, now' = now,
  }

  // A workspace admin connects or disconnects Slack.
  action reconnect = {
    nondet c = oneOf(0.to(MaxConnected))
    all {
      connected' = c,
      saved' = saved, savedBy' = savedBy, savedVia' = savedVia,
      directRotate' = directRotate, directConnected' = directConnected,
      confirmReadBy' = confirmReadBy, confirmReadExpires' = confirmReadExpires,
      confirmReadAt' = confirmReadAt,
      pending' = pending, pendingBy' = pendingBy,
      pendingExpires' = pendingExpires, pendingSealedUnder' = pendingSealedUnder,
      envApp' = envApp, sealingKey' = sealingKey, now' = now,
    }
  }

  action step = any { submit, confirm, remove, tick, sweep, move, reconnect }

  // The provenance claim.
  val provenanceInv =
    (saved == NoApp) or
    (savedVia == "direct" and (directRotate or directConnected == 0)) or
    (savedVia == "confirm" and confirmReadBy == savedBy and confirmReadAt < confirmReadExpires)

  // The re-seal claim: a held pending row is sealed under the sealing key.
  val pendingSealedInv = (pending == NoApp) or (pendingSealedUnder == sealingKey)

  // Reachability witnesses, each EXPECTED to be violated by `quint run`:
  // a counterexample is the construction trace.
  val noSaveReachable = saved == NoApp
  val noConfirmRouteReachable = savedVia != "confirm"
}
