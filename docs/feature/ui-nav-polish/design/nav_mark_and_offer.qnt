// ui-nav-polish value C — what the left nav MARKS and what it OFFERS.
// Built from brief.md promises P1 and P2 only (public product facts) plus the
// shipped gate facts: /teams and the Home family admit every signed-in
// operator; /workspace/invites and /admin/tokens admit a workspace admin and
// answer every other caller with the non-enumerable 404.
//
// Design variants are fixed per run by the init action, so one model carries
// the proposed design AND the states it must exclude:
//   mark  "own"   = each of the five primary destinations marks its own page
//                   family (the proposal, P1)
//         "home"  = Teams / Invite a member / Machine tokens mark nothing and
//                   Home stays marked there (today's shipped behaviour)
//   offer "gated" = Invite a member and Machine tokens are offered only to a
//                   workspace admin (the proposal, P2)
//         "all"   = both are offered to every signed-in operator (today's
//                   shipped behaviour)
module navMarkAndOffer {
  type State = {
    mark: str, offer: str,
    rights: str, page: str
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

  // The gate on the page itself: a page whose own item would refuse her answers
  // 404, so no rail is rendered there at all.
  pure def pageAdmits(page: str, r: str): bool =
    if (page == "invites" or page == "tokens") isWorkspaceAdmin(r) else true

  pure def offeredBy(st: State): Set[str] =
    if (st.offer == "gated")
      PRIMARY.filter(i => admits(i, st.rights))
    else
      PRIMARY

  pure def markedBy(st: State): Set[str] =
    if (st.mark == "own")
      // The item that owns this page family, and Home for a family that owns
      // none. Never an item the rail does not offer.
      offeredBy(st).filter(i =>
        if (ownerOf(st.page) == "none") i == "Home" else i == ownerOf(st.page))
    else
      // Today: only the board family marks Board; every other authed page marks
      // Home, the three moved destinations included.
      offeredBy(st).filter(i => if (st.page == "board") i == "Board" else i == "Home")

  pure def observe(st: State): Obs = {
    offered: offeredBy(st),
    marked: markedBy(st),
    reachableHere: pageAdmits(st.page, st.rights)
  }

  action init = all {
    nondet m = Set("own", "home").oneOf()
    nondet o = Set("gated", "all").oneOf()
    nondet r = RIGHTS.oneOf()
    nondet p = PAGES.oneOf()
    all {
      s' = { mark: m, offer: o, rights: r, page: p },
      obs' = observe({ mark: m, offer: o, rights: r, page: p })
    }
  }

  // One render per request: the operator's only move is to open another page
  // with the same identity and the same shipped design.
  action openAnotherPage = all {
    nondet p = PAGES.oneOf()
    all {
      s' = { ...s, page: p },
      obs' = observe({ ...s, page: p })
    }
  }

  action step = openAnotherPage

  // ── The laws, over the states the product can actually render ──────────────

  // P1 safety: on a page she can open, the nav marks exactly one primary item.
  val exactlyOneMarked: bool =
    (s.mark == "own" and obs.reachableHere) implies obs.marked.size() == 1

  // P1: the marked item is the one that owns the page she is on, and for a page
  // no primary item owns, Home.
  val markedIsOwnPage: bool =
    (s.mark == "own" and obs.reachableHere) implies
      obs.marked == Set(if (ownerOf(s.page) == "none") "Home" else ownerOf(s.page))

  // Safety across both promises: the nav never marks an item it does not offer.
  val markedIsOffered: bool = obs.marked.subseteq(obs.offered)

  // P2 safety: every link the nav offers her opens.
  val offeredImpliesAdmitted: bool =
    (s.offer == "gated") implies obs.offered.forall(i => admits(i, s.rights))

  // P2 completeness: she loses no destination her rights already open — the
  // gate removes exactly the two links that would refuse her.
  val admittedIsOffered: bool =
    (s.offer == "gated") implies
      PRIMARY.filter(i => admits(i, s.rights)).subseteq(obs.offered)

  val navLaws: bool = exactlyOneMarked and markedIsOwnPage and markedIsOffered
                      and offeredImpliesAdmitted and admittedIsOffered
}

// ── The same laws, UNGUARDED by the design variant ─────────────────────────
// Run these from `initProposal` (the proposed design) and they must hold; run
// them from `init` (which also admits today's shipped design) and the checker
// must answer with a trace naming the shipped variant — that trace is the kill
// of each baseline mutant.
module navProposal {
  import navMarkAndOffer.* from "./nav_mark_and_offer"

  action initProposal = all {
    nondet r = RIGHTS.oneOf()
    nondet p = PAGES.oneOf()
    all {
      s' = { mark: "own", offer: "gated", rights: r, page: p },
      obs' = observe({ mark: "own", offer: "gated", rights: r, page: p })
    }
  }

  // On every page she can open, the item that owns the page is the marked one.
  val ownPageIsMarked: bool =
    obs.reachableHere implies
      obs.marked == Set(if (ownerOf(s.page) == "none") "Home" else ownerOf(s.page))

  // No link the nav offers her would refuse her.
  val noRefusedLinkOffered: bool = obs.offered.forall(i => admits(i, s.rights))

  val proposalLaws: bool = ownPageIsMarked and noRefusedLinkOffered
                           and obs.marked.subseteq(obs.offered)
}
