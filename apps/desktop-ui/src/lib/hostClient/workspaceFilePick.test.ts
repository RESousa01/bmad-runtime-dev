import { describe, expect, it } from "vitest";
import { parseWorkspaceFilePickReply } from "./workspaceProtocol";

const requestId = "req_01ARZ3NDEKTSV4RRFFQ69G5FAV";

function dispatchReply(data: unknown) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    sequence: 4,
    status: "ok",
    receipt: {
      requestId,
      acceptedAt: 1_725_000_000_005,
      operationId: null as string | null,
    },
    data,
  };
}

const pickedValue = {
  workspaceId: "workspace_01ARZ3NDEKTSV4RRFFQ69G5FAV",
  relativePaths: ["src/app.ts", "README.md"],
  selectedCount: 2,
  rejectedOutsideRoot: 1,
  rejectedUnreadable: 0,
  truncated: false,
};

describe("parseWorkspaceFilePickReply", () => {
  it("parses a picked-files reply with rejection counts", () => {
    const parsed = parseWorkspaceFilePickReply(
      dispatchReply({ kind: "picked_files", value: pickedValue }),
      requestId,
    );
    expect(parsed.pick.kind).toBe("picked");
    if (parsed.pick.kind === "picked") {
      expect(parsed.pick.value.relativePaths).toEqual(["src/app.ts", "README.md"]);
      expect(parsed.pick.value.rejectedOutsideRoot).toBe(1);
      expect(parsed.pick.value.selectedCount).toBe(2);
    }
  });

  it("parses a cancelled pick as no_selection", () => {
    const parsed = parseWorkspaceFilePickReply(
      dispatchReply({ kind: "no_selection" }),
      requestId,
    );
    expect(parsed.pick.kind).toBe("no_selection");
  });

  it("rejects a selectedCount that disagrees with the array length", () => {
    expect(() =>
      parseWorkspaceFilePickReply(
        dispatchReply({
          kind: "picked_files",
          value: { ...pickedValue, selectedCount: 5 },
        }),
        requestId,
      ),
    ).toThrow();
  });

  it("rejects an absolute or traversal path", () => {
    expect(() =>
      parseWorkspaceFilePickReply(
        dispatchReply({
          kind: "picked_files",
          value: { ...pickedValue, relativePaths: ["/etc/passwd"], selectedCount: 1 },
        }),
        requestId,
      ),
    ).toThrow();
    expect(() =>
      parseWorkspaceFilePickReply(
        dispatchReply({
          kind: "picked_files",
          value: { ...pickedValue, relativePaths: ["../escape"], selectedCount: 1 },
        }),
        requestId,
      ),
    ).toThrow();
  });

  it("rejects duplicate relative paths", () => {
    expect(() =>
      parseWorkspaceFilePickReply(
        dispatchReply({
          kind: "picked_files",
          value: { ...pickedValue, relativePaths: ["a.ts", "a.ts"], selectedCount: 2 },
        }),
        requestId,
      ),
    ).toThrow();
  });

  it("rejects extra keys on the projection", () => {
    expect(() =>
      parseWorkspaceFilePickReply(
        dispatchReply({
          kind: "picked_files",
          value: { ...pickedValue, absolutePath: "C:/secret" },
        }),
        requestId,
      ),
    ).toThrow();
  });
});
