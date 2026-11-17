// nav-collapse-icon-rail, value S1: the head-script guards.
//
// Two dimensions are fixed per run by the init action.
//
// READING of "every other head script defers" (P12 totality):
//   "as-written"   : no verdict for a non-stamper loaded async or type=module
//   "defer-attr"   : only the `defer` attribute counts as deferring  <-- CHOSEN
//   "deferred-sem" : defer and type=module count; async does not
//   "non-blocking" : defer, type=module and async all count
//
// RECOGNITION — how the gate decides that an asset stamps the document from
// stored state (the review finding on xtask check_arch):
//   "wide"   : the asset NAMES localStorage or sessionStorage AND NAMES
//              documentElement, whatever idiom it spells them in  <-- CHOSEN
//   "narrow" : only the direct `.getItem` / `documentElement.setAttribute`
//              idioms are recognised, so an aliased stamper escapes the set
module headGuard {
  type Shape = { storage: str, docEl: bool, loading: str }
  type Verdict = { storage: str, docEl: bool, loading: str, verdict: str, message: str }
  type Script = { name: str, storage: str, docEl: bool, loading: str, idiom: str }

  pure val STORAGES = Set("localStorage", "sessionStorage", "neither")
  pure val LOADINGS = Set("plain", "defer", "async", "type=module")
  pure val SHAPES: Set[Shape] =
    tuples(STORAGES, Set(true, false), LOADINGS).map(t => { storage: t._1, docEl: t._2, loading: t._3 })

  pure def mustPrePaint(storage: str, docEl: bool): bool = storage != "neither" and docEl
  pure def isStamper(sc: Script): bool = mustPrePaint(sc.storage, sc.docEl)

  // What the GATE sees. Under "wide" this is the truth; under "narrow" an
  // aliased stamper is invisible to the gate and falls into the "every other
  // head script" branch, where `defer` is accepted.
  pure def recognised(rec: str, sc: Script): bool =
    isStamper(sc) and (rec == "wide" or sc.idiom == "direct")

  pure def judge(reading: str, sh: Shape): Verdict = {
    val r =
      if (mustPrePaint(sh.storage, sh.docEl)) {
        if (sh.loading == "plain")
          ("accept", "accepted: reads stored state and stamps documentElement; runs before first paint")
        else
          ("refuse", "refused: reads stored state and stamps documentElement, so it must run before first paint; drop defer, async and type=module")
      } else {
        if (sh.loading == "defer")
          ("accept", "accepted: does not stamp stored state on documentElement, and it defers")
        else if (sh.loading == "plain")
          ("refuse", "refused: does not stamp stored state on documentElement, so it must defer; add defer")
        else if (reading == "as-written")
          ("none", "no verdict: the rule says 'defers' and does not say whether this loading mode defers")
        else if (reading == "defer-attr")
          ("refuse", "refused: does not stamp stored state on documentElement, so it must carry defer")
        else if (reading == "deferred-sem")
          (if (sh.loading == "type=module") "accept" else "refuse",
           if (sh.loading == "type=module") "accepted: module scripts are deferred"
           else "refused: async may run before first paint; use defer")
        else
          ("accept", "accepted: does not block parsing")
      }
    { storage: sh.storage, docEl: sh.docEl, loading: sh.loading, verdict: r._1, message: r._2 }
  }

  // Paint-order heads. All scripts are external (src); see assumptions.
  pure val NAV   = { name: "nav-state stamp", storage: "localStorage", docEl: true, loading: "plain", idiom: "direct" }
  pure val THEME = { name: "theme stamp", storage: "sessionStorage", docEl: true, loading: "plain", idiom: "direct" }
  pure val APP   = { name: "app bundle", storage: "neither", docEl: false, loading: "defer", idiom: "direct" }
  pure val READ  = { name: "stored-state reader (no stamp)", storage: "localStorage", docEl: false, loading: "defer", idiom: "direct" }
  // A third stamp added later, spelled through a local alias and an element
  // reference, and given `defer` by a contributor copying its neighbours.
  pure val ALIAS = { name: "aliased stamp", storage: "localStorage", docEl: true, loading: "defer", idiom: "alias" }

  pure def headOf(h: str): List[Script] =
    if (h == "accepted head") [NAV, THEME, APP, READ]
    else if (h == "stamper deferred") [NAV, THEME.with("loading", "defer"), APP, READ]
    else if (h == "stamper async") [NAV, THEME.with("loading", "async"), APP, READ]
    else [NAV, THEME, ALIAS, APP, READ]
  pure val HEADS = Set("accepted head", "stamper deferred", "stamper async", "aliased stamp deferred")
  pure def runsEarly(sc: Script): bool = sc.loading == "plain"
  pure def runsLate(sc: Script): bool = sc.loading == "defer" or sc.loading == "type=module"

  var reading: str
  var rec: str
  var verdicts: Set[Verdict]
  var hd: str
  var headVerdict: str
  var phase: str        // "parse head" | "run non-deferred" | "first paint" | "run deferred" | "done"
  var ran: List[str]
  var stampsLanded: Set[str]
  var stampsAtPaint: Set[str]
  var last: str

  pure def gateShape(rc: str, sc: Script): Shape =
    if (recognised(rc, sc)) { storage: sc.storage, docEl: sc.docEl, loading: sc.loading }
    else { storage: "neither", docEl: false, loading: sc.loading }

  pure def gate(rd: str, rc: str, h: str): str = {
    val vs = headOf(h).foldl(Set(), (acc, sc) => acc.union(Set(judge(rd, gateShape(rc, sc)).verdict)))
    if (vs.contains("refuse")) "refuse" else if (vs.contains("none")) "none" else "accept"
  }

