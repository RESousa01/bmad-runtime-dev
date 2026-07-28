/**
 * The shell chrome: title bar, command palette, and the workbench region.
 *
 * FE-10 step 2. What this isolates is the modal-containment contract, which was
 * previously spread across three places in App's JSX and one comment.
 *
 * The palette renders outside `AppShellLayout`, so the layout's own modal
 * bookkeeping does not cover it. `aria-modal="true"` on the palette is a claim
 * about the rest of the page rather than about the palette, and marking the
 * workbench `inert` is the other half of keeping it -- without it, Tab walked
 * out of the "modal" dialog and into the title bar behind it. The title bar
 * sits outside the inert region (it is a sibling, not a child), so it is told
 * separately.
 *
 * Keeping those two facts adjacent is the point: they have to change together,
 * and when they were apart one of them was already wrong.
 */

import type { ReactNode } from "react";

import { AppShellLayout } from "../components/redesign/AppShellLayout";
import { TitleBar, type TitleBarTaskTab } from "../components/TitleBar";

export interface AppWorkbenchProps {
  /** True while any overlay owns the page: a modal, the palette, an overlay drawer. */
  readonly shellOverlayOpen: boolean;
  /**
   * True only for the palette. Narrower than `shellOverlayOpen` on purpose:
   * `AppShellLayout` already contains focus for the drawer and the utility
   * panels, so marking the workbench inert for those would be a second,
   * conflicting containment.
   */
  readonly paletteOpen: boolean;
  readonly commandPalette: ReactNode;
  readonly drawer?: ReactNode;
  readonly main: ReactNode;
  readonly modal?: ReactNode;
  readonly onCloseDrawer: () => void;
  readonly onCloseModal: () => void;
  readonly titleBar: {
    readonly activeTaskId?: string | undefined;
    readonly taskTabs: readonly TitleBarTaskTab[];
    readonly onHome: () => void;
    readonly onMenu: () => void;
    readonly onNewTask: () => void;
    readonly onCloseTask: (id: string) => void;
    readonly onSelectTask: (id: string) => void;
    readonly onOpenSettings: () => void;
  };
}

export function AppWorkbench({
  commandPalette,
  drawer,
  main,
  modal,
  onCloseDrawer,
  onCloseModal,
  paletteOpen,
  shellOverlayOpen,
  titleBar,
}: AppWorkbenchProps) {
  return (
    <div className="app-shell app-shell--forge-v2">
      {commandPalette}
      <TitleBar
        activeTaskId={titleBar.activeTaskId}
        isInert={shellOverlayOpen}
        onCloseTask={titleBar.onCloseTask}
        onHome={titleBar.onHome}
        onMenu={titleBar.onMenu}
        onNewTask={titleBar.onNewTask}
        onOpenSettings={titleBar.onOpenSettings}
        onSelectTask={titleBar.onSelectTask}
        taskTabs={titleBar.taskTabs}
      />
      <div
        aria-hidden={paletteOpen || undefined}
        className="workbench workbench--task-shell"
        inert={paletteOpen || undefined}
      >
        <AppShellLayout
          drawer={drawer}
          main={main}
          modal={modal}
          onCloseDrawer={onCloseDrawer}
          onCloseModal={onCloseModal}
        />
      </div>
    </div>
  );
}
