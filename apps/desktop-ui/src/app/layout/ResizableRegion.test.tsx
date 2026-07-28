import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { ResizableRegion } from "./ResizableRegion";
import { loadLayout, resetLayoutStore, setLayoutStore, type RegionBounds } from "./layoutPersistence";

const bounds: RegionBounds = { min: 320, max: 720, initial: 420 };

describe("ResizableRegion", () => {
  beforeEach(() => { resetLayoutStore(); });

  it("exposes a separator that says where it is within its range", () => {
    render(<ResizableRegion bounds={bounds} label="Resize inspector" regionId="inspector">x</ResizableRegion>);
    const handle = screen.getByRole("separator", { name: "Resize inspector" });
    expect(handle.getAttribute("aria-valuemin")).toBe("320");
    expect(handle.getAttribute("aria-valuemax")).toBe("720");
    expect(handle.getAttribute("aria-valuenow")).toBe("420");
  });

  it("resizes from the keyboard, not only by dragging", async () => {
    const user = userEvent.setup();
    render(<ResizableRegion bounds={bounds} label="Resize inspector" regionId="inspector">x</ResizableRegion>);
    const handle = screen.getByRole("separator", { name: "Resize inspector" });

    await user.tab();
    expect(handle).toBe(document.activeElement);
    await user.keyboard("{ArrowLeft}");
    expect(handle.getAttribute("aria-valuenow")).toBe("436");
  });

  it("cannot be driven below the usable minimum", async () => {
    const user = userEvent.setup();
    render(<ResizableRegion bounds={bounds} label="Resize inspector" regionId="inspector">x</ResizableRegion>);
    const handle = screen.getByRole("separator", { name: "Resize inspector" });
    await user.tab();
    await user.keyboard("{Home}");
    expect(handle.getAttribute("aria-valuenow")).toBe("320");
    await user.keyboard("{ArrowRight}");
    expect(handle.getAttribute("aria-valuenow")).toBe("320");
  });

  it("remembers the size, and re-clamps one that is no longer legal", async () => {
    const user = userEvent.setup();
    const { unmount } = render(
      <ResizableRegion bounds={bounds} label="Resize inspector" regionId="inspector">x</ResizableRegion>,
    );
    await user.tab();
    await user.keyboard("{End}");
    expect(loadLayout().sizes["inspector"]).toBe(720);
    unmount();

    // A later build narrows the range; the stored width must not win.
    render(
      <ResizableRegion bounds={{ min: 200, max: 500, initial: 300 }} label="Resize inspector" regionId="inspector">
        x
      </ResizableRegion>,
    );
    expect(screen.getByRole("separator").getAttribute("aria-valuenow")).toBe("500");
  });

  it("opens at the default when the stored layout is corrupt", () => {
    setLayoutStore({ read: () => "{ not json", write: () => undefined });
    render(<ResizableRegion bounds={bounds} label="Resize inspector" regionId="inspector">x</ResizableRegion>);
    expect(screen.getByRole("separator").getAttribute("aria-valuenow")).toBe("420");
  });
});
