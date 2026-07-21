// @vitest-environment jsdom
import "../../test/setup";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppShellLayout, type AppShellLayoutProps } from "./AppShellLayout";

function setResponsiveState({ drawerOverlay = false }: { drawerOverlay?: boolean } = {}): void {
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn((query: string) => ({
      addEventListener: () => undefined,
      dispatchEvent: () => false,
      matches: drawerOverlay,
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
    onCloseDrawer: vi.fn(),
    onCloseModal: vi.fn(),
    ...overrides,
  };
}

beforeEach(() => setResponsiveState());

describe("AppShellLayout", () => {
  it("renders a single full-width task column when the drawer is absent", () => {
    const { container } = render(<AppShellLayout {...createProps()} />);

    expect(window.matchMedia).toHaveBeenCalledWith("(max-width: 1240px)");
    const layout = container.querySelector<HTMLElement>("[data-app-shell-layout]");
    expect(layout?.dataset.drawerOpen).toBe("false");
    expect(screen.getByRole("main")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Task action" })).toBeTruthy();
    expect(container.querySelector(".task-shell-layout__sidebar")).toBeNull();
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
    setResponsiveState({ drawerOverlay: true });
    const onCloseDrawer = vi.fn();
    const onCloseModal = vi.fn();
    const { container } = render(
      <AppShellLayout
        {...createProps({
          drawer: <section aria-label="Files" role="dialog">Drawer</section>,
          modal: (
            <section aria-label="Settings" role="dialog">
              <button type="button">Close settings</button>
            </section>
          ),
          onCloseDrawer,
          onCloseModal,
        })}
      />,
    );

    const settings = screen.getByRole("dialog", { name: "Settings" });
    expect(settings.closest("[data-modal-layer]")).toBeTruthy();
    expect(settings.closest("[data-shell-workspace]")).toBeNull();

    fireEvent.keyDown(document, { key: "Escape" });
    expect(onCloseModal).toHaveBeenCalledOnce();
    expect(onCloseDrawer).not.toHaveBeenCalled();

    const modalScrim = container.querySelector<HTMLButtonElement>(".task-shell-layout__scrim--modal")!;
    expect(modalScrim.tabIndex).toBe(-1);
    expect(modalScrim.getAttribute("aria-hidden")).toBe("true");
    fireEvent.click(modalScrim);
    expect(onCloseModal).toHaveBeenCalledTimes(2);
    expect(container.querySelector("[data-modal-layer]")).toBeTruthy();
  });
});
