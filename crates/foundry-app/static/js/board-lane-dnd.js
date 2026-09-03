// board-lane-reorder — Pointer Events drag for board LANES (ADR-BOARD-LANE-007).
//
// Deliberately NOT the mechanism `board-dnd.js` uses for cards. Native HTML5
// drag-and-drop emits NOTHING on touch, so a lane drag built that way would be
// inert on a phone — days after fix-lane-menu-clipped-mobile shipped to make
// this board usable on one. The two modules share a DOM region and no code:
// a gesture beginning on `.issue-card` is a card move, one beginning on a
// column header (`[data-lane-drag]`) is a lane move, and neither ever becomes
// the other.
//
// Behaviour is copied from board-dnd.js on purpose so the two drags feel the
// same to the hand: optimistic DOM move on release, ONE POST naming the
// destination NEIGHBOUR by slug (never a numeric index — an index captured at
// drag-start is stale the instant another operator inserts a lane), and a
// revert to the EXACT origin slot on a non-2xx response or a network error.
//
// Escape does not have a listener here. It is an ARM of
// keyboard.js::closeTopLayer() (BR-4: exactly one Escape owner), which finds an
// in-flight drag by its DOM marker and dispatches `foundry:cancel-lane-drag`.
(function () {
  "use strict";

  // Below this many pixels of travel the gesture is a click, not a drag — which
  // is what keeps the ⋯ trigger and everything else in the header clickable.
  var THRESHOLD = 6;
  // How close to the board's edge the pointer must be before the board scrolls
  // under it, and how fast. Without this a lane cannot reach an off-screen
  // destination at all, which on a 390px board is most of them.
  var EDGE_ZONE = 48;
  var EDGE_STEP = 14;

  var drag = null;

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

  function board() {
    return document.getElementById("board-columns");
  }

  function columns() {
    var b = board();
    return b ? Array.prototype.slice.call(b.querySelectorAll("section.column")) : [];
  }

  // The column the dragged one should be inserted BEFORE, given the pointer X:
  // the first column whose horizontal midpoint is right of the cursor. The
  // dragged column itself is skipped. Returns null when the cursor is past the
  // last column (append to the end).
  function insertBeforeTarget(x) {
    var cols = columns();
    for (var i = 0; i < cols.length; i++) {
      if (cols[i] === drag.column) {
        continue;
      }
      var rect = cols[i].getBoundingClientRect();
      if (x < rect.left + rect.width / 2) {
        return cols[i];
      }
    }
    return null;
  }

  function showIndicator(before) {
    if (!drag.indicator) {
      drag.indicator = document.createElement("div");
      drag.indicator.className = "lane-drop-indicator";
      drag.indicator.setAttribute("data-lane-drop-indicator", "");
    }
    var b = board();
    if (!b) {
      return;
    }
    if (before) {
      b.insertBefore(drag.indicator, before);
    } else {
      b.appendChild(drag.indicator);
    }
  }

  function clearIndicator(g) {
    if (g && g.indicator && g.indicator.parentNode) {
      g.indicator.parentNode.removeChild(g.indicator);
    }
  }

  // Scroll the BOARD, never the page (AC-3.2), and never past its own extent.
  function autoScroll(x) {
    var b = board();
    if (!b) {
      return;
    }
    var rect = b.getBoundingClientRect();
    var max = b.scrollWidth - b.clientWidth;
    if (x > rect.right - EDGE_ZONE) {
      b.scrollLeft = Math.min(max, b.scrollLeft + EDGE_STEP);
    } else if (x < rect.left + EDGE_ZONE) {
      b.scrollLeft = Math.max(0, b.scrollLeft - EDGE_STEP);
    }
  }

  function begin() {
    drag.started = true;
    drag.column.classList.add("lane-dragging");
    // DOM-derived drag state (ADR-BOARD-LANE-005 rule 2): a stored handle would
    // be left detached by the out-of-band #board-columns swap, and Escape would
    // then no-op with a drag on screen.
    drag.column.setAttribute("data-lane-dragging", "");
  }

  // Put the column back exactly where it started. Used by every exit path that
  // is not a successful drop: Escape, pointercancel, a refusal, a network
  // error.
  function revert(g) {
    if (g && g.column && g.origin) {
      g.origin.insertBefore(g.column, g.originNext);
    }
  }

  function settle(g) {
    if (g && g.column) {
      g.column.classList.remove("lane-dragging");
      g.column.removeAttribute("data-lane-dragging");
    }
  }

  function finish() {
    clearIndicator(drag);
    settle(drag);
    drag = null;
  }

  // Swap the board with the fragment the move route returns (the same
  // out-of-band envelope the lane dialogs answer with).
  function applyBoard(markup) {
    var host = document.createElement("template");
    host.innerHTML = markup.trim();
    var next = host.content.querySelector("#board-columns");
    var current = board();
    if (next && current) {
      current.replaceWith(next);
    }
  }

  function postMove(moveUrl, beforeSlug) {
    return fetch(moveUrl, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "x-csrf-token": readCookie("foundry_csrf")
      },
      body: "before=" + encodeURIComponent(beforeSlug || "")
    });
  }

  // The ⋯ menu's two Move items. They share this module rather than htmx so
  // both surfaces take the SAME client path to the SAME server seam (DDD-8) —
  // and so the CSRF token travels the cookie->header route the card drag has
  // shipped on since issue-status-move.
  document.addEventListener("click", function (event) {
    var target = event.target;
    // Match the MENU ITEMS only. `[data-lane-move-url]` also sits on
    // `section.column` (the drag reads it there), so selecting on that
    // attribute makes `closest()` walk up from ANY click inside a column —
    // including the ⋯ trigger, and including the click a drag's own pointerup
    // synthesises — and fire a move with no destination, sending the lane to
    // the end. Measured: it broke the menu, the threshold scenario and all
    // three drags at once.
    var item =
      target && target.closest
        ? target.closest('[data-action="move-lane-left"], [data-action="move-lane-right"]')
        : null;
    if (!item || item.disabled || !item.hasAttribute("data-lane-move-url")) {
      return;
    }
    event.preventDefault();
    postMove(item.getAttribute("data-lane-move-url"), item.getAttribute("data-lane-move-before"))
      .then(function (response) {
        if (!response.ok) {
          return null;
        }
        return response.text();
      })
      .then(function (markup) {
        if (markup) {
          applyBoard(markup);
        }
      })
      .catch(function () {
        /* a failed move leaves the board exactly as it was */
      });
  });

  document.addEventListener("pointerdown", function (event) {
    if (event.button !== 0 && event.pointerType === "mouse") {
      return;
    }
    var target = event.target;
    var handle = target && target.closest ? target.closest("[data-lane-drag]") : null;
    if (!handle) {
      return;
    }
    var column = handle.closest("section.column");
    if (!column) {
      return;
    }
    drag = {
      pointerId: event.pointerId,
      column: column,
      handle: handle,
      origin: column.parentElement,
      // Remember the exact origin slot so a refusal can restore it precisely.
      originNext: column.nextElementSibling,
      startX: event.clientX,
      started: false,
      indicator: null
    };
  });

  document.addEventListener("pointermove", function (event) {
    if (!drag || event.pointerId !== drag.pointerId) {
      return;
    }
    if (!drag.started) {
      if (Math.abs(event.clientX - drag.startX) < THRESHOLD) {
        return; // still a click
      }
      begin();
    }
    // Once dragging, stop the gesture scrolling the page under our feet.
    if (event.cancelable) {
      event.preventDefault();
    }
    autoScroll(event.clientX);
    showIndicator(insertBeforeTarget(event.clientX));
  });

  document.addEventListener("pointerup", function (event) {
    if (!drag || event.pointerId !== drag.pointerId) {
      return;
    }
    if (!drag.started) {
      drag = null; // a click; leave it to whatever it landed on
      return;
    }
    var column = drag.column;
    var b = board();
    var before = insertBeforeTarget(event.clientX);
    clearIndicator(drag);
    if (!b) {
      finish();
      return;
    }
    // Optimistic move — land the column at the exact slot under the cursor.
    if (before) {
      b.insertBefore(column, before);
    } else {
      b.appendChild(column);
    }
    var moveUrl = column.getAttribute("data-lane-move-url");
    var mover = column.getAttribute("data-column");
    if (!moveUrl || !mover) {
      finish();
      return;
    }
    // Name the destination NEIGHBOUR, resolved AFTER the optimistic move so it
    // is the column we now precede. Empty means "place last" (D7).
    var next = column.nextElementSibling;
    while (next && !next.hasAttribute("data-column")) {
      next = next.nextElementSibling;
    }
    var beforeSlug = next ? next.getAttribute("data-column") || "" : "";
    var pending = drag;
    postMove(moveUrl, beforeSlug)
      .then(function (response) {
        if (!response.ok) {
          revert(pending); // back to the exact origin slot
        }
      })
      .catch(function () {
        revert(pending);
      });
    // The gesture is over the moment the pointer lifts — the request outcome
    // only decides whether the optimistic move survives. Settling here (rather
    // than in the fetch callback) means a second drag can start immediately,
    // and the callbacks above touch only their OWN captured gesture.
    settle(pending);
    drag = null;
  });

  document.addEventListener("pointercancel", function (event) {
    if (!drag || event.pointerId !== drag.pointerId) {
      return;
    }
    if (drag.started) {
      revert(drag);
    }
    finish();
  });

  // Escape's ONLY owner is keyboard.js::closeTopLayer(); it finds an in-flight
  // drag by its DOM marker and dispatches this. A `keydown` listener here would
  // race that one for the same press and peel two layers — BR-4's failure.
  document.addEventListener("foundry:cancel-lane-drag", function () {
    if (!drag) {
      return;
    }
    if (drag.started) {
      revert(drag);
    }
    finish();
  });
})();
