// Theme control (feature ui-theme-toggle, 2026-08-27).
//
// WHAT THIS EXISTS TO DO. The stylesheet ships two palettes and, until now,
// only the operating system could choose between them. An operator working a
// bright room at night — or a dark room at noon — had no way to say so. This
// asset gives her the say, in three states rather than two:
//
//   system  no `data-theme` attribute; `prefers-color-scheme` decides (default)
//   light   `data-theme="light"` on <html>; the media query is overruled
//   dark    `data-theme="dark"`  on <html>; the media query is overruled
//
// The three-state cycle is deliberate. A two-state toggle can only ever be ON
// or OFF, and once she has touched it she can never hand the decision back to
// the device — which is the setting most people actually want most of the time.
//
// WHY IT IS LOADED FROM <head>. Stamping the attribute must happen BEFORE first
// paint, or a dark-preferring operator sees a white flash on every navigation.
// The tag is a plain external <script> in the document shell's head, which is
// render-blocking by specification, so the attribute is on <html> before the
// body is painted. Nothing else in this file runs that early.
//
// WHY THE BUTTON IS BUILT HERE AND NOT BY THE RENDERER. DISCUSS D5: every
// screen and every mutating control must work with JavaScript disabled. A
// server-rendered toggle could not honour that — it would be a dead control on
// a page with no script, and a dead control is worse than no control. Built
// here, the affordance exists exactly when something can service it; with the
// script gone the page simply follows the operating system, which is precisely
// what shipped before this file existed.
//
// The button carries its state in its ACCESSIBLE NAME, not only in its glyph:
// the name says which theme is active and which one the next press selects.
//
// Storage is best-effort. A browser with site data blocked throws on the first
// touch of localStorage; every access here is guarded, and a failure costs the
// operator persistence across reloads, never the control itself.
//
// Keep this logic straight-line; Rust cannot unit-test it.
(function () {
  "use strict";

  var STORAGE_KEY = "foundry.theme";
  var ORDER = ["system", "light", "dark"];
  var GLYPH = { system: "◐", light: "☀", dark: "☾" };
  var WORD = { system: "System", light: "Light", dark: "Dark" };

  function stored() {
    try {
      var value = window.localStorage.getItem(STORAGE_KEY);
      return ORDER.indexOf(value) === -1 ? "system" : value;
    } catch (error) {
      return "system"; // site data blocked — follow the device, silently
    }
  }

  function remember(mode) {
    try {
      if (mode === "system") {
        window.localStorage.removeItem(STORAGE_KEY);
      } else {
        window.localStorage.setItem(STORAGE_KEY, mode);
      }
    } catch (error) {
      // Nothing to repair: the mode is applied either way, it just will not
      // survive a reload.
    }
  }

  // The whole mechanism. "system" REMOVES the attribute rather than writing a
  // third value, because the stylesheet's dark block is written as
  // `:root:not([data-theme="light"])` inside the media query — absence is what
  // hands the decision back to the device.
  function apply(mode) {
    if (mode === "system") {
      document.documentElement.removeAttribute("data-theme");
    } else {
      document.documentElement.setAttribute("data-theme", mode);
    }
  }

  var current = stored();
  apply(current); // before paint — this is why the tag is in <head>

  function next(mode) {
    return ORDER[(ORDER.indexOf(mode) + 1) % ORDER.length];
  }

  function describe(mode) {
    if (mode === "system") {
      return "Theme: following your device. Switch to " + WORD[next(mode)].toLowerCase() + ".";
    }
    return "Theme: " + WORD[mode].toLowerCase() + ". Switch to " + WORD[next(mode)].toLowerCase() + ".";
  }

  function build() {
    var nav = document.querySelector(".sidebar__user");
    if (!nav) {
      return; // a document without the shared navigation has nowhere to put it
    }

    var button = document.createElement("button");
    button.type = "button";
    button.className = "theme-toggle";

    var glyph = document.createElement("span");
    glyph.className = "theme-toggle__glyph";
    glyph.setAttribute("aria-hidden", "true");

    var word = document.createElement("span");
    word.className = "theme-toggle__mode";

    button.appendChild(glyph);
    button.appendChild(word);

    function paint() {
      glyph.textContent = GLYPH[current];
      word.textContent = WORD[current];
      button.setAttribute("aria-label", describe(current));
      button.setAttribute("title", describe(current));
    }

    button.addEventListener("click", function () {
      current = next(current);
      apply(current);
      remember(current);
      paint();
    });

    paint();
    nav.appendChild(button);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", build);
  } else {
    build();
  }
})();
