// The live-replica gate window against the beat cadence a running Foundry
// server keeps (US-IIS-07 AC 2, D5). One unit of modelled time is a second;
// the model carries the ratio of the beat interval to the reload interval,
// never milliseconds. The reading server's clock and the other server's
// reload ticks are separate, so the gate is read between two ticks as well.
module live_replica_gate {
  /// `BEAT_EVERY` of crates/foundry-app/src/integrations/reload.rs: how often
  /// a server beats its integration_replica_states row.
  pure val BEAT_EVERY: int = 10

  /// `REPLICA_STATE_RETENTION` of crates/foundry-app/src/integrations/worker.rs:
  /// the hour the prune keeps a replica state row for.
  pure val RETENTION: int = 3600

  /// The reload settings the model crosses, as multiples of a second: one well
  /// below the beat interval (standing for every setting at or below it, the
  /// 200 ms of the acceptance lane included), the two around the default, the
  /// beat interval itself, and one above it.
  pure val RELOADS: Set[int] = Set(1, 3, 5, 10, 17)

  /// The reload interval both servers poll and beat on.
  var reload: int
  /// The clock the gate is read against.
  var now: int
  /// When the other server, which still opens the stored secrets key, last
  /// beat its row.
  var lastBeat: int
  /// When that server's next reload tick falls.
  var nextTick: int

  /// The gate window of D5: two beat intervals plus one reload interval.
  pure def window(r: int): int = 2 * BEAT_EVERY + r

  /// What the gate answers about that server at `now`.
  def gateReadsLive: bool = now - lastBeat <= window(reload)

  /// The beat is due at the first tick at least one beat interval after the
  /// last beat (reload.rs: `now - at >= BEAT_EVERY`).
  def beatDue(at: int): bool = at - lastBeat >= BEAT_EVERY

  /// A due beat may be missed - a failed row write, a slow tick, a stalled
  /// scheduler - while it stays at most one beat interval late.
  def mayStayLate(at: int): bool = at - lastBeat - BEAT_EVERY <= BEAT_EVERY

  action init = nondet r = oneOf(RELOADS) all {
    reload' = r,
    now' = 0,
    lastBeat' = 0,
    nextTick' = r,
  }

  /// One second of the clock. The other server's tick falls on it or does not.
  action step = all {
    now' = now + 1,
    if (now + 1 == nextTick) all {
      nextTick' = now + 1 + reload,
      any {
        all { beatDue(now + 1), lastBeat' = now + 1 },
        all { beatDue(now + 1), mayStayLate(now + 1), lastBeat' = lastBeat },
        all { not(beatDue(now + 1)), lastBeat' = lastBeat },
      },
    } else all {
      nextTick' = nextTick,
      lastBeat' = lastBeat,
    },
    reload' = reload,
  }

  /// The gate never reads a running server that still opens the stored key as
  /// stale, at any reload setting and at any moment between its beats.
  val gateAlwaysLive = gateReadsLive

  /// The window never outlives the row the prune keeps.
  val windowInsideRetention = window(reload) <= RETENTION

  /// Negated on purpose: a counterexample is the witness that the longest gap
  /// the model admits comes within one second of the window, so the window is
  /// tight and the invariant is not vacuous.
  val neverNearTheWindow = now - lastBeat < window(reload) - 1
}
