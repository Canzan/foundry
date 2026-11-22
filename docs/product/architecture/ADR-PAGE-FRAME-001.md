# PAGE\-FRAME\-001 The frame owns page geometry
Status: Accepted

## Context

```text
Three stylesheet writers decided one fact, where an app page content starts: the app shell content box, the element rule main { padding: 20px } which applied only to the pages that wrap their content in <main>, and the centred .dash column whose inline start is a function of the window. The operator saw three different left edges (272px, 292px, 424px) and three different top spacings across pages that are meant to read as one application. No checker and no test saw it, because each rule is correct on its own and the defect is a SUM over the boxes on the path from the shell to the page first content box.
```

## Decision

```text
The app shell content box (.app-shell__content) is the ONE padding box between the window and an app page first content box, and it owns the content left edge, the top spacing and the content width. A page supplies content and never its own padding box, centring or width cap. The element rule is scoped to body > main, so a page with no frame still pads itself and a framed page is padded by the frame. The frame sets no maximum content width: the width promise is kept by the frame content box being identical on every page, and readable line length is a field and a section decision instead.
```

## Alternatives

```text
Delete the element rule outright: refused, because four out-of-frame pages (email_stop, unsubscribe_confirm, unsubscribe_result, notifications) render main as a direct child of body and would lose all padding. Override the rule with padding: 0 inside the frame: refused as a guard over an unchanged representation, which leaves two writers in place. Give the frame a centred max-inline-size: refused, because it restores a window-dependent left edge, which is the Home defect. Give the frame a left-aligned max-inline-size: refused, because it narrows the board, whose lanes need the window.
```

## Consequences

```text
Home loses its centred 880px column and its 2.5rem top padding: its content starts at the common edge and spans the frame width. Every app page gains one left edge and one top spacing at both viewports. CSS has no checker, so the rule is centralisation and not proof: the detectors are the all-pages browser reading and review, and a page that does not extend app_shell.html is outside the rule. A Quint module at docs/feature/ui-page-frame/design/page_frame.qnt carries the sum-over-the-path algebra and refutes the three variants that break it.
```
