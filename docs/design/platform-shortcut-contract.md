# Platform shortcut contract

**Task:** Phase 5, D1 — the governing contract `FE-28` is tested against.
**Companions:** [state-matrix.md](state-matrix.md),
[accessibility-contract.md](accessibility-contract.md).

Forge ships as a Windows desktop app. Everything below exists because a shortcut
that is displayed differently from how it behaves is worse than no shortcut: the
user learns the wrong thing and blames themselves when it does not work.

## Resolution and presentation are separate

A binding is declared once, platform-neutrally, in
`apps/desktop-ui/src/app/commands/keybindings.ts`:

```
Mod+Enter        Mod+Shift+P        Mod+K
```

`Mod` is not a platform. It is resolved twice, for two different questions:

| Question | Function | Windows | macOS |
|---|---|---|---|
| Did the user press it? | `matchesChord` | `ctrlKey` | `metaKey` |
| What do we call it? | `formatKeybinding` | `Ctrl+K` | `⌘K` |

Both come from the same `Chord`, which is what makes them unable to disagree.
Two independent tables — one for matching, one for labels — is how a menu comes
to advertise a chord that does nothing.

**Windows labels are words, joined by `+`.** `Ctrl+Shift+P`. Not `⌘⇧P`, and not
`Ctrl⇧P`: the glyphs are Apple keyboard conventions, and a Windows user reading
`⌘` has to translate it before they can use it. macOS labels are glyphs, joined
by nothing, because that is the convention there.

`Enter` is spelled out on Windows and rendered `⏎` on macOS for the same reason.

## The other modifier must be absent

`matchesChord` requires the *non*-platform modifier to be up. Without that,
`Ctrl+Enter` on macOS fires a `Cmd+Enter` binding and vice versa — the user
presses something they did not mean and gets an action they did not ask for.

## One chord, one command

`findKeybindingConflicts` reports two commands claiming the same chord, and the
build fails on it. It is reported rather than resolved: silently letting the
first or the last registration win is how a shortcut starts doing something
other than what its own menu item says, with nothing anywhere recording the
choice.

## Display comes from the registry

A shortcut label is never typed into a component. It is read from the command
registry through `formatKeybinding`, so a rebinding changes the label everywhere
it appears, and a command with no binding shows none rather than a stale one.

The same applies to availability: a command the registry marks `unavailable`
carries a **required** `reason`, so the palette can say why a thing is greyed
out instead of leaving the user to guess. The compiler enforces that a reason
exists, because "unavailable" with no explanation is the state that sends people
to Settings to fix something that is somewhere else.

## Where shortcuts do not apply

- **Inside a text field**, a single-key shortcut does not fire. The composer is
  a text field; `P` means the letter P.
- **While a native dialog is open**, the renderer has no focus and no shortcut
  can fire. The composer's state machine models the dialog explicitly for this
  reason.
- **A destructive action gets no single-key shortcut.** Discarding a proposal
  and deleting a task are reached through their controls.
