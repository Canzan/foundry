# Acceptance brief: ui\-page\-frame

## Slices

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
Moving between the signed-in pages (Home, the board, Teams, invites, tokens, settings), the user sees each page's content begin at the same left edge beside the nav, so the content never jumps sideways from page to page.
```

```text
A user sees only the pages she visits: a signed-in page outside the sampled set, or a page in an empty, error or loading state, can still start at a different edge, and she cannot see whether a page added later keeps the edge.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
On every signed-in page the gap between the top of the page frame and the first line of content is the same, so the first heading sits at the same height on each page.
```

```text
The user judges the top gap on a page as she first lands on it; she cannot see whether the gap holds while the page is scrolled, with a long or wrapped page title, or when a banner or flash message is present above the content.
```

None.

None.

None.

#### E3

##### Promises

P1

```text
At desk width and on a phone the content keeps the same frame: it fills the same width as the other pages, is not clipped, and needs no sideways scrolling to read.
```

```text
Only the two widths she tries are visible to her: tablet and in-between widths, landscape on a phone, browser zoom and a larger system font are unobserved, and 'same width' is not something she can measure by eye.
```

None.

None.

None.

#### E4

##### Promises

P1

```text
Home no longer shows a narrow centred column: its content starts at the same left edge as the other pages and its cards span the frame width (confirmed decision 2026-11-22).
```

```text
She sees the card row width on her own window only; she cannot see whether a wider or much narrower window keeps the cards inside the frame, nor whether a page with a single card still reads as intended.
```

None.

None.

None.

#### E5

##### Promises

P2

```text
A short text field on an app form (for example a project key, a name or a number) is drawn narrow, about as wide as the value it holds, instead of stretching across the whole page beside its label.
```

```text
'As wide as what it holds' has no stated measure she can check: she cannot see whether the field still fits the longest value the form accepts, and a field that is too narrow for real content looks the same to her as one that is correctly sized.
```

None.

None.

None.

#### E6

##### Promises

P2

```text
Every list (drop-down), number and file control on the app's forms is drawn in the same style as Foundry's other controls, so a form reads as one set of Foundry controls and not a mix of Foundry fields and plain browser widgets.
```

```text
She sees one browser on one operating system: the native look of these controls differs elsewhere, and the open drop-down list, the file picker, the number spinner and the focused, disabled and error states can still fall back to the browser default without her noticing on the closed, resting form.
```

None.

None.

None.

#### E7

##### Promises

P3

```text
On a page that holds several forms, each form appears as its own visible section under its own heading, so she can tell where one form ends and the next begins.
```

```text
She can see that headings exist and that sections are separated, but not whether each heading names the right form, nor the heading order a screen reader announces.
```

None.

None.

None.

#### E8

##### Promises

P3

```text
Each section's action button sits with that section's own fields, so no button is left over at the end of the page away from the fields it belongs to; Teams shows no orphaned Save.
```

```text
Position does not show binding: she cannot see which form a button actually submits, and she learns a wrong pairing only by pressing it and watching the result.
```

None.

None.

None.

#### E9

##### Promises

P3

```text
Instance admin shows its rename forms as sections rather than nested bullet lists, and settings shows labelled sections rather than raw browser fieldsets with their default boxes and legends.
```

```text
Only the three named pages are promised: she cannot see whether any other page with several forms still shows bullet lists, raw fieldsets or an orphaned button.
```

None.

None.

None.

#### E10

##### Promises

P4

```text
In the issue dialog Delete reads as a quiet, secondary control: the dialog's main action is clearly the more prominent one, and Delete is not what her eye or a stray Enter lands on first.
```

```text
'Quiet' is a judgement, not a value she can read off the screen: prominence depends on her own perception, and the keyboard default and focus order of the dialog are not visible from how the buttons look.
```

None.

None.

None.

#### E11

##### Promises

P4

```text
Pressing Delete in the issue dialog still asks her to confirm before anything is deleted, and declining the confirmation leaves the issue in place.
```

```text
Seeing the confirmation appear does not show her the outcome: whether declining truly keeps the issue and confirming truly deletes it is only visible after she returns to the board, and dismissing by Escape or by clicking outside is a separate path she may never try.
```

None.

None.

None.

#### E12

##### Promises

P5

```text
The board header shows the project key as a labelled value: the label 'Key prefix' with the key beside it, read as label and value rather than one run of plain text.
```

```text
She can see the label and the key, but not whether the pairing is announced as a label and value to a screen reader, nor how the header reads with an unusually long key.
```

None.

None.

None.

#### E13

##### Promises

P5

```text
In the board header Change report and Integrations appear as two separate, clearly drawn controls with visible separation, not two underlined links running together into one word.
```

```text
A resting header does not show her the hover, focus and pressed states or the size of each control's clickable area, so two visually separate labels can still be hard to hit apart, and a narrow phone header may wrap them back together.
```

None.

None.

None.

## Charter

None.

## Examination

None.
