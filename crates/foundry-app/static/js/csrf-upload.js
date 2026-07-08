// comment-add-csrf 01-02 — CSRF-carrying multipart upload for issue attachments.
//
// App-owned, self-contained, CSP-safe: loaded as an external same-origin script
// (no inline handlers), all wiring via addEventListener. A plain multipart HTML
// form CANNOT set a request header, but `csrf_middleware` requires the CSRF
// double-submit token in the `x-csrf-token` header for multipart POSTs (the
// urlencoded `_csrf` body field is only read for urlencoded forms). So on submit
// of the attachment upload form (`[data-csrf-upload]`) this reads the
// non-HttpOnly `foundry_csrf` cookie the issue page minted and mirrors it into
// the `x-csrf-token` header of a fetch POST — the same cookie->header idiom as
// board-dnd.js.
//
// Success (2xx, hx-request) returns the htmx OOB `<li>` row fragment; we append
// it into `[data-attachment-list]` and reset the form. A non-2xx response
// renders the server's error fragment above the form; a network error surfaces a
// short inline message. Progressive enhancement is moot here — without JS a
// browser cannot send the required header at all, so the plain form was never a
// CSRF-workable upload path.
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

  // Append the newly-uploaded attachment row. The success fragment is an OOB
  // envelope (`<div hx-swap-oob=...>`) wrapping the `<li>`; outside htmx the
  // wrapper is inert, so we lift the inner row out and append it directly.
  function appendRow(html) {
    var list = document.querySelector("[data-attachment-list]");
    if (!list) {
      return;
    }
    var tpl = document.createElement("template");
    tpl.innerHTML = html.trim();
    var wrapper = tpl.content.firstElementChild;
    var row = wrapper ? wrapper.firstElementChild || wrapper : null;
    if (row) {
      list.appendChild(row);
    }
  }

  function showError(form, html) {
    var holder = form.querySelector("[data-upload-error]");
    if (!holder) {
      holder = document.createElement("div");
      holder.setAttribute("data-upload-error", "");
      form.insertBefore(holder, form.firstChild);
    }
    holder.innerHTML = html;
  }

  function submitUpload(form) {
    var url = form.getAttribute("action");
    if (!url) {
      return;
    }
    var data = new FormData(form);
    fetch(url, {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "x-csrf-token": readCookie("foundry_csrf"),
        "hx-request": "true"
      },
      body: data
    })
      .then(function (response) {
        return response.text().then(function (body) {
          return { ok: response.ok, body: body };
        });
      })
      .then(function (result) {
        if (result.ok) {
          appendRow(result.body);
          form.reset();
        } else {
          showError(form, result.body);
        }
      })
      .catch(function () {
        showError(form, "Upload failed. Please try again.");
      });
  }

  function init() {
    var forms = document.querySelectorAll("form[data-csrf-upload]");
    for (var i = 0; i < forms.length; i++) {
      forms[i].addEventListener("submit", function (event) {
        event.preventDefault();
        submitUpload(event.currentTarget);
      });
    }
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
