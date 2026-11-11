// iis-secrets-key-replace: Replace and Start over of the stored secrets key.
//
// The six obligations D7 hands DESIGN, each as one named invariant:
//   Q1 allOrNothing   - the re-seal is all-or-nothing.
//   Q2 noLostNoEmail  - no lost marking and no email in the window where
//                       replicas hold different keys.
//   Q3 gateHeld       - the live-replica gate is checked again inside the POST.
//   Q4 writesOnlyFromPosts, noticeHonest - reload, prefetch and double submit.
//   Q5 noDeadEnd      - no state offers nothing.
//   Q6 offersExclusive - replace-then-start-over ordering.
//
// The key generations are numbers: a generation is "the key the Nth write
// made". `instKey` is the instance key each replica holds; `wrappedBy` is the
// instance key generation the stored secrets key row is wrapped under. A
// replica opens the stored key exactly while its instance key is that one,
// which is how I-26 arises on this server (instKey[acting] /= wrappedBy).
// MUTANT: the replace stores the new key and its check without re-sealing the saved values in the same transaction (Q1)
module m1_nonatomic_reseal {
  type Replica = str
  type Slot = str

  // This server and one other running replica.
  pure val Acting: Replica = "a"
  pure val Replicas: Set[Replica] = Set("a", "b")
  // Two connection credentials and the saved Slack app: the slots
  // reseal_sealed_values walks.
  pure val Slots: Set[Slot] = Set("c1", "c2", "app")

  // Every saved slot sealed again under the generation `g`; an empty slot
  // stays empty. Pure, so the re-seal is one value, not a sequence of writes.
  pure def resealAll(m: Slot -> int, g: int): Slot -> int =
    Slots.mapBy(s => if (m.get(s) == 0) 0 else g)

  // The slots sealed under `openable` sealed again under `g`; every other
  // saved slot discarded.
  pure def discardUnopenable(m: Slot -> int, g: int, openable: int): Slot -> int =
    Slots.mapBy(s =>
      if (m.get(s) == 0) 0
      else if (m.get(s) == openable) g
      else 0)

  // ---- durable state (the rows under the instance-integrations lock)
  // The stored secrets key generation; 0 = no key stored.
  var storedKey: int
  // The instance key generation the stored secrets key is wrapped under.
  var wrappedBy: int
  // What each slot's sealed value is sealed under; 0 = nothing saved there.
  var sealedUnder: Slot -> int
  // The stored key check value's generation (0 = none).
  var keyCheck: int
  var version: int
  var replacedLines: int
  var startedOverLines: int

  // ---- per-replica state
  // The instance key each replica holds.
  var instKey: Replica -> int
  // The applied config version of each replica.
  var applied: Replica -> int
  // The secrets key generation each replica holds in memory; 0 = none.
  var held: Replica -> int
  // Whether this replica's heartbeat row is fresh.
  var beating: Replica -> bool

  // ---- the acting admin's session (D16: post, redirect, get, read once)
  var notice: str

  // ---- ghosts: counters and the forbidden outcomes
  var posts: int
  var writes: int
  var commits: int
  var readNotices: int
  var lostMarked: bool
  var emailed: bool
  var badStartOver: bool

  // ---- observations
  val savedSlots: Set[Slot] = Slots.filter(s => sealedUnder.get(s) != 0)
  // I-22's counting rule: each connection one, the saved Slack app one.
  val savedCount: int = savedSlots.size()
  // Whether this server opens the stored secrets key (KeyMode::Sealed).
  def opensStored(r: Replica): bool =
    storedKey != 0 and instKey.get(r) == wrappedBy
  // I-4: some saved value is sealed under something other than the stored key.
  val degraded: bool = savedSlots.exists(s => sealedUnder.get(s) != storedKey)
  // The live-replica gate: some replica whose fresh heartbeat says it opens
  // the stored secrets key (integration_replica_states.opens_secrets_key).
  val liveOpener: bool =
    Replicas.exists(r => beating.get(r) and held.get(r) == storedKey and storedKey != 0)
  // I-26 on this server.
  val unopenable: bool = storedKey != 0 and not(opensStored(Acting))
  // The page's offers.
  val replaceOffered: bool = opensStored(Acting) and not(degraded)
  val startOverOffered: bool = unopenable and not(liveOpener)
  // The credentials this Foundry cannot open: under I-26, every saved one.
  val cannotOpen: int =
    savedSlots.filter(s => sealedUnder.get(s) != held.get(Acting)).size()

