export type WindowAction = "minimize" | "toggleMaximize" | "close";

export async function performWindowAction(action: WindowAction): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return;
  }

  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const currentWindow = getCurrentWindow();

  switch (action) {
    case "minimize":
      await currentWindow.minimize();
      break;
    case "toggleMaximize":
      await currentWindow.toggleMaximize();
      break;
    case "close":
      await currentWindow.close();
      break;
  }
}
