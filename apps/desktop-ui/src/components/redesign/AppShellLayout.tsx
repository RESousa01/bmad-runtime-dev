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

export interface AppShellLayoutProps {
  drawer?: ReactNode;
  main: ReactNode;
  modal?: ReactNode;
  onCloseDrawer: () => void;
  onCloseModal: () => void;
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
  modal,
  onCloseDrawer,
  onCloseModal,
}: AppShellLayoutProps) {
  const drawerOverlay = useMediaQuery(DRAWER_OVERLAY_QUERY);
  const hasDrawer = drawer != null;
  const hasModal = modal != null;
  const drawerModalOpen = drawerOverlay && hasDrawer;
  const drawerIsTopmost = drawerModalOpen && !hasModal;
  const drawerRef = useModalPanelFocus(drawerIsTopmost);
  const modalRef = useModalPanelFocus(hasModal);

  useEffect(() => {
    if (!hasModal && !drawerModalOpen) {
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
      } else {
        onCloseDrawer();
      }
    };

    document.addEventListener("keydown", closeTopmost);
    return () => document.removeEventListener("keydown", closeTopmost);
  }, [drawerModalOpen, hasModal, onCloseDrawer, onCloseModal]);

  const containDrawerFocus = (event: KeyboardEvent<HTMLElement>) => {
    containModalPanelFocus(event, drawerRef, drawerIsTopmost);
  };
  const containModalFocus = (event: KeyboardEvent<HTMLElement>) => {
    containModalPanelFocus(event, modalRef, hasModal);
  };
  const mainBlocked = hasModal || drawerModalOpen;

  return (
    <div
      className="task-shell-layout"
      data-app-shell-layout=""
      data-drawer-open={hasDrawer}
      data-drawer-overlay={drawerModalOpen}
    >
      <div className="task-shell-layout__workspace" data-shell-workspace="">
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