  action init = all {
    storedKey' = 1,
    wrappedBy' = 1,
    sealedUnder' = Slots.mapBy(s => 1),
    keyCheck' = 1,
    version' = 1,
    replacedLines' = 0,
    startedOverLines' = 0,
    instKey' = Replicas.mapBy(r => 1),
    applied' = Replicas.mapBy(r => 1),
    held' = Replicas.mapBy(r => 1),
    beating' = Replicas.mapBy(r => true),
    notice' = "",
    posts' = 0,
    writes' = 0,
    commits' = 0,
    readNotices' = 0,
    lostMarked' = false,
    emailed' = false,
    badStartOver' = false,
  }

  // Everything a write does not touch.
  action replicasUnchanged = all {
    instKey' = instKey, applied' = applied, held' = held, beating' = beating,
  }
  action durableUnchanged = all {
    storedKey' = storedKey, wrappedBy' = wrappedBy, sealedUnder' = sealedUnder,
    keyCheck' = keyCheck, version' = version,
    replacedLines' = replacedLines, startedOverLines' = startedOverLines,
    writes' = writes, commits' = commits,
  }
  action ghostsUnchanged = all {
    lostMarked' = lostMarked, emailed' = emailed, badStartOver' = badStartOver,
  }

  // GET /admin/instance/integrations/secrets-key/replace (I-23), and the
  // same page prefetched: a ReadOnlyDoor, so it changes nothing at all.
  action getReplace = all {
    replaceOffered,
    durableUnchanged, replicasUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // GET /admin/instance/integrations/secrets-key/start-over (I-28, I-28a).
  action getStartOver = all {
    startOverOffered,
    durableUnchanged, replicasUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // GET /admin/instance/integrations: the notice waits once in the session.
  action getPage = all {
    durableUnchanged, replicasUnchanged, ghostsUnchanged,
    posts' = posts,
    notice' = "",
    readNotices' = if (notice != "") readNotices + 1 else readNotices,
  }

  // POST .../secrets-key/replace where it applies: one transaction under the
  // exclusive lock. The new key, every saved value and the key check move
  // together; the version bump and the activity line commit with them.
  action postReplaceApplies = all {
    replaceOffered,
    storedKey' = storedKey + 1,
    wrappedBy' = instKey.get(Acting),
    sealedUnder' = sealedUnder,
    keyCheck' = storedKey + 1,
    version' = version + 1,
    replacedLines' = replacedLines + 1,
    startedOverLines' = startedOverLines,
    writes' = writes + 1,
    commits' = commits + 1,
    posts' = posts + 1,
    notice' = "replaced",
    readNotices' = readNotices,
    // The acting replica rebuilds and installs before it answers.
    held' = held.set(Acting, storedKey + 1),
    applied' = applied.set(Acting, version + 1),
    instKey' = instKey, beating' = beating,
    ghostsUnchanged,
  }

  // The same POST where the re-seal refuses (a value this server cannot open,
  // or sealing fails): the transaction is rolled back, so nothing changed and
  // no notice is earned.
  action postReplaceRefused = all {
    durableUnchanged, replicasUnchanged, ghostsUnchanged,
    posts' = posts + 1, notice' = notice, readNotices' = readNotices,
  }

  // POST .../secrets-key/start-over with the typed confirmation, the gate
  // checked again inside the POST.
  action postStartOverApplies = all {
    startOverOffered,
    storedKey' = storedKey + 1,
    wrappedBy' = instKey.get(Acting),
    // The credentials this Foundry cannot open are discarded.
    sealedUnder' = discardUnopenable(sealedUnder, storedKey + 1, held.get(Acting)),
    keyCheck' = storedKey + 1,
    version' = version + 1,
    replacedLines' = replacedLines,
    startedOverLines' = startedOverLines + 1,
    writes' = writes + 1,
    commits' = commits + 1,
    posts' = posts + 1,
    notice' = "generated",
    readNotices' = readNotices,
    held' = held.set(Acting, storedKey + 1),
    applied' = applied.set(Acting, version + 1),
    instKey' = instKey, beating' = beating,
    lostMarked' = lostMarked, emailed' = emailed,
    // The gate must hold at the apply, not only at the GET.
    badStartOver' = if (liveOpener) true else badStartOver,
  }

  // A wrong or empty confirmation, or a running replica that opens the key:
  // I-28 again, nothing changed.
  action postStartOverRefused = all {
    durableUnchanged, replicasUnchanged, ghostsUnchanged,
    posts' = posts + 1, notice' = notice, readNotices' = readNotices,
  }

  // A reload: the replica adopts the version and re-derives the key it holds.
  action reload(r: Replica): bool = all {
    applied.get(r) != version,
    applied' = applied.set(r, version),
    held' = held.set(r, if (instKey.get(r) == wrappedBy) storedKey else 0),
    instKey' = instKey, beating' = beating,
    durableUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // A heartbeat going stale or fresh again.
  action heartbeat(r: Replica, fresh: bool): bool = all {
    beating' = beating.set(r, fresh),
    instKey' = instKey, applied' = applied, held' = held,
    durableUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // A delivery on a replica that holds a key the credential is not sealed
  // under: SecretUnavailable, settled by the backoff policy. It marks no
  // connection lost and emails nobody (DD-2 is a judgement, not a loss).
  action deliver(r: Replica): bool = all {
    savedSlots.exists(s => sealedUnder.get(s) != held.get(r)),
    durableUnchanged, replicasUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // The instance key file is new (a recreated docker compose volume) and this
  // server restarted: its instance key no longer opens the stored secrets key
  // and it holds none (I-26).
  action instanceKeyLost = all {
    storedKey != 0,
    instKey.get(Acting) == wrappedBy,
    instKey' = instKey.set(Acting, wrappedBy + 100),
    held' = held.set(Acting, 0),
    applied' = applied, beating' = beating,
    durableUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // An instance operator restores the instance key this server lacks and
  // restarts Foundry, which opens the stored key again.
  action restoreInstanceKey = all {
    instKey.get(Acting) != wrappedBy,
    instKey' = instKey.set(Acting, wrappedBy),
    held' = held.set(Acting, storedKey),
    applied' = applied, beating' = beating,
    durableUnchanged, ghostsUnchanged,
    notice' = notice, posts' = posts, readNotices' = readNotices,
  }

  // foundry doctor rotate-secrets: the instance key itself is rotated and the
  // stored secrets key re-wrapped under it, in one transaction.
  action rotateInstanceKey = all {
    storedKey != 0,
    instKey.get(Acting) == wrappedBy,
    instKey' = instKey.set(Acting, instKey.get(Acting) + 1),
    wrappedBy' = instKey.get(Acting) + 1,
    version' = version + 1,
    storedKey' = storedKey, sealedUnder' = sealedUnder, keyCheck' = keyCheck,
    replacedLines' = replacedLines, startedOverLines' = startedOverLines,
    writes' = writes + 1, commits' = commits,
    applied' = applied.set(Acting, version + 1), held' = held, beating' = beating,
    ghostsUnchanged,
    notice' = notice, posts' = posts + 1, readNotices' = readNotices,
  }

  action step = any {
    getPage,
    getReplace,
    getStartOver,
    postReplaceApplies,
    postReplaceRefused,
    postStartOverApplies,
    postStartOverRefused,
    nondet r = Replicas.oneOf() { reload(r) },
    nondet r = Replicas.oneOf() { heartbeat(r, false) },
    nondet r = Replicas.oneOf() { heartbeat(r, true) },
    nondet r = Replicas.oneOf() { deliver(r) },
    restoreInstanceKey,
    instanceKeyLost,
    rotateInstanceKey,
  }

  // ---- Q1: the re-seal is all-or-nothing. No reachable state has a saved
  // value sealed under one key and another under a different one, and the key
  // check always names the stored key.
  val allOrNothing: bool = and {
    savedSlots.forall(s => sealedUnder.get(s) == storedKey),
    storedKey != 0 implies keyCheck == storedKey,
  }

  // ---- Q2: no lost marking and no email in the window.
  val noLostNoEmail: bool = not(lostMarked) and not(emailed)

  // ---- Q3: the gate held inside every start over that committed.
  val gateHeld: bool = not(badStartOver)

  // ---- Q4a: every durable write came from a post, so a GET, a prefetch and
  // a link preview change nothing.
  val writesOnlyFromPosts: bool = writes <= posts

  // ---- Q4b: the notice is read at most once per committed change, so a
  // reload claims no second replacement.
  val noticeHonest: bool = readNotices <= commits

  // ---- Q5: no dead end. While a secrets key is stored, the page always
  // offers a way on: Replace, Start over, or the restore another live replica
  // still makes possible.
  val noDeadEnd: bool =
    storedKey != 0 implies (replaceOffered or startOverOffered or liveOpener)

  // ---- Q6: Replace and Start over are never offered together, in any order
  // of replacements and start overs.
  val offersExclusive: bool = not(replaceOffered and startOverOffered)

  val safety: bool = and {
    allOrNothing, noLostNoEmail, gateHeld,
    writesOnlyFromPosts, noticeHonest, noDeadEnd, offersExclusive,
  }
}
