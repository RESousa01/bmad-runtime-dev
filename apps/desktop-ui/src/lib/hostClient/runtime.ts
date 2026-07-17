import { DesktopHostClient } from "./client";
import {
  type BootstrapReply,
  HostCapabilityError,
  HostCommandError,
  type TauriInvoke,
} from "./contracts";

export type HostRuntime =
  | { kind: "browser_demo"; client: null; bootstrap: null }
  | { kind: "ready"; client: DesktopHostClient; bootstrap: BootstrapReply }
  | {
      kind: "read_only_recovery";
      client: DesktopHostClient;
      bootstrap: BootstrapReply;
    }
  | { kind: "unavailable"; client: null; bootstrap: null; message: string };

export interface HostRuntimeDependencies {
  isTauri?: () => boolean;
  loadInvoke?: () => Promise<TauriInvoke>;
  now?: () => number;
  requestId?: () => string;
}

export function defaultIsTauri(): boolean {
  return (
    typeof window !== "undefined" && window.__TAURI_INTERNALS__ !== undefined
  );
}

export async function defaultLoadInvoke(): Promise<TauriInvoke> {
  const { invoke } = await import("@tauri-apps/api/core");
  return (command, args) => invoke<unknown>(command, args);
}

export async function initializeHostRuntime({
  isTauri = defaultIsTauri,
  loadInvoke = defaultLoadInvoke,
  now,
  requestId,
}: HostRuntimeDependencies = {}): Promise<HostRuntime> {
  if (!isTauri()) {
    return { kind: "browser_demo", client: null, bootstrap: null };
  }
  try {
    const client = new DesktopHostClient({
      invoke: await loadInvoke(),
      ...(now ? { now } : {}),
      ...(requestId ? { requestId } : {}),
    });
    const bootstrap = await client.bootstrap();
    return {
      kind: bootstrap.bootMode === "ready" ? "ready" : "read_only_recovery",
      client,
      bootstrap,
    };
  } catch {
    return {
      kind: "unavailable",
      client: null,
      bootstrap: null,
      message:
        "The signed Windows host could not be verified. Local actions remain unavailable.",
    };
  }
}

export let defaultRuntimePromise: Promise<HostRuntime> | null = null;

export function getDefaultHostRuntime(): Promise<HostRuntime> {
  defaultRuntimePromise ??= initializeHostRuntime();
  return defaultRuntimePromise;
}

export function getSafeHostMessage(error: unknown): string {
  if (
    error instanceof HostCommandError ||
    error instanceof HostCapabilityError
  ) {
    return error.message;
  }
  return "The Windows host could not complete that request. Nothing was changed.";
}
