// issue-status-move slice 02 — native HTML5 drag-and-drop for board cards.
//
// App-owned, self-contained, CSP-safe: loaded as an external same-origin script
// (no inline handlers), all wiring via addEventListener. Progressive
// enhancement — without this script the cards carry `draggable="true"` from the
// server but nothing accepts a drop, so the board is unchanged and the edit
// dialog (slice 01) remains the no-JS status path.
//
// On drop the card is moved optimistically into the target column, then the new
// state is POSTed to the card's `data-state-url`. The CSRF double-submit token
// rides the `x-csrf-token` header, read from the non-HttpOnly `foundry_csrf`
// cookie (csrf.rs accepts the header form). A non-2xx response or a network
// error reverts the card to its origin column.
(function () {
  "use strict";

  function readCookie(name) {
    var prefix = name + "=";
    var parts = document.cookie ? document.cookie.split(";") : [];
    for (var i = 0; i < parts.length; i++) {
      var part = parts[i].trim();
      if (part.indexOf(prefix) === 0) {
        return part.substring(prefix.length);
      }
    }
    return "";
  }

  function init() {
    var dragged = null;
    var origin = null;

    // Delegated on document so htmx-appended cards (dialog relocation, new
    // issue) are draggable without re-wiring.
    document.addEventListener("dragstart", function (event) {
      var target = event.target;
      var card = target && target.closest ? target.closest(".issue-card") : null;
      if (!card) {
        return;
      }
      dragged = card;
      origin = card.parentElement;
      if (event.dataTransfer) {
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData(
          "text/plain",
          card.getAttribute("data-issue-key") || ""
        );
      }
    });

    var columns = document.querySelectorAll("[data-column]");
    for (var c = 0; c < columns.length; c++) {
      var column = columns[c];

      column.addEventListener("dragover", function (event) {
        event.preventDefault();
        if (event.dataTransfer) {
          event.dataTransfer.dropEffect = "move";
        }
      });

      column.addEventListener("drop", function (event) {
        event.preventDefault();
        var card = dragged;
        var from = origin;
        var into = event.currentTarget;
        dragged = null;
        origin = null;
        if (!card || !into) {
          return;
        }
        var stateUrl = card.getAttribute("data-state-url");
        var slug = into.getAttribute("data-column");
        if (!stateUrl || !slug) {
          return;
        }
        // Optimistic move — land the card in the target column immediately.
        into.appendChild(card);
        fetch(stateUrl, {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/x-www-form-urlencoded",
            "x-csrf-token": readCookie("foundry_csrf")
          },
          body: "state=" + encodeURIComponent(slug)
        })
          .then(function (response) {
            if (!response.ok && from) {
              from.appendChild(card); // revert on refusal
            }
          })
          .catch(function () {
            if (from) {
              from.appendChild(card); // revert on network error
            }
          });
      });
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
