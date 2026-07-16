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
// Slice 01 binds `?` (open help) and `Esc` (close it). c / / / j / k / Enter land
// in slices 02-05 through this same dispatch point.
(function () {
  "use strict";

  var OVERLAY_HOST_ID = "kb-overlay-root";
  var HELP_URL = "/keyboard-help";

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

  document.addEventListener("keydown", function (event) {
    if (event.key === "?") {
      openHelp();
      return;
    }
    if (event.key === "Escape" && helpIsOpen()) {
      closeHelp();
    }
  });

  markReady();
})();
