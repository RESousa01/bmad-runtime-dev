# Renderer baseline — FE-01

**Recorded:** 2026-07-28
**Backlog item:** FE-01, phase 0, no dependencies
**Purpose:** an authoritative visual and behavioural baseline, so later phases
can delete legacy CSS and move components against something rather than against
memory.

## Acceptance, and how each is met

**1. Every redesigned surface has one named baseline state and owner.**

| Surface | Named state | Owner | Reachable in browser preview |
|---|---|---|---|
| Home | empty — no tasks, one sample project | `HomeView.tsx` | yes |
| Task shell | empty — "What do you want to build?" | `TaskWorkspace.tsx` | yes, via the tab strip `+` |
| Transcript | user turn, agent turn, tool call, agent turn | `TaskWorkspace.tsx` → `ChatTranscript` | no — see fixture below |
| Review | sample-data git diff, 3 files | `GitReviewPanel.tsx`, `review-panel.css` | yes, via Review in the task header |
| Approval | proposal awaiting decision | `GovernedChangesPanel.tsx` | no — needs a host proposal |
| Settings | General pane | `settings/SettingsDialog.tsx` | yes |
| Context drawer | tab strip with Review open | `redesign/ContextDrawer.tsx` | yes |

**2. No screenshot is accepted without a reproducible fixture.**

Screenshots are not the baseline; **computed styles are**. The reproducible
fixture is a DOM walk that reads the properties that matter and compares them,
which is stronger than an image because it says *which* property moved.

For a surface the preview cannot reach, inject representative markup and read
computed styles off it. The transcript baseline was captured exactly that way —
the preview blocks sending, so a transcript never populates on its own — and
doing so found three defects that were already shipping.

The measurements below are the frozen baseline. Anything that changes them is a
deliberate change, not a side effect.

| Role | Value | Token |
|---|---|---|
| Transcript prose | 13px / line-height 1.7 | `--text-body`, `--lh-prose` |
| Uppercase section label | 10px / weight 500 / 0.12em | `--text-label`, `--label-weight`, `--label-tracking` |
| Metadata, timestamps | 11px | `--text-meta` |
| Interactive labels | 12px | `--text-ui` |
| Composer input | 14px | `--text-input` |
| Surface title | 17px | `--text-title` |
| Empty-state title | 24px | `--text-display` |
| Radii | 4 / 6 / 10px, plus the pill for true statuses only | `--radius-row`, `--radius-control`, `--radius-overlay` |

Invariants that hold across Home, the task surface and Review:

- every rendered text element computes to one of the seven steps above;
- every radius is 0, 4, 6, 10 or the reserved pill;
- nothing inside Home carries a fill or a radius — only four 1px hairlines;
- nothing on the task surface carries a pill radius.

**3. The baseline explicitly records that Add file must not navigate to Files.**

Stated, and asserted rather than described. Pressing the composer control
invokes the native dialog immediately — no Forge popover, no menu, no workspace
search step, no route change, per design specification 7.4.

`TaskWorkspace.test.tsx` asserts that no `menu` or `menuitem` role exists after
the press. That assertion is mutation-verified: giving the container
`role="menu"` fails it, and removing that passes again.

Folder support is a **sibling control**, not a menu item. Windows has no dialog
mode accepting files and folders together — `IFileOpenDialog` either has
`FOS_PICKFOLDERS` set or it does not — so two controls, each one press from the
dialog it opens, is the shape that satisfies 7.4 without dropping the
capability.

## What the baseline deliberately does not freeze

Control heights. Variant 3a draws a 26px compact control; design specification
3.4 requires a 28px minimum interactive hit target. There is no evidence about
which of 3a's 26px elements are interactive, and trading an accessibility floor
for two pixels on a guess is not a baseline decision. The shell keeps 30px until
that is settled.

## Verification

`pnpm --filter @forge/desktop-ui test` and
`pnpm --filter @forge/desktop-ui typecheck`, which are the commands FE-01 names.
550 frontend tests across 42 files.
