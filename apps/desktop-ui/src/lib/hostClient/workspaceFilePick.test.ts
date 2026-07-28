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
  rejected: [{ relativePath: null as string | null, reason: "outside_workspace" }],
  rejectedOutsideRoot: 1,
  rejectedUnreadable: 0,
  truncated: false,
};

function picked(rejected: readonly unknown[]) {
  return dispatchReply({
    kind: "picked_files",
    value: { ...pickedValue, rejected },
  });
}

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

/**
 * The refusal list is a new channel for paths from the host, and it is the one
 * least likely to be thought of as carrying data: these strings describe files
 * the host *declined* to hand over. Everything the accepted paths are checked
 * for has to be checked here too, because a leak through an error message is
 * still a leak.
 */
describe("refusal list", () => {
  it("carries the named refusals through", () => {
    const parsed = parseWorkspaceFilePickReply(
      picked([
        { relativePath: "src/image.bin", reason: "not_text" },
        { relativePath: ".env", reason: "blocked" },
      ]),
      requestId,
    );

    expect(parsed.pick.kind).toBe("picked");
    if (parsed.pick.kind !== "picked") return;
    expect(parsed.pick.value.rejected).toEqual([
      { relativePath: "src/image.bin", reason: "not_text" },
      { relativePath: ".env", reason: "blocked" },
    ]);
  });

  it("keeps null as a value, not as a missing name", () => {
    // It is the host saying this file cannot be named without crossing the
    // grant. Coercing it to "" would make it indistinguishable from a name the
    // host failed to send.
    const parsed = parseWorkspaceFilePickReply(
      picked([{ relativePath: null, reason: "outside_workspace" }]),
      requestId,
    );

    expect(parsed.pick.kind).toBe("picked");
    if (parsed.pick.kind !== "picked") return;
    expect(parsed.pick.value.rejected[0]?.relativePath).toBeNull();
  });

  /// The one that matters. A named refusal goes through `asRelativePath`, so a
  /// host that tried to name a file outside the grant is refused here rather
  /// than having its absolute path rendered into the attachment list.
  it("refuses an absolute path in a refusal", () => {
    for (const escape of [
      "C:\\Users\\someone\\.ssh\\id_rsa",
      "/etc/passwd",
      "../../outside.txt",
    ]) {
      expect(() =>
        parseWorkspaceFilePickReply(
          picked([{ relativePath: escape, reason: "outside_workspace" }]),
          requestId,
        ),
      ).toThrow();
    }
  });

  it("refuses a reason it does not know", () => {
    expect(() =>
      parseWorkspaceFilePickReply(
        picked([{ relativePath: "a.ts", reason: "because_i_said_so" }]),
        requestId,
      ),
    ).toThrow();
  });

  it("refuses a refusal carrying anything else", () => {
    // An extra key is a contract drift, and accepting it silently is how a
    // renderer comes to be one version behind without anyone noticing.
    expect(() =>
      parseWorkspaceFilePickReply(
        picked([{ relativePath: "a.ts", reason: "not_text", absolutePath: "C:\\x" }]),
        requestId,
      ),
    ).toThrow();
  });

  it("refuses a refusal list without the field at all", () => {
    const { rejected, ...withoutRejected } = pickedValue;
    void rejected;
    expect(() =>
      parseWorkspaceFilePickReply(
        dispatchReply({ kind: "picked_files", value: withoutRejected }),
        requestId,
      ),
    ).toThrow();
  });

  it("bounds the list", () => {
    // A pick that refused thousands is a pick whose refusals nobody will read,
    // and an unbounded list from the host is an unbounded allocation here.
    const many = Array.from({ length: 101 }, () => ({
      relativePath: null,
      reason: "unreadable",
    }));
    expect(() => parseWorkspaceFilePickReply(picked(many), requestId)).toThrow();
  });
});
