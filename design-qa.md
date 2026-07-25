# Design QA — Forge simplified composer

## Visual sources

- Removed authority-profile reference: `C:\Users\rodri\AppData\Local\Temp\codex-clipboard-a4b2eda3-542b-4e26-a016-a7d4b27db8d7.png` (379 × 309).
- Composer reference: `C:\Users\rodri\AppData\Local\Temp\codex-clipboard-dc0af350-f1bb-4fdc-98ab-72c69a8d3e51.png`.
- Codex selector reference: `C:\Users\rodri\AppData\Local\Temp\codex-clipboard-e11fe669-e49a-4946-9df9-decb88c3c328.png`.
- Verified local implementation: `http://127.0.0.1:1420/`.
- Browser state: dark theme, browser-preview task, empty context, compact agent/model selector open.

## Result

The authority-profile control and its Observe / Assist / Build popover are absent. The native host still enforces its existing grants internally, but those implementation details no longer compete with the task flow.

The composer now presents only the controls needed to work:

- a compact context strip;
- one direct add-files-or-folder control;
- the task input;
- a two-row Agent / Model selector;
- one Send action.

There is no `Review request` action. A message without local attachments follows one send path. When local files are attached, Forge shows one concise `Send attached context?` confirmation listing only those files before sending.

## Reference comparison

The supplied authority-profile capture and the current selector-open task state were inspected together. The old state devoted an entire dense card to three internal permission tiers and explanatory policy copy. The current state replaces that surface with a small two-row selector and leaves the surrounding workspace visually quiet. No authority vocabulary, selected-profile badge, or redundant grant explanation remains.

The task surface was also checked at the normal desktop viewport for overflow, cropped controls, incorrect padding, heavy borders, and inconsistent radii. No P0 or P1 visual defects were found.

## Interaction evidence

- The rendered task DOM contains neither `Authority profile` nor `Review request`.
- `Agent and model settings` opens the compact selector.
- The selector contains only `Agent` and `Model`.
- The composer attachment control is announced as `Add files or folder`.
- In the native host, that control offers `Add files…` and `Add folder…`; it does not navigate to Files.
- Browser preview keeps native file picking disabled because it has no device grant.
- The final browser pass reported no console warnings or errors.
- The Docker preview returns HTTP 200 and its container reports healthy.
- The complete source gate passed, including 36 frontend files / 382 tests and the production build.
- The all-feature locked Rust workspace suite passed with zero failures.

## Remaining proof boundary

The browser preview cannot display an operating-system file/folder chooser. Its renderer behavior, command contract, bounded workspace resolution, context preparation, and native host integration are covered by frontend integration and Rust tests.

final result: passed
