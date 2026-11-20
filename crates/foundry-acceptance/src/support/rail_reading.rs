//! ONE reading of the LIVE left rail, for every oracle that asserts what the
//! rail offers the operator.
//!
//! Two lanes now assert things about the same rail — the one that narrows it and
//! the one that walks its icons — and both need the SAME moment of the same
//! document: the geometry, the mark the rules select on, which members the
//! operator can see and reach, what each of them is called, and what the member
//! holding the focus shows her. Read twice, those claims drift into two moments
//! and a lane can report a geometry from before a press with an accessibility
//! fact from after it.
//!
//! WHY HERE AND NOT IN A STEP MODULE. The reading started as a private const and
//! a private function inside the fold lane's step module. A second lane could
//! reach them only by importing across step modules or by widening their
//! visibility, and no step module may depend on another one. So the reading is
//! owned here, beside `served_markup.rs` and for the same reason: both step
//! modules depend on this module, this module depends on neither of them. It
//! knows no feature; it depends on `fantoccini`, `serde_json` and the session
//! machinery in `browser_harness`.
//!
//! WHAT IS OBSERVED, AND WHAT IS DELIBERATELY NOT. Observed: that a member is in
//! the layout, that it can be seen, that it can take the keyboard focus, the
//! name the BROWSER computes for it, which member is marked as the page the
//! operator is on, the box each one occupies, and the box of the label the
//! focused member shows. Not observed: the strip's width in pixels, the glyph an
//! icon draws, the order of the members, the colour of the active tint and the
//! styling of the label — those are design values, not promises, so two strips
//! of different widths read as the same rail here.
//!
//! WHY FOCUSABILITY IS READ FROM THE DISPLAY CHAIN AND NOT BY FOCUSING. A probe
//! that focused each member to find out whether it can be focused would move the
//! operator's own focus and change what the next assertion reads. `display: none`
//! and `visibility: hidden` take an element out of the focus order; being
//! clipped, zero-sized or transparent does NOT. That is exactly the distinction
//! the promise turns on — a rail that hides its links by width or by opacity
//! leaves them reachable from the keyboard while invisible — so the chain is
//! walked instead of the focus being stolen.
//!
//! EXACTLY WHAT "VISIBLE" MEANS HERE, mechanism by mechanism. A member counts as
//! visible when nothing in its ancestry carries `display: none`,
//! `visibility: hidden`, `inert` or `hidden`, nothing in its ancestry is
//! transparent, its own box has a real size, AND the point at the centre of that
//! box hit-tests back to the member itself or to a node INSIDE it — its glyph,
//! its word. An ancestor answering that point does NOT count: a member clipped
//! away is exactly the case where the paint at its centre belongs to whatever
//! encloses it, so accepting an ancestor would pass the one case the test is
//! here to catch. The test is needed because every other fact above is read
//! from the LAYOUT box, which `clip`, `clip-path` and an ancestor's `overflow`
//! leave untouched: without it a destination clipped away while it keeps its box
//! reads as visible, and clipping is the mechanism the design's own model
//! records as a violating state. When the centre falls outside the viewport the
//! hit test cannot be taken and is not held against the member — that is
//! recorded per member rather than guessed at. The same test decides the
//! `visible` of a keyboard stop, so a walk and a sweep cannot disagree about
//! what the operator can see.

use crate::support::browser_harness;

/// The one channel from the script to the stylesheet: a VALUELESS attribute on
/// the document element. Owned here because the reading is what consumes it, and
/// two lanes that read the mark from two constants could read two marks.
pub const FOLD_MARK: &str = "data-nav-collapsed";

/// The shipped width of the open rail. Named because the promises are "back at
/// the width it shipped with" and "strictly narrower than the width it shipped
/// with" — never to bound the narrow strip, whose width no promise fixes.
pub const OPEN_RAIL_PX: f64 = 240.0;

