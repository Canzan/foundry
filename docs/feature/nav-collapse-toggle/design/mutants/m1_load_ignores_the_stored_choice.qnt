// MUTANT — not a design artefact. A document load shows expanded whatever the key holds.
// One witness for one invariant: the guarded behaviour is removed and the
// invariant is left untouched, so a counterexample proves the invariant is
// load-bearing rather than vacuous.
// The operator's left-nav collapse choice, as it travels between the ONE place
// it is stored (the browser origin's `foundry.nav` key) and the SEVERAL
// documents that read it. Two documents are modelled, not one: the app shell
// renders in every tab, every tab's script writes the same key, and nothing in
// the product promises a tab learns about another tab's press. Two actors on one
// piece of state is the whole reason this file exists — a single-document model
// would be an example, not a check.
//
// WHAT IS NOT HERE, DELIBERATELY. Three of the laws this feature needs are not
// invariants of this model because the chosen carrier makes their violation
// unrepresentable rather than checkable:
//
//   * the per-document state is a JS BOOLEAN, so "a third rendering" has no
//     value to be written from;
//   * the document marker is a VALUELESS attribute (`data-nav-collapsed`
//     present or absent), so the stylesheet matches on presence and no stray
//     value can select a third geometry;
//   * the stored form is the single sentinel `"collapsed"` or NO KEY, written
//     from that same boolean, so "expanded" is never a stored word.
//
// A foreign value CAN still appear in the key — another tool, an extension, a
// later version — so `interpret` below is total over strings by construction
// and `outsideWrite` exercises it. That is modelled; the carrier facts above
// are claims about the representation, recorded as such in the design.
//
// ALSO NOT COVERED: paint ordering. "No flash of the expanded nav" is a fact
// about when the browser is permitted to paint relative to a render-blocking
// <head> script. It is not a state-machine property and this model says nothing
// about it; the design carries it as a separate observation.
//
// One unit of modelled behaviour is one operator event or one document load.
module nav_collapse_choice {
  // MUTATED: loadA/loadB never read the key.
  /// The one value `remember()` of static/js/nav-collapse.js ever writes.
  pure val COLLAPSED: str = "collapsed"
  /// No key under `foundry.nav` — the expanded state, and the state of every
  /// operator who has never touched the control.
  pure val ABSENT: str = ""

  /// What a writer OTHER than this feature can leave in the key: the two
  /// canonical forms, the word the feature deliberately never writes, a case
  /// variant, and two shapes a careless producer emits.
  pure val FOREIGN_VALUES: Set[str] = Set(
    ABSENT, COLLAPSED, "expanded", "Collapsed", "1", "null"
  )

  /// The shared `window.localStorage` entry for key `foundry.nav`.
  var stored: str

  /// Document A: is the rail shown collapsed? The runtime carrier is the
  /// boolean `collapsed` held by the script's closure and mirrored by the
  /// presence of `data-nav-collapsed` on <html>.
  var collapsedA: bool
  /// Document B, a second tab on the same origin.
  var collapsedB: bool

  /// What this document's own operator last asked for. Before her first press
  /// it is what the document loaded with.
  var choiceA: bool
  var choiceB: bool

  /// True while this document's rail agrees with the key, by ORDERING: the
  /// document has read storage more recently than any press that rewrote it.
  /// A press in ANOTHER tab desynchronises this one, and nothing in the
  /// product promises otherwise.
  var syncedA: bool
  var syncedB: bool

  /// The canonical stored form of the most recent press by either document.
  var lastPress: str

  /// False once site storage is refused for the origin. Reads and writes both
  /// throw there (measured for the theme control on chromedriver 151), so the
  /// script's guards return the default and the choice stops travelling.
  var storageUsable: bool

  /// True while the stored value is still the one this feature last wrote:
  /// storage has been usable throughout and no outside writer has touched the
  /// key since the last press. The persistence law is claimed only here; with
  /// storage refused or the key overwritten, the operator loses persistence,
  /// which is the documented degradation and not a defect.
  var trusted: bool

  /// Has document A ever shown collapsed? Only the re-expansion witness reads
  /// it; no law depends on it.
  var wasCollapsedA: bool

  /// TOTAL over every string the key can hold: only the sentinel means
  /// collapsed, so a foreign value reads as the expanded default rather than
  /// as a state no stylesheet matches.
  pure def interpret(value: str): bool = value == COLLAPSED

  /// The inverse: the boolean's canonical stored form. `remember()` writes
  /// this value or removes the key; it never writes a word for expanded.
  pure def sentinel(isCollapsed: bool): str = if (isCollapsed) COLLAPSED else ABSENT

  /// What a document shows when it loads. With storage refused the guard's
  /// catch yields the expanded default.
  pure def onLoad(value: str, usable: bool): bool = usable and interpret(value)

  action init = all {
    stored' = ABSENT,
    collapsedA' = false,
    collapsedB' = false,
    choiceA' = false,
    choiceB' = false,
    syncedA' = true,
    syncedB' = true,
    lastPress' = ABSENT,
    storageUsable' = true,
    trusted' = true,
    wasCollapsedA' = false,
  }

