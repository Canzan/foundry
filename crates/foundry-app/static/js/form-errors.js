// form-error-display-contract slice 01 — make htmx form validation errors VISIBLE.
//
// THE DEFECT (RCA, escalated from /nw-bugfix): htmx 2.0.4 does not swap the body
// of a 4xx response. The app returns validation errors CORRECTLY — a 400 + the
// server's error fragment (issues.rs `bad_request_fragment`) — but the browser
// discards it, so the form silently does nothing and Mei never learns why her
// submit was rejected. The HTTP acceptance lane cannot see this: the response is
// byte-identical before and after; only the rendered DOM differs.
//
// THE FIX (DESIGN ADR-001): ONE document-delegated `htmx:beforeSwap` listener.
// When a request comes back 4xx AND the triggering element opts in with an error
// slot, tell htmx to swap the 4xx body INTO that slot (and treat it as a normal,
// non-error swap). A form with no slot is left entirely alone — htmx's default
// no-swap stands, so unrelated 4xx responses (not-found, forbidden, the OOB
// success flow, board-dnd.js) are untouched. The server stays byte-identical;
// this is client-side only.
//
// App-owned, self-contained, CSP-safe: loaded as an external same-origin script
// (no inline handlers), wired via addEventListener on document.body — the same
// delegation idiom as board-dnd.js / keyboard.js, so it survives htmx swaps.
(function () {
  "use strict";

  // The ADR-001 readiness marker (mirrors keyboard.js's data-kb-ready). Set at
  // init, so its presence means the beforeSwap listener is ATTACHED — the
  // @needs-browser lane waits on it before submitting, and its ABSENCE is what
  // makes the defect reproduction fail loudly rather than race.
  function markReady() {
    document.documentElement.dataset.formErrorsReady = "1";
  }

  // Resolve the error slot for the element that triggered this request:
  //   1. an EXPLICIT `data-error-target="#selector"` on the element (wins), else
  //   2. the element's own `<form>`'s `[data-error-slot]`.
  // Returns null when neither resolves — the signal to leave htmx's default
  // (no swap) in place, so a 4xx with no opted-in slot is discarded exactly as
  // before and this handler changes nothing for it.
  function resolveErrorSlot(elt) {
    if (!elt || elt.nodeType !== 1) {
      return null;
    }
    var explicit = elt.getAttribute("data-error-target");
    if (explicit) {
      return document.querySelector(explicit);
    }
    var form = elt.closest("form");
    if (!form) {
      return null;
    }
    return form.querySelector("[data-error-slot]");
  }

  document.body.addEventListener("htmx:beforeSwap", function (evt) {
    var xhr = evt.detail.xhr;
    // Only client-error responses (400..=499). A 2xx (incl. the OOB create-card
    // success) and a 5xx are none of this handler's business.
    if (!xhr || xhr.status < 400 || xhr.status > 499) {
      return;
    }
    var slot = resolveErrorSlot(evt.detail.requestConfig.elt);
    if (!slot) {
      // No opted-in slot: leave htmx's default (do not swap the 4xx body). This
      // is what scopes the fix — unrelated 4xx responses are unaffected.
      return;
    }
    // Route the 4xx fragment into the slot as an ordinary swap: the form (and its
    // dialog) stay mounted, only the slot fills, so Mei sees the reason and can
    // fix and resubmit without a reload.
    evt.detail.shouldSwap = true;
    evt.detail.target = slot;
    evt.detail.isError = false;
  });

  markReady();
})();