/// EVERY member of the rail that opens somewhere: each destination link and the
/// Sign out submit. One spelling, used by the live probe AND by the browser's own
/// accessible-name reading, so the two cannot disagree about which members they
/// are talking about or about the order they are in.
pub const RAIL_MEMBERS: &str =
    ".sidebar a.sidebar__item, .sidebar form[action=\"/sign-out\"] button";

/// The control the operator presses to narrow the rail and to open it again,
/// located BY ITS ACCESSIBLE CONTRACT and not by a class of its own: a class
/// would be a fixture fact nobody promised, while the contract is the promise.
/// One owner, so the two lanes that press it cannot press two different things.
pub const NAV_CONTROL: &str = ".sidebar__brand button[aria-expanded]";

/// How far a keyboard walk goes before it reports that it never arrived.
pub const MAX_TAB_STOPS: usize = 40;

const NAV_PROBE: &str = r#"
var mark = arguments[0];
var memberSelector = arguments[1];
var controlSelector = arguments[2];
var rail = document.querySelector('.sidebar');
var content = document.querySelector('.app-shell__content');
var shell = document.querySelector('.app-shell');
var nav = document.querySelector('.sidebar__nav');
var control = document.querySelector(controlSelector);
var theme = document.querySelector('.theme-toggle');
var items = Array.prototype.slice.call(document.querySelectorAll('.sidebar__nav .sidebar__item'));
var members = Array.prototype.slice.call(document.querySelectorAll(memberSelector));
function width(el) { return el ? el.getBoundingClientRect().width : null; }
function display(el) { return el ? getComputedStyle(el).display : null; }
function shown(el) {
  return !!el && getComputedStyle(el).display !== 'none' && el.getBoundingClientRect().width > 0;
}
function box(el) {
  if (!el) { return null; }
  var r = el.getBoundingClientRect();
  return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height };
}
function laidOut(el) {
  var r = el.getBoundingClientRect();
  return r.width > 0 || r.height > 0;
}
function inTheFocusOrder(el) {
  for (var n = el; n && n.nodeType === 1; n = n.parentElement) {
    var s = getComputedStyle(n);
    if (s.display === 'none' || s.visibility === 'hidden' || s.visibility === 'collapse') { return false; }
    if (n.hasAttribute('inert') || n.hasAttribute('hidden')) { return false; }
  }
  return true;
}
function faded(el) {
  for (var n = el; n && n.nodeType === 1; n = n.parentElement) {
    var o = parseFloat(getComputedStyle(n).opacity);
    if (!isNaN(o) && o < 0.05) { return true; }
  }
  return false;
}
function centreInViewport(el) {
  var r = el.getBoundingClientRect();
  if (r.width < 1 || r.height < 1) { return false; }
  var x = r.left + r.width / 2;
  var y = r.top + r.height / 2;
  return x >= 0 && y >= 0 && x <= document.documentElement.clientWidth &&
         y <= document.documentElement.clientHeight;
}
// THE CLIP-AWARE HALF. Everything above reads the LAYOUT box, which `clip`,
// `clip-path` and an ancestor's `overflow` do not change: a member clipped away
// keeps its box and would read as visible. The point at the centre of the box
// is asked what is actually painted there, which also catches a member another
// box covers. Only the element itself or something INSIDE it counts: an
// ancestor answering that point is what a clipped-away member looks like, so
// accepting one would pass the very case this test exists to catch.
function hitsItsOwnCentre(el) {
  var r = el.getBoundingClientRect();
  var hit = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2);
  if (!hit) { return false; }
  return hit === el || el.contains(hit);
}
function visibleNow(el) {
  if (!inTheFocusOrder(el) || faded(el)) { return false; }
  var r = el.getBoundingClientRect();
  if (r.width < 1 || r.height < 1) { return false; }
  if (centreInViewport(el) && !hitsItsOwnCentre(el)) { return false; }
  return true;
}
// THE PAINTED FILL, not the declared one. A control whose own background is
// fully transparent is painted by whatever is behind it, so the reading walks
// outwards to the first ancestor that paints. Read off the declared value
// alone, a transparent control and a control filled with the rail's own colour
// would read as different fills while the operator sees one surface, and a
// control filled black over a transparent rail would read the same as its
// neighbour link.
function paintedFill(el) {
  for (var n = el; n && n.nodeType === 1; n = n.parentElement) {
    var fill = getComputedStyle(n).backgroundColor;
    var parts = /rgba?\(([^)]+)\)/.exec(fill);
    if (!parts) { return fill; }
    var channels = parts[1].split(',');
    var alpha = channels.length > 3 ? parseFloat(channels[3]) : 1;
    if (!isNaN(alpha) && alpha > 0.01) { return fill; }
  }
  return 'rgba(0, 0, 0, 0)';
}
function widestBorder(el) {
  var s = getComputedStyle(el);
  return Math.max(
    parseFloat(s.borderTopWidth) || 0,
    parseFloat(s.borderRightWidth) || 0,
    parseFloat(s.borderBottomWidth) || 0,
    parseFloat(s.borderLeftWidth) || 0
  );
}
// How a member of the rail's LOWER GROUP is drawn, in the terms the promise is
// written in: the fill the operator sees, whether it carries a border its
// neighbours do not, and the text tier. Never the token that produced the
// colour and never the exact hue — two palettes answer different hues for the
// same promise.
function readPresentation(el, kind) {
  if (!el) { return null; }
  var s = getComputedStyle(el);
  return {
    kind: kind,
    name: (el.getAttribute('aria-label') || el.textContent || '').trim(),
    fill: paintedFill(el),
    ownFill: s.backgroundColor,
    borderWidth: widestBorder(el),
    colour: s.color,
    fontWeight: s.fontWeight,
    fontSize: s.fontSize,
    visible: visibleNow(el),
    box: box(el)
  };
}
function focusableNow(el) {
  if (el.disabled) { return false; }
  if (el.tabIndex < 0) { return false; }
  if (el.tagName.toLowerCase() === 'a' && !el.hasAttribute('href')) { return false; }
  return inTheFocusOrder(el);
}
function readMember(el) {
  var label = el.querySelector('.sidebar__label');
  var icon = el.querySelector('svg.sidebar__icon');
  var style = getComputedStyle(el);
  return {
    text: (el.textContent || '').trim(),
    shownText: (el.innerText || '').trim(),
    href: el.getAttribute('href'),
    opens: el.tagName.toLowerCase() === 'button' ? 'the end of her session' : el.getAttribute('href'),
    inLayout: laidOut(el),
    visible: visibleNow(el),
    focusable: focusableNow(el),
    current: el.getAttribute('aria-current'),
    background: style.backgroundColor,
    fontWeight: style.fontWeight,
    labelPresent: !!label,
    labelText: label ? (label.textContent || '').trim() : null,
    labelBox: box(label),
    iconPresent: !!icon,
    iconLaidOut: !!icon && laidOut(icon),
    iconBox: box(icon),
    centreHitTestable: centreInViewport(el),
    centreHitsItself: centreInViewport(el) && hitsItsOwnCentre(el),
    box: box(el)
  };
}
var read = members.map(readMember);
var controlled = null;
if (control && control.getAttribute('aria-controls')) {
  controlled = document.getElementById(control.getAttribute('aria-controls'));
}
return {
  marked: document.documentElement.hasAttribute(mark),
  markValue: document.documentElement.getAttribute(mark),
  railWidth: width(rail),
  contentWidth: width(content),
  shellWidth: width(shell),
  viewport: document.documentElement.clientWidth,
  docWidth: document.documentElement.scrollWidth,
  // THE SAME MOMENT carries the geometry facts the sticky and the phone
  // promises are read from, so a rail box can never be reported from before a
  // scroll beside a mark read after it.
  viewportHeight: document.documentElement.clientHeight,
  docHeight: document.documentElement.scrollHeight,
  scrolledTo: window.scrollY,
  railPosition: rail ? getComputedStyle(rail).position : null,
  railSurface: rail ? paintedFill(rail) : null,
  lowerGroup: Array.prototype.slice.call(
    document.querySelectorAll('.sidebar__user a.sidebar__item')
  ).map(function (el) { return readPresentation(el, 'a neighbour link'); }).concat(
    [readPresentation(document.querySelector('.sidebar__user form button'), 'Sign out')],
    [readPresentation(theme, 'the theme control')]
  ).filter(function (m) { return m !== null; }),
  shellDirection: shell ? getComputedStyle(shell).flexDirection : null,
  navDisplay: display(nav),
  workspaceDisplay: display(document.querySelector('.sidebar__workspace')),
  identityDisplay: display(document.querySelector('.sidebar__identity')),
  monogramDisplay: display(document.querySelector('.sidebar__monogram')),
  railBox: box(rail),
  itemCount: items.length,
  itemsShown: items.filter(shown).length,
  itemsFocusable: items.filter(function (el) {
    return el.tabIndex >= 0 && el.hasAttribute('href') && el.textContent.trim().length > 0;
  }).length,
  members: read,
  invisibleFocusable: read.filter(function (m) {
    return m.focusable && !m.visible;
  }).map(function (m) { return m.text || '(an unnamed rail member)'; }),
  control: control ? {
    tag: control.tagName.toLowerCase(),
    expanded: control.getAttribute('aria-expanded'),
    controls: control.getAttribute('aria-controls'),
    controlsTheNav: controlled !== null && controlled === nav,
    name: (control.getAttribute('aria-label') || control.textContent || '').trim(),
    shown: shown(control),
    tabindex: control.getAttribute('tabindex')
  } : null,
  themeControl: theme ? {
    shown: shown(theme),
    name: (theme.getAttribute('aria-label') || '').trim()
  } : null,
  kbReady: document.querySelectorAll('[data-kb-ready]').length
};
"#;

