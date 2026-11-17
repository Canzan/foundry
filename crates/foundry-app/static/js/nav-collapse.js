// Left-nav collapse control (feature nav-collapse-toggle, 2026-11-16).
//
// WHAT THIS EXISTS TO DO. The rail is a fixed 240px column, and an operator who
// wants the page wider had no way to say so. This asset gives her the say, in
// two states rather than three:
//
//   expanded   no `data-nav-collapsed` attribute on <html>; the shipped rail
//   collapsed  `data-nav-collapsed` (VALUELESS) on <html>; the rail is a strip
//
// THE CARRIER IS A BOOLEAN, AND THE MARKER CARRIES NO VALUE (ADR-NAV-COLLAPSE-001).
// The stylesheet selects on the attribute's PRESENCE, never on a value, so even
// an outside script, an extension or devtools writing `data-nav-collapsed="…"`
// still selects one of exactly two geometries. There is no third rail width to
// reach, which is why the state here is `true`/`false` and never a word.
//
// THE STORED FORM IS ONE SENTINEL, AND EXPANDED IS THE ABSENCE OF THE KEY.
// Collapsing writes "collapsed" under `foundry.nav`; expanding REMOVES the key.
// A word written for expanded would make an operator who opened the rail again
// indistinguishable from a browser that never touched the control, and the
// pre-feature default would stop being "no key" — which is exactly what the
// no-JavaScript promise rests on. `interpret` is total over strings: anything
// that is not the sentinel reads as expanded, so a value another tool or a
// later version leaves behind can never be refused and can never crash a read.
//
// WHY IT IS LOADED FROM <head>. Stamping the marker must happen BEFORE first
// paint, or an operator who collapsed the rail sees it flash open on every
// navigation. The tag is a plain external <script> in the document shell's
// head, which is render-blocking by specification, so the attribute is on
// <html> before the body is painted. `defer`, `async` and `type="module"` each
// reintroduce the flash SILENTLY — every settled-page assertion still passes —
// so `cargo xtask check-arch`'s pre-paint-script layer refuses them on this tag
// and on every other head script that stamps the document from stored state.
//
// WHY THE BUTTON IS BUILT HERE AND NOT BY THE RENDERER. Same reason as
// `theme.js`: a server-rendered toggle would be present but DEAD with scripting
// off, and a dead control is worse than no control. Built here, the affordance
// exists exactly when something can service it; with the script gone the rail is
// simply open, as it was before this file existed. The one thing the renderer
// does own is the `id` on `nav.sidebar__nav` that `aria-controls` names: an id is
// static, inert without scripting, and keeps this asset to its own two DOM
// effects.
//
// The button carries its state in its ACCESSIBLE NAME, not only in its glyph,
// and the name names its SUBJECT: an operator hearing "Collapsed. Expand." out
// of the page's context could not tell what it is about.
//
// Storage is best-effort. A browser with site data blocked throws on the first
// touch of localStorage; every access here is guarded, and a failure costs the
// operator persistence across reloads, never the control itself.
//
// This asset is NOT theme.js and must never be folded into it: `theme.js` is a
// byte-identical port of canzan-lift's own file (see static/VENDOR.md), and a
// third divergence would make the two unmergeable.
//
// Keep this logic straight-line; Rust cannot unit-test it.
(function () {
  "use strict";

  var STORAGE_KEY = "foundry.nav";
  var COLLAPSED = "collapsed"; // the only value ever written; expanded removes the key
  var MARKER = "data-nav-collapsed"; // valueless: the one channel to the stylesheet
  var NAV_ID = "sidebar-nav"; // server-rendered on nav.sidebar__nav
  var MOUNT = ".sidebar__brand";

  // Total over strings AND over the absence of the key: exactly one spelling
  // means collapsed, everything else is the expanded default.
  function interpret(value) {
    return value === COLLAPSED;
  }

  function stored() {
    try {
      return interpret(window.localStorage.getItem(STORAGE_KEY));
    } catch (error) {
      return false; // site data blocked — the rail opens, silently
    }
  }

  function remember(isCollapsed) {
    try {
      if (isCollapsed) {
        window.localStorage.setItem(STORAGE_KEY, COLLAPSED);
      } else {
        window.localStorage.removeItem(STORAGE_KEY);
      }
    } catch (error) {
      // Nothing to repair: the rail moved either way, the choice just will not
      // survive a reload.
    }
  }

  // The whole mechanism. Presence or absence of a VALUELESS attribute — never a
  // value, because a value would let a stray writer select a third geometry.
  function applyMarker(isCollapsed) {
    if (isCollapsed) {
      document.documentElement.setAttribute(MARKER, "");
    } else {
      document.documentElement.removeAttribute(MARKER);
    }
  }

  var collapsed = stored();
  applyMarker(collapsed); // before paint — this is why the tag is in <head>

  function describe(isCollapsed) {
    if (isCollapsed) {
      return "Left nav: collapsed. Expand the left nav.";
    }
    return "Left nav: expanded. Collapse the left nav.";
  }

  function buildToggle() {
    var brand = document.querySelector(MOUNT);
    if (!brand) {
      return; // a document without the rail has nowhere to put it
    }

    var button = document.createElement("button");
    button.type = "button";
    button.className = "nav-collapse";
    button.setAttribute("aria-controls", NAV_ID);

    var glyph = document.createElement("span");
    glyph.className = "nav-collapse__glyph";
    glyph.setAttribute("aria-hidden", "true");
    button.appendChild(glyph);

    // Re-derived from the boolean on every paint, never written once at build
    // time: a name describing the state the rail USED to be in tells the
    // operator the wrong thing about her next press.
    function paint() {
      glyph.textContent = collapsed ? "»" : "«";
      button.setAttribute("aria-expanded", collapsed ? "false" : "true");
      button.setAttribute("aria-label", describe(collapsed));
      button.setAttribute("title", describe(collapsed));
    }

    button.addEventListener("click", function () {
      collapsed = !collapsed;
      applyMarker(collapsed);
      remember(collapsed);
      paint();
    });

    paint();
    brand.appendChild(button);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", buildToggle);
  } else {
    buildToggle();
  }
})();
