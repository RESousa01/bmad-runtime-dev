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
    setupFiles: ["./src/test/setup.ts"],
  },
});
