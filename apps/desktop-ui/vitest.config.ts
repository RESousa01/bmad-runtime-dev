import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
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
