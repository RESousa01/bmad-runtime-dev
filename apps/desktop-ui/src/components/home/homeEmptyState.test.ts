import { describe, expect, it } from "vitest";

import { homeEmptyState, type HomeHostKind } from "./homeEmptyState";

const HOST_KINDS: readonly HomeHostKind[] = [
  "browser_demo", "loading", "read_only_recovery", "ready", "unavailable",
];

describe("homeEmptyState", () => {
  it("says nothing while the index has rows to show", () => {
    // An explanation is never allowed to stand in front of the thing it would
    // be explaining, whatever else is wrong with the host.
    for (const hostKind of HOST_KINDS) {
      expect(homeEmptyState({ hostKind, projectCount: 0, taskCount: 1 }), hostKind).toBeNull();
      expect(homeEmptyState({ hostKind, projectCount: 2, taskCount: 5 }), hostKind).toBeNull();
    }
  });

  it("blames the host before the missing project it caused", () => {
    // Outside in: an unreachable host is why there is no project, so naming the
    // project would send the user to open a folder through a host that cannot
    // open one.
    const state = homeEmptyState({ hostKind: "unavailable", projectCount: 0, taskCount: 0 });
    expect(state?.reason).toBe("host-unready");
    expect(state?.message).toContain("unavailable");
  });

  it("blames the missing project before the missing task it caused", () => {
    // The composer that "describe what you want above" refers to is disabled
    // without a project, so the task sentence would point at a dead control.
    const state = homeEmptyState({ hostKind: "ready", projectCount: 0, taskCount: 0 });
    expect(state?.reason).toBe("no-project");
    expect(state?.message).toContain("folder");
  });

  it("blames nothing but the empty task list once a project is open", () => {
    const state = homeEmptyState({ hostKind: "ready", projectCount: 1, taskCount: 0 });
    expect(state?.reason).toBe("no-tasks");
    expect(state?.message).toContain("No tasks yet");
  });

  it("gives the three reasons three different sentences", () => {
    // The failure this exists to prevent: one "nothing here" covering three
    // unrelated causes, which is the state Home was in before.
    const messages = [
      homeEmptyState({ hostKind: "unavailable", projectCount: 0, taskCount: 0 })?.message,
      homeEmptyState({ hostKind: "ready", projectCount: 0, taskCount: 0 })?.message,
      homeEmptyState({ hostKind: "ready", projectCount: 1, taskCount: 0 })?.message,
    ];
    for (const message of messages) expect(typeof message).toBe("string");
    expect(new Set(messages).size).toBe(3);
  });

  it("never tells a host that cannot open a folder to open one", () => {
    for (const hostKind of HOST_KINDS.filter((kind) => kind !== "ready")) {
      const state = homeEmptyState({ hostKind, projectCount: 0, taskCount: 0 });
      expect(state?.reason, hostKind).toBe("host-unready");
    }
  });

  it("keeps explaining itself when a host that cannot carry a task holds a project", () => {
    // A project is listed but the shell disables drafting entirely in these
    // states, so "describe what you want above" would be an instruction the
    // user cannot follow.
    for (const hostKind of ["loading", "read_only_recovery", "unavailable"] as const) {
      const state = homeEmptyState({ hostKind, projectCount: 1, taskCount: 0 });
      expect(state?.reason, hostKind).toBe("host-unready");
    }
  });

  it("treats browser preview as ready enough once it has a project", () => {
    // The preview is the one non-ready host that can still carry a task.
    // Blaming the missing desktop host over a demo project the user can work in
    // would be true and useless.
    expect(homeEmptyState({ hostKind: "browser_demo", projectCount: 1, taskCount: 0 })?.reason)
      .toBe("no-tasks");
    expect(homeEmptyState({ hostKind: "browser_demo", projectCount: 0, taskCount: 0 })?.reason)
      .toBe("host-unready");
  });

  it("has a sentence for every host state it claims to cover", () => {
    for (const hostKind of HOST_KINDS) {
      const state = homeEmptyState({ hostKind, projectCount: 0, taskCount: 0 });
      expect(state?.message.length ?? 0, hostKind).toBeGreaterThan(0);
    }
  });
});
