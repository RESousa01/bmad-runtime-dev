/**
 * WCAG AA over the tokens the renderer actually ships.
 *
 * These pairs are not hypothetical combinations of the palette -- each one is a
 * foreground and a background that some rule in the stylesheets puts together,
 * cited by selector. A pair nobody renders is not evidence of anything, and a
 * failing ratio on a pair nobody renders is noise that trains people to ignore
 * the check.
 *
 * The thresholds are WCAG 2.2: 4.5:1 for text below the large-text cut
 * (18.66px bold / 24px regular -- every pair here is 10-14px), and 3:1 for the
 * focus ring under 1.4.11 non-text contrast.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { contrastRatio, parseColor, parseTokenBlocks, reportRatio, resolveTheme } from "./contrast";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../../../..");
const read = (relative: string) => readFileSync(path.join(repoRoot, relative), "utf8");

const blocks = [
  ...parseTokenBlocks(read("packages/ui/src/tokens.css")),
  ...parseTokenBlocks(read("apps/desktop-ui/src/forge-v2.css")),
];

/*
 * Cascade order, not file order. `main.tsx` sets data-app-shell="forge-v2"
 * unconditionally, so the forge-v2 block (0,2,0) beats the base
 * [data-theme="dark"] block (0,1,0) wherever both declare a token -- and the
 * light block (0,3,0) beats forge-v2's own defaults. Listing these in the wrong
 * order measures a palette no user ever sees.
 */
const THEMES = {
  dark: resolveTheme(blocks, [":root", '[data-theme="dark"]', ':root[data-app-shell="forge-v2"]']),
  light: resolveTheme(blocks, [
    ":root",
    ':root[data-app-shell="forge-v2"]',
    ':root[data-app-shell="forge-v2"][data-theme="light"]',
  ]),
} as const;

const BODY_TEXT = 4.5;
const NON_TEXT = 3;

type Pair = {
  /** The rule that puts these two together. */
  where: string;
  foreground: string;
  background: string;
  /** What the background sits on, when the background is not opaque. */
  over?: string;
  minimum: number;
};

const PAIRS: Pair[] = [
  {
    where: ".recovery-banner",
    foreground: "var(--color-warning)",
    background: "color-mix(in srgb, var(--color-warning), transparent 91%)",
    over: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: ".browser-demo-banner",
    foreground: "var(--color-warning)",
    background: "color-mix(in srgb, var(--color-warning), transparent 92%)",
    over: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: ".browser-demo-banner span",
    foreground: "var(--color-text-muted)",
    background: "color-mix(in srgb, var(--color-warning), transparent 92%)",
    over: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: ".workspace-read-error",
    foreground: "var(--color-danger)",
    background: "color-mix(in srgb, var(--color-danger), transparent 93%)",
    over: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: ".workspace-panel__error",
    foreground: "var(--color-danger)",
    background: "color-mix(in srgb, var(--color-danger), transparent 92%)",
    over: "var(--color-surface-raised)",
    minimum: BODY_TEXT,
  },
  {
    where: ".sapphirus-button--danger",
    foreground: "var(--color-danger)",
    background: "color-mix(in srgb, var(--color-danger), transparent 86%)",
    over: "var(--color-surface)",
    minimum: BODY_TEXT,
  },
  {
    where: ".bmad-help",
    foreground: "var(--color-accent-strong)",
    background: "color-mix(in srgb, var(--color-accent), transparent 94%)",
    over: "var(--color-surface)",
    minimum: BODY_TEXT,
  },
  {
    where: '.bmad-availability[data-availability="available"]',
    foreground: "var(--color-status-active)",
    background: "var(--color-status-active-soft)",
    minimum: BODY_TEXT,
  },
  {
    where: '.bmad-availability[data-availability="capability_disabled"]',
    foreground: "var(--color-status-pending)",
    background: "var(--color-status-pending-soft)",
    minimum: BODY_TEXT,
  },
  {
    where: '.bmad-availability[data-availability="inactive"]',
    foreground: "var(--color-status-inactive)",
    background: "var(--color-status-inactive-soft)",
    minimum: BODY_TEXT,
  },
  {
    where: ".bmad-library-panel__reviewed-run-help",
    foreground: "var(--color-text-soft)",
    background: "var(--color-status-active-soft)",
    minimum: BODY_TEXT,
  },
  {
    where: ".bmad-fidelity",
    foreground: "var(--color-status-pending)",
    background: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: '.bmad-fidelity[data-fidelity="inert_draft"]',
    foreground: "var(--color-status-inactive)",
    background: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: '.bmad-fidelity[data-fidelity="inert_draft"] on a raised panel',
    foreground: "var(--color-status-inactive)",
    background: "var(--color-surface-raised)",
    minimum: BODY_TEXT,
  },
  {
    where: ".file-kind-icon",
    foreground: "var(--color-on-warning)",
    background: "var(--color-warning)",
    minimum: BODY_TEXT,
  },
  {
    where: ".workflow-panel__gate",
    foreground: "var(--color-status-pending)",
    background: "var(--color-surface-raised)",
    minimum: BODY_TEXT,
  },
  {
    where: ".workflow-panel__evidence",
    foreground: "var(--color-text-muted)",
    background: "var(--color-surface-raised)",
    minimum: BODY_TEXT,
  },
  {
    where: ".title-strip__tab",
    foreground: "var(--color-text-muted)",
    background: "var(--color-chrome)",
    minimum: BODY_TEXT,
  },
  {
    where: ".title-strip__tab:hover",
    foreground: "var(--color-text-soft)",
    background: "var(--color-surface-hover)",
    minimum: BODY_TEXT,
  },
  {
    where: ".title-strip__tab[data-active]",
    foreground: "var(--color-text-soft)",
    background: "var(--color-surface-selected)",
    minimum: BODY_TEXT,
  },
  {
    where: "muted label on a selected row",
    foreground: "var(--color-text-muted)",
    background: "var(--color-surface-selected)",
    minimum: BODY_TEXT,
  },
  {
    where: "muted label on a hovered row",
    foreground: "var(--color-text-muted)",
    background: "var(--color-surface-hover)",
    minimum: BODY_TEXT,
  },
  {
    where: ".message--user > div:last-child > p",
    foreground: "var(--color-text-muted)",
    background: "color-mix(in srgb, var(--color-accent), transparent 90%)",
    over: "var(--color-canvas)",
    minimum: BODY_TEXT,
  },
  {
    where: ".sapphirus-button--primary",
    foreground: "var(--color-on-accent)",
    background: "var(--color-accent)",
    minimum: BODY_TEXT,
  },
  {
    where: ".context-window__mode--automatic",
    foreground: "var(--color-accent)",
    background: "var(--color-accent-soft)",
    minimum: BODY_TEXT,
  },
];

