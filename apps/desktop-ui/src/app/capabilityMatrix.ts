/**
 * What the bound host can actually do, feature by feature.
 *
 * FE-12. Ordinary conversation was gated on the whole BMAD command bundle: a
 * host advertising `chat.send`, a granted workspace and a connected model was
 * reported as unable to send a message because it did not also advertise
 * `bmad.help.prepare`, `bmad.help.approve`, `bmad.help.submit`,
 * `bmad.help.cancel`, `bmad.help.latest`, `bmad.library.snapshot` and
 * `run.create`. The composer greyed out and said the model was not connected,
 * which was not true.
 *
 * The fix is not a looser check. It is one check per feature, each naming the
 * exact closure that feature needs, so a gap in one cannot disable another:
 *
 *   chat        — a chat transport, a granted workspace, a usable model route
 *   help        — the BMAD help lifecycle, and nothing else
 *   run         — the capability-run closure, and nothing else
 *   history     — reading back a transcript, which needs no model at all
 *
 * Every unavailable state carries a reason, for the same reason FE-09's
 * command registry does: a greyed control with no explanation tells the user
 * something is impossible and nothing about what to change.
 */

import type { DesktopHostCommand } from "../lib/hostClient";

/**
 * A host command name.
 *
 * The generated union, not `string`. With `string` a typo compiled cleanly and
 * the feature went permanently unavailable with nothing to point at -- which is
 * the FE-12 defect returning by a different route, since the composer would
 * grey out and blame the model provider again.
 */
export type HostCommandName = DesktopHostCommand;

export type FeatureAvailability =
  | { readonly kind: "available" }
  | { readonly kind: "unavailable"; readonly reason: string };

const AVAILABLE: FeatureAvailability = { kind: "available" };

function unavailable(reason: string): FeatureAvailability {
  return { kind: "unavailable", reason };
}

/**
 * The command closures, stated once.
 *
 * Exported so a test can assert that chat's closure does not overlap BMAD's --
 * the overlap *was* the defect, and a comment saying they are separate is not
 * the same as them being separate.
 */
export const featureCommands = {
  /** Sending a turn. The transcript read is separate: see `history`. */
  chat: ["chat.send"],
  history: ["chat.history"],
  /**
   * The BMAD help lifecycle. `run.create` belongs here because a help request
   * is delivered as a run; it is not part of chat.
   */
  help: [
    "bmad.help.prepare",
    "bmad.help.approve",
    "bmad.help.cancel",
    "bmad.help.submit",
    "bmad.help.latest",
    "run.create",
  ],
  /** The reviewed capability run, which is a different closure from help. */
  run: [
    "bmad.capability.prepare",
    "bmad.capability.approve",
    "bmad.capability.cancel",
    "bmad.capability.submit",
    "bmad.capability.latest",
  ],
  /** Reading the method library. A projection; grants nothing. */
  library: ["bmad.library.snapshot"],
  /** Choosing and authenticating a model route. */
  model: [
    "model.auth.status",
    "model.auth.sign_in",
    "model.auth.sign_out",
    "model.selection.set",
  ],
} as const satisfies Record<string, readonly HostCommandName[]>;

export interface CapabilityInput {
  /** False unless a live, ready host is bound. */
  readonly hostReady: boolean;
  /** True in the browser preview, where there is no host at all. */
  readonly browserPreview: boolean;
  readonly supportedCommands: readonly HostCommandName[];
  /** Null when no workspace is bound. */
  readonly workspaceGrantEpoch: number | null;
  /** True once a provider is connected and a model selected. */
  readonly modelReady: boolean;
  /** True once the method library projection has loaded. */
  readonly libraryLoaded: boolean;
}

export interface CapabilityMatrix {
  /** Sending an ordinary conversation turn. */
  readonly chat: FeatureAvailability;
  /** Reading back what was already said. Needs no model. */
  readonly history: FeatureAvailability;
  /** Asking BMAD for guidance. */
  readonly help: FeatureAvailability;
  /** Running a reviewed capability. */
  readonly run: FeatureAvailability;
  /** Browsing the method library. */
  readonly library: FeatureAvailability;
  /** Choosing or connecting a model provider. */
  readonly model: FeatureAvailability;
}

