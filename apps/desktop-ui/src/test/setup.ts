import { cleanup, configure } from "@testing-library/react";
import { afterEach } from "vitest";

// Async queries wait on observable state; the ceiling only bounds how long a
// loaded machine may take to reach it, so it is deliberately generous.
configure({ asyncUtilTimeout: 10_000 });

afterEach(cleanup);

const cssApi = typeof CSS === "undefined" ? {} : CSS;

if (typeof CSS === "undefined") {
  Object.defineProperty(globalThis, "CSS", { value: cssApi });
}

if (!("escape" in cssApi)) {
  Object.defineProperty(cssApi, "escape", {
    value: (value: string) => value.replace(/[^a-zA-Z0-9_-]/g, (character) => `\\${character}`),
  });
}

Object.defineProperty(window, "matchMedia", {
  configurable: true,
  value: (query: string) => ({
    addEventListener: () => undefined,
    dispatchEvent: () => false,
    matches: query.includes("dark"),
    media: query,
    onchange: null,
    removeEventListener: () => undefined,
  }),
  writable: true,
});

window.requestAnimationFrame = (callback: FrameRequestCallback) => {
  callback(0);
  return 1;
};
