// keyboard-shortcut-bindings slice 01 — the client keyboard dispatch layer.
//
// ADR-001: ONE vanilla IIFE, NOT alpine.js. App-owned, self-contained, CSP-safe:
// loaded as an external same-origin script (no inline handlers), all wiring via
// addEventListener. ONE document-delegated `keydown` listener — the exact
// board-dnd.js house idiom — because delegation survives htmx swaps: a handler
// bound to a card would die the moment the card is replaced.
//
// ADR-003: the overlay renders into `#kb-overlay-root`, a host DISTINCT from
// `#modal-root`, so a later slice's layered-Esc has two hosts to peel (Esc closes
// the TOPMOST layer only).
//
// Progressive enhancement: without this script the pages are unchanged and every
// no-JS link/form still works. `GET /keyboard-help` is the shipped, public source
// of the shortcut list, so the overlay's contents cannot drift from the server's
// SHORTCUTS table (keyboard.rs:48-56) — bound == advertised, by construction.
//
// ADR-002: ONE guard chain (`isInert`), evaluated ONCE before `dispatch` is
// reachable, for every key with no per-shortcut exemptions and no call-site
// checks. Falling off the end of the chain is the only path to a shortcut, so a
// new binding is guarded by default rather than by the author remembering.
// Guard 4's domain is narrowed to the keys a text-entry context can CONSUME
// (upstream-issues.md UI-3, ratified 2026-07-16) — a property of the predicate,
// not a carve-out; it names no shortcut.
//
// Bound today: `?` (open help), `Esc` (peel the topmost layer — help, else the
// modal, else nothing), `c` (open the new-issue modal).
// `/`, `j`, `k` and `Enter` are advertised by SHORTCUTS (keyboard.rs:48-56) but
// NOT yet bound; they land in slices 04-05 through this same dispatch point.
(function () {
  "use strict";

  var OVERLAY_HOST_ID = "kb-overlay-root";
  var MODAL_HOST_ID = "modal-root";
  var HELP_URL = "/keyboard-help";
  var NEW_ISSUE_TRIGGER = "[data-action='new-issue']";
  var SEARCH_PANEL_ID = "kb-search-panel";
  var NEW_ISSUE_URL_SUFFIX = "/issues/new";

  // The ADR-001 readiness marker. The @needs-browser lane waits on
  // `[data-kb-ready]` before pressing any key, and US-02's paired-assertion guard
  // uses it as its "the layer is live" precondition. Set at init, so its presence
  // means "the delegated listener is attached", never merely "the file parsed".
  function markReady() {
    document.documentElement.dataset.kbReady = "1";
  }

  function overlayHost() {
    return document.getElementById(OVERLAY_HOST_ID);
  }

  function helpIsOpen() {
    var host = overlayHost();
    return !!host && host.childElementCount > 0;
  }

  function closeHelp() {
    var host = overlayHost();
    if (host) {
      host.innerHTML = "";
    }
  }

  function modalHost() {
    return document.getElementById(MODAL_HOST_ID);
  }

  // Asked of the HOST's contents, exactly as `helpIsOpen()` is — a layer is open
  // when it is holding something, not when its mount exists. The distinction is
  // load-bearing now that a third layer sits below this one: `#modal-root` is
  // present on every board page whether or not a modal is up (board.html:13), so
  // "the host exists" would claim the modal layer on every press and the search
  // panel below it could never be reached.
  function modalIsOpen() {
    var host = modalHost();
    return !!host && host.childElementCount > 0;
  }

  function closeModal() {
    var host = modalHost();
    if (host) {
      host.innerHTML = "";
    }
  }

  // Esc peels the TOPMOST layer only (ADR-003: `#kb-overlay-root` sits above
  // `#modal-root` in base.html), one layer per press (BR-4).
  //
  // The stack is DERIVED from the DOM on every press, never stored (ADR-003 §2):
  // `helpIsOpen()` asks the host, so htmx replacing `#modal-root` behind our back
  // cannot desync it. An `openLayers` array would claim a layer that is gone and
  // turn Esc into a silent no-op while Mei stares at an open dialog.
  //
  // This function arrived at step 02-01 as a disclosed spillover (AC-02.6's Given
  // needs Esc to really close the modal). Step 03-02 added NO code here: it added
  // the ASSERTIONS, and its @layered scenario is what now HOLDS this shape — the
  // proof, run rather than assumed:
  //   - clear both hosts on one press  → "the new-issue modal is still open" REDS
  //   - point the overlay at #modal-root (one shared host, the design ADR-003
  //     rejects) → the layered scenario REDS on its own premise, and 8 more with it
  // So the two-host split and the early return below are load-bearing, and a test
  // says so.
  // Step 04-02 added the THIRD arm, which is the whole of ADR-003 §2's stack:
  // help → modal → search panel → no-op. The search panel does NOT register a
  // handler of its own; `Esc` has exactly one owner and it is this function.
  // A second `key === "Escape"` listener would race this one for the same press
  // and close two layers at once — which is BR-4's failure, and precisely what
  // 03-02's @layered scenario reds on.
  function closeTopLayer() {
    if (helpIsOpen()) {
      closeHelp();
      return;
    }
    if (modalIsOpen()) {
      closeModal();
      return;
    }
    if (searchIsOpen()) {
      closeSearch();
      return;
    }
    // Empty stack: no-op. Never navigate, never touch selection (ADR-003 §2).
  }

  function openHelp() {
    var host = overlayHost();
    if (!host || helpIsOpen()) {
      return;
    }
    fetch(HELP_URL, { credentials: "same-origin" })
      .then(function (response) {
        if (!response.ok) {
          throw new Error("keyboard-help responded " + response.status);
        }
        return response.text();
      })
      .then(function (markup) {
        host.innerHTML = markup;
      })
      .catch(function (err) {
        // Never swallow silently: a help overlay that fails to load is a real
        // defect, and a quiet catch is how the last keyboard layer went missing.
        console.error("foundry: could not load the keyboard help overlay", err);
      });
  }

  // --- The guard chain (ADR-002) --------------------------------------------
  //
  // Every advertised shortcut is a plain printable character or a bare key —
  // exactly what people type. Bound naively, `c` cannot be typed into an issue
  // title, which is strictly worse than shipping nothing. So the chain below runs
  // ONCE, here, before `dispatch` is reachable, for EVERY key: falling off the
  // end of `isInert` is the only path to a shortcut. There is deliberately no
  // per-shortcut check inside `dispatch` — a guard you have to remember at each
  // of seven call sites is a guard the eighth shortcut forgets.

  // INPUT types that are NOT text entry. An allow-list of non-text types, not a
  // deny-list of text ones: `text`, `search`, `email`, `password`, `url`, `tel`,
  // `number`, `date` — and any type this list has never heard of — are guarded by
  // default. A deny-list fails OPEN (a new input type silently loses its guard);
  // this fails CLOSED, the correct direction when a false negative means Mei
  // cannot type.
  var NON_TEXT_INPUT_TYPES = [
    "button",
    "submit",
    "reset",
    "checkbox",
    "radio",
    "file",
    "image",
    "range",
    "color",
    "hidden",
  ];

  // ARIA text widgets. Foundry renders none of these today; they are here because
  // a future ARIA text widget quietly escaping the guard is precisely the
  // regression this predicate exists to prevent, and three array entries is a
  // cheap way to fail closed.
  var TEXT_ENTRY_ROLES = ["textbox", "searchbox", "combobox", "spinbutton"];

  // Keys a text-entry context consumes NATIVELY without producing a character:
  // the browser moves the caret, edits the value, or submits the form. These are
  // the field's, exactly as a typed character is. This is a list of platform
  // facts about text inputs — NOT a list of our shortcuts, and it names none of
  // them. `Enter` earns its place from ADR-002's own Consequences ("Enter in a
  // form submits ... as a consequence of BR-2"); the rest are the other keys a
  // browser acts on inside a field, listed so a future binding on one of them
  // fails CLOSED rather than silently stealing caret movement.
  var NATIVE_TEXT_ENTRY_KEYS = [
    "Enter",
    "Tab",
    "Backspace",
    "Delete",
    "Insert",
    "ArrowLeft",
    "ArrowRight",
    "ArrowUp",
    "ArrowDown",
    "Home",
    "End",
    "PageUp",
    "PageDown",
  ];

  // Can a text-entry context CONSUME this key? True when the key produces a
  // character (`event.key` is a single printable char — `c`, `/`, `j`, `k`, `?`)
  // or when the field acts on it natively (above).
  //
  // This is guard 4's DOMAIN, narrowed per upstream-issues.md UI-3 (ratified
  // 2026-07-16). Guard 4's rationale was always "let the text-entry context
  // handle the key natively" — a key the field does nothing with has no native
  // handling to protect, so suppressing it protects no keystroke. `Escape` is
  // such a key: it produces no character, and Foundry's modals are `div`s, not
  // `<dialog>`, so nothing in a field consumes it. Under the old wording the one
  // key advertised as "Close modal" (keyboard.rs:75) was the one key that could
  // never close a modal, because every modal autofocuses its title input.
  //
  // The rule is "a text-entry context keeps the keys it can consume". It is a
  // property of this predicate, evaluated once, and it names no shortcut — there
  // is still no `key === "Escape"` test anywhere on the guard path (BR-2).
  function isConsumableByTextEntry(key) {
    if (typeof key !== "string") {
      return true;
    }
    return key.length === 1 || NATIVE_TEXT_ENTRY_KEYS.indexOf(key) !== -1;
  }

  function isTextEntry(target) {
    if (!target || target.nodeType !== 1) {
      return false;
    }
    if (target.tagName === "TEXTAREA" || target.tagName === "SELECT") {
      return true;
    }
    if (target.tagName === "INPUT") {
      // An absent or unknown type is "text" — both to the browser and to us.
      var type = (target.getAttribute("type") || "text").toLowerCase();
      return NON_TEXT_INPUT_TYPES.indexOf(type) === -1;
    }
    // The PROPERTY, not getAttribute("contenteditable"): the property is true for
    // DESCENDANTS of an editable region, so typing inside a nested element in a
    // rich-text field is guarded with no ancestor walk. It also resolves an
    // explicit contenteditable="false" island correctly, which `closest()` would
    // get backwards.
    if (target.isContentEditable === true) {
      return true;
    }
    var role = (target.getAttribute("role") || "").toLowerCase();
    return TEXT_ENTRY_ROLES.indexOf(role) !== -1;
  }

  function isInert(event) {
    // 1. IME composition. `keyCode === 229` sits beside `isComposing` because it
    //    is the legacy composition sentinel and stays reliable on IME/browser
    //    pairs where `isComposing` is unset on the composition-terminating event
    //    — the exact way an IME-commit Enter gets misread as "open selected".
    if (event.isComposing === true || event.keyCode === 229) {
      return true;
    }
    // 2. Modifier chords belong to the browser and the OS: Cmd+C copies.
    //    `shiftKey` is deliberately ABSENT — `?` IS Shift+/ on a US layout, so
    //    suppressing Shift here would silently kill one of the seven.
    if (event.ctrlKey || event.metaKey || event.altKey) {
      return true;
    }
    // 3. Another handler already owns this key.
    if (event.defaultPrevented) {
      return true;
    }
    // 4. A text-entry context has the key and can consume it. Read from the LIVE
    //    event target, so leaving a field re-enables the shortcuts on the very
    //    next keypress with no state to reset — a focus flag would be a global
    //    toggle, and a global toggle is what leaves the shortcuts dead after a
    //    field is touched.
    return isTextEntry(event.target) && isConsumableByTextEntry(event.key);
  }

  // `c` opens the new-issue modal by CLICKING the board's own shipped trigger
  // (board.html:6) rather than reconstructing its URL. That button already
  // carries the hx-get, the hx-target and the swap; going through it means the
  // keyboard path and the pointer path open the same modal by the same
  // mechanism, and this file needs no knowledge of routes or CSRF.
  //
  // Slice 03 (step 03-01) added NO code here: reusing the shipped trigger is
  // what made `c`'s full contract fall out of the four lines below, and the
  // scenarios were green the moment they were unskipped. Each arm is earned by
  // the REUSE, not by a branch:
  //   - FOCUS (AC-03.1) — `new_issue_modal.html:6`'s own `autofocus`. Removing
  //     that attribute reds "the title field is focused and ready for typing"
  //     while the modal still opens: verified, not assumed.
  //   - SCOPE (AC-03.3) — the early return below. There is no trigger on a page
  //     with no project, so the no-op needs no surface check. A version that
  //     reconstructed the URL and navigated instead reds the dashboard scenario.
  //   - FILING (AC-03.2) — the browser submits `new_issue_modal.html:4`'s
  //     `hx-post`; `Enter` reaches it because guard 4's domain declines the keys
  //     a field consumes natively. No client code is on that path at all.
  // Slice 03 is COMPLETE as of step 03-02: `Esc`-closes-the-modal, the layered
  // `Esc` (help over a still-open modal), and the empty-stack no-op are all live
  // and green — see `closeTopLayer()` above.
  function openNewIssue() {
    var trigger = document.querySelector(NEW_ISSUE_TRIGGER);
    if (!trigger) {
      // No project in context — the dashboard has no such trigger. Doing nothing
      // is the shortcut's correct behaviour where there is nothing to create.
      return;
    }
    trigger.click();
  }

  // --- The search panel (ADR-005) -------------------------------------------
  //
  // Board-only, and the board is IDENTIFIED by its own shipped "New issue"
  // trigger (board.html:6). Its `hx-get` already carries the team+project
  // context, so `projectContext()` READS the context rather than reconstructing
  // it from the URL — the same reason `c` clicks the trigger instead of
  // rebuilding its URL (ADR-005 §1: `c` and the button cannot disagree). A page
  // with no trigger has no project, so `/` is a silent no-op there (BR-3) with
  // no surface check: the panel simply was never injected.
  function projectContext() {
    var trigger = document.querySelector(NEW_ISSUE_TRIGGER);
    if (!trigger) {
      return null;
    }
    var url = trigger.getAttribute("hx-get") || "";
    if (url.slice(-NEW_ISSUE_URL_SUFFIX.length) !== NEW_ISSUE_URL_SUFFIX) {
      // The trigger's shape changed under us. Fail LOUDLY rather than guessing a
      // URL: a silently wrong search endpoint is a 404 Mei reads as "no results".
      console.error(
        "foundry: the new-issue trigger's hx-get no longer ends in " +
          NEW_ISSUE_URL_SUFFIX +
          "; cannot derive the search URL from it",
        url
      );
      return null;
    }
    return url.slice(0, -NEW_ISSUE_URL_SUFFIX.length);
  }

  function searchPanel() {
    return document.getElementById(SEARCH_PANEL_ID);
  }

  function searchInput() {
    var panel = searchPanel();
    return panel ? panel.querySelector("input[name='q']") : null;
  }

  // The results are the SHIPPED `GET …/search?q=` fragment, honoured as-is
  // (ADR-005 §2). This function fetches and mounts markup; it does NOT match,
  // rank or filter. The server already implements exact-key, case-insensitive
  // substring and the `data-empty="true"` empty state (keyboard.rs), and a
  // second client-side implementation of those rules is exactly the duplication
  // ADR-005 refuses. `q` is passed through `URLSearchParams`, so a query
  // containing `&` or `#` reaches the server intact.
  // Monotonic request token. One fetch is in flight PER KEYSTROKE, and fetches
  // have no delivery order: typing "AUTH-2" issues a request for "AUTH-" too,
  // which legitimately matches NOTHING (the exact-key branch parses "" and the
  // substring branch finds no title containing "auth-"). If that reply lands
  // after "AUTH-2"'s, a stale empty state overwrites the right answer and Mei
  // sees "no results" for an issue that exists.
  //
  // Not hypothetical: the AC-04.3 scenario RED'd on exactly this before the
  // token existed. Last request wins — never last response.
  var searchSequence = 0;

  function runSearch(panel) {
    var input = searchInput();
    var results = panel.querySelector("[data-search-results]");
    var base = panel.getAttribute("data-search-base");
    if (!input || !results || !base) {
      return;
    }
    searchSequence += 1;
    var sequence = searchSequence;
    var url = base + "/search?" + new URLSearchParams({ q: input.value }).toString();
    fetch(url, { credentials: "same-origin" })
      .then(function (response) {
        if (!response.ok) {
          throw new Error("search responded " + response.status);
        }
        return response.text();
      })
      .then(function (markup) {
        if (sequence !== searchSequence) {
          return; // A newer keystroke is already in flight; this reply is stale.
        }
        results.innerHTML = markup;
      })
      .catch(function (err) {
        // A search that silently returns nothing is indistinguishable from a
        // search that matched nothing — the empty state would LIE. Never quiet.
        console.error("foundry: the issue search failed", err);
      });
  }

  // Injected, not templated (ADR-005 §2 + its accepted cost): search has no
  // no-JS surface today — nothing links to the route at all — so rendering the
  // box server-side would advertise an affordance that breaks without JS, since
  // `search_issues` returns a bare fragment with no full-page fork. Injecting it
  // keeps the no-JS board byte-for-byte unchanged (BR-6 / NFR-4).
  //
  // The pointer control is NOT optional garnish: BR-6 forbids any action being
  // reachable by keyboard alone. `/` is an ACCELERATOR for the button beside it.
  function injectSearchPanel() {
    var base = projectContext();
    if (!base) {
      return;
    }
    var trigger = document.querySelector(NEW_ISSUE_TRIGGER);
    var panel = document.createElement("div");
    panel.id = SEARCH_PANEL_ID;
    panel.hidden = true;
    panel.setAttribute("data-search-base", base);
    var input = document.createElement("input");
    // `type=search` and `name=q`: the name is the SHIPPED route's own query
    // parameter (SearchQuery, keyboard.rs). Note `search` is absent from
    // NON_TEXT_INPUT_TYPES, so this box is a text-entry context to guard 4 —
    // which is what makes a `/` typed INTO it insert literally (AC-04.5) with
    // no code here at all.
    input.type = "search";
    input.name = "q";
    input.autocomplete = "off";
    input.setAttribute("aria-label", "Search issues");
    input.placeholder = "Search issues";
    var results = document.createElement("div");
    results.setAttribute("data-search-results", "");
    panel.appendChild(input);
    panel.appendChild(results);
    trigger.parentNode.insertBefore(panel, trigger.nextSibling);

    var control = document.createElement("button");
    control.type = "button";
    control.setAttribute("data-action", "search");
    control.textContent = "Search";
    control.addEventListener("click", function () {
      openSearch();
    });
    trigger.parentNode.insertBefore(control, trigger.nextSibling);

    input.addEventListener("input", function () {
      runSearch(panel);
    });
  }

  function searchIsOpen() {
    var panel = searchPanel();
    return !!panel && panel.hidden === false;
  }

  // `Esc`'s third arm (ADR-005 §2: "hides it, clears the query and results, and
  // restores the board"). The board needs no restoring: the panel OVERLAYS it and
  // never owned those cards, which is the property ADR-005 §4's Enter-via-the-
  // board-card rests on. So "restore" here is only "stop covering it".
  //
  // The query and results are cleared because the panel is REVEALED, not rebuilt
  // — the same node comes back on the next `/`, and a box still holding Mei's
  // last search would reopen an answer to a question she has not asked yet.
  function closeSearch() {
    var panel = searchPanel();
    if (!panel) {
      return;
    }
    panel.hidden = true;
    var input = searchInput();
    if (input) {
      input.value = "";
    }
    var results = panel.querySelector("[data-search-results]");
    if (results) {
      results.innerHTML = "";
    }
  }

  // `/` reveals the panel and focuses the box. The caller preventDefault()s.
  function openSearch() {
    var panel = searchPanel();
    if (!panel) {
      // No project in context — nothing to search. Silent (BR-3).
      return;
    }
    panel.hidden = false;
    var input = searchInput();
    if (input) {
      input.focus();
    }
  }

  document.addEventListener("keydown", function (event) {
    if (isInert(event)) {
      return;
    }
    dispatch(event);
  });

  function dispatch(event) {
    if (event.key === "?") {
      openHelp();
      return;
    }
    if (event.key === "c") {
      openNewIssue();
      return;
    }
    if (event.key === "/") {
      // THE CLASSIC BUG (FR-7): without this preventDefault, the very slash that
      // opens the box is then typed INTO the box it just focused, and Mei's
      // first search is for "/session". The focus happens synchronously above,
      // so the default action would land in an already-focused field.
      //
      // This preventDefault is `/`'s OWN keypress only — a slash typed into the
      // already-focused box never reaches here at all (guard 4: the box is a
      // text-entry context and `/` is a character it consumes), which is why
      // AC-04.5 needs no code and no exemption.
      event.preventDefault();
      openSearch();
      return;
    }
    if (event.key === "Escape") {
      closeTopLayer();
    }
  }

  injectSearchPanel();
  markReady();
})();
