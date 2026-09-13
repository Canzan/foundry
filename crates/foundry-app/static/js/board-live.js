// issue-card-delete slice 03 — the board's FIRST client-side live-update surface.
//
// Until this file existed, foundry shipped the whole outbox -> LISTEN ->
// broadcast -> SSE topology and nobody at the browser end: `/events` had
// server-side subscribers in the acceptance lane and not one EventSource in
// `static/js/`. DISCUSS AC-3.4 ("the other window stops showing the card
// without being reloaded") assumed a consumer that was never built. This is it.
//
// DELIBERATELY NARROW. It consumes exactly ONE event name, `IssueDeleted`, and
// does exactly one thing with it: drop the matching card. The stream already
// carries CommentAdded, CommentEdited, CommentDeleted and IssueUpdated, and a
// general live board — cards appearing, moving, retitling in place — is a
// separate feature with its own ordering and conflict questions. Subscribing by
// event NAME (rather than `onmessage`) is what keeps that boundary honest: the
// other four are never delivered to this module at all, so it cannot drift into
// half-handling them.
//
// App-owned, self-contained, CSP-safe, zero dependencies (DB6: no Node, no
// bundler, no package.json, no CDN) — the same shape as its five siblings.
//
// TWO CONSTRAINTS SHAPE THE CODE BELOW, both learned the hard way here:
//
//  1. `base.html` serves EVERY page — sign-in, dashboard, settings, the issue
//     page. So this module must derive its endpoint FROM THE DOM and leave
//     quietly when the board's markup is absent. A throw on the sign-in page
//     would be an app-wide regression shipped by a board feature.
//
//  2. The out-of-band `#board-columns` refresh REPLACES that subtree, so any
//     card node captured earlier is left detached and removing it is a no-op on
//     a board that still shows the card. This is the same failure class
//     ADR-BOARD-LANE-005 records for the lane menu, and the same answer: hold
//     nothing. Every event re-queries the live document. There is no cache here
//     and there must not be one.
(function () {
  "use strict";

  // The board page is `/team/{team}/project/{project}`; its events endpoint is
  // that path + `/events` (lib.rs). The two checks below are SEPARATE and both
  // required: the `.board#board-columns` probe is the "am I on a board" test
  // (base.html serves every page, so this module must leave quietly off the
  // board — constraint 1 above), and the report link supplies the team/project
  // slugs, which the board markup carries NOWHERE else.
  //
  // KNOWN SEAM (weighed and kept, DELIVER Phase 3): reading the slugs off
  // `a.report-link[href]` couples this module to another feature's markup
  // (issue-change-history's Change report link). Rename, move or conditionally
  // hide that link and the board silently stops updating live — no error, no
  // throw. The obvious alternative, a `data-events-url` attribute on
  // `#board-columns`, is NOT cheaper: `#board-columns` is emitted by TWO
  // templates, `board.html` and `partials/oob/board_columns_oob.html` (the
  // shared `partials/board_columns.html` holds only the inner columns loop),
  // and `hx-swap-oob="true"` is an outerHTML swap — so the attribute would have
  // to be duplicated across both to keep the full render and the OOB refresh
  // byte-identical, trading a cross-feature read for a two-template copy that
  // can drift. The coupling is covered end-to-end by the `@needs-browser`
  // second-window scenario, which fails if the endpoint stops resolving.
  // Returns "" off the board.
  function eventsEndpoint() {
    // `.board#board-columns` is the browser harness's BOARD_READY_SELECTOR —
    // the same marker the acceptance lane treats as "a board is on screen".
    var board = document.querySelector(".board#board-columns");
    if (!board) {
      return "";
    }
    var link = document.querySelector("a.report-link[href]");
    var href = link ? link.getAttribute("href") : "";
    var suffix = "/report";
    if (!href || href.length <= suffix.length) {
      return "";
    }
    if (href.slice(-suffix.length) !== suffix) {
      return "";
    }
    return href.slice(0, href.length - suffix.length) + "/events";
  }

  // Remove every card carrying this key, re-querying the LIVE document each
  // time (constraint 2 above). Scanning and comparing the attribute — rather
  // than building an `[data-issue-key="..."]` selector — also means a key that
  // is not a valid CSS identifier can never throw a SyntaxError out of the
  // event handler and kill the subscription.
  function dropCard(key) {
    if (!key) {
      return;
    }
    var cards = document.querySelectorAll("article.issue-card");
    for (var i = 0; i < cards.length; i++) {
      if (cards[i].getAttribute("data-issue-key") === key) {
        cards[i].remove();
      }
    }
  }

  function init() {
    var endpoint = eventsEndpoint();
    if (!endpoint) {
      return; // not a board — this page has nothing to keep live
    }
    // No `withCredentials`: the stream is same-origin, so the session cookie
    // rides along by default, and asking for CORS credentials on a same-origin
    // URL only narrows what the browser will accept.
    var source = new EventSource(endpoint);
    source.addEventListener("IssueDeleted", function (event) {
      var payload;
      try {
        payload = JSON.parse(event.data);
      } catch (err) {
        return; // an unparseable frame is not a reason to drop a card
      }
      // `key` is the payload field written by issue_delete.rs, and it is
      // byte-identical to the card's `data-issue-key` ("AUTH-42").
      dropCard(payload && payload.key);
    });
    // EventSource reconnects on its own after a transport error; there is
    // nothing useful to do here and closing the stream would make a transient
    // blip permanent. A board that misses an event while disconnected is
    // corrected by the next navigation, which is the pre-existing behaviour.
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
