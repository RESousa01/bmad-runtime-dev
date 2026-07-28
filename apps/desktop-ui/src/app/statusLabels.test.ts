import { describe, expect, it } from "vitest";

import type { ModelAuthStatusProjection } from "../lib/hostClient";
import {
  explorerAvailabilityMessage,
  hostStatusLabel,
  methodStatusLabel,
  modelAccess,
  providerLabel,
  workspaceDescription,
  type RuntimeKind,
} from "./statusLabels";

const KINDS: readonly RuntimeKind[] = [
  "loading", "browser_demo", "ready", "read_only_recovery", "unavailable",
];

function auth(overrides: Partial<ModelAuthStatusProjection> = {}): ModelAuthStatusProjection {
  return {
    status: "development_ready",
    destinationLabel: "OpenAI gpt-5",
    providerId: "openai",
    modelId: "gpt-5",
    mode: "direct_provider",
    authEpoch: 1,
    ...overrides,
  } as ModelAuthStatusProjection;
}

describe("hostStatusLabel", () => {
  it("gives every runtime kind its own label", () => {
    const labels = KINDS.map(hostStatusLabel);
    expect(new Set(labels).size).toBe(KINDS.length);
    for (const label of labels) expect(label.length).toBeGreaterThan(0);
  });
});

describe("workspaceDescription", () => {
  it("calls the preview sample data whatever its fixture permissions say", () => {
    // Nothing here touches a real folder, so reporting "Governed edits" would
    // be a claim about a workspace that does not exist.
    expect(workspaceDescription({ runtimeKind: "browser_demo", permissions: "governed_edits" }))
      .toBe("Sample data · Read only");
  });

  it("distinguishes the two permission levels", () => {
    expect(workspaceDescription({ runtimeKind: "ready", permissions: "governed_edits" }))
      .toBe("Governed edits");
    expect(workspaceDescription({ runtimeKind: "ready", permissions: "read_only" }))
      .toBe("Read only");
  });

  it("prompts rather than describing when no workspace is bound", () => {
    expect(workspaceDescription({ runtimeKind: "ready", permissions: null }))
      .toBe("Choose a workspace");
  });
});

describe("modelAccess", () => {
  const base = {
    libraryLoaded: true,
    modelAuth: null,
    modelCommandsAvailable: true,
    runtimeKind: "ready" as RuntimeKind,
  };

  it("names the destination the host reported", () => {
    expect(modelAccess({ ...base, modelAuth: auth() }).label).toBe("OpenAI gpt-5");
  });

  it("falls back when a ready status names no destination", () => {
    expect(modelAccess({ ...base, modelAuth: auth({ destinationLabel: "" }) }).label)
      .toBe("Development model");
  });

  it("names the deterministic mode ahead of the provider", () => {
    // Deterministic development does not reach a provider at all, so naming
    // one would be wrong.
    expect(modelAccess({
      ...base,
      modelAuth: auth({ mode: "deterministic_development" }),
    }).detail).toBe("Deterministic development · ready");
  });

  /// The distinction most easily lost. The host can answer model commands but
  /// has not said what the connection is; telling the user to connect a
  /// provider sends them to Settings to fix something that may not be broken.
  it("says it is checking rather than telling the user to connect", () => {
    const access = modelAccess({ ...base, modelAuth: null, modelCommandsAvailable: true });
    expect(access.label).toBe("Checking model access");
  });

  it("tells the user to connect once the host has no model commands", () => {
    const access = modelAccess({ ...base, modelAuth: null, modelCommandsAvailable: false });
    expect(access.label).toBe("Connect a provider");
  });

  it("mentions the catalogue only when it is the thing that is missing", () => {
    expect(modelAccess({ ...base, modelCommandsAvailable: false, libraryLoaded: false }).detail)
      .toContain("skills catalog");
    expect(modelAccess({ ...base, modelCommandsAvailable: false, libraryLoaded: true }).detail)
      .toContain("Settings");
  });

  it("says the preview has no model access at all", () => {
    expect(modelAccess({ ...base, runtimeKind: "browser_demo", modelAuth: auth() }).label)
      .toBe("No model access");
  });

  it("waits rather than judging while the host is still loading", () => {
    expect(modelAccess({ ...base, runtimeKind: "loading" }).label).toBe("Checking access");
  });
});

describe("providerLabel", () => {
  it("names each provider as the user chose it", () => {
    expect(providerLabel("azure")).toBe("Azure OpenAI");
    expect(providerLabel("openai")).toBe("OpenAI");
    expect(providerLabel("forge-local")).toBe("Forge local");
  });

  it("falls back for an unknown or absent provider", () => {
    expect(providerLabel(undefined)).toBe("Forge BMAD");
    expect(providerLabel("something-new")).toBe("Forge BMAD");
  });
});

describe("methodStatusLabel", () => {
  it("distinguishes a real host with no model from a preview with no host", () => {
    // Collapsing these would tell a preview user their local host is fine.
    expect(methodStatusLabel({
      chatAvailable: false, guidanceCommandsAvailable: true, runtimeKind: "ready",
    })).toBe("Local only");
    expect(methodStatusLabel({
      chatAvailable: false, guidanceCommandsAvailable: false, runtimeKind: "browser_demo",
    })).toBe("Demo only");
    expect(methodStatusLabel({
      chatAvailable: false, guidanceCommandsAvailable: false, runtimeKind: "unavailable",
    })).toBe("Not connected");
  });

  it("reports ready whenever chat can send", () => {
    expect(methodStatusLabel({
      chatAvailable: true, guidanceCommandsAvailable: false, runtimeKind: "ready",
    })).toBe("Model ready");
  });
});

describe("explorerAvailabilityMessage", () => {
  it("passes through the host's own reason when it is unavailable", () => {
    // Substituting a generic sentence discards the only specific information
    // available.
    expect(explorerAvailabilityMessage({
      hasWorkspace: false,
      runtimeKind: "unavailable",
      unavailableMessage: "The host binary failed verification.",
    })).toBe("The host binary failed verification.");
  });

  it("still says something when the host gave no reason", () => {
    expect(explorerAvailabilityMessage({
      hasWorkspace: false, runtimeKind: "unavailable", unavailableMessage: null,
    })).toContain("could not reach");
  });

  it("distinguishes no workspace from missing read capabilities", () => {
    const noWorkspace = explorerAvailabilityMessage({
      hasWorkspace: false, runtimeKind: "ready", unavailableMessage: null,
    });
    const noCapability = explorerAvailabilityMessage({
      hasWorkspace: true, runtimeKind: "ready", unavailableMessage: null,
    });
    expect(noWorkspace).toContain("Choose a local workspace");
    expect(noCapability).not.toBe(noWorkspace);
  });

  it("answers for every runtime kind", () => {
    for (const runtimeKind of KINDS) {
      const message = explorerAvailabilityMessage({
        hasWorkspace: false, runtimeKind, unavailableMessage: "host said so",
      });
      expect(message.length, runtimeKind).toBeGreaterThan(0);
    }
  });
});