/*
 * The focus ring is the one non-text pair that has to hold everywhere: it is
 * drawn as an outline directly on whichever surface the control sits on, so
 * every surface is an adjacent colour for 1.4.11 purposes.
 */
const FOCUS_SURFACES = [
  "--color-canvas",
  "--color-chrome",
  "--color-surface",
  "--color-surface-raised",
  "--color-surface-hover",
  "--color-surface-selected",
];

describe("contrast math", () => {
  it("agrees with the WCAG reference points", () => {
    const none = new Map<string, string>();
    const ratio = (fg: string, bg: string) =>
      reportRatio(contrastRatio(parseColor(fg, none)!, parseColor(bg, none)!));
    expect(ratio("#ffffff", "#000000")).toBe(21);
    expect(ratio("#ffffff", "#ffffff")).toBe(1);
    // The two ratios the forge-v2 token comments already record, recomputed.
    expect(ratio("#949494", "#080808")).toBe(6.6);
    expect(ratio("#777777", "#181818")).toBe(3.96);
  });

  it("composites a translucent foreground before measuring", () => {
    const none = new Map<string, string>();
    const half = parseColor("rgb(255 255 255 / 0.5)", none)!;
    const black = parseColor("#000000", none)!;
    // 50% white over black is mid grey, nowhere near white's 21:1.
    expect(reportRatio(contrastRatio(half, black))).toBeLessThan(11);
  });
});

describe.each(Object.entries(THEMES))("%s theme", (_name, theme) => {
  it.each(PAIRS)("$where meets $minimum:1", (pair) => {
    const foreground = parseColor(pair.foreground, theme);
    const background = parseColor(pair.background, theme);
    const base = parseColor(pair.over ?? pair.background, theme);
    expect(foreground, `${pair.foreground} did not resolve`).toBeDefined();
    expect(background, `${pair.background} did not resolve`).toBeDefined();
    const measured = reportRatio(contrastRatio(foreground!, background!, base!));
    expect(measured, `${pair.foreground} on ${pair.background}`).toBeGreaterThanOrEqual(pair.minimum);
  });

  it.each(FOCUS_SURFACES)("the focus ring stays visible on %s", (surface) => {
    const focus = parseColor("var(--color-focus)", theme)!;
    const background = parseColor(`var(${surface})`, theme)!;
    const measured = reportRatio(contrastRatio(focus, background));
    expect(measured, `--color-focus on ${surface}`).toBeGreaterThanOrEqual(NON_TEXT);
  });
});
