// issue-status-move slice 02 — native HTML5 drag-and-drop for board cards.
//
// App-owned, self-contained, CSP-safe: loaded as an external same-origin script
// (no inline handlers), all wiring via addEventListener. Progressive
// enhancement — without this script the cards carry `draggable="true"` from the
// server but nothing accepts a drop, so the board is unchanged and the edit
// dialog (slice 01) remains the no-JS status path.
//
// On drop the card is moved optimistically to the exact slot the marker showed
// (within its own column or into another), then `state` + `after` are POSTed to
// the card's `data-state-url` (card-ranking-within-status, ADR-002). `after` is
// the `data-issue-key` of the card now immediately ABOVE the dropped card
// (omitted when dropped at the top). The CSRF double-submit token rides the
// `x-csrf-token` header, read from the non-HttpOnly `foundry_csrf` cookie
// (csrf.rs accepts the header form). A non-2xx response or a network error
// reverts the card to its exact origin slot.
//
// card-drag-drop-feedback (ADR-BOARD-CARD-001): every listener is delegated on
// `document` and resolves `#board-columns` and the lane from the live document
// at event time. No listener is bound to any node inside `#board-columns`, so an
// in-place board refresh (htmx OOB swap, `applyBoard`) needs no re-wiring. The
// origin is held as identity (keys and a lane slug), never as nodes, and is
// resolved against the live board when a refused move is reverted.
//
// Lane activation (ADR-BOARD-CARD-002 rules 1-3): `dragover` is the only
// activator. Each session dragover lights the lane under the pointer with
// `data-card-drop-target` and clears every other lane found by query; a
// dragover over no lane clears them all. A dragleave into nothing (the pointer
// left the window) clears on the next animation frame unless a dragover comes
// first. A drag with no session never lights a lane.
//
// The slot marker (ADR-BOARD-CARD-002 rules 5-8): each session dragover over a
// lane computes ONE slot with `slotFor` and shows it as the board's only
// `[data-card-drop-marker]`, in the gap above the card the slot precedes or
// below the last card; its `data-before-key` names that card ("" is the end).
// It is absolutely positioned, so it takes no space: showing it moves no card,
// and a still pointer cannot move it. A drop lands the card at the slot of the
// live marker in the drop lane, so the marker, the landing and the POST's
// `after` never disagree. A session dragover over no lane removes it, and every
// ending of the drag removes it (rule 9, the session's `end()`). Not shared
// with board-lane-dnd.js's drop indicator (ADR-BOARD-LANE-007).
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

  // The board's DOM hooks: a card, a lane, and the key a card carries.
  var CARD = ".issue-card";
  var LANE = "[data-column]";
  var KEY = "data-issue-key";

  // The live `#board-columns` an event happened in, or null off the board (the
  // script is loaded on every page, base.html).
  function boardOf(target) {
    return target && target.closest ? target.closest("#board-columns") : null;
  }

  // A card's `data-issue-key`, or "" when it has none.
  function keyOf(card) {
    return card.getAttribute(KEY) || "";
  }

  // The first element under `root` matching `selector` whose `attr` equals
  // `value`, compared as a string (no selector built from data).
  function findByAttr(root, selector, attr, value) {
    if (!root || !value) {
      return null;
    }
    var nodes = root.querySelectorAll(selector);
    for (var i = 0; i < nodes.length; i++) {
      if (nodes[i].getAttribute(attr) === value) {
        return nodes[i];
      }
    }
    return null;
  }

  // The cards in `lane` in board order, less the dragged `card` (it may still be
  // in this lane), so its own slot is never a neighbour.
  function otherCards(lane, card) {
    var cards = lane.querySelectorAll(CARD);
    var others = [];
    for (var i = 0; i < cards.length; i++) {
      if (cards[i] !== card) {
        others.push(cards[i]);
      }
    }
    return others;
  }

  // The slot `y` points at in `lane`, as the card that slot precedes: the first
  // card other than the dragged `card` whose vertical midpoint is below `y`, or
  // null past the last card (the end).
  function slotFor(lane, y, card) {
    var others = otherCards(lane, card);
    for (var i = 0; i < others.length; i++) {
      var rect = others[i].getBoundingClientRect();
      if (y < rect.top + rect.height / 2) {
        return others[i];
      }
    }
    return null;
  }

  // The nearest card beside `card` through the sibling property `step`
  // ("previousElementSibling" or "nextElementSibling"), skipping any non-card
  // sibling (the placeholder, the marker); null when there is none.
  function nearestCard(card, step) {
    var sibling = card[step];
    while (sibling && sibling.className.indexOf("issue-card") === -1) {
      sibling = sibling[step];
    }
    return sibling;
  }

  // The key of the card immediately above `card` in its column, or "" when
  // `card` is at the top.
  function neighbourAbove(card) {
    var above = nearestCard(card, "previousElementSibling");
    return above ? keyOf(above) : "";
  }

  // The key of the card immediately below `card`, or "" at the end.
  function neighbourBelow(card) {
    var below = nearestCard(card, "nextElementSibling");
    return below ? keyOf(below) : "";
  }

  var ACTIVATED = "data-card-drop-target";

  // Light exactly `lane`, or no lane when it is null. Every other lit lane is
  // found by query in the live document; the DOM is written only on a change.
  function activate(lane) {
    var lit = document.querySelectorAll("[" + ACTIVATED + "]");
    for (var i = 0; i < lit.length; i++) {
      if (lit[i] !== lane) {
        lit[i].removeAttribute(ACTIVATED);
      }
    }
    if (lane && !lane.hasAttribute(ACTIVATED)) {
      lane.setAttribute(ACTIVATED, "");
    }
  }

  var MARKER = "data-card-drop-marker";
  // The key of the card the marker's slot precedes ("" is the end).
  var BEFORE_KEY = "data-before-key";
  // Half the 8px gap every card keeps below it (and the lane header above the
  // first card): the marker's midline sits in the middle of that gap.
  var HALF_GAP = 4;

  // The viewport Y of the middle of the gap the slot before `before` (null: the
  // end) occupies in `lane`, ignoring the dragged `card`.
  function slotMidline(lane, before, card) {
    if (before) {
      return before.getBoundingClientRect().top - HALF_GAP;
    }
    var others = otherCards(lane, card);
    if (others.length) {
      return others[others.length - 1].getBoundingClientRect().bottom + HALF_GAP;
    }
    var header = lane.querySelector("h3");
    var edge = header ? header.getBoundingClientRect().bottom : lane.getBoundingClientRect().top;
    return edge + HALF_GAP;
  }

  // Keep at most one marker, the first found inside `lane`, and remove every
  // other marker in the live document (all of them when `lane` is null). The
  // kept marker, or null.
  function keepOneMarkerIn(lane) {
    var found = document.querySelectorAll("[" + MARKER + "]");
    var kept = null;
    for (var i = 0; i < found.length; i++) {
      if (!kept && lane && found[i].parentNode === lane) {
        kept = found[i];
      } else {
        found[i].parentNode.removeChild(found[i]);
      }
    }
    return kept;
  }

  // Show the slot before `before` (null: the end) as the board's only marker,
  // inside `lane`; with no lane, show none. Every marker is found by query in the
  // live document, never through a stored handle; the DOM is written only on a
  // change, so a still pointer leaves the marker untouched.
  function showMarker(lane, before, card) {
    var marker = keepOneMarkerIn(lane);
    if (!lane) {
      return;
    }
    if (!marker) {
      marker = document.createElement("div");
      marker.setAttribute(MARKER, "");
      marker.setAttribute("aria-hidden", "true");
      lane.appendChild(marker);
    }
    var key = before ? keyOf(before) : "";
    if (marker.getAttribute(BEFORE_KEY) !== key) {
      marker.setAttribute(BEFORE_KEY, key);
    }
    var box = lane.getBoundingClientRect();
    var top = Math.round(slotMidline(lane, before, card) - box.top - lane.clientTop) + "px";
    if (marker.style.top !== top) {
      marker.style.top = top;
    }
  }

  // Where a card came from, as identity only: its key, its lane's slug and the
  // keys of its neighbours. Resolved against the live board when it is needed.
  function Origin(card) {
    var lane = card.closest(LANE);
    this.key = keyOf(card);
    this.laneSlug = lane ? lane.getAttribute("data-column") || "" : "";
    this.nextKey = neighbourBelow(card);
    this.prevKey = neighbourAbove(card);
  }

  // Put the card back in its origin slot on the board as it is NOW: by the next
  // key, else just after the previous key, else at the end of the origin lane.
  // Does nothing when the card or its lane is no longer on the live board.
  Origin.prototype.restore = function () {
    var board = document.getElementById("board-columns");
    var card = findByAttr(board, CARD, KEY, this.key);
    var lane = findByAttr(board, LANE, "data-column", this.laneSlug);
    if (!card || !lane) {
      return;
    }
    var next = findByAttr(lane, CARD, KEY, this.nextKey);
    if (next && next !== card) {
      lane.insertBefore(card, next);
      return;
    }
    var prev = findByAttr(lane, CARD, KEY, this.prevKey);
    if (prev && prev !== card) {
      lane.insertBefore(card, prev.nextSibling);
      return;
    }
    lane.appendChild(card);
  };

  // One card drag on this page: the dragged card and where it came from. It
  // never holds a lane.
  function CardDragSession(card) {
    this.card = card;
    this.origin = new Origin(card);
  }

  // The card a drop into `lane` lands before (null: the end): the slot the live
  // marker in that lane shows, or — a drop with no dragover before it, so no
  // marker — the slot under `y`.
  CardDragSession.prototype.landingIn = function (lane, y) {
    var marker = lane.querySelector("[" + MARKER + "]");
    if (!marker) {
      return slotFor(lane, y, this.card);
    }
    return findByAttr(lane, CARD, KEY, marker.getAttribute(BEFORE_KEY) || "");
  };

  // The move's form body: the destination lane's `state` and `after`, the key of
  // the card now immediately above the dropped one (omitted at the top).
  function moveBody(slug, after) {
    var body = "state=" + encodeURIComponent(slug);
    if (after) {
      body += "&after=" + encodeURIComponent(after);
    }
    return body;
  }

  // Optimistically land the card in `lane` before `before` (null: at the end)
  // and send the move. A refusal or a network error restores the origin at
  // response time.
  CardDragSession.prototype.dropInto = function (lane, before) {
    var card = this.card;
    var origin = this.origin;
    var stateUrl = card.getAttribute("data-state-url");
    var slug = lane.getAttribute("data-column");
    if (!stateUrl || !slug) {
      return;
    }
    if (before) {
      lane.insertBefore(card, before);
    } else {
      lane.appendChild(card);
    }
    var body = moveBody(slug, neighbourAbove(card));
    fetch(stateUrl, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "x-csrf-token": readCookie("foundry_csrf")
      },
      body: body
    })
      .then(function (response) {
        if (!response.ok) {
          origin.restore(); // revert to exact origin slot
        }
      })
      .catch(function () {
        origin.restore(); // revert on network error
      });
  };

  // End this drag. Idempotent, and the single teardown owner: whatever a drag
  // leaves on the board is found by querying the live DOM, never through a
  // stored handle. Every activated lane goes dark and every marker is removed
  // (ADR-BOARD-CARD-002 rules 4 and 9) — at a drop before the move is sent, at
  // dragend, and at a new dragstart.
  CardDragSession.prototype.end = function () {
    activate(null);
    showMarker(null, null, null);
  };

  function init() {
    // The one drag begun on an `.issue-card` on this page, if any. Every other
    // drag (a file, a text selection, a card from another tab) has no session
    // and is foreign: `dataTransfer.types` is never inspected.
    var session = null;
    // The next-frame clear a leave into nothing scheduled, if any.
    var pendingClear = 0;

    function cancelPendingClear() {
      if (pendingClear) {
        window.cancelAnimationFrame(pendingClear);
      }
      pendingClear = 0;
    }

    function endSession() {
      if (session) {
        session.end();
      }
      session = null;
    }

    document.addEventListener("dragstart", function (event) {
      var board = boardOf(event.target);
      if (!board) {
        return;
      }
      var card = event.target.closest(CARD);
      if (!card) {
        return;
      }
      endSession();
      session = new CardDragSession(card);
      if (event.dataTransfer) {
        event.dataTransfer.effectAllowed = "move";
        event.dataTransfer.setData("text/plain", keyOf(card));
      }
    });

    // Inside `#board-columns` every drag is claimed: a card drag may move, a
    // foreign one is swallowed (`none`) so the browser never opens it here.
    // Outside `#board-columns` the browser default applies. A card drag lights
    // the lane under the pointer and marks the slot it points at; over no lane
    // (off the board too) nothing is lit or marked.
    document.addEventListener("dragover", function (event) {
      var board = boardOf(event.target);
      if (session) {
        cancelPendingClear();
        var lane = board ? event.target.closest(LANE) : null;
        activate(lane);
        showMarker(lane, lane ? slotFor(lane, event.clientY, session.card) : null, session.card);
      }
      if (!board) {
        return;
      }
      event.preventDefault();
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = session ? "move" : "none";
      }
    });

    // The landing slot is read from the live marker BEFORE the session ends.
    document.addEventListener("drop", function (event) {
      if (!boardOf(event.target)) {
        return;
      }
      event.preventDefault();
      var current = session;
      var lane = current ? event.target.closest(LANE) : null;
      var before = lane ? current.landingIn(lane, event.clientY) : null;
      endSession();
      if (!lane) {
        return; // foreign (swallowed, never acted on), or no lane
      }
      current.dropInto(lane, before);
    });

    // A card drag leaving into nothing (`relatedTarget` null) may have left the
    // window: clear on the next frame, unless a dragover cancels it first.
    document.addEventListener("dragleave", function (event) {
      if (!boardOf(event.target) || !session || event.relatedTarget) {
        return;
      }
      cancelPendingClear();
      pendingClear = window.requestAnimationFrame(function () {
        pendingClear = 0;
        activate(null);
      });
    });

    // Every drag end (a drop, Escape, a release outside any lane) clears the
    // in-flight drag, wherever the dragged card now is.
    document.addEventListener("dragend", endSession);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
