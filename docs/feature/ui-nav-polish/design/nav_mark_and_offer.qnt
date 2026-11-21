// ui-nav-polish value C — what the left nav MARKS and what it OFFERS.
// Built from brief.md promises P1 and P2 only (public product facts) plus the
// shipped gate facts: /teams and the Home family admit every signed-in
// operator; /workspace/invites and /admin/tokens admit a workspace admin and
// answer every other caller with the non-enumerable 404.
//
// TWO IDENTITIES, NOT ONE. The ROUTE GATE decides admission from the acting
// session (gateRights). The RAIL decides what it offers from its own
// nav_identity read (navRights), which falls back to a neutral identity when
// the read fails. The two therefore diverge on exactly one branch: the gate
// admits a workspace admin while the rail sees a plain operator. Folding them
// into one variable hid the zero-mark state this module now carries.
//
// Design variants are fixed per run by the init action, so one model carries
// the proposed design AND the states it must exclude:
//   mark    "own"   = each of the five primary destinations marks its own page
//                     family (the proposal, P1)
//           "home"  = Teams / Invite a member / Machine tokens mark nothing and
//                     Home stays marked there (today's shipped behaviour)
//   offer   "gated" = Invite a member and Machine tokens are offered only to a
//                     workspace admin the rail's own read reported (P2)
//           "all"   = both are offered to every signed-in operator (today's
//                     shipped behaviour)
//   resolve "home"  = a page whose own member the rail will not offer marks
//                     Home instead (the proposal)
//           "none"  = the page keeps naming its own member even when the rail
//                     does not offer it, so the rail marks nothing
module navMarkAndOffer {
  type State = {
    mark: str, offer: str, resolve: str,
    gateRights: str, navRights: str, page: str
  }
  type Obs = {
    offered: Set[str],
    marked: Set[str],
    reachableHere: bool
  }

  var s: State
  var obs: Obs

  // The rights domain is the closed four-member set the shipped lookups can
  // report: the workspace-admin fact and the instance-admin fact are
  // independent, and only the first one gates a nav member.
  pure val RIGHTS = Set("plain", "workspace admin", "instance admin", "both")
  // The page families that carry a primary nav item, plus one that carries none
  // (account settings) so "no own item" stays represented.
  pure val PAGES = Set("dashboard", "board", "teams", "invites", "tokens", "settings")
  pure val PRIMARY = Set("Home", "Board", "Teams", "Invite a member", "Machine tokens")
  pure val ADMIN_ONLY = Set("Invite a member", "Machine tokens")
  pure val NONE: Set[str] = Set()

  pure def isWorkspaceAdmin(r: str): bool = r == "workspace admin" or r == "both"

  // Who the DESTINATION lets in — the shipped gate, which this feature does not
  // change. Everything outside ADMIN_ONLY admits every signed-in operator.
  pure def admits(dest: str, r: str): bool =
    if (ADMIN_ONLY.contains(dest)) isWorkspaceAdmin(r) else true

  // The page family an item owns. "settings" is owned by no primary item.
  pure def ownerOf(page: str): str =
    if (page == "dashboard") "Home"
    else if (page == "board") "Board"
    else if (page == "teams") "Teams"
    else if (page == "invites") "Invite a member"
    else if (page == "tokens") "Machine tokens"
    else "none"

  // The gate on the page itself, read with the ACTING session: a page whose own
  // item would refuse her answers 404, so no rail is rendered there at all.
  pure def pageAdmits(page: str, r: str): bool =
    if (page == "invites" or page == "tokens") isWorkspaceAdmin(r) else true

  // The rail offers from its OWN read, never from the gate's identity.
  pure def offeredBy(st: State): Set[str] =
    if (st.offer == "gated")
      PRIMARY.filter(i => admits(i, st.navRights))
    else
      PRIMARY

  // The member the page names for itself, and Home for a family that owns none.
  pure def ownMemberOf(st: State): str =
    if (ownerOf(st.page) == "none") "Home" else ownerOf(st.page)

  // The member the rail actually marks. Under resolve = "home" a page whose own
  // member the rail will not offer marks Home, which the rail always offers.
  pure def markBy(st: State): str =
    if (offeredBy(st).contains(ownMemberOf(st))) ownMemberOf(st)
    else if (st.resolve == "home") "Home"
    else ownMemberOf(st)

  pure def markedBy(st: State): Set[str] =
    if (st.mark == "own")
      offeredBy(st).filter(i => i == markBy(st))
    else
      // Today: only the board family marks Board; every other authed page marks
      // Home, the three moved destinations included.
      offeredBy(st).filter(i => if (st.page == "board") i == "Board" else i == "Home")

