/**
 * The model provider controller: select a model, connect a provider, disconnect.
 *
 * FE-10 step 3. All three actions shared one shape -- guard on the host
 * capability and explain if it is missing, mark busy, clear the notice, call the
 * host, turn a failure into a safe message, clear busy -- written out three
 * times. `runModelAction` is that shape, once.
 *
 * The duplication that mattered
 * -----------------------------
 * Every one of these changes the *model authority*: which provider and model
 * the host will send to, and whether it is authenticated. Anything derived from
 * the old authority has to be discarded with it -- an in-flight BMAD request
 * approved against the previous model, and capability runs whose results were
 * produced by it. `connectModelProvider` and `disconnectModelProvider` called
 * `resetModelBoundUi` to do that; `changeModelSelection` had the same six lines
 * copied inline. Identical today, and the kind of pair that stops being
 * identical the first time one of them is extended.
 */

import {
  getSafeHostMessage,
  type ModelAuthStatusProjection,
  type ModelProviderConnection,
  type ModelSelection,
} from "../../lib/hostClient";

/**
 * The whole authority the model feature holds.
 *
 * Three calls and three capability flags rather than the host client, so a
 * later change cannot quietly reach for more.
 */
export interface ModelHostPort {
  readonly canSelectModel: boolean;
  readonly canSignIn: boolean;
  readonly canSignOut: boolean;
  /// `ModelSelection["providerId"]`, not `string`: the provider id is a closed
  /// union, and widening it here would let an unroutable id reach the host.
  readonly setModelSelection: (
    providerId: ModelSelection["providerId"],
    modelId: string,
  ) => Promise<ModelAuthStatusProjection>;
  readonly signIn: (
    connection: ModelProviderConnection,
  ) => Promise<ModelAuthStatusProjection>;
  readonly signOut: () => Promise<ModelAuthStatusProjection>;
}

export interface ModelControllerPorts {
  /** Null unless a live, ready host is bound. */
  readonly host: ModelHostPort | null;
  readonly setBusy: (busy: boolean) => void;
  readonly setNotice: (notice: string | null) => void;
  /**
   * Adopts a new model authority and discards everything derived from the old
   * one: the in-flight guidance request and every capability run.
   */
  readonly adoptAuthority: (status: ModelAuthStatusProjection) => void;
}

export interface ModelActions {
  readonly changeModelSelection: (selection: ModelSelection) => Promise<void>;
  readonly connectModelProvider: (
    connection: ModelProviderConnection,
  ) => Promise<void>;
  readonly disconnectModelProvider: () => Promise<void>;
}

/**
 * Why an action cannot run, in the user's terms.
 *
 * Each names the thing to do rather than the capability that is absent: "needs
 * the desktop host" is actionable, "model.auth.sign_in unsupported" is not.
 */
const UNAVAILABLE = {
  selection: "Model selection requires the Forge desktop host.",
  signIn: "Provider connection requires the Forge desktop host.",
  signOut: "Provider disconnection requires the Forge desktop host.",
} as const;

export function createModelActions(ports: ModelControllerPorts): ModelActions {
  const { adoptAuthority, host, setBusy, setNotice } = ports;

  /** The shape every model action shares. */
  async function runModelAction(
    allowed: boolean,
    unavailableNotice: string,
    action: (host: ModelHostPort) => Promise<string>,
  ): Promise<void> {
    if (host === null || !allowed) {
      setNotice(unavailableNotice);
      return;
    }
    setBusy(true);
    setNotice(null);
    try {
      setNotice(await action(host));
    } catch (error) {
      // `getSafeHostMessage` rather than the raw error: a provider failure can
      // carry the endpoint and enough of the request to be worth not printing.
      setNotice(getSafeHostMessage(error));
    } finally {
      // A busy flag left set by a thrown action is a control the user can never
      // press again.
      setBusy(false);
    }
  }

  return {
    async changeModelSelection(selection: ModelSelection) {
      await runModelAction(
        host?.canSelectModel ?? false,
        UNAVAILABLE.selection,
        async (bound) => {
          const status = await bound.setModelSelection(
            selection.providerId,
            selection.modelId,
          );
          adoptAuthority(status);
          // Selecting is not connecting. Saying so here is what stops a user
          // picking a model and waiting for a reply that will never come.
          return status.status === "development_ready"
            ? `${status.destinationLabel} is ready.`
            : `${status.destinationLabel} is selected. Enter its API key below to connect.`;
        },
      );
    },

    async connectModelProvider(connection: ModelProviderConnection) {
      await runModelAction(
        host?.canSignIn ?? false,
        UNAVAILABLE.signIn,
        async (bound) => {
          const status = await bound.signIn(connection);
          adoptAuthority(status);
          // "for this session" is literal: the key is held in memory by the
          // host and is gone when the app closes.
          return `${status.destinationLabel} is connected for this session.`;
        },
      );
    },

    async disconnectModelProvider() {
      await runModelAction(
        host?.canSignOut ?? false,
        UNAVAILABLE.signOut,
        async (bound) => {
          const status = await bound.signOut();
          adoptAuthority(status);
          return "Provider disconnected. The in-memory API key was cleared.";
        },
      );
    },
  };
}

export const modelUnavailableNotices = UNAVAILABLE;
