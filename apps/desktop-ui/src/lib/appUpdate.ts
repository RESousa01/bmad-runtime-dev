import { defaultLoadInvoke } from "./hostClient/runtime";
import type { TauriInvoke } from "./hostClient/contracts";

export type AppUpdateResult =
  | { state: "disabled" }
  | { state: "current"; version: string }
  | { state: "installed"; version: string };

export async function installAppUpdate(
  loadInvoke: () => Promise<TauriInvoke> = defaultLoadInvoke,
): Promise<AppUpdateResult> {
  const invoke = await loadInvoke();
  return parseAppUpdateResult(await invoke("install_app_update"));
}

function parseAppUpdateResult(value: unknown): AppUpdateResult {
  if (typeof value !== "object" || value === null || !("state" in value)) {
    throw new Error("Invalid app update response");
  }
  const result = value as Record<string, unknown>;
  if (result.state === "disabled" && Object.keys(result).length === 1) {
    return { state: "disabled" };
  }
  if (
    (result.state === "current" || result.state === "installed")
    && typeof result.version === "string"
    && result.version.length > 0
    && Object.keys(result).length === 2
  ) {
    return { state: result.state, version: result.version };
  }
  throw new Error("Invalid app update response");
}