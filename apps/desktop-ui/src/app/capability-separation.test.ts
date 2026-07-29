import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import {
  featureCommands,
  isAvailable,
  resolveCapabilities,
  type CapabilityInput,
  type HostCommandName,
} from "./capabilityMatrix";

/**
 * ENG-18 characterization.
 *
 * FE-12 (see `capabilityMatrix.ts`) already fixed the coupling this ticket
 * describes: chat's closure is `chat.send` alone, `methodGuidanceClient` is no
 * longer read on the send path, and `ChatClient` (`lib/hostClient/clients`) is
 * a `Pick` that cannot name a BMAD method even if a caller wanted it to. This
 * file is not a second implementation of that fix -- it is the lock that
 * catches the fix silently regressing, from three angles the existing
 * `capabilityMatrix.test.ts` does not: the three explicit availability states
 * this ticket names, the source-level proof that the send path does not read
 * `methodGuidanceClient`, and the source-level proof that `ChatClient` cannot
 * reach a BMAD command even by type.
 */

const ALL_COMMANDS: readonly HostCommandName[] = [
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

function without(...commands: readonly HostCommandName[]): readonly HostCommandName[] {
  return ALL_COMMANDS.filter((command) => !commands.includes(command));
}

describe("the three states ENG-18 names", () => {
  it("state 1: chat available, BMAD unavailable", () => {
    const capabilities = resolveCapabilities(input({
      libraryLoaded: false,
      supportedCommands: without(
        ...featureCommands.help,
        ...featureCommands.run,
        ...featureCommands.library,
      ),
    }));
    expect(isAvailable(capabilities.chat)).toBe(true);
    expect(isAvailable(capabilities.help)).toBe(false);
    expect(isAvailable(capabilities.run)).toBe(false);
    expect(isAvailable(capabilities.library)).toBe(false);
  });

  it("state 2: the offline-capable BMAD read (library) available, model unavailable", () => {
    // Acceptance 3 is "Help remains offline-capable". In this codebase the
    // model-free BMAD read is `library` (browsing the installed method
    // catalogue); `help` itself is a *request*, which is inherently a model
    // call and is correctly gated on `modelReady`. `library` is the feature
    // that must survive a disconnected model, and it does: it consults
    // `granted` and its own commands only.
    const capabilities = resolveCapabilities(input({ modelReady: false }));
    expect(isAvailable(capabilities.library)).toBe(true);
    expect(isAvailable(capabilities.history)).toBe(true);
    expect(isAvailable(capabilities.chat)).toBe(false);
    expect(isAvailable(capabilities.help)).toBe(false);
  });

  it("state 3: both chat and BMAD available together", () => {
    const capabilities = resolveCapabilities(input());
    expect(isAvailable(capabilities.chat)).toBe(true);
    expect(isAvailable(capabilities.help)).toBe(true);
    expect(isAvailable(capabilities.run)).toBe(true);
    expect(isAvailable(capabilities.library)).toBe(true);
  });
});

describe("structural proof: the send path cannot reach BMAD", () => {
  const APP_SOURCE = readFileSync(
    resolve(process.cwd(), "src/App.tsx"),
    "utf8",
  );

  /// Isolates one function body by brace counting from its declaration line,
  /// so the assertion is scoped to the send path itself and cannot pass by
  /// coincidence just because `methodGuidanceClient` is absent somewhere else
  /// in a 2,000+ line file that legitimately uses it for other BMAD features.
  function functionBody(declaration: string): string {
    const start = APP_SOURCE.indexOf(declaration);
    expect(start, `could not find "${declaration}" in App.tsx`).toBeGreaterThanOrEqual(0);
    const braceStart = APP_SOURCE.indexOf("{", start);
    let depth = 0;
    for (let index = braceStart; index < APP_SOURCE.length; index += 1) {
      const char = APP_SOURCE[index];
      if (char === "{") depth += 1;
      if (char === "}") {
        depth -= 1;
        if (depth === 0) return APP_SOURCE.slice(braceStart, index + 1);
      }
    }
    throw new Error(`unterminated function body for "${declaration}"`);
  }

  it(
    // Break caught: ENG-18 step 3, "remove methodGuidanceClient from normal
    // send", silently regressing -- e.g. a future change adding a guidance
    // suggestion or an inline BMAD lookup to the composer's send handler.
    "keeps methodGuidanceClient out of sendChatMessage's body",
    () => {
      const body = functionBody("async function sendChatMessage(intent: string, sessionId?: string): Promise<void> {");
      expect(body).not.toContain("methodGuidanceClient");
    },
  );

  it(
    // Break caught: the send transport being re-typed (or re-cast) as the
    // full `DesktopHostClient` facade, which would let a later change reach a
    // BMAD method from the send path without the type system objecting --
    // exactly what `ChatClient` (`lib/hostClient/clients/index.ts`) exists to
    // prevent structurally rather than by convention.
    "types the send transport as ChatClient, not the full host facade",
    () => {
      expect(APP_SOURCE).toContain("const chatSendClient: ChatClient | null =");
    },
  );
});

describe("structural proof: ChatClient cannot name a BMAD command by type", () => {
  const CLIENTS_SOURCE = readFileSync(
    resolve(process.cwd(), "src/lib/hostClient/clients/index.ts"),
    "utf8",
  );

  it(
    // Break caught: someone widening `ChatClient`'s `Pick` to include a BMAD
    // method (e.g. to save an import elsewhere) -- which would let a chat
    // feature reach BMAD state through the very type meant to make that a
    // compile error.
    "picks only sendChatMessage and chatHistory for ChatClient",
    () => {
      const match = /export type ChatClient = Pick<DesktopHostClient,\s*([\s\S]*?)>;/u.exec(
        CLIENTS_SOURCE,
      );
      expect(match, "could not find the ChatClient declaration").not.toBeNull();
      const members = [...(match?.[1] ?? "").matchAll(/"([^"]+)"/gu)].map((m) => m[1]);
      expect(members.sort()).toEqual(["chatHistory", "sendChatMessage"]);
    },
  );
});
