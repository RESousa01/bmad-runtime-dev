import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const tokensCss = readFileSync(
  resolve(process.cwd(), "../../packages/ui/src/tokens.css"),
  "utf8",
);

function themeBlock(start, end) {
  const startIndex = tokensCss.indexOf(start);
  const endIndex = tokensCss.indexOf(end, startIndex + start.length);
  expect(startIndex).toBeGreaterThanOrEqual(0);
  expect(endIndex).toBeGreaterThan(startIndex);
  return tokensCss.slice(startIndex, endIndex);
}

function expectExactDeclarations(block, declarations) {
  for (const declaration of declarations) {
    expect(block.split(declaration)).toHaveLength(2);
  }
}

describe("Sapphirus visual contracts", () => {
  it("keeps the approved light and dark semantic palettes unchanged", () => {
    const lightTheme = themeBlock(":root", "[data-theme=\"dark\"]");
    const darkTheme = themeBlock("[data-theme=\"dark\"]", ".sapphirus-button");

    expectExactDeclarations(lightTheme, [
      "--color-canvas: #f4f7fb;",
      "--color-chrome: #eef3f8;",
      "--color-surface: #ffffff;",
      "--color-surface-raised: #f9fbfd;",
      "--color-surface-hover: #edf3ff;",
      "--color-surface-selected: #e7efff;",
      "--color-border: #cdd8e6;",
      "--color-border-subtle: #dfe7f0;",
      "--color-text: #152033;",
      "--color-text-soft: #48566b;",
      "--color-text-muted: #5f6d82;",
      "--color-accent: #4564df;",
      "--color-accent-strong: #3557df;",
      "--color-accent-soft: #e9efff;",
      "--color-on-accent: #ffffff;",
      "--color-success: #178451;",
      "--color-warning: #a96700;",
      "--color-danger: #c33b4a;",
    ]);

    expectExactDeclarations(darkTheme, [
      "--color-canvas: #06121f;",
      "--color-chrome: #071522;",
      "--color-surface: #0a1927;",
      "--color-surface-raised: #0d1d2b;",
      "--color-surface-hover: #10243a;",
      "--color-surface-selected: #13283e;",
      "--color-border: #26394d;",
      "--color-border-subtle: #1b2e41;",
      "--color-text: #f1f4f8;",
      "--color-text-soft: #c9d0da;",
      "--color-text-muted: #98a6b8;",
      "--color-accent: #6d88ff;",
      "--color-accent-strong: #8da2ff;",
      "--color-accent-soft: #162c5c;",
      "--color-on-accent: #06121f;",
      "--color-success: #64d899;",
      "--color-warning: #f2ac43;",
      "--color-danger: #f17884;",
    ]);
  });
});