function missingCommands(
  supported: readonly HostCommandName[],
  required: readonly HostCommandName[],
): readonly HostCommandName[] {
  return required.filter((command) => !supported.includes(command));
}

/**
 * The reason a host is missing a feature's commands.
 *
 * Names the feature, not the commands. "Requires bmad.help.approve" is true
 * and useless; the user cannot install a command. What they can do is update,
 * so that is what it says.
 */
function unsupported(feature: string): FeatureAvailability {
  return unavailable(`${feature} is not available in this host version.`);
}

/**
 * Resolves what each feature can do right now.
 *
 * The shared preconditions are checked first and identically -- a host that is
 * not ready fails everything, for the same stated reason -- and after that each
 * feature consults only its own closure.
 */
export function resolveCapabilities(input: CapabilityInput): CapabilityMatrix {
  const {
    browserPreview,
    hostReady,
    libraryLoaded,
    modelReady,
    supportedCommands,
    workspaceGrantEpoch,
  } = input;

  if (browserPreview) {
    const preview = unavailable("Not available in the browser preview — run the desktop app.");
    return {
      chat: preview, help: preview, history: preview,
      library: preview, model: preview, run: preview,
    };
  }
  if (!hostReady) {
    const noHost = unavailable("Forge is not connected to the Windows host.");
    return {
      chat: noHost, help: noHost, history: noHost,
      library: noHost, model: noHost, run: noHost,
    };
  }

  /*
   * The grant epoch, not merely a selected workspace. Epoch 0 is a workspace
   * the host has projected but not yet granted, and every one of these commands
   * is authorised against the grant -- so acting on epoch 0 fails at the host
   * with an error the user cannot interpret.
   */
  const granted = workspaceGrantEpoch !== null && workspaceGrantEpoch >= 1;
  const noWorkspace = unavailable("Open a workspace folder first.");

  const has = (commands: readonly HostCommandName[]) =>
    missingCommands(supportedCommands, commands).length === 0;

  const model: FeatureAvailability = !has(featureCommands.model)
    ? unsupported("Model selection")
    : AVAILABLE;

  return {
    /*
     * Chat's closure is `chat.send` alone. It is deliberately *not* a superset
     * of help's: that was the defect, and a test asserts the two closures stay
     * disjoint so it cannot come back by someone adding "just one" BMAD command
     * to the chat gate.
     */
    chat: !has(featureCommands.chat)
      ? unsupported("Chat")
      : !granted
        ? noWorkspace
        : !modelReady
          ? unavailable("Connect a model provider in Settings before sending a message.")
          : AVAILABLE,

    help: !has(featureCommands.help)
      ? unsupported("Method guidance")
      : !granted
        ? noWorkspace
        : !modelReady
          ? unavailable("Connect a model provider in Settings before asking for guidance.")
          : !libraryLoaded
            // Distinct from "not supported": the host can do this, the
            // catalogue simply has not arrived, and it is worth waiting for.
            ? unavailable("The method library is still loading.")
            : AVAILABLE,

    // No model and no workspace grant: reading back what was already said is
    // not a model feature, and refusing it because a provider is disconnected
    // hides the conversation the user is trying to re-read.
    history: !has(featureCommands.history)
      ? unsupported("Reading past messages")
      : !granted
        ? noWorkspace
        : AVAILABLE,

    library: !has(featureCommands.library)
      ? unsupported("The method library")
      : !granted
        ? noWorkspace
        : AVAILABLE,

    model,

    run: !has(featureCommands.run)
      ? unsupported("Capability runs")
      : !granted
        ? noWorkspace
        : AVAILABLE,
  };
}

export function isAvailable(availability: FeatureAvailability): boolean {
  return availability.kind === "available";
}

/** The reason, or null when the feature is available. */
export function unavailableReason(availability: FeatureAvailability): string | null {
  return availability.kind === "unavailable" ? availability.reason : null;
}
