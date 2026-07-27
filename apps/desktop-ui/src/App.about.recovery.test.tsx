// @vitest-environment jsdom
import "./test/setup";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { App } from "./App";
import { DesktopHostClient } from "./lib/hostClient";
import type { BootstrapReply, HostRuntime, TauriInvoke } from "./lib/hostClient";

const rendererSessionId = "renderer_about_guard";

const bootstrap: BootstrapReply = {
  schemaVersion: "desktop-bootstrap.v1",
  rendererSessionId,
  installationId: "install_about_guard",
  windowLabel: "main",
  bootMode: "ready",
  supportedCommands: [
    "app.get_boot_state",
    "app.about",
    "workspace.list",
  ],
  workspaces: [
    {
      workspaceId: "workspace_about_guard",
      projectId: "project_about_guard",
      displayName: "opaque-workspace-name",
      grantEpoch: 3,
      permissions: "read_only",
      contextReadEpoch: 1,
      governedEditEpoch: 1,
    },
  ],
  projectionSequence: 18,
};

function successfulReply(requestId: string, data: unknown) {
  return {
    schemaVersion: "desktop-dispatch-reply.v1",
    requestId,
    // Ahead of the recovery event below: the client refuses a reply that does
    // not advance the projection sequence, and a refusal would look exactly
    // like the guard working.
    sequence: 30,
    status: "ok",
    receipt: { requestId, acceptedAt: 1_725_000_000_005, operationId: null },
    data,
  };
}

/// A host whose `app.about` reply is held open until the test releases it, and
/// whose projection stream can be made to report read-only recovery on demand.
async function createDeferredAboutRuntime(): Promise<{
  runtime: HostRuntime;
  releaseAbout: () => void;
  enterRecovery: () => void;
  aboutDelivered: Promise<void>;
}> {
  let releaseAbout = () => undefined as void;
  const aboutHeld = new Promise<void>((resolve) => {
    releaseAbout = () => resolve();
  });
  let markAboutDelivered = () => undefined as void;
  const aboutDelivered = new Promise<void>((resolve) => {
    markAboutDelivered = () => resolve();
  });
  let recoveryPending = false;
  let request = 0;

  const invoke = vi.fn<TauriInvoke>(async (command, args) => {
    if (command === "host_bootstrap") {
      return bootstrap;
    }
    if (command === "host_projection_events") {
      if (!recoveryPending) {
        return {
          schemaVersion: "desktop-projection-reply.v1",
          rendererSessionId,
          status: "events",
          events: [],
        };
      }
      recoveryPending = false;
      return {
        schemaVersion: "desktop-projection-reply.v1",
        rendererSessionId,
        status: "events",
        events: [{
          sequence: 19,
          occurredAt: 1_725_000_000_010,
          event: {
            type: "boot_state_changed",
            projection: { mode: "read_only_recovery" },
          },
        }],
      };
    }
    const envelope = JSON.parse(String(args?.body)) as {
      command: string;
      requestId: string;
    };
    if (envelope.command === "app.about") {
      await aboutHeld;
      // Resolved on the next tick, so awaiting it in the test guarantees the
      // renderer's own `.then` has already run.
      queueMicrotask(markAboutDelivered);
      return successfulReply(envelope.requestId, {
        kind: "about",
        value: {
          appVersion: "9.9.9-stale",
          installationId: bootstrap.installationId,
          foundationPackageName: "bmad-method",
          foundationPackageVersion: "6.10.0",
          bootMode: "ready",
          inactiveBuilderPackageCount: 0,
          updateConfigured: false,
          updateInstallAvailable: false,
        },
      });
    }
    throw new Error(`Unexpected about-guard command ${envelope.command}`);
  });

  const client = new DesktopHostClient({
    invoke,
    requestId: () => `request_about_${request += 1}`,
  });
  await client.bootstrap();
  return {
    runtime: { kind: "ready", client, bootstrap },
    releaseAbout,
    aboutDelivered,
    enterRecovery: () => {
      recoveryPending = true;
    },
  };
}

/// `loadAboutProjection` was the one host read in `App.tsx` with no generation
/// guard. `invalidateHostBoundUi` clears the About projection precisely so a
/// host that has entered read-only recovery is not still described by the
/// binding that preceded it — and an `app.about` call already in flight landed
/// afterwards and put the previous binding's version and boot mode back on
/// screen, marked ready.
describe("App About projection under a host rebind", () => {
  it("drops an About reply that resolves after the host entered recovery", async () => {
    const { aboutDelivered, runtime, releaseAbout, enterRecovery } =
      await createDeferredAboutRuntime();
    const user = userEvent.setup();
    render(
      <App hostRuntimeLoader={async () => runtime} projectionPollIntervalMs={10} />,
    );
    const titlebar = await screen.findByRole("banner", { name: "Forge application" });
    fireEvent.click(within(titlebar).getByRole("button", { name: "New task" }));
    await screen.findAllByText("Local host ready");

    // Opening Settings starts the About read, which the fixture holds open.
    await user.click(within(titlebar).getByRole("button", { name: "Settings" }));
    const settings = await screen.findByRole("dialog", { name: "Settings" });
    await user.click(within(settings).getByRole("button", { name: "About" }));

    // The host now reports read-only recovery, which rebinds the renderer.
    enterRecovery();
    await screen.findAllByText("Read-only recovery");

    releaseAbout();
    await act(async () => {
      await aboutDelivered;
      // Generous: the reply still has to travel back through the client's
      // parsing and the renderer's own continuation, and the assertion below
      // is that nothing lands — so the flush has to outlast the write it is
      // looking for rather than merely race it.
      for (let tick = 0; tick < 10; tick += 1) {
        await new Promise((resolve) => setTimeout(resolve, 0));
      }
    });

    // The stale binding's version must not reappear, and the panel must still
    // be reporting that About is unavailable in recovery rather than "ready".
    expect(screen.queryByText("9.9.9-stale")).toBeNull();
    expect(
      within(screen.getByRole("dialog", { name: "Settings" }))
        .getByText("Version information is unavailable in this mode."),
    ).toBeTruthy();
  });
});