  action initWith(rd: str, rc: str): bool = {
    nondet h = HEADS.oneOf()
    all {
      reading' = rd,
      rec' = rc,
      verdicts' = SHAPES.map(sh => judge(rd, sh)),
      hd' = h, headVerdict' = gate(rd, rc, h),
      phase' = "parse head", ran' = [], stampsLanded' = Set(), stampsAtPaint' = Set(),
      last' = "parse the head"
    }
  }
  // The design as recorded: the defer-attribute reading over a wide classifier.
  action init_defer_attr_wide = initWith("defer-attr", "wide")
  action init = init_defer_attr_wide
  // Falsifying variants.
  action init_as_written_wide = initWith("as-written", "wide")
  action init_defer_attr_narrow = initWith("defer-attr", "narrow")
  action init_deferred_sem_wide = initWith("deferred-sem", "wide")
  action init_non_blocking_wide = initWith("non-blocking", "wide")

  pure def firstPending(h: str, rn: List[str], p: Script => bool): List[Script] =
    headOf(h).select(p).select(sc => not(rn.foldl(false, (b, n) => b or n == sc.name)))

  action runOne(sc: Script, nextPhase: str): bool = all {
    ran' = ran.append(sc.name),
    stampsLanded' = if (isStamper(sc)) stampsLanded.union(Set(sc.name)) else stampsLanded,
    phase' = nextPhase, stampsAtPaint' = stampsAtPaint,
    last' = sc.name,
    reading' = reading, rec' = rec, verdicts' = verdicts, hd' = hd, headVerdict' = headVerdict
  }
  action keep(p: str, l: str): bool = all {
    phase' = p, last' = l, ran' = ran, stampsLanded' = stampsLanded, stampsAtPaint' = stampsAtPaint,
    reading' = reading, rec' = rec, verdicts' = verdicts, hd' = hd, headVerdict' = headVerdict
  }

  // Parse the head -> run non-deferred in order -> first paint -> run deferred in order.
  // An async script may run at any point after parse (before or after first paint).
  action startEarly = all { phase == "parse head", keep("run non-deferred", "head parsed") }
  action runEarly = {
    val q = firstPending(hd, ran, runsEarly)
    all { phase == "run non-deferred", q.length() > 0, runOne(q[0], "run non-deferred") }
  }
  action paint = all { phase == "run non-deferred", firstPending(hd, ran, runsEarly).length() == 0,
    phase' = "first paint", stampsAtPaint' = stampsLanded, last' = "first paint",
    ran' = ran, stampsLanded' = stampsLanded,
    reading' = reading, rec' = rec, verdicts' = verdicts, hd' = hd, headVerdict' = headVerdict }
  action toLate = all { phase == "first paint", keep("run deferred", "start deferred") }
  action runLate = {
    val q = firstPending(hd, ran, runsLate)
    all { phase == "run deferred", q.length() > 0, runOne(q[0], "run deferred") }
  }
  action runAsync = {
    val q = firstPending(hd, ran, sc => sc.loading == "async")
    all { phase != "parse head", phase != "done", q.length() > 0, runOne(q[0], phase) }
  }
  action finish = all { phase == "run deferred",
    firstPending(hd, ran, runsLate).length() == 0,
    firstPending(hd, ran, sc => sc.loading == "async").length() == 0,
    keep("done", "all scripts ran") }

  action step = any { startEarly, runEarly, paint, toLate, runLate, runAsync, finish }

  // L-S1-1 / P12: one verdict per shape, 24 shapes, verdict in {accept, refuse}.
  val totalVerdicts = verdicts.size() == 24 and verdicts.map(v => (v.storage, v.docEl, v.loading)).size() == 24
  val noThirdVerdict = verdicts.forall(v => v.verdict == "accept" or v.verdict == "refuse")
  // L-S1-2 / P8: the classifier sees every script that really stamps stored
  // state onto the document element, whatever idiom it is spelled in.
  val classifierSeesEveryStamper =
    headOf(hd).select(isStamper).foldl(true, (b, sc) => b and recognised(rec, sc))
  // Paint order: every stamp in the head has landed by the first paint.
  pure def stampers(h: str): Set[str] =
    headOf(h).select(isStamper).foldl(Set(), (acc, sc) => acc.union(Set(sc.name)))
  val stampsBeforePaint =
    (phase == "first paint" or phase == "run deferred" or phase == "done") implies stampsAtPaint == stampers(hd)
  // L-S1-3 / P8: a head the gate ACCEPTS has all of its stamps landed by the
  // first paint. This is the law the narrow classifier breaks.
  val gateMatchesPaint =
    (headVerdict == "accept" and (phase == "first paint" or phase == "run deferred" or phase == "done"))
      implies stampsAtPaint == stampers(hd)
  val stampsBeforePaint_deferredHead = hd == "stamper deferred" implies stampsBeforePaint
  val stampsBeforePaint_asyncHead = hd == "stamper async" implies stampsBeforePaint
  val stampsBeforePaint_acceptedHead = hd == "accepted head" implies stampsBeforePaint
  // Witnesses (negated goal; a violation is a trace that reaches the goal).
  val w_acceptedHeadDone = not(hd == "accepted head" and phase == "done")
  val w_aliasAcceptedWithLateStamp =
    not(hd == "aliased stamp deferred" and headVerdict == "accept" and phase == "done"
        and stampsAtPaint != stampers(hd))
}
