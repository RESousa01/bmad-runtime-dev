// @vitest-environment jsdom
import "../../test/setup";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppShellLayout, type AppShellLayoutProps } from "./AppShellLayout";

interface ResponsiveState {
  drawerOverlay?: boolean;
  sidebarOverlay?: boolean;
}

function setResponsiveState({
  drawerOverlay = false,
  sidebarOverlay = false,
}: ResponsiveState = {}): void {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string) => ({
      addEventListener: () => undefined,
      dispatchEvent: () => false,
      matches: query.includes("820px") ? sidebarOverlay : drawerOverlay,
      media: query,
      onchange: null,
      removeEventListener: () => undefined,
    })),
    writable: true,
  });
}

function createProps(
  overrides: Partial<AppShellLayoutProps> = {},
): AppShellLayoutProps {
  return {
    main: <main><button type="button">Task action</button></main>,
    mobileSidebarOpen: false,
    onCloseDrawer: vi.fn(),
    onCloseModal: vi.fn(),
    onCloseSidebar: vi.fn(),
    sidebar: <button type="button">New task</button>,
    ...overrides,
  };
}

beforeEach(() => setResponsiveState());

describe("AppShellLayout", () => {
  it("lets the desktop task column expand when the drawer is absent", () => {
    const { container } = render(<AppShellLayout {...createProps()} />);

    expect(window.matchMedia).toHaveBeenCalledWith("(max-width: 1240px)");
    const layout = container.querySelector<HTMLElement>("[data-app-shell-layout]");
    expect(layout?.dataset.drawerOpen).toBe("false");
    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.getByRole("complementary", { name: "Task navigation" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Task action" })).toBeTruthy();
  });

  it("keeps the desktop task present beside an optional drawer", () => {
    const { container } = render(
      <AppShellLayout
        {...createProps({ drawer: <section>Files drawer</section> })}
      />,
    );

    const layout = container.querySelector<HTMLElement>("[data-app-shell-layout]");
    expect(layout?.dataset.drawerOpen).toBe("true");
    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.getByText("Files drawer")).toBeTruthy();
    expect(container.querySelector(".task-shell-layout__scrim--drawer")).toBeNull();
  });

  it("makes the 820px Sidebar a named modal with close, scrim, and focus restoration", () => {
    setResponsiveState({ drawerOverlay: true, sidebarOverlay: true });
    const onCloseSidebar = vi.fn();
    const props = createProps({ onCloseSidebar });
    const { rerender } = render(<AppShellLayout {...props} />);
    const taskAction = screen.getByRole("button", { name: "Task action" });
    taskAction.focus();

    rerender(<AppShellLayout {...props} mobileSidebarOpen />);

    const navigation = screen.getByRole("dialog", { name: "Task navigation" });
    expect(navigation.getAttribute("aria-modal")).toBe("true");
    const closeButton = within(navigation).getByRole("button", {
      name: "Close task navigation",
    });
    expect(document.activeElement).toBe(closeButton);

    const sidebarScrim = document.querySelector<HTMLButtonElement>(".task-shell-layout__scrim--sidebar")!;
    expect(sidebarScrim.tabIndex).toBe(-1);
    expect(sidebarScrim.getAttribute("aria-hidden")).toBe("true");
    fireEvent.click(sidebarScrim);
    expect(onCloseSidebar).toHaveBeenCalledOnce();

    rerender(<AppShellLayout {...props} mobileSidebarOpen={false} />);
    expect(document.activeElement).toBe(taskAction);
  });

  it("layers the desktop-minimum drawer without duplicating its dialog or close control", () => {
    setResponsiveState({ drawerOverlay: true });
    const onCloseDrawer = vi.fn();
    render(
      <AppShellLayout
        {...createProps({
          drawer: (
            <section aria-label="Files" aria-modal="true" role="dialog">
              <button type="button">Close Files</button>
              Drawer content
            </section>
          ),
          onCloseDrawer,
        })}
      />,
    );

    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(screen.getByRole("dialog", { name: "Files" })).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Close Files" })).toHaveLength(1);
    expect(
      screen.getByRole("main", { hidden: true }).closest(".task-shell-layout__main")
        ?.getAttribute("aria-hidden"),
    ).toBe("true");

    const drawerScrim = document.querySelector<HTMLButtonElement>(".task-shell-layout__scrim--drawer")!;
    expect(drawerScrim.tabIndex).toBe(-1);
    expect(drawerScrim.getAttribute("aria-hidden")).toBe("true");
    fireEvent.click(drawerScrim);
    expect(onCloseDrawer).toHaveBeenCalledOnce();
  });

  it("keeps modal content in a separate topmost layer", () => {
    setResponsiveState({ drawerOverlay: true, sidebarOverlay: true });
    const onCloseDrawer = vi.fn();
    const onCloseModal = vi.fn();
    const onCloseSidebar = vi.fn();
    const { container } = render(
      <AppShellLayout
        {...createProps({
          drawer: <section aria-label="Files" role="dialog">Drawer</section>,
          mobileSidebarOpen: true,
          modal: (
            <section aria-label="Settings" role="dialog">
              <button type="button">Close settings</button>
            </section>
          ),
          onCloseDrawer,
          onCloseModal,
          onCloseSidebar,
        })}
      />,
    );

    const settings = screen.getByRole("dialog", { name: "Settings" });
    expect(settings.closest("[data-modal-layer]")).toBeTruthy();
    expect(settings.closest("[data-shell-workspace]")).toBeNull();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCloseModal).toHaveBeenCalledOnce();
    expect(onCloseDrawer).not.toHaveBeenCalled();
    expect(onCloseSidebar).not.toHaveBeenCalled();

    const modalScrim = container.querySelector<HTMLButtonElement>(".task-shell-layout__scrim--modal")!;
    expect(modalScrim.tabIndex).toBe(-1);
    expect(modalScrim.getAttribute("aria-hidden")).toBe("true");
    fireEvent.click(modalScrim);
    expect(onCloseModal).toHaveBeenCalledTimes(2);
    expect(container.querySelector("[data-modal-layer]")).toBeTruthy();
  });
});
