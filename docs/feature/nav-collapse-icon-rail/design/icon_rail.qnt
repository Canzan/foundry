// Value A: the icon rail. Built from brief.md only (public product facts).
// Design variants are fixed per run by the init action:
//   design   "icons" = collapsed nav shows icons + Sign out (the follow-up)
//            "hide"  = collapsed nav hides its links (comparison baseline)
//   hideMode "display-none" | "clip" (how "hide" hides; only for design "hide")
//   signOutClears: does Sign out clear the stored choice? (brief does not settle)
module iconRail {
  type State = {
    design: str, hideMode: str, signOutClears: bool,
    stored: bool, scripting: bool, rights: str, loadedRights: str,
    signedIn: bool, page: str, startedCollapsed: bool,
    focus: str, signedOutCollapsed: bool, act: str, target: str
  }
  type Obs = {
    navMode: str,
    storedChoice: str,
    toggleExists: bool,
    destinationsOperatorHas: Set[str],
    visibleActivatableIcons: Set[str],
    activatableLinks: Set[str],
    signOutActivatable: bool,
    invisibleFocusable: Set[str],
    nextLoadIfScriptingOn: str,
    nextLoadIfScriptingOff: str
  }
  type Row = { stored: bool, scripting: bool, rights: str, design: str, hideMode: str,
               has: Set[str], activatable: Set[str], invisibleFocusable: Set[str] }

  var s: State
  var obs: Obs
  var matrix: Set[Row]

  pure val RIGHTS = Set("no admin rights", "workspace admin", "instance admin")
  pure val BASE = Set("Home", "Board", "Settings", "Keyboard shortcuts")
  pure val ALL_LINKS = Set("Home", "Board", "Settings", "Keyboard shortcuts",
                           "Integrations", "Instance admin", "Sign out")
  pure val CONTROLS = Set("Monogram", "Collapse toggle", "Theme control")
  pure val NONE: Set[str] = Set()

  pure def has(r: str): Set[str] =
    BASE.union(if (r == "workspace admin") Set("Integrations") else NONE)
        .union(if (r == "instance admin") Set("Instance admin") else NONE)

  pure def collapsed(st: State): bool = st.scripting and st.stored
  pure def links(st: State): Set[str] =
    if (st.signedIn) has(st.loadedRights).union(Set("Sign out")) else NONE
  pure def linkVisible(st: State): bool = not(collapsed(st)) or st.design == "icons"
  pure def linkFocusable(st: State): bool =
    not(collapsed(st)) or st.design == "icons" or st.hideMode == "clip"
  pure def activatable(st: State, l: str): bool =
    links(st).contains(l) and linkVisible(st) and linkFocusable(st)
  pure def controls(st: State): Set[str] =
    if (not(st.signedIn)) NONE
    else if (st.scripting) CONTROLS
    else CONTROLS.exclude(Set("Collapse toggle"))
  pure def focusables(st: State): Set[str] =
    links(st).filter(l => linkFocusable(st)).union(controls(st))
  pure def invisibleFocusable(st: State): Set[str] =
    links(st).filter(l => linkFocusable(st) and not(linkVisible(st)))

  pure def observe(st: State): Obs = {
    navMode: if (not(st.signedIn)) "signed out (no nav)"
             else if (collapsed(st)) (if (st.design == "icons") "collapsed icon rail" else "collapsed, links hidden")
             else "expanded text nav",
    storedChoice: if (st.stored) "collapsed" else "expanded",
    toggleExists: st.signedIn and st.scripting,
    destinationsOperatorHas: has(st.rights),
    visibleActivatableIcons:
      if (collapsed(st)) links(st).filter(l => l != "Sign out" and activatable(st, l)) else NONE,
    activatableLinks: links(st).filter(l => activatable(st, l)),
    signOutActivatable: activatable(st, "Sign out"),
    invisibleFocusable: invisibleFocusable(st),
    nextLoadIfScriptingOn: if (st.stored) "collapsed" else "expanded",
    nextLoadIfScriptingOff: "expanded"
  }

  pure def mkMatrix(design: str, hideMode: str): Set[Row] =
    Set(true, false).map(stv =>
      Set(true, false).map(sc =>
        RIGHTS.map(r => {
          val st: State = { design: design, hideMode: hideMode, signOutClears: false,
            stored: stv, scripting: sc, rights: r, loadedRights: r, signedIn: true,
            page: "Home", startedCollapsed: stv and sc, focus: "none",
            signedOutCollapsed: false, act: "", target: "" }
          { stored: stv, scripting: sc, rights: r, design: design, hideMode: hideMode,
            has: has(r), activatable: links(st).filter(l => activatable(st, l)),
            invisibleFocusable: invisibleFocusable(st) }
        })).flatten()).flatten()