/// The rail, the content column, the shell, the mark and every rail member, read
/// in ONE moment of the live document.
pub async fn read_nav(client: &fantoccini::Client) -> serde_json::Value {
    client
        .execute(
            NAV_PROBE,
            vec![
                serde_json::Value::String(FOLD_MARK.to_string()),
                serde_json::Value::String(RAIL_MEMBERS.to_string()),
                serde_json::Value::String(NAV_CONTROL.to_string()),
            ],
        )
        .await
        .expect("read the rail, the content column and the control from the live page")
}

/// The same reading, with the name the BROWSER computes for each member written
/// onto it.
///
/// The name is taken from the browser's OWN accessible-name computation and not
/// from the markup, because the promise is what an operator using assistive
/// technology is told, and the two part company exactly where it matters: a word
/// taken out of the layout with `display: none` is still in `textContent` and is
/// NOT in the accessibility tree, so a name read from the markup would be green
/// over a rail whose icons announce nothing.
pub async fn read_rail_with_names(client: &fantoccini::Client) -> serde_json::Value {
    let mut reading = read_nav(client).await;
    let names = member_accessible_names(client).await;
    let members = reading["members"]
        .as_array()
        .expect("the reading carries its rail members")
        .len();
    assert_eq!(
        names.len(),
        members,
        "the live document answered with {members} rail member(s) and the browser's accessibility \
         reading with {} name(s) for `{RAIL_MEMBERS}`. The two readings are of the same selector \
         in the same document, so a mismatch means the page changed between them and nothing \
         below could be attributed to a member.",
        names.len()
    );
    for (index, name) in names.into_iter().enumerate() {
        reading["members"][index]["name"] = serde_json::Value::String(name);
    }
    reading
}

