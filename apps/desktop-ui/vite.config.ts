import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { forgeUiBuildDefine } from "./build-stamp";

export default defineConfig({
  clearScreen: false,
  // Stamps the renderer bundle so About can say which build is running.
  // Shared with `vitest.config.ts` — see `build-stamp.ts`.
  define: forgeUiBuildDefine(),
  plugins: [react(), tailwindcss()],
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
  },
});
