import { describe, expect, it } from "vitest";

import {
  featureCommands,
  isAvailable,
  resolveCapabilities,
  unavailableReason,
  type CapabilityInput,
} from "./capabilityMatrix";

const ALL_COMMANDS = [
  ...featureCommands.chat,
  ...featureCommands.history,
  ...featureCommands.help,
  ...featureCommands.run,
  ...featureCommands.library,
  ...featureCommands.model,
];

function input(overrides: Partial<CapabilityInput> = {}): CapabilityInput {
  return {
    browserPreview: false,
    hostReady: true,
    libraryLoaded: true,
    modelReady: true,
    supportedCommands: ALL_COMMANDS,
    workspaceGrantEpoch: 1,
    ...overrides,
  };
}

function without(...commands: readonly string[]): readonly string[] {
  return ALL_COMMANDS.filter((command) => !commands.includes(command));
}

describe("the closures are disjoint", () => {
  /// The defect this file exists to fix, stated as an invariant. Chat was
  /// gated on the whole BMAD bundle, so a host that could hold a conversation
  /// reported itself unable to.
  it("shares no command between chat and BMAD", () => {
    const bmad = new Set<string>([...featureCommands.help, ...featureCommands.run,
      ...featureCommands.library]);
    const overlap = (featureCommands.chat as readonly string[])
      .filter((command) => bmad.has(command));
    expect(overlap).toEqual([]);
  });

  it("shares no command between help and capability runs", () => {
    // Two different review lifecycles. A host may implement one and not the
    // other, and neither should take the other down.
    const runs = new Set<string>(featureCommands.run);
    expect((featureCommands.help as readonly string[]).filter((c) => runs.has(c))).toEqual([]);
  });

  it("keeps reading a transcript out of the send closure", () => {
    const send = new Set<string>(featureCommands.chat);
    expect((featureCommands.history as readonly string[]).filter((c) => send.has(c))).toEqual([]);
  });
});

describe("plain chat can send without BMAD", () => {
  /// FE-12 acceptance criterion 1.
  it("stays available when the whole BMAD help closure is missing", () => {
    const capabilities = resolveCapabilities(input({
      libraryLoaded: false,
      supportedCommands: without(...featureCommands.help, ...featureCommands.library),
    }));
    expect(isAvailable(capabilities.chat)).toBe(true);
    expect(isAvailable(capabilities.help)).toBe(false);
  });

  it("stays available when capability runs are missing", () => {
    const capabilities = resolveCapabilities(input({
      supportedCommands: without(...featureCommands.run),
    }));
    expect(isAvailable(capabilities.chat)).toBe(true);
    expect(isAvailable(capabilities.run)).toBe(false);
  });

  it("goes unavailable only when its own command is missing", () => {
    const capabilities = resolveCapabilities(input({
      supportedCommands: without("chat.send"),
    }));
    expect(isAvailable(capabilities.chat)).toBe(false);
    // And nothing else is dragged down with it.
    expect(isAvailable(capabilities.help)).toBe(true);
    expect(isAvailable(capabilities.history)).toBe(true);
  });
});

describe("a BMAD outage does not disable local functions", () => {
  /// FE-12 acceptance criterion 2.
  it("leaves reading past messages available", () => {
    const capabilities = resolveCapabilities(input({
      libraryLoaded: false,
      supportedCommands: without(...featureCommands.help, ...featureCommands.library),
    }));
    expect(isAvailable(capabilities.history)).toBe(true);
  });

  it("leaves model selection available", () => {
    const capabilities = resolveCapabilities(input({
      supportedCommands: without(...featureCommands.help),
    }));
    expect(isAvailable(capabilities.model)).toBe(true);
  });

  it("does not need a model to read back a transcript", () => {
    // Refusing this because a provider is disconnected hides the conversation
    // the user is trying to re-read.
    const capabilities = resolveCapabilities(input({ modelReady: false }));
    expect(isAvailable(capabilities.history)).toBe(true);
    expect(isAvailable(capabilities.chat)).toBe(false);
  });
});

describe("the inverse: BMAD present, chat unsupported", () => {
  it("keeps guidance available when the chat transport is missing", () => {
    const capabilities = resolveCapabilities(input({
      supportedCommands: without("chat.send", "chat.history"),
    }));
    expect(isAvailable(capabilities.help)).toBe(true);
    expect(isAvailable(capabilities.run)).toBe(true);
    expect(isAvailable(capabilities.chat)).toBe(false);
    expect(isAvailable(capabilities.history)).toBe(false);
  });
});

describe("guidance's own preconditions", () => {
  it("waits for the library rather than reporting the host unsupported", () => {
    // The distinction matters: one is worth waiting for, the other is not.
    const capabilities = resolveCapabilities(input({ libraryLoaded: false }));
    expect(unavailableReason(capabilities.help)).toBe("The method library is still loading.");
  });

  it("is unavailable when its own commands are missing", () => {
    const capabilities = resolveCapabilities(input({
      supportedCommands: without("bmad.help.approve"),
    }));
    expect(unavailableReason(capabilities.help))
      .toBe("Method guidance is not available in this host version.");
  });
});

describe("shared preconditions", () => {
  it("refuses everything in the browser preview, with one reason", () => {
    const capabilities = resolveCapabilities(input({ browserPreview: true }));
    for (const availability of Object.values(capabilities)) {
      expect(unavailableReason(availability))
        .toBe("Not available in the browser preview — run the desktop app.");
    }
  });

  it("refuses everything when no host is bound", () => {
    const capabilities = resolveCapabilities(input({ hostReady: false }));
    for (const availability of Object.values(capabilities)) {
      expect(isAvailable(availability)).toBe(false);
    }
  });

  it("requires a granted workspace, not merely a projected one", () => {
    // Epoch 0 is a workspace the host has projected but not granted. Acting on
    // it fails at the host with an error the user cannot interpret.
    const projected = resolveCapabilities(input({ workspaceGrantEpoch: 0 }));
    expect(unavailableReason(projected.chat)).toBe("Open a workspace folder first.");
    expect(unavailableReason(projected.history)).toBe("Open a workspace folder first.");

    const granted = resolveCapabilities(input({ workspaceGrantEpoch: 1 }));
    expect(isAvailable(granted.chat)).toBe(true);
  });

  it("does not gate model selection on a workspace", () => {
    // Connecting a provider is how a user gets out of "no model connected",
    // and needing a workspace first makes that a dead end.
    const capabilities = resolveCapabilities(input({ workspaceGrantEpoch: null }));
    expect(isAvailable(capabilities.model)).toBe(true);
  });
});

describe("every reason is written for a person", () => {
  it("names what to do rather than the command that is missing", () => {
    // "Requires bmad.help.approve" is true and useless: the user cannot
    // install a command.
    const contexts = [
      input({ browserPreview: true }),
      input({ hostReady: false }),
      input({ workspaceGrantEpoch: 0 }),
      input({ modelReady: false }),
      input({ libraryLoaded: false }),
      input({ supportedCommands: [] }),
    ];
    for (const context of contexts) {
      for (const [feature, availability] of Object.entries(resolveCapabilities(context))) {
        const reason = unavailableReason(availability);
        if (reason === null) continue;
        expect(reason, feature).toMatch(/^[A-Z].*[.?]$/u);
        expect(reason, feature).not.toMatch(/bmad\.|chat\.|model\.|run\./u);
      }
    }
  });
});
