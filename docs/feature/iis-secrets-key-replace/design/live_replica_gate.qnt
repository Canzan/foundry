// The live-replica gate window against the beat cadence a running Foundry
// server keeps (US-IIS-07 AC 2, D5). One unit of modelled time is a second.
// The reading server's clock and the other server's reload ticks are separate
// variables, so the gate is also read between two beats, and the two servers
// carry different FOUNDRY_INTEGRATIONS_RELOAD_MS settings: the window is a
// constant, so the reading server's own setting cannot enter the judgement.
//
// Checked in the shared Lima VM local-dev, the only environment this project
// runs tools in, through scripts/test-in-lima.sh; the receipt beside this file
// records each command, its exit status and the sha256 of these bytes.
module live_replica_gate {
  /// `BEAT_EVERY` of crates/foundry-app/src/integrations/reload.rs: how often
  /// a server beats its integration_replica_states row.
  pure val BEAT_EVERY: int = 10

  /// The reload envelope this value is designed for, the same 20 s K4 names
  /// for the 60-second figure. The window is sized from it, never from the
  /// reading server's own setting.
  pure val RELOAD_ENVELOPE: int = 20

  /// `REPLICA_STATE_RETENTION` of crates/foundry-app/src/integrations/worker.rs:
  /// the hour the prune keeps a replica state row for.
  pure val RETENTION: int = 3600

  /// The gate window of D5: two beat intervals plus the reload envelope.
  pure val WINDOW: int = 2 * BEAT_EVERY + RELOAD_ENVELOPE

  /// Pairs of settings the model crosses: the writer is the other server,
  /// which still opens the stored secrets key and beats on its own setting;
  /// the reader is the server the gate is read on. They differ on purpose,
  /// in both directions, including the two ends of the envelope.
  pure val SETTINGS: Set[{ writer: int, reader: int }] = Set(
    { writer: 1, reader: 1 },
    { writer: 20, reader: 1 },
    { writer: 17, reader: 1 },
    { writer: 5, reader: 1 },
    { writer: 20, reader: 5 },
    { writer: 10, reader: 10 },
    { writer: 1, reader: 20 },
    { writer: 20, reader: 20 }
  )

  /// What the other server polls and beats on.
  var writerReload: int
  /// What the reading server polls on. No definition below reads it: the gate
  /// cannot depend on it. Mutant m9 is exactly the version that does.
  var readerReload: int
  /// The clock the gate is read against.
  var now: int
  /// When the other server last beat its row.
  var lastBeat: int
  /// When that server's next reload tick falls.
  var nextTick: int

  /// What the gate answers about that server at `now`.
  def gateReadsLive: bool = now - lastBeat <= WINDOW

  /// The beat is due at the first tick at least one beat interval after the
  /// last beat (reload.rs: `now - at >= BEAT_EVERY`).
  def beatDue(at: int): bool = at - lastBeat >= BEAT_EVERY

  /// A due beat may be missed - a failed row write, a slow tick, a stalled
  /// scheduler - while it stays at most one beat interval late.
  def mayStayLate(at: int): bool = at - lastBeat - BEAT_EVERY <= BEAT_EVERY

  action init = nondet s = oneOf(SETTINGS) all {
    writerReload' = s.writer,
    readerReload' = s.reader,
    now' = 0,
    lastBeat' = 0,
    nextTick' = s.writer,
  }

  /// One second of the clock. The other server's tick falls on it or does not.
  action step = all {
    now' = now + 1,
    if (now + 1 == nextTick) all {
      nextTick' = now + 1 + writerReload,
      any {
        all { beatDue(now + 1), lastBeat' = now + 1 },
        all { beatDue(now + 1), mayStayLate(now + 1), lastBeat' = lastBeat },
        all { not(beatDue(now + 1)), lastBeat' = lastBeat },
      },
    } else all {
      nextTick' = nextTick,
      lastBeat' = lastBeat,
    },
    writerReload' = writerReload,
    readerReload' = readerReload,
  }

  /// The gate never reads a server that still opens the stored secrets key as
  /// stale, at any pair of settings inside the envelope and at any moment
  /// between its beats.
  val gateAlwaysLive = gateReadsLive

  /// The window never outlives the row the prune keeps.
  val windowInsideRetention = WINDOW <= RETENTION

  /// Negated on purpose: a counterexample is the witness that the longest gap
  /// the model admits comes within one second of the window, so the window is
  /// tight and gateAlwaysLive is not vacuous.
  val neverNearTheWindow = now - lastBeat < WINDOW - 1
}