  pure def observe(st: State): Obs = {
    offered: offeredBy(st),
    marked: markedBy(st),
    reachableHere: pageAdmits(st.page, st.gateRights)
  }

  // The rail's read either reports the acting identity or, on failure, the
  // neutral one. Nothing else is representable.
  pure def railIdentity(gate: str, lookupFailed: bool): str =
    if (lookupFailed) "plain" else gate

  action init = all {
    nondet m = Set("own", "home").oneOf()
    nondet o = Set("gated", "all").oneOf()
    nondet rv = Set("home", "none").oneOf()
    nondet g = RIGHTS.oneOf()
    nondet f = Set(false, true).oneOf()
    nondet p = PAGES.oneOf()
    val st = { mark: m, offer: o, resolve: rv, gateRights: g,
               navRights: railIdentity(g, f), page: p }
    all { s' = st, obs' = observe(st) }
  }

  // One render per request: the operator's only move is to open another page
  // with the same acting identity; each render re-reads the rail identity, so
  // the read can fail on one page and succeed on the next.
  action openAnotherPage = all {
    nondet p = PAGES.oneOf()
    nondet f = Set(false, true).oneOf()
    val st = { ...s, page: p, navRights: railIdentity(s.gateRights, f) }
    all { s' = st, obs' = observe(st) }
  }

  action step = openAnotherPage

  // ── The laws, over the states the product can actually render ──────────────

  // P1 safety: on a page she can open, the nav marks exactly one primary item —
  // never two, and never ZERO, the failed-read branch included.
  val exactlyOneMarked: bool =
    (s.mark == "own" and obs.reachableHere) implies obs.marked.size() == 1

  // P1: the marked item is the one that owns the page she is on, and Home both
  // for a page no primary item owns and for a page whose own member the rail
  // will not offer her.
  val markedIsOwnPageOrHome: bool =
    (s.mark == "own" and s.resolve == "home" and obs.reachableHere) implies
      obs.marked == Set(markBy(s))

  // Safety across both promises: the nav never marks an item it does not offer.
  val markedIsOffered: bool = obs.marked.subseteq(obs.offered)

  // P2 safety: every link the nav offers OPENS — read against the gate's own
  // identity, not the rail's, because the gate is what answers the click.
  val offeredImpliesAdmitted: bool =
    (s.offer == "gated") implies obs.offered.forall(i => admits(i, s.gateRights))

  // P2 completeness: with the rail's read in agreement with the acting session
  // she loses no destination her rights already open. A failed read is the
  // declared degradation (failure B1), so the law is bounded to a read that
  // resolved.
  val admittedIsOffered: bool =
    (s.offer == "gated" and s.navRights == s.gateRights) implies
      PRIMARY.filter(i => admits(i, s.gateRights)).subseteq(obs.offered)

  val navLaws: bool = exactlyOneMarked and markedIsOwnPageOrHome and markedIsOffered
                      and offeredImpliesAdmitted and admittedIsOffered
}

// ── The same laws, UNGUARDED by the design variant ─────────────────────────
// Run these from `initProposal` (the proposed design) and they must hold; run
// them from `init` (which also admits today's shipped design and the
// unreconciled mark) and the checker must answer with a trace naming the
// variant — that trace is the kill of each baseline mutant.
module navProposal {
  import navMarkAndOffer.* from "./nav_mark_and_offer"

  action initProposal = all {
    nondet g = RIGHTS.oneOf()
    nondet f = Set(false, true).oneOf()
    nondet p = PAGES.oneOf()
    val st = { mark: "own", offer: "gated", resolve: "home",
               gateRights: g, navRights: railIdentity(g, f), page: p }
    all { s' = st, obs' = observe(st) }
  }

  // On every page she can open, the rail marks exactly one member, and it is
  // the member that owns the page whenever the rail offers it.
  val ownPageIsMarked: bool =
    obs.reachableHere implies obs.marked == Set(markBy(s))

  // The rail marks a member on every page she can open.
  val markNeverEmpty: bool = obs.reachableHere implies obs.marked.size() == 1

  // No link the nav offers her would refuse her.
  val noRefusedLinkOffered: bool = obs.offered.forall(i => admits(i, s.gateRights))

  val proposalLaws: bool = ownPageIsMarked and markNeverEmpty and noRefusedLinkOffered
                           and obs.marked.subseteq(obs.offered)
}