  /// Document A loads: a reload, or the operator opening another page of the
  /// app shell. The <head> script reads the key and stamps the marker before
  /// the browser is permitted to paint.
  action loadA = all {
    collapsedA' = false,
    choiceA' = false,
    syncedA' = true,
    wasCollapsedA' = wasCollapsedA,
    trusted' = trusted and storageUsable,
    stored' = stored,
    collapsedB' = collapsedB,
    choiceB' = choiceB,
    syncedB' = syncedB,
    lastPress' = lastPress,
    storageUsable' = storageUsable,
  }

  action loadB = all {
    collapsedB' = false,
    choiceB' = false,
    syncedB' = true,
    trusted' = trusted and storageUsable,
    stored' = stored,
    collapsedA' = collapsedA,
    choiceA' = choiceA,
    syncedA' = syncedA,
    lastPress' = lastPress,
    storageUsable' = storageUsable,
    wasCollapsedA' = wasCollapsedA,
  }

  /// The operator activates document A's toggle: the boolean flips, the marker
  /// follows it, and the key is rewritten. A refused write costs persistence
  /// and nothing else — the rail still moves.
  action pressA = {
    val wanted = not(collapsedA)
    all {
      collapsedA' = wanted,
      choiceA' = wanted,
      syncedA' = storageUsable,
      syncedB' = false,
      wasCollapsedA' = if (wanted) true else wasCollapsedA,
      stored' = if (storageUsable) sentinel(wanted) else stored,
      lastPress' = sentinel(wanted),
      trusted' = storageUsable,
      collapsedB' = collapsedB,
      choiceB' = choiceB,
      storageUsable' = storageUsable,
    }
  }

  action pressB = {
    val wanted = not(collapsedB)
    all {
      collapsedB' = wanted,
      choiceB' = wanted,
      syncedB' = storageUsable,
      syncedA' = false,
      stored' = if (storageUsable) sentinel(wanted) else stored,
      lastPress' = sentinel(wanted),
      trusted' = storageUsable,
      collapsedA' = collapsedA,
      choiceA' = choiceA,
      storageUsable' = storageUsable,
      wasCollapsedA' = wasCollapsedA,
    }
  }

  /// Site storage is refused for the origin from here on.
  action refuseStorage = all {
    storageUsable' = false,
    trusted' = false,
    stored' = stored,
    collapsedA' = collapsedA,
    collapsedB' = collapsedB,
    choiceA' = choiceA,
    choiceB' = choiceB,
    syncedA' = syncedA,
    syncedB' = syncedB,
    lastPress' = lastPress,
    wasCollapsedA' = wasCollapsedA,
  }

  /// Somebody that is not this feature writes the key.
  action outsideWrite = {
    nondet value = oneOf(FOREIGN_VALUES)
    all {
      stored' = value,
      trusted' = false,
      collapsedA' = collapsedA,
      collapsedB' = collapsedB,
      choiceA' = choiceA,
      choiceB' = choiceB,
      syncedA' = false,
      syncedB' = false,
      lastPress' = lastPress,
      storageUsable' = storageUsable,
      wasCollapsedA' = wasCollapsedA,
    }
  }

  action step = any {
    loadA,
    loadB,
    pressA,
    pressB,
    refuseStorage,
    outsideWrite,
  }

  // ─────────────────────────────── the laws ───────────────────────────────

  /// P1/P3. The rail shows what the operator of THIS document last asked for
  /// — her own press, or the choice the document loaded with — and nothing
  /// else moves it. Unconditional: it holds with site storage refused, which
  /// is what makes the control worth more than the persistence behind it, and
  /// it is what forbids another tab's press from moving this rail.
  val railShowsThisDocumentsOwnChoice: bool =
    collapsedA == choiceA and collapsedB == choiceB

  /// P2/P3. A document that has read the key more recently than any press
  /// shows the last choice made in ANY document of this browser — that is what
  /// "stays collapsed across reloads and pages" means. Claimed only while the
  /// stored value is still the one this feature wrote.
  val aSyncedDocumentShowsTheLastChoice: bool =
    (not(syncedA) or not(trusted) or collapsedA == interpret(lastPress))
      and (not(syncedB) or not(trusted) or collapsedB == interpret(lastPress))

  /// The write side of persistence: after a press the key holds that press's
  /// canonical form, and a document load never rewrites the key — a load that
  /// "normalised" storage would erase a real collapsed choice on the first
  /// page opened while storage happens to be unreadable.
  val storageHoldsTheLastPress: bool = not(trusted) or stored == lastPress

  // ───────────────────────── reachability witnesses ──────────────────────
  // Each is EXPECTED to be refuted. The counterexample is the construction
  // path: a value reached through the operator's own public stimulus.

  /// Refuted by any trace that collapses the rail.
  val neverCollapsed: bool = not(collapsedA)

  /// Refuted by a trace that collapses and then expands again, which is P3.
  val neverReexpanded: bool = not(wasCollapsedA and not(collapsedA))
}
