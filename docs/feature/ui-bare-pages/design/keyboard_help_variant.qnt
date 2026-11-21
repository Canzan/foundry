// Which body `GET /keyboard-help` answers, decided from the destination the
// client declares (`keyboard.rs::show_keyboard_help`). One route, two arms: the
// bare overlay fragment every fetch gets today, and the framed page a top-level
// navigation gets, both advertising the same `keyboard.rs::SHORTCUTS` rows.
module keyboardHelpVariant {
  // The ordered shortcut keys of `keyboard.rs::SHORTCUTS`.
  pure val ROWS: List[str] = ["c", "/", "j", "k", "Enter", "?", "Esc"]

  // Every destination a client can declare on this route: a top-level
  // navigation, the overlay's own fetch, an embedded frame, and a client that
  // sends no fetch metadata at all.
  pure val DESTS: Set[str] = Set("document", "empty", "iframe", "absent")

  type Served = { kind: str, rows: List[str], framed: bool }

  pure val NOTHING: Served = { kind: "none", rows: [], framed: false }

  // The whole decision, total over DESTS.
  pure def serve(d: str): Served =
    if (d == "document")
      { kind: "page", rows: ROWS, framed: true }
    else
      { kind: "fragment", rows: ROWS, framed: false }

  var dest: str
  var served: Served

  action init = all { dest' = "none", served' = NOTHING }

  action request(d: str): bool = all {
    DESTS.contains(d),
    dest' = d,
    served' = serve(d),
  }

  action step = nondet d = DESTS.oneOf() { request(d) }

  // Every client that does not declare a document destination still receives
  // the bare fragment `keyboard.js` assigns into `#kb-overlay-root`.
  val fragmentUnlessDocument: bool =
    (served.kind != "none" and dest != "document") implies served.kind == "fragment"

  // A page body is answered only to a navigation, and it is always framed: the
  // full document that links the stylesheet.
  val pageIsFramed: bool =
    (served.kind == "page") implies (served.framed and dest == "document")

  // Neither arm may drop, add or reorder a shortcut row.
  val rowsNeverDrift: bool =
    (served.kind != "none") implies served.rows == ROWS
}
