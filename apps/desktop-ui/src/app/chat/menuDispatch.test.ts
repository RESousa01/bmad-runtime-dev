import { describe, expect, it } from "vitest";

import type { BmadLibraryUiState } from "../../lib/bmadProjection";
import { resolveMenuDispatch } from "./menuDispatch";

function library(menus: readonly unknown[], agentCode = "pm"): BmadLibraryUiState {
  return {
    kind: "ready",
    projection: { methodAgents: [{ agentCode, menus }] },
  } as unknown as BmadLibraryUiState;
}

/// A runnable menu item as the host projects one. `code` and `availability`
/// both matter: `matchMenuIntent` only considers items that are available and
/// carry a capability, and the code is the strongest match signal.
const DRAFT_PRD = {
  code: "PRD",
  description: "Draft a product requirements document.",
  targetKind: "skill_target",
  displayLabel: "Draft requirements",
  capabilityId: "cap.prd",
  availability: "available",
  availabilityReason: null,
};

describe("dispatching an agent menu item", () => {
  it("dispatches a message that names a menu item", () => {
    const dispatch = resolveMenuDispatch({
      boundAgent: "pm",
      intent: "PRD",
      library: library([DRAFT_PRD]),
    });
    expect(dispatch).toEqual({ capabilityId: "cap.prd", label: "Draft requirements" });
  });

  it("carries the label the user sees, for the transcript", () => {
    // A dispatch is still a turn the user took, and the conversation records
    // it by name rather than by capability id.
    const dispatch = resolveMenuDispatch({
      boundAgent: "pm",
      intent: "PRD",
      library: library([DRAFT_PRD]),
    });
    expect(dispatch?.label).toBe("Draft requirements");
  });
});

describe("the three conditions, each a way to get this wrong", () => {
  it("does nothing in plain chat", () => {
    // No agent bound means no menus, so nothing to match against.
    expect(resolveMenuDispatch({
      boundAgent: null,
      intent: "PRD",
      library: library([DRAFT_PRD]),
    })).toBeNull();
  });

  it("does nothing before the library has loaded", () => {
    // Matching against a catalogue that has not arrived turns every message
    // into ordinary chat, which looks identical to the feature not existing.
    for (const kind of ["idle", "loading", "unavailable"]) {
      expect(resolveMenuDispatch({
        boundAgent: "pm",
        intent: "PRD",
        library: { kind } as BmadLibraryUiState,
      }), kind).toBeNull();
    }
  });

  it("does nothing in plain chat even if the projection holds a null agent code", () => {
    /*
     * The explicit `boundAgent === null` check earns its place here and only
     * here. Without it the agent lookup compares `agentCode === null`, so a
     * projection carrying a null-coded agent would bind plain chat to that
     * agent's menus and dispatch a capability the user never selected.
     */
    expect(resolveMenuDispatch({
      boundAgent: null,
      intent: "PRD",
      library: library([DRAFT_PRD], null as unknown as string),
    })).toBeNull();
  });

  it("does nothing when the bound agent is not in the library", () => {
    expect(resolveMenuDispatch({
      boundAgent: "architect",
      intent: "PRD",
      library: library([DRAFT_PRD], "pm"),
    })).toBeNull();
  });

  it("refuses a menu entry that carries no capability", () => {
    // A label with nothing behind it. Dispatching would open a run for a
    // capability the host never declared.
    for (const capabilityId of [null, undefined]) {
      expect(resolveMenuDispatch({
        boundAgent: "pm",
        intent: "PRD",
        library: library([{ ...DRAFT_PRD, capabilityId }]),
      }), String(capabilityId)).toBeNull();
    }
  });
});

describe("an ordinary message stays an ordinary message", () => {
  it("does not dispatch a message that names nothing", () => {
    // Null is the common answer and the safe one: treating a message as a
    // command takes the user somewhere they did not ask to go.
    expect(resolveMenuDispatch({
      boundAgent: "pm",
      intent: "what do you think about the login flow?",
      library: library([DRAFT_PRD]),
    })).toBeNull();
  });

  it("does not dispatch when the agent has no menus at all", () => {
    expect(resolveMenuDispatch({
      boundAgent: "pm",
      intent: "PRD",
      library: library([]),
    })).toBeNull();
  });
});