  action go(st: State): bool = all { s' = st, obs' = observe(st), matrix' = matrix }

  action initWith(design: str, hideMode: str, clears: bool): bool = {
    nondet stv = Set(true, false).oneOf()
    nondet sc = Set(true, false).oneOf()
    nondet r = RIGHTS.oneOf()
    val st: State = { design: design, hideMode: hideMode, signOutClears: clears,
      stored: stv, scripting: sc, rights: r, loadedRights: r, signedIn: true,
      page: "Home", startedCollapsed: stv and sc, focus: "none",
      signedOutCollapsed: false, act: "page load", target: "" }
    all { s' = st, obs' = observe(st), matrix' = mkMatrix(design, hideMode) }
  }
  action init_icons_keep = initWith("icons", "n/a", false)
  action init_icons_clear = initWith("icons", "n/a", true)
  action init_hide_none = initWith("hide", "display-none", false)
  action init_hide_clip = initWith("hide", "clip", false)
  action init = init_icons_keep

  action doCollapse = all { s.signedIn, s.scripting, not(s.stored),
    go(s.with("stored", true).with("focus", "Collapse toggle").with("act", "collapse").with("target", "")) }
  action doExpand = all { s.signedIn, s.scripting, s.stored,
    go(s.with("stored", false).with("focus", "Collapse toggle").with("act", "expand").with("target", "")) }
  action doTab = {
    nondet f = ALL_LINKS.union(CONTROLS).oneOf()
    all { focusables(s).contains(f),
      go(s.with("focus", f).with("act", "move focus (Tab)").with("target", f)) }
  }
  action doActivate = {
    nondet l = ALL_LINKS.exclude(Set("Sign out")).oneOf()
    all { activatable(s, l),
      go(s.with("page", if (has(s.rights).contains(l)) l else "Refused (no right)")
           .with("loadedRights", s.rights).with("startedCollapsed", collapsed(s))
           .with("focus", "none").with("act", "activate").with("target", l)) }
  }
  action doSignOut = all { activatable(s, "Sign out"),
    go(s.with("signedIn", false).with("signedOutCollapsed", collapsed(s))
         .with("stored", if (s.signOutClears) false else s.stored)
         .with("page", "Sign-in page").with("focus", "none")
         .with("act", "sign out").with("target", "Sign out")) }
  action doSignIn = all { not(s.signedIn),
    go(s.with("signedIn", true).with("loadedRights", s.rights)
         .with("startedCollapsed", s.scripting and s.stored).with("page", "Home")
         .with("focus", "none").with("act", "sign in").with("target", "")) }
  action doReload = {
    nondet sc = Set(true, false).oneOf()
    go(s.with("scripting", sc).with("loadedRights", s.rights)
         .with("startedCollapsed", sc and s.stored).with("focus", "none")
         .with("act", "reload").with("target", if (sc) "scripting on" else "scripting off"))
  }
  action doChangeRights = {
    nondet r = RIGHTS.oneOf()
    all { r != s.rights,
      go(s.with("rights", r).with("act", "change of admin rights").with("target", r)) }
  }

  action step = any { doCollapse, doExpand, doTab, doActivate, doSignOut, doSignIn, doReload, doChangeRights }

  // Q1: every destination the operator has (as rendered), and Sign out, is activatable.
  val noTrap = s.signedIn implies has(s.loadedRights).union(Set("Sign out")).forall(l => activatable(s, l))
  // Q1, exhaustive over the twelve combinations (static table).
  val matrixNoTrap = matrix.forall(row => row.has.union(Set("Sign out")).subseteq(row.activatable))
  // Q2: no invisible link takes the focus.
  val noInvisibleFocusable = invisibleFocusable(s).size() == 0
  val focusNeverInvisible = not(invisibleFocusable(s).contains(s.focus))
  val matrixNoInvisibleFocusable = matrix.forall(row => row.invisibleFocusable.size() == 0)
  // Q3: after sign-out from the collapsed rail, the next sign-in (scripting on) starts collapsed.
  val nextSignInStartsCollapsed =
    (s.act == "sign in" and s.signedOutCollapsed and s.scripting) implies s.startedCollapsed
  // Extra: rights granted without a page load, checked only in the expanded nav.
  val expandedShowsCurrentRights =
    (s.signedIn and not(collapsed(s))) implies has(s.rights).forall(l => activatable(s, l))

  // Witnesses (negated goal; a violation is a trace that reaches the goal).
  val w_railSignOutThenSignIn =
    not(s.act == "sign in" and s.signedOutCollapsed and s.scripting and s.startedCollapsed)
  val w_collapseThenActivateIcon =
    not(s.act == "activate" and s.startedCollapsed and s.design == "icons")
}
