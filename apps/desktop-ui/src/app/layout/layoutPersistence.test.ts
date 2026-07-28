import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  clampToBounds,
  layoutSchemaVersion,
  loadLayout,
  resolveRegionSize,
  saveLayout,
  type LayoutStore,
  type RegionBounds,
} from "./layoutPersistence";

const bounds: RegionBounds = { min: 320, max: 720, initial: 420 };

function memoryStorage(seed?: string): LayoutStore {
  let held: string | null = seed ?? null;
  return {
    read: () => held,
    write: (value) => { held = value; },
  };
}

describe("clampToBounds", () => {
  it("never returns a size below the usable minimum", () => {
    // FE-08 acceptance: no panel renders below its usable minimum width.
    expect(clampToBounds(10, bounds)).toBe(bounds.min);
    expect(clampToBounds(-500, bounds)).toBe(bounds.min);
  });

  it("never returns a size above the maximum", () => {
    expect(clampToBounds(5000, bounds)).toBe(bounds.max);
  });

  it("falls back to the default for a value that is not a number", () => {
    expect(clampToBounds(Number.NaN, bounds)).toBe(bounds.initial);
    expect(clampToBounds(Number.POSITIVE_INFINITY, bounds)).toBe(bounds.initial);
  });
});

describe("loadLayout", () => {
  it("returns an empty layout when nothing is stored", () => {
    expect(loadLayout(memoryStorage()).sizes).toEqual({});
  });

  it("discards a layout written by a different schema version", () => {
    // FE-08 acceptance: an old persisted layout falls back safely. These are
    // window sizes, so re-deriving costs one drag and a bad migration costs a
    // confusing session.
    const stored = JSON.stringify({ version: layoutSchemaVersion + 1, sizes: { inspector: 500 } });
    expect(loadLayout(memoryStorage(stored)).sizes).toEqual({});
  });

  it("discards unparseable JSON rather than throwing", () => {
    expect(loadLayout(memoryStorage("{ not json")).sizes).toEqual({});
  });

  it("discards a layout whose shape is wrong", () => {
    for (const stored of ["[]", '"a string"', "42", JSON.stringify({ version: layoutSchemaVersion })]) {
      expect(loadLayout(memoryStorage(stored)).sizes).toEqual({});
    }
  });

  it("keeps the good entries when only one region is corrupt", () => {
    // A user who has dragged four panels should not lose three because the
    // fourth was written during a crash.
    const stored = JSON.stringify({
      version: layoutSchemaVersion,
      sizes: { inspector: 500, navigator: "wide", panel: Number.NaN, bottom: -3 },
    });
    expect(loadLayout(memoryStorage(stored)).sizes).toEqual({ inspector: 500 });
  });

  it("survives a store that throws on read", () => {
    const hostile: LayoutStore = {
      read: () => { throw new Error("denied"); },
      write: () => undefined,
    };
    expect(loadLayout(hostile).sizes).toEqual({});
  });
});

describe("saveLayout", () => {
  it("round-trips through storage", () => {
    const storage = memoryStorage();
    saveLayout({ sizes: { inspector: 480 } }, storage);
    expect(loadLayout(storage).sizes).toEqual({ inspector: 480 });
  });

  it("does not throw when the store refuses the write", () => {
    const full: LayoutStore = {
      read: () => null,
      write: () => { throw new Error("refused"); },
    };
    expect(() => saveLayout({ sizes: { inspector: 480 } }, full)).not.toThrow();
  });
});

describe("resolveRegionSize", () => {
  beforeEach(() => { vi.restoreAllMocks(); });

  it("uses the default when the region has never been sized", () => {
    expect(resolveRegionSize({ sizes: {} }, "inspector", bounds)).toBe(bounds.initial);
  });

  it("clamps a stored size that is legal no longer", () => {
    // Bounds move between builds. A width stored when the minimum was 200 must
    // not win against a minimum that is now 320.
    expect(resolveRegionSize({ sizes: { inspector: 200 } }, "inspector", bounds)).toBe(bounds.min);
    expect(resolveRegionSize({ sizes: { inspector: 9000 } }, "inspector", bounds)).toBe(bounds.max);
  });

  it("uses a stored size that is still usable", () => {
    expect(resolveRegionSize({ sizes: { inspector: 500 } }, "inspector", bounds)).toBe(500);
  });
});