/// What the browser computes as each rail member's accessible name, in document
/// order, read through the browser's own accessibility tree.
async fn member_accessible_names(client: &fantoccini::Client) -> Vec<String> {
    browser_harness::devtools(client, "DOM.enable", serde_json::json!({})).await;
    browser_harness::devtools(client, "Accessibility.enable", serde_json::json!({})).await;
    let document =
        browser_harness::devtools(client, "DOM.getDocument", serde_json::json!({ "depth": 1 }))
            .await;
    let root = document["root"]["nodeId"].as_u64().unwrap_or_else(|| {
        panic!(
            "the browser returned no document node to read the accessibility tree from: \
             {document}"
        )
    });
    let found = browser_harness::devtools(
        client,
        "DOM.querySelectorAll",
        serde_json::json!({ "nodeId": root, "selector": RAIL_MEMBERS }),
    )
    .await;
    let ids = found["nodeIds"]
        .as_array()
        .unwrap_or_else(|| {
            panic!("the browser answered no node list for `{RAIL_MEMBERS}`: {found}")
        })
        .clone();
    let mut names = Vec::with_capacity(ids.len());
    for id in ids {
        let tree = browser_harness::devtools(
            client,
            "Accessibility.getPartialAXTree",
            serde_json::json!({ "nodeId": id, "fetchRelatives": false }),
        )
        .await;
        names.push(
            tree["nodes"]
                .as_array()
                .and_then(|nodes| nodes.first())
                .and_then(|node| node["name"]["value"].as_str())
                .unwrap_or_default()
                .trim()
                .to_string(),
        );
    }
    names
}

