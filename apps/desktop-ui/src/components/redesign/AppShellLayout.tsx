import {
  useEffect,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  containModalPanelFocus,
  useModalPanelFocus,
} from "../../lib/panelFocus";

export const DRAWER_OVERLAY_QUERY = "(max-width: 1240px)";
export const SIDEBAR_OVERLAY_QUERY = "(max-width: 820px)";

export interface AppShellLayoutProps {
  drawer?: ReactNode;
  main: ReactNode;
  mobileSidebarOpen: boolean;
  modal?: ReactNode;
  onCloseDrawer: () => void;
  onCloseModal: () => void;
  onCloseSidebar: () => void;
  sidebar: ReactNode;
}

function readMediaQuery(query: string): boolean {
  return typeof window !== "undefined" && window.matchMedia(query).matches;
}

function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() => readMediaQuery(query));

  useEffect(() => {
    const mediaQuery = window.matchMedia(query);
    const update = () => setMatches(mediaQuery.matches);
    update();
    mediaQuery.addEventListener("change", update);
    return () => mediaQuery.removeEventListener("change", update);
  }, [query]);

  return matches;
}

export function AppShellLayout({
  drawer,
  main,
  mobileSidebarOpen,
  modal,
  onCloseDrawer,
  onCloseModal,
  onCloseSidebar,
  sidebar,
}: AppShellLayoutProps) {
  const drawerOverlay = useMediaQuery(DRAWER_OVERLAY_QUERY);
  const sidebarOverlay = useMediaQuery(SIDEBAR_OVERLAY_QUERY);
  const hasDrawer = drawer != null;
  const hasModal = modal != null;
  const drawerModalOpen = drawerOverlay && hasDrawer;
  const sidebarModalOpen = sidebarOverlay && mobileSidebarOpen;
  const drawerIsTopmost = drawerModalOpen && !hasModal;
  const sidebarIsTopmost = sidebarModalOpen && !drawerModalOpen && !hasModal;
  const sidebarRef = useModalPanelFocus(sidebarIsTopmost);
  const drawerRef = useModalPanelFocus(drawerIsTopmost);
  const modalRef = useModalPanelFocus(hasModal);

  useEffect(() => {
    if (!hasModal && !drawerModalOpen && !sidebarModalOpen) {
      return;
    }

    const closeTopmost = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      if (hasModal) {
        onCloseModal();
      } else if (drawerModalOpen) {
        onCloseDrawer();
      } else {
        onCloseSidebar();
      }
    };

    document.addEventListener("keydown", closeTopmost);
    return () => document.removeEventListener("keydown", closeTopmost);
  }, [
    drawerModalOpen,
    hasModal,
    onCloseDrawer,
    onCloseModal,
    onCloseSidebar,
    sidebarModalOpen,
  ]);

  const containSidebarFocus = (event: KeyboardEvent<HTMLElement>) => {
    containModalPanelFocus(event, sidebarRef, sidebarIsTopmost);
  };
  const containDrawerFocus = (event: KeyboardEvent<HTMLElement>) => {
    containModalPanelFocus(event, drawerRef, drawerIsTopmost);
  };
  const containModalFocus = (event: KeyboardEvent<HTMLElement>) => {
    containModalPanelFocus(event, modalRef, hasModal);
  };
  const mainBlocked = hasModal || drawerModalOpen || sidebarModalOpen;
  const sidebarBlocked = hasModal
    || drawerModalOpen
    || (sidebarOverlay && !mobileSidebarOpen);

  return (
    <div
      className="task-shell-layout"
      data-app-shell-layout=""
      data-drawer-open={hasDrawer}
      data-drawer-overlay={drawerModalOpen}
      data-sidebar-overlay={sidebarOverlay}
    >
      <div className="task-shell-layout__workspace" data-shell-workspace="">
        <aside
          aria-hidden={sidebarBlocked || undefined}
          aria-label="Task navigation"
          aria-modal={sidebarModalOpen || undefined}
          className="task-shell-layout__sidebar"
          data-open={!sidebarOverlay || mobileSidebarOpen}
          inert={sidebarBlocked || undefined}
          onKeyDown={containSidebarFocus}
          ref={sidebarRef}
          role={sidebarOverlay ? "dialog" : undefined}
        >
          {sidebarOverlay ? (
            <button
              aria-label="Close task navigation"
              className="task-shell-layout__close"
              onClick={onCloseSidebar}
              type="button"
            >
              <span aria-hidden="true">×</span>
            </button>
          ) : null}
          {sidebar}
        </aside>

        <div
          aria-hidden={mainBlocked || undefined}
          className="task-shell-layout__main"
          inert={mainBlocked || undefined}
        >
          {main}
        </div>

        {hasDrawer ? (
          <section
            aria-hidden={hasModal || undefined}
            className="task-shell-layout__drawer"
            inert={hasModal || undefined}
            onKeyDown={containDrawerFocus}
            ref={drawerRef}
          >
            {drawer}
          </section>
        ) : null}
      </div>

      {sidebarModalOpen ? (
        <button
          aria-hidden="true"
          className="task-shell-layout__scrim task-shell-layout__scrim--sidebar"
          inert={(drawerModalOpen || hasModal) || undefined}
          onClick={onCloseSidebar}
          tabIndex={-1}
          type="button"
        />
      ) : null}

      {drawerModalOpen ? (
        <button
          aria-hidden="true"
          className="task-shell-layout__scrim task-shell-layout__scrim--drawer"
          inert={hasModal || undefined}
          onClick={onCloseDrawer}
          tabIndex={-1}
          type="button"
        />
      ) : null}

      {hasModal ? (
        <>
          <button
            aria-hidden="true"
            className="task-shell-layout__scrim task-shell-layout__scrim--modal"
            onClick={onCloseModal}
            tabIndex={-1}
            type="button"
          />
          <section
            className="task-shell-layout__modal"
            data-modal-layer=""
            onKeyDown={containModalFocus}
            ref={modalRef}
          >
            {modal}
          </section>
        </>
      ) : null}
    </div>
  );
}
