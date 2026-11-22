// ui-page-frame value B1 — WHERE A PAGE'S CONTENT STARTS, AND WHO DECIDES IT.
//
// Built from brief.md promise P1 only (a public product fact: every signed-in
// app page starts its content at the same left edge and the same top spacing
// beside the nav, at the same width, at desk width and on a phone) plus the
// shipped stylesheet facts the screenshot review measured: the rail is 240px,
// `.app-shell__content` pads 24px/32px (16px below the phone breakpoint), the
// element rule `main { padding: 20px }` adds a SECOND box on every page that
// wraps its content in <main>, and `.dash` adds a THIRD kind of box — a
// centred `max-width: 880px` column whose inline start is a function of the
// WINDOW, which is the 424px Home edge.
//
// THE ALGEBRA IS A SUM OVER A PATH, not a per-page property. A page's content
// inline start is the rail plus EVERY padding box between the shell and the
// page's first content box. That is why the defect is invisible per page and
// obvious across pages, and why the cure is an OWNERSHIP rule rather than a
// per-page correction: exactly one box on that path may pad.
//
// Design variants are fixed per run by `init`, so one module carries the
// proposal AND the three states it must exclude:
//   "owner"       = the frame is the only padding box on every page (proposal)
//   "mainPads"    = the shipped `main { padding: 20px }` element rule stays
//   "homeCentres" = the shipped centred `.dash` column stays
//   "phonePerPage"= the phone block pads one page family differently
module pageFrame {
  // The signed-in app pages group B names, by the box chain each renders.
  // "framed"  — content directly in the frame (board, invites)
  // "mainWrap"— content wrapped in <main> (teams, tokens, settings, instance
  //             admin, the integrations pages)
  // "centred" — the dashboard's own centred column (home)
  type PageKind = Framed | MainWrap | Centred
  type Viewport = Desk | Phone

  pure val pages: Set[str] = Set("home", "board", "teams", "invites", "tokens", "settings", "instanceAdmin")

  pure def shippedKind(p: str): PageKind =
    if (p == "home") Centred
    else if (p == "board" or p == "invites") Framed
    else MainWrap

  var choice: str
  var viewport: Viewport

  pure val rail: int = 240
  pure def frameInline(v: Viewport): int = if (v == Desk) 32 else 16
  pure def frameBlock(v: Viewport): int = if (v == Desk) 24 else 16

  // The box chain the DESIGN gives a page under the chosen choice.
  pure def kindOf(ch: str, p: str): PageKind =
    if (ch == "owner") Framed
    else if (ch == "mainPads") (if (shippedKind(p) == Centred) Framed else shippedKind(p))
    else if (ch == "homeCentres") (if (shippedKind(p) == MainWrap) Framed else shippedKind(p))
    else shippedKind(p)

  // Padding boxes between the shell and the page's first content box.
  pure def paddingBoxes(ch: str, p: str): int =
    val k = kindOf(ch, p)
    if (k == Framed) 1 else 2

  // A centred column's inline start is a function of the window, not of the
  // frame: it is the one chain the model must be able to call out as such.
  pure def windowDependent(ch: str, p: str): bool = kindOf(ch, p) == Centred

  // The observation: the inline and block start of the page's first content
  // box, in the frame's own units. A centred column is recorded as -1, which
  // no uniform chain can produce, because its value is not a constant at all.
  pure def inlineStart(ch: str, p: str, v: Viewport): int =
    if (windowDependent(ch, p)) -1
    else if (kindOf(ch, p) == MainWrap) rail + frameInline(v) + 20
    else if (ch == "phonePerPage" and v == Phone and p == "teams") rail + frameInline(v) + 4
    else rail + frameInline(v)

  pure def blockStart(ch: str, p: str, v: Viewport): int =
    if (windowDependent(ch, p)) -1
    else if (kindOf(ch, p) == MainWrap) frameBlock(v) + 20
    else frameBlock(v)

  action init = all {
    nondet ch = oneOf(Set("owner", "mainPads", "homeCentres", "phonePerPage"))
    choice' = ch,
    nondet v = oneOf(Set(Desk, Phone))
    viewport' = v,
  }

  action step = all {
    choice' = choice,
    nondet v = oneOf(Set(Desk, Phone))
    viewport' = v,
  }

  // P1, as four separate refusals over the whole page set at the live viewport.
  val sameInlineStart: bool =
    pages.forall(p => pages.forall(q => inlineStart(choice, p, viewport) == inlineStart(choice, q, viewport)))
  val sameBlockStart: bool =
    pages.forall(p => pages.forall(q => blockStart(choice, p, viewport) == blockStart(choice, q, viewport)))
  val noWindowDependentStart: bool = pages.forall(p => not(windowDependent(choice, p)))
  val oneOwningPaddingBox: bool = pages.forall(p => paddingBoxes(choice, p) == 1)

  val uniformFrameHere: bool =
    sameInlineStart and sameBlockStart and noWindowDependentStart and oneOwningPaddingBox

  // The proposal's claim.
  val uniformFrame: bool = (choice != "owner") or uniformFrameHere
}
