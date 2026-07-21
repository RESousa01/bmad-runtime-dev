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

    // Approved 2026-07-21: warm-graphite dark palette with a peach accent
    // in the OpenCode style, replacing the original navy dark theme.
    expectExactDeclarations(darkTheme, [
      "--color-canvas: #161313;",
      "--color-chrome: #1b1818;",
      "--color-surface: #1f1c1c;",
      "--color-surface-raised: #252121;",
      "--color-surface-hover: #2d2828;",
      "--color-surface-selected: #343030;",
      "--color-border: #3e3939;",
      "--color-border-subtle: #2d2828;",
      "--color-text: #ededed;",
      "--color-text-soft: #b7b1b1;",
      "--color-text-muted: #8f8a8a;",
      "--color-accent: #fab283;",
      "--color-accent-strong: #ffc39a;",
      "--color-accent-soft: #3a2b20;",
      "--color-on-accent: #1b1210;",
      "--color-success: #64d899;",
      "--color-warning: #f2ac43;",
      "--color-danger: #f17884;",
    ]);
  });
});
