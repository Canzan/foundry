// issue-status-move slice 02 — drag-and-drop for board cards; since
// card-pointer-drag (ADR-BOARD-CARD-004) the gesture runs on Pointer Events.
//
// App-owned, self-contained, CSP-safe: loaded as an external same-origin script
// (no inline handlers), all wiring via addEventListener. Progressive
// enhancement — without this script nothing lifts a card, so the board is
// unchanged and the edit dialog (slice 01) remains the no-JS status path.
//
// On release the card is moved optimistically to the exact slot the marker
// showed (within its own column or into another), then `state` + `after` are
// POSTed to the card's `data-state-url` (card-ranking-within-status, ADR-002).
// `after` is the `data-issue-key` of the card now immediately ABOVE the dropped
// card (omitted when dropped at the top). The CSRF double-submit token rides the
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
// The gesture (ADR-BOARD-CARD-004, DDD-26): a primary-button mouse press on an
// `.issue-card` inside `#board-columns` that travels past THRESHOLD lifts the
// card; a touch or pen press on the card's grip (`[data-card-grip]`) lifts it
// past GRIP_THRESHOLD, with no timer. A touch on the grip is claimed at
// `touchstart` (DDD-25) and a click on the grip never opens the card (DDD-28).
// A touch or pen press on the card's text (the body) lifts after HOLD_MS held
// within HOLD_TOLERANCE; while the hold is pending the card carries
// `data-card-arming` (DDD-27). Before the lift a move past the tolerance, a
// `touchmove` past it, any `scroll` or a `pointercancel` aborts the hold and
// leaves the gesture to the browser; a release first is a tap that opens the
// card. After a body lift a non-passive `touchmove` guard keeps the page
// still (DDD-4), and a release before the card was carried anywhere lands
// nowhere.
// From the lift, `<html>` carries `data-card-dragging`, the origin card
// stays in its slot marked `data-card-lifted`, and one fixed clone (the ghost,
// `pointer-events: none`) follows the pointer. The lane and slot are resolved
// from the POINT (`document.elementFromPoint`), never from `event.target`,
// which a touch pointer captures to the origin card (DDD-3). A release over a
// lane lands at the live marker; a release off every lane, Escape, a card a
// replace detached mid-drag, or a `pointercancel` after the lift ends as
// cancelled through the one teardown (`end()`) and sends nothing; a
// `pointercancel` before the lift only abandons the press (DDD-8). A
// release after a lift arms a one-shot click guard so the drag never opens the
// card; the next press resets it (DDD-6).
//
// Edge auto-scroll (DDD-11, AC-3.1-3.4): while a carried card is held within
// EDGE_ZONE of the board's side edge, `#board-columns` scrolls EDGE_STEP a
// frame toward it, clamped to its extent; near the viewport's top or bottom
// the page scrolls. It runs from the last carried point, so a still finger
// keeps it going, and after every step the lane and marker are resolved again
// from that point. It stops in `end()`, on every exit.
//
// Cards keep `draggable="true"` (issue-status-move), so the browser would start
// its OWN drag of them and take the pointer away (`pointercancel`, spike Q1):
// that `dragstart` is cancelled (DDD-19). HTML5 drag-and-drop stays only to
// swallow foreign drags — a file, a text selection, a card from another tab —
// inside `#board-columns` (DDD-1).
//
// Escape has no listener here. It is an ARM of keyboard.js::closeTopLayer()
// (BR-4: exactly one Escape owner), which finds the drag by
// `html[data-card-dragging]` and dispatches `foundry:cancel-card-drag` (DDD-7).
//
// Lane activation (ADR-BOARD-CARD-002 rules 1-3): each lifted move lights the
// lane under the pointer with `data-card-drop-target` and clears every other
// lane found by query; a move over no lane clears them all. No press that has
// not lifted ever lights a lane.
//
// The slot marker (ADR-BOARD-CARD-002 rules 5-8): each lifted move over a lane
// computes ONE slot with `slotFor` and shows it as the board's only
// `[data-card-drop-marker]`, in the gap above the card the slot precedes or
// below the last card; its `data-before-key` names that card ("" is the end).
// It is absolutely positioned, so it takes no space: showing it moves no card,
// and a still pointer cannot move it. A release lands the card at the slot of
// the live marker in the release lane, so the marker, the landing and the
// POST's `after` never disagree. A move over no lane removes it, and every
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

  // One lifted card drag on this page: the dragged card and where it came from.
  // It never holds a lane.
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

  // DDD-7 / DDD-17 hooks. `<html>` marks a drag in flight (the Escape arm finds
  // it there, where no board replace can detach it); the origin card is marked
  // lifted and stays in its slot; the ghost is a fixed clone under the pointer.
  var DRAGGING = "data-card-dragging";
  var LIFTED = "data-card-lifted";
  // A touch or pen body hold is pending on this card (DDD-27).
  var ARMING = "data-card-arming";

  // Clear the arming cue from every card, found by query (rule 9's idiom).
  function clearArming() {
    var arming = document.querySelectorAll("[" + ARMING + "]");
    for (var i = 0; i < arming.length; i++) {
      arming[i].removeAttribute(ARMING);
    }
  }
  var GHOST = "data-card-ghost";
  // The ghost sits this far up-left of where the card was grabbed, so the hand
  // (a thumb, on touch) does not hide it.
  var GHOST_OFFSET = 12;
  // The attributes a clone must not carry: it is a picture of the card, never a
  // second card (no key, no id, no htmx wiring, not draggable).
  var GHOST_STRIPPED = [
    "id", KEY, "draggable", "data-state-url", "hx-get", "hx-target", "hx-swap",
    "aria-selected", "style", LIFTED, ARMING
  ];

  // Lift the card: mark the drag in flight, dim the origin in place, and put
  // the ghost under the pointer at (`x`, `y`), keeping where it was grabbed.
  // The arming cue is cleared first: the card is measured with its transform,
  // and a ghost measured mid-cue would be too small (DDD-27).
  CardDragSession.prototype.lift = function (x, y) {
    var card = this.card;
    clearArming();
    this.carried = false;
    var rect = card.getBoundingClientRect();
    var ghost = card.cloneNode(true);
    for (var i = 0; i < GHOST_STRIPPED.length; i++) {
      ghost.removeAttribute(GHOST_STRIPPED[i]);
    }
    ghost.className = "card-drag-ghost";
    ghost.setAttribute(GHOST, "");
    ghost.setAttribute("aria-hidden", "true");
    ghost.style.width = rect.width + "px";
    this.grabX = x - rect.left + GHOST_OFFSET;
    this.grabY = y - rect.top + GHOST_OFFSET;
    document.documentElement.setAttribute(DRAGGING, "");
    card.setAttribute(LIFTED, "");
    document.body.appendChild(ghost);
    this.follow(x, y);
    this.ended = false;
    this.scrollEdges();
  };

  // Carry the ghost to the pointer at (`x`, `y`). Found by query, like every
  // other thing a drag leaves on the page.
  CardDragSession.prototype.follow = function (x, y) {
    var ghost = document.querySelector("[" + GHOST + "]");
    if (ghost) {
      ghost.style.left = Math.round(x - this.grabX) + "px";
      ghost.style.top = Math.round(y - this.grabY) + "px";
    }
  };

  // The lane under the point (`x`, `y`) on the live board, or null. The ghost
  // takes no pointer events, so the point reads what is beneath it (DDD-3).
  function laneAt(x, y) {
    var under = document.elementFromPoint(x, y);
    return boardOf(under) ? under.closest(LANE) : null;
  }

  // How close to an edge a carried card must be held before the view scrolls
  // under it, and by how much per step (the lane drag's constants,
  // board-lane-dnd.js; copied, not shared, DDD-18).
  var EDGE_ZONE = 48;
  var EDGE_STEP = 14;

  // One step of `scroller`'s `axis` ("scrollLeft" or "scrollTop") away from the
  // nearer edge when `at` is within EDGE_ZONE of `low` or `high`, clamped to
  // [0, `max`]. True when it scrolled.
  function edgeStep(scroller, axis, at, low, high, max) {
    var was = scroller[axis];
    if (at > high - EDGE_ZONE) {
      scroller[axis] = Math.min(max, was + EDGE_STEP);
    } else if (at < low + EDGE_ZONE) {
      scroller[axis] = Math.max(0, was - EDGE_STEP);
    }
    return scroller[axis] !== was;
  }

  // Scroll what a card held at (`x`, `y`) is near the edge of: the board
  // sideways (never the page, AC-3.2) and the page up or down (DDD-11). The
  // edges are measured inside the viewport, where the point can still be read
  // (`elementFromPoint` is null past it, spike Q4). True when anything moved.
  function edgeScroll(x, y) {
    var moved = false;
    var board = document.getElementById("board-columns");
    if (board) {
      var rect = board.getBoundingClientRect();
      moved = edgeStep(board, "scrollLeft", x, Math.max(rect.left, 0),
        Math.min(rect.right, window.innerWidth), board.scrollWidth - board.clientWidth);
    }
    var page = document.scrollingElement || document.documentElement;
    return edgeStep(page, "scrollTop", y, 0, window.innerHeight,
      page.scrollHeight - page.clientHeight) || moved;
  }

  // While the session lasts, step the edge scroll once a frame from the last
  // point the card was carried to: a still finger sends no `pointermove`, so
  // after every step the lane and marker are resolved again from that point.
  // Auto-scroll changes what is visible, never what the marker addresses.
  CardDragSession.prototype.scrollEdges = function () {
    var session = this;
    session.frame = requestAnimationFrame(function () {
      if (session.ended) {
        return;
      }
      if (session.carried && edgeScroll(session.x, session.y)) {
        session.track(session.x, session.y);
      }
      session.scrollEdges();
    });
  };

  // Light the lane under (`x`, `y`) and mark the slot it points at; over no
  // lane, nothing is lit or marked.
  CardDragSession.prototype.track = function (x, y) {
    this.carried = true;
    this.x = x;
    this.y = y;
    var lane = laneAt(x, y);
    activate(lane);
    showMarker(lane, lane ? slotFor(lane, y, this.card) : null, this.card);
  };

  // End this drag. Idempotent, and the single teardown owner: whatever a drag
  // leaves on the page is found by querying the live DOM, never through a
  // stored handle. Every activated lane goes dark, every marker and ghost is
  // removed, and nothing is marked lifted or in flight (ADR-BOARD-CARD-002
  // rules 4 and 9) — at a release before the move is sent, and at a cancel.
  // The edge scroller stops here, so no scroll outlives the session.
  CardDragSession.prototype.end = function () {
    this.ended = true;
    if (this.frame) {
      cancelAnimationFrame(this.frame);
      this.frame = 0;
    }
    clearArming();
    activate(null);
    showMarker(null, null, null);
    var left = document.querySelectorAll("[" + GHOST + "], [" + LIFTED + "]");
    for (var i = 0; i < left.length; i++) {
      if (left[i].hasAttribute(GHOST)) {
        left[i].parentNode.removeChild(left[i]);
      } else {
        left[i].removeAttribute(LIFTED);
      }
    }
    document.documentElement.removeAttribute(DRAGGING);
  };

  // Below this many pixels of travel a mouse press is a click, not a drag (the
  // lane drag's threshold, board-lane-dnd.js; DDD-5).
  var THRESHOLD = 6;
  // A touch or pen press on the grip lifts after this much travel, with no
  // timer: 3, not 0, so a tap never flashes a ghost and pen jitter is not a
  // drag (DDD-26).
  var GRIP_THRESHOLD = 3;
  // A touch or pen press on the card's text lifts after this hold, held within
  // HOLD_TOLERANCE px (DDD-5, DDD-26). 10 px is under Chrome's ~15 px touch
  // slop, so no scroll starts before the hold decides. The hold is written
  // once to `<html>` as `--card-hold-ms`, the arming cue's duration (DDD-27).
  var HOLD_MS = 500;
  var HOLD_TOLERANCE = 10;
  var GRIP = "[data-card-grip]";

  // The own card an event began on, or null: an `.issue-card` inside
  // `#board-columns`.
  function ownCard(target) {
    return boardOf(target) ? target.closest(CARD) : null;
  }

  // True when `target` is inside the grip of an own card.
  function onOwnGrip(target) {
    var grip = target && target.closest ? target.closest(GRIP) : null;
    return !!grip && !!ownCard(grip);
  }

  // The distance from (`x0`, `y0`) to (`x`, `y`).
  function travel(x0, y0, x, y) {
    var dx = x - x0;
    var dy = y - y0;
    return Math.sqrt(dx * dx + dy * dy);
  }

  function init() {
    document.documentElement.style.setProperty("--card-hold-ms", HOLD_MS + "ms");
    // The one press on a card this page is following, if any: which pointer,
    // where it went down, the card, and — from the lift — its session. One
    // pointer, one gesture: every other pointer is ignored (DDD-9).
    var press = null;
    // Armed by the release of a lifted drag, consumed by the next click and
    // reset by the next press (DDD-6). The reset is what keeps it one-shot: a
    // lifted mouse release may deliver NO click at all (Chrome drops it once
    // the cancelled `dragstart` has fired), and an armed guard left standing
    // would eat the next genuine click on a card.
    var swallowClick = false;

    // End the press. A pending hold is dropped with its cue; a lifted drag is
    // torn down; the card never left its slot, so a cancel sends nothing and
    // restores nothing (DDD-8).
    function endPress() {
      if (press && press.hold !== null) {
        clearTimeout(press.hold);
      }
      if (press && press.session) {
        press.session.end();
      }
      clearArming();
      press = null;
    }

    // True while a touch or pen body hold is pending (armed, not yet lifted).
    function holding() {
      return !!press && press.hold !== null;
    }

    // The hold is up: lift `held` if it is still the press being followed and
    // its card is still on the page; a board replace mid-hold abandons it.
    function liftHeld(held) {
      if (press !== held || held.hold === null) {
        return;
      }
      held.hold = null;
      if (!held.card.isConnected) {
        endPress();
        return;
      }
      held.session = new CardDragSession(held.card);
      held.session.lift(held.x, held.y);
    }

    // Arm a touch or pen press on the card's text: the cue starts and one timer
    // is set for the hold (DDD-26, DDD-27).
    function arm(held) {
      held.card.setAttribute(ARMING, "");
      held.hold = setTimeout(function () {
        liftHeld(held);
      }, HOLD_MS);
    }

    // Cancel a lifted drag but keep following the pointer, so its release is
    // still a lifted release (the click guard) that lands nowhere.
    function cancelDrag() {
      if (!press || !press.session || press.cancelled) {
        return;
      }
      press.session.end();
      press.cancelled = true;
    }

    document.addEventListener("pointerdown", function (event) {
      if (press && press.pointerId !== event.pointerId) {
        return; // a second pointer never takes the card
      }
      endPress();
      swallowClick = false;
      // Primary contact only: mouse button 0, the pen tip, the first finger.
      if (!event.isPrimary || event.button !== 0) {
        return;
      }
      var card = ownCard(event.target);
      if (!card) {
        return;
      }
      // The per-origin lift rule (DDD-26): the mouse lifts past THRESHOLD
      // anywhere on the card; touch and pen lift past GRIP_THRESHOLD on the
      // grip, and after a HOLD_MS hold on the body.
      var mouse = event.pointerType === "mouse";
      var body = !mouse && !onOwnGrip(event.target);
      press = {
        pointerId: event.pointerId,
        card: card,
        startX: event.clientX,
        startY: event.clientY,
        x: event.clientX,
        y: event.clientY,
        threshold: mouse ? THRESHOLD : GRIP_THRESHOLD,
        hold: null,
        session: null,
        cancelled: false
      };
      if (body) {
        arm(press);
      }
    });

    // Abort a pending hold: the cue clears, nothing lifts, and the gesture is
    // the browser's (a scroll).
    function abortHold() {
      if (holding()) {
        endPress();
      }
    }

    document.addEventListener("pointermove", function (event) {
      if (!press || event.pointerId !== press.pointerId || press.cancelled) {
        return;
      }
      if (holding()) {
        if (travel(press.startX, press.startY, event.clientX, event.clientY) > HOLD_TOLERANCE) {
          abortHold(); // a swipe on the text: the browser scrolls
          return;
        }
        press.x = event.clientX;
        press.y = event.clientY;
        return;
      }
      if (!press.session) {
        if (travel(press.startX, press.startY, event.clientX, event.clientY) < press.threshold) {
          return; // still a click or a tap
        }
        press.session = new CardDragSession(press.card);
        press.session.lift(event.clientX, event.clientY);
      }
      press.session.follow(event.clientX, event.clientY);
      press.session.track(event.clientX, event.clientY);
    });

    // The landing slot is read from the live marker BEFORE the session ends.
    document.addEventListener("pointerup", function (event) {
      if (!press || event.pointerId !== press.pointerId) {
        return;
      }
      var current = press.session;
      if (!current) {
        endPress(); // a click or a tap; leave it to whatever it landed on
        return;
      }
      swallowClick = true;
      // A card lifted by a hold and released before it was carried anywhere
      // lands nowhere: nothing moves and nothing is sent.
      var lane = press.cancelled || !current.carried || !current.card.isConnected
        ? null
        : laneAt(event.clientX, event.clientY);
      var before = lane ? current.landingIn(lane, event.clientY) : null;
      endPress();
      if (lane) {
        current.dropInto(lane, before);
      }
    });

    // The browser took the pointer: after the lift this reverts exactly as
    // Escape does (the same `end()`: no request, the card never left its slot,
    // nothing left behind); before it, the press is simply abandoned (DDD-8).
    // Only the session's own pointer counts (DDD-9). No `pointerup` follows, so
    // no click guard is armed.
    document.addEventListener("pointercancel", function (event) {
      if (press && event.pointerId === press.pointerId) {
        endPress();
      }
    });

    // Escape's ONLY owner is keyboard.js::closeTopLayer(); it finds the drag by
    // `html[data-card-dragging]` and dispatches this. A `keydown` listener here
    // would race that one for the same press and peel two layers (BR-4).
    document.addEventListener("foundry:cancel-card-drag", cancelDrag);

    // The click a lifted release produces — on the card it began on, or on
    // whatever the press and release share — never opens anything. Nor does
    // any click on an own card's grip, whatever the pointer: that branch is
    // not one-shot and no press resets it (DDD-28). Capture phase, so it is
    // stopped before htmx or keyboard.js see it; either way an armed guard is
    // cleared.
    document.addEventListener(
      "click",
      function (event) {
        if (!swallowClick && !onOwnGrip(event.target)) {
          return;
        }
        swallowClick = false;
        event.preventDefault();
        event.stopPropagation();
      },
      true
    );

    // Before a body lift, a finger past HOLD_TOLERANCE aborts the hold (WebKit
    // stops sending `pointermove` once it claims a pan); after any lift, every
    // `touchmove` is prevented so neither the page nor the board scrolls under
    // the carried card (DDD-4). Non-passive explicitly.
    document.addEventListener(
      "touchmove",
      function (event) {
        if (holding()) {
          var touch = event.touches[0];
          if (touch && travel(press.startX, press.startY, touch.clientX, touch.clientY) > HOLD_TOLERANCE) {
            abortHold();
          }
          return;
        }
        if (press && press.session && !press.cancelled) {
          event.preventDefault();
        }
      },
      { passive: false }
    );

    // Any scroll before the lift means the browser took the gesture: a pending
    // hold is over (DDD-26). Capture phase (scroll does not bubble) and
    // passive; scrolls after the lift (edge auto-scroll) are ignored.
    document.addEventListener("scroll", abortHold, { capture: true, passive: true });

    // No callout or context menu while a hold is arming or a card is lifted
    // (DDD-4): the OS long-press would race the 500 ms hold.
    document.addEventListener("contextmenu", function (event) {
      if (holding() || (press && press.session)) {
        event.preventDefault();
      }
    });

    // A touch on an own card's grip is the board's (DDD-25): prevented at
    // `touchstart`, so iOS never starts its own drag interaction there and the
    // pointer events arrive at once. Non-passive explicitly (a `document` touch
    // listener is passive by default); off a grip it does nothing, so a touch
    // on the card's text still scrolls and still taps.
    document.addEventListener(
      "touchstart",
      function (event) {
        if (!onOwnGrip(event.target)) {
          return;
        }
        event.preventDefault();
      },
      { passive: false }
    );

    // An own card never starts the browser's own drag: it would take the
    // pointer away mid-gesture with `pointercancel` (DDD-19, spike Q1).
    document.addEventListener("dragstart", function (event) {
      if (ownCard(event.target)) {
        event.preventDefault();
      }
    });

    // Inside `#board-columns` every foreign drag is swallowed (`none`) so the
    // browser never opens it here. Outside `#board-columns` the browser default
    // applies. No card drag is native any more, so nothing here lights a lane.
    document.addEventListener("dragover", function (event) {
      if (!boardOf(event.target)) {
        return;
      }
      event.preventDefault();
      if (event.dataTransfer) {
        event.dataTransfer.dropEffect = "none";
      }
    });

    document.addEventListener("drop", function (event) {
      if (boardOf(event.target)) {
        event.preventDefault(); // swallowed, never acted on
      }
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
