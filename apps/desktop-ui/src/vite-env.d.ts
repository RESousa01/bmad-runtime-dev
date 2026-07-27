/// <reference types="vite/client" />

/// Build stamp for the renderer bundle, injected by `vite.config.ts`.
declare const __FORGE_UI_BUILD__: string;

declare module "@fontsource-variable/inter";
declare module "@fontsource-variable/jetbrains-mono";

interface Window {
  __TAURI_INTERNALS__?: unknown;
}