/// A measurement off a reading, with the diagnostic for a page that has nothing
/// to measure.
pub fn px(reading: &serde_json::Value, key: &str) -> f64 {
    reading[key].as_f64().unwrap_or_else(|| {
        panic!(
            "the page has no {key} to measure — the rail, the content column or the shell is \
             missing, so this is not the application shell at all. Reading: {reading}"
        )
    })
}

/// Every rail member of a reading.
pub fn members(reading: &serde_json::Value) -> &[serde_json::Value] {
    reading["members"]
        .as_array()
        .unwrap_or_else(|| {
            panic!(
                "the reading carries no rail members at all, so nothing can be said about what the \
                 rail offers the operator. Reading: {reading}"
            )
        })
        .as_slice()
}

const FOCUS_PROBE: &str = r#"
var memberSelector = arguments[0];
var el = document.activeElement;
if (!el) {
  return { description: 'nothing', isNavControl: false, isRailMember: false, isBody: false };
}
function inTheFocusOrder(node) {
  for (var n = node; n && n.nodeType === 1; n = n.parentElement) {
    var s = getComputedStyle(n);
    if (s.display === 'none' || s.visibility === 'hidden' || s.visibility === 'collapse') { return false; }
    if (n.hasAttribute('inert') || n.hasAttribute('hidden')) { return false; }
  }
  return true;
}
function faded(node) {
  for (var n = node; n && n.nodeType === 1; n = n.parentElement) {
    var o = parseFloat(getComputedStyle(n).opacity);
    if (!isNaN(o) && o < 0.05) { return true; }
  }
  return false;
}
// The same clip-aware reading the sweep uses, for the same reason and with the
// same rule: only the stop itself or something inside it counts, because an
// ancestor answering the centre point is what a clipped-away stop looks like.
// A stop whose centre lies outside the viewport cannot be hit-tested, and that
// is not held against it.
function centreInViewport(node) {
  var b = node.getBoundingClientRect();
  if (b.width < 1 || b.height < 1) { return false; }
  var x = b.left + b.width / 2;
  var y = b.top + b.height / 2;
  return x >= 0 && y >= 0 && x <= document.documentElement.clientWidth &&
         y <= document.documentElement.clientHeight;
}
function hitsItsOwnCentre(node) {
  var b = node.getBoundingClientRect();
  var hit = document.elementFromPoint(b.left + b.width / 2, b.top + b.height / 2);
  if (!hit) { return false; }
  return hit === node || node.contains(hit);
}
function visibleNow(node) {
  if (!inTheFocusOrder(node) || faded(node)) { return false; }
  var b = node.getBoundingClientRect();
  if (b.width < 1 || b.height < 1) { return false; }
  if (centreInViewport(node) && !hitsItsOwnCentre(node)) { return false; }
  return true;
}
var rect = el.getBoundingClientRect();
var description = el.tagName.toLowerCase();
if (el.className && typeof el.className === 'string' && el.className.trim()) {
  description += '.' + el.className.trim().split(/\s+/).join('.');
}
var text = (el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 40);
var style = getComputedStyle(el);
var label = el.querySelector ? el.querySelector('.sidebar__label') : null;
var labelRect = label ? label.getBoundingClientRect() : null;
// THE LABEL'S BOX WHILE THIS MEMBER HOLDS THE FOCUS — a reading, not a verdict.
// Whether that box means the word was REVEALED can only be told by comparing it
// with the box the same label has while the member does not hold the focus: the
// collapsed geometry leaves the word clipped to a sliver that is still in the
// document, so a box of its own proves nothing. The centre hit test travels with
// it as corroboration and is never the whole reading, because the painted sliver
// of a clipped label can answer a hit test too.
var labelHit = null;
if (labelRect && labelRect.width >= 1 && labelRect.height >= 1) {
  var hit = document.elementFromPoint(
    labelRect.left + labelRect.width / 2,
    labelRect.top + labelRect.height / 2
  );
  labelHit = !!hit && (hit === label || label.contains(hit));
}
return {
  description: description + (text ? ' [' + text + ']' : ''),
  text: (el.textContent || '').trim(),
  isNavControl: !!(el.closest && el.closest('.sidebar__brand') && el.hasAttribute('aria-expanded')),
  isRailMember: !!(el.matches && el.matches(memberSelector)),
  visible: visibleNow(el),
  labelPresent: !!label,
  labelText: label ? (label.textContent || '').trim() : null,
  labelBox: labelRect ? {
    left: labelRect.left, right: labelRect.right, top: labelRect.top,
    bottom: labelRect.bottom, width: labelRect.width, height: labelRect.height
  } : null,
  labelInTheLayout: !!label && inTheFocusOrder(label) && !faded(label),
  labelCentreHitsTheLabel: labelHit,
  outlineStyle: style.outlineStyle,
  outlineWidth: parseFloat(style.outlineWidth) || 0,
  isBody: el === document.body
};
"#;

