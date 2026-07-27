import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

afterEach(cleanup);

// jsdom ships neither `CSS.escape` nor `matchMedia`; `react-aria-components`
// calls both while managing focus and selection, so a primitive throws on its
// first interaction without these. Mirrors `apps/desktop-ui/src/test/setup.ts`.
const cssApi = typeof CSS === "undefined" ? {} : CSS;

if (typeof CSS === "undefined") {
  Object.defineProperty(globalThis, "CSS", { value: cssApi });
}

if (!("escape" in cssApi)) {
  Object.defineProperty(cssApi, "escape", {
    value: (value: string) => value.replace(/[^a-zA-Z0-9_-]/g, (character) => `\\${character}`),
  });
}

// `react-aria-components` reads `matchMedia` while deciding pointer and
// reduced-motion behaviour, and jsdom does not implement it. Without this the
// first render of any Sapphirus primitive throws before an assertion runs.
Object.defineProperty(window, "matchMedia", {
  configurable: true,
  value: (query: string) => ({
    addEventListener: () => undefined,
    dispatchEvent: () => false,
    matches: false,
    media: query,
    onchange: null,
    removeEventListener: () => undefined,
  }),
  writable: true,
});
