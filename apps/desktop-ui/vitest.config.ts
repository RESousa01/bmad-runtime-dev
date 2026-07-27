import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { forgeUiBuildDefine } from "./build-stamp";

export default defineConfig({
  // The same stamp the production bundle gets. Without it every component
  // reading `__FORGE_UI_BUILD__` throws under test, which would make the About
  // panel untestable — and a value that exists only in production is a value
  // nothing verifies.
  define: forgeUiBuildDefine(),
  plugins: [react()],
  test: {
    coverage: {
      provider: "v8",
      reporter: ["text", "json-summary"],
    },
    environment: "jsdom",
    // Tests assert observable state; the elevated ceiling absorbs CPU
    // contention during full-suite runs without hiding real hangs.
    testTimeout: 30_000,
    hookTimeout: 30_000,
    setupFiles: ["./src/test/setup.ts"],
  },
});
