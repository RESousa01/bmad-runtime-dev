# Accessibility contract

**Task:** Phase 5, D1 — the governing contract `FE-28` is tested against.
**Companions:** [state-matrix.md](state-matrix.md),
[platform-shortcut-contract.md](platform-shortcut-contract.md).

This is not a checklist of WCAG clauses. It is the set of rules this app has
broken, written so that breaking them again fails a test.

## Focus destinations

Every one of these has a defined destination. "Wherever the browser puts it" is
how focus ends up on `<body>`, after which the next keystroke goes nowhere and a
keyboard user has to Tab in from the top of the document.

| Event | Focus goes to |
|---|---|
| Panel or drawer opens | The panel's heading |
| Panel or drawer closes | The control that opened it |
| Recovery review opens | The review's heading |
| Recovery review closes | The control that prepared it |
| Task switch | The composer |
| Native file dialog returns | The control that opened it — Add file or Add folder |
| Command executes from the palette | Whatever the command's own surface defines; the palette does not restore |
| Attachment error appears | Nothing moves. The row is announced, not focused |
| Model-context warning appears | Nothing moves |
| Overflowed tab selected from the menu | The tab it becomes, which is why the active tab never overflows |
| Modal dismissed | The trigger |

Two rules govern restoration, and both come from real failures:

**Restoration goes through `useDeferredFocusReturn` or `useFocusReturnOnUnmount`.**
Not a hand-rolled `focus()` in an effect cleanup. Two hand-rolled copies had
already drifted before FE-10 created the helper, and a third was found in
`RecoveryReview` calling `focus()` from a cleanup guarded by an `isConnected`
check made at that instant. Passive effects are flushed ahead of the DOM
mutations of any render the same commit schedules synchronously, so a
`useLayoutEffect` writing state *anywhere in the shell* could make that check
run against a half-applied DOM, decide the target was gone, and give up. Adding
a layout effect to the title bar was enough to break it.

**A return is offered, never imposed.** If some other surface has claimed focus
since, taking it back pulls the user out of whatever they just opened — the
worse of the two bugs. `focusIsUnclaimed` is the gate.

## Announcements

- A live region announces **state changes the user did not cause**. It does not
  narrate their own actions.
- `role="status"` is for the two task states that are about the user —
  `awaiting_review` and `failed`. Announcing every transition to "Working" makes
  the region noise, and the announcement that matters then arrives inside it.
- An announcement names its subject. "Run completed" with no identity is not
  evidence, it is a rumour about some.
- Scope announcements to the surface that changed. `aria-relevant="additions"`
  on the transcript, so appending a turn does not re-read the conversation.

## Naming

- Every interactive element has an accessible name, and repeated controls are
  named by their subject: "Remove `src/a.ts`", not "Remove". A column of
  identical names is one a screen-reader user cannot choose between.
- A control's accessible name contains its visible text (WCAG 2.5.3). The tab
  overflow trigger reads "3 more open tasks" and shows "3 more".
- Section titles are headings, not bold text. Heading navigation is how a
  screen-reader user finds where they just landed.
- `aria-setsize` and `aria-posinset` count the whole set, not the part that
  fits. Tabs that overflow are still tabs.

## Colour, size, motion

- **No information by colour alone.** A status dot carries a label beside it.
  The two toned task statuses (`action`, `warning`) both also change their text.
- **Contrast is measured, not judged.** 4.5:1 for body text, 3:1 for large text
  and for the bounds of interactive controls, in **both** themes.
  `styles/contrast.test.ts` parses the shipped stylesheets and computes the
  ratios, because a restated table of colours agrees with the CSS on the day it
  is written and stops agreeing silently. It caught the light theme running the
  dark theme's semantic palette — warning text at 1.57:1 — which no amount of
  looking had.
- **28px minimum hit target.** The title bar's 28px controls are the floor, not
  the exception.
- **No 10px text carries critical meaning.** 10px uppercase is the label scale:
  it names a thing whose value is stated at a readable size next to it.
- **`prefers-reduced-motion: reduce` is honoured**, including `scroll-behavior`,
  which people forget because it is not called an animation. Reduced means
  reduced: an instant state change is usually right, but removing a loading
  indicator's motion entirely can leave no sign that anything is happening.
- **`forced-colors: active` is a supported mode.** A control whose only
  affordance is a background colour disappears in Windows high contrast.

## Keyboard

- Every pointer affordance has a keyboard route. A control reachable only by
  hovering is not reachable.
- Composite widgets take one tab stop and move within it by arrows: the file
  tree, the tab strip, the overflow menu. One tab stop per row turns a 200-file
  tree into a 200-stop tab sequence.
- `Escape` dismisses the innermost dismissible thing, and dismissing returns
  focus to its trigger.
- A decision cannot be dismissed by browsing away from it. The Review Center
  pins the surface to a pending approval, because a decision the user can walk
  away from is one they can lose track of.