/// What holds the keyboard focus, and what it shows the operator.
pub async fn read_focus(client: &fantoccini::Client) -> serde_json::Value {
    client
        .execute(
            FOCUS_PROBE,
            vec![serde_json::Value::String(RAIL_MEMBERS.to_string())],
        )
        .await
        .expect("read what holds the keyboard focus")
}

pub async fn clear_focus(client: &fantoccini::Client) {
    client
        .execute(
            "if (document.activeElement && document.activeElement.blur) {
               document.activeElement.blur();
             }
             return true;",
            Vec::new(),
        )
        .await
        .expect("clear the focus before walking the reading order");
}

/// Tab from a cleared focus until the nav control holds it, returning the stops
/// made on the way so a failure names the order that DID happen.
pub async fn tab_until_the_control_holds_focus(client: &fantoccini::Client) -> Vec<String> {
    clear_focus(client).await;
    let mut walked = Vec::new();
    for _ in 0..MAX_TAB_STOPS {
        browser_harness::press_key(client, "Tab").await;
        let focus = read_focus(client).await;
        walked.push(
            focus["description"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
        if focus["isNavControl"].as_bool().unwrap_or(false) {
            return walked;
        }
    }
    panic!(
        "the nav control was never reached in {MAX_TAB_STOPS} Tab presses, so it is not in the \
         reading order and an operator working from the keyboard cannot fold the nav at all. The \
         walk stopped at: {walked:?}"
    );
}

/// Walk the WHOLE reading order once and record every stop, so an assertion can
/// ask what the keyboard could reach rather than only where it ended up.
pub async fn walk_the_tab_order(client: &fantoccini::Client) -> serde_json::Value {
    clear_focus(client).await;
    let mut stops = Vec::new();
    for _ in 0..MAX_TAB_STOPS {
        browser_harness::press_key(client, "Tab").await;
        let focus = read_focus(client).await;
        let back_at_the_start = focus["isBody"].as_bool().unwrap_or(false) && !stops.is_empty();
        stops.push(focus);
        if back_at_the_start {
            break;
        }
    }
    serde_json::Value::Array(stops)
}
