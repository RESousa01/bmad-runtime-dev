import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
    // Matches `apps/desktop-ui`: the ceiling absorbs CPU contention during a
    // full workspace run without hiding a real hang.
    testTimeout: 30_000,
    hookTimeout: 30_000,
  },
});
