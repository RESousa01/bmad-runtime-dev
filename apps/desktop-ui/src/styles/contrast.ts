/**
 * Measures WCAG contrast against the tokens the app actually ships.
 *
 * The point of parsing the stylesheets rather than restating their values is
 * that a restated table is a second source of truth: it agrees with the CSS on
 * the day it is written and silently stops agreeing on the day someone edits a
 * token. Everything here reads `tokens.css` and `forge-v2.css` at test time, so
 * a token change either keeps the ratios or fails the suite.
 *
 * Theme resolution has to follow the real cascade, not the file order.
 * `main.tsx` sets `data-app-shell="forge-v2"` unconditionally, so the forge-v2
 * block (specificity 0,2,0) outranks the base `[data-theme="dark"]` block
 * (0,1,0) for every token it declares -- and falls through to it for every
 * token it does not. Getting that order wrong measures a palette no user sees.
 */

export type Rgba = { r: number; g: number; b: number; a: number };

const HEX_SHORT = 4;
const HEX_FULL = 7;
const HEX_ALPHA = 9;
const FULL = 255;
const PERCENT = 100;

/** Custom-property blocks in source order, keyed by their selector. */
export function parseTokenBlocks(css: string): { selector: string; props: Map<string, string> }[] {
  const withoutComments = css.replace(/\/\*[\s\S]*?\*\//gu, "");
  const blocks: { selector: string; props: Map<string, string> }[] = [];
  for (const match of withoutComments.matchAll(/([^{}]+)\{([^{}]*)\}/gu)) {
    // Everything between the previous block and this one lands in the prelude,
    // so a file that opens with `@import` would otherwise hand back a selector
    // of "@import ...; :root" and get skipped as an at-rule. Statement at-rules
    // end in a semicolon; the selector is whatever follows the last one.
    const rawSelector = match[1] ?? "";
    const prelude = rawSelector.slice(rawSelector.lastIndexOf(";") + 1);
    const selector = prelude.replaceAll(/\s+/gu, " ").trim();
    if (!selector || selector.startsWith("@")) continue;
    const props = new Map<string, string>();
    for (const declaration of (match[2] ?? "").split(";")) {
      const colon = declaration.indexOf(":");
      if (colon < 1) continue;
      const property = declaration.slice(0, colon).trim();
      if (!property.startsWith("--")) continue;
      props.set(property, declaration.slice(colon + 1).trim());
    }
    if (props.size > 0) blocks.push({ selector, props });
  }
  return blocks;
}

/**
 * Flattens the blocks that apply to one theme, in ascending specificity.
 *
 * The caller passes the selectors in cascade order; later entries win, which is
 * what lets a forge-v2 token override a base one and lets an undeclared one
 * fall through.
 */
export function resolveTheme(
  blocks: { selector: string; props: Map<string, string> }[],
  selectorsInCascadeOrder: string[],
): Map<string, string> {
  const resolved = new Map<string, string>();
  for (const selector of selectorsInCascadeOrder) {
    for (const block of blocks) {
      if (block.selector !== selector) continue;
      for (const [key, value] of block.props) resolved.set(key, value);
    }
  }
  return resolved;
}

const NAMED = {
  black: { r: 0, g: 0, b: 0, a: 1 },
  transparent: { r: 0, g: 0, b: 0, a: 0 },
  white: { r: FULL, g: FULL, b: FULL, a: 1 },
} as const satisfies Record<string, Rgba>;

/** Splits on top-level commas, so nested `color-mix(...)` survives. */
function splitTopLevel(input: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = "";
  for (const char of input) {
    if (char === "(") depth += 1;
    if (char === ")") depth -= 1;
    if (char === "," && depth === 0) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += char;
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

function parseHex(value: string): Rgba | undefined {
  if (!value.startsWith("#")) return undefined;
  const expand = (text: string) => Number.parseInt(text, 16);
  if (value.length === HEX_SHORT) {
    return {
      r: expand((value[1] ?? "0").repeat(2)),
      g: expand((value[2] ?? "0").repeat(2)),
      b: expand((value[3] ?? "0").repeat(2)),
      a: 1,
    };
  }
  if (value.length === HEX_FULL || value.length === HEX_ALPHA) {
    return {
      r: expand(value.slice(1, 3)),
      g: expand(value.slice(3, 5)),
      b: expand(value.slice(5, 7)),
      a: value.length === HEX_ALPHA ? expand(value.slice(7, 9)) / FULL : 1,
    };
  }
  return undefined;
}

function parseRgbFunction(value: string, tokens: Map<string, string>): Rgba | undefined {
  const match = /^rgba?\((.*)\)$/iu.exec(value);
  if (!match) return undefined;
  const [channels = "", alphaPart] = (match[1] ?? "").split("/").map((piece) => piece.trim());
  const numbers = channels.split(/[\s,]+/u).filter(Boolean).map(Number);
  const alpha = alphaPart === undefined
    ? 1
    : alphaPart.endsWith("%")
      ? Number.parseFloat(alphaPart) / PERCENT
      : Number.parseFloat(alphaPart);
  void tokens;
  const [r = 0, g = 0, b = 0] = numbers;
  return { r, g, b, a: alpha };
}

/**
 * `color-mix(in srgb, A p%, B q%)`, including the `transparent` form.
 *
 * Mixing is premultiplied, which matters: a naive per-channel average of a
 * colour with `transparent` drags the result toward black instead of leaving
 * the hue alone and dropping only the alpha.
 */
function parseColorMix(value: string, tokens: Map<string, string>, seen: Set<string>): Rgba | undefined {
  const match = /^color-mix\((.*)\)$/iu.exec(value);
  if (!match) return undefined;
  const args = splitTopLevel(match[1] ?? "");
  if (args.length < 3 || !/^in\s+srgb$/iu.test((args[0] ?? "").trim())) return undefined;

  const readOperand = (operand: string): { color: Rgba; weight?: number | undefined } => {
    const percent = /\s(\d+(?:\.\d+)?)%$/u.exec(operand);
    const colorText = percent ? operand.slice(0, percent.index).trim() : operand.trim();
    return {
      color: parseColor(colorText, tokens, seen) ?? NAMED.transparent,
      weight: percent ? Number.parseFloat(percent[1] ?? "0") : undefined,
    };
  };

  const first = readOperand(args[1] ?? "");
  const second = readOperand(args[2] ?? "");
  let p1 = first.weight;
  let p2 = second.weight;
  if (p1 === undefined && p2 === undefined) {
    p1 = PERCENT / 2;
    p2 = PERCENT / 2;
  } else if (p1 === undefined) p1 = PERCENT - (p2 ?? 0);
  else if (p2 === undefined) p2 = PERCENT - p1;

  const total = (p1 ?? 0) + (p2 ?? 0);
  if (total === 0) return NAMED.transparent;
  const w1 = (p1 ?? 0) / total;
  const w2 = (p2 ?? 0) / total;

  const alpha = w1 * first.color.a + w2 * second.color.a;
  if (alpha === 0) return NAMED.transparent;
  const channel = (key: "r" | "g" | "b") =>
    (w1 * first.color.a * first.color[key] + w2 * second.color.a * second.color[key]) / alpha;
  return { r: channel("r"), g: channel("g"), b: channel("b"), a: alpha };
}

/** Resolves a CSS colour expression, following `var()` through the token map. */
export function parseColor(
  value: string,
  tokens: Map<string, string>,
  seen: Set<string> = new Set(),
): Rgba | undefined {
  const text = value.trim();
  if (!text) return undefined;

  const varMatch = /^var\(\s*(--[\w-]+)\s*(?:,([\s\S]*))?\)$/u.exec(text);
  if (varMatch) {
    const name = varMatch[1] ?? "";
    if (seen.has(name)) return undefined;
    const next = new Set(seen).add(name);
    const declared = tokens.get(name);
    if (declared !== undefined) return parseColor(declared, tokens, next);
    return varMatch[2] ? parseColor(varMatch[2], tokens, next) : undefined;
  }

  const lower = text.toLowerCase();
  if (lower === "black" || lower === "white" || lower === "transparent") return NAMED[lower];
  return parseHex(text) ?? parseRgbFunction(text, tokens) ?? parseColorMix(text, tokens, seen);
}

/** Paints `fg` onto an opaque `bg`, so an alpha colour gets a real luminance. */
export function composite(fg: Rgba, bg: Rgba): Rgba {
  return {
    r: fg.a * fg.r + (1 - fg.a) * bg.r,
    g: fg.a * fg.g + (1 - fg.a) * bg.g,
    b: fg.a * fg.b + (1 - fg.a) * bg.b,
    a: 1,
  };
}

const LINEAR_THRESHOLD = 0.04045;
const LINEAR_DIVISOR = 12.92;
const GAMMA_OFFSET = 0.055;
const GAMMA_SCALE = 1.055;
const GAMMA_EXPONENT = 2.4;

function toLinear(value: number): number {
  const channel = value / FULL;
  return channel <= LINEAR_THRESHOLD
    ? channel / LINEAR_DIVISOR
    : ((channel + GAMMA_OFFSET) / GAMMA_SCALE) ** GAMMA_EXPONENT;
}

/** WCAG 2.x relative luminance. */
export function luminance(color: Rgba): number {
  const R_WEIGHT = 0.2126;
  const G_WEIGHT = 0.7152;
  const B_WEIGHT = 0.0722;
  return R_WEIGHT * toLinear(color.r) + G_WEIGHT * toLinear(color.g) + B_WEIGHT * toLinear(color.b);
}

/** WCAG 2.x contrast ratio. Both colours are composited onto `base` first. */
export function contrastRatio(fg: Rgba, bg: Rgba, base: Rgba = NAMED.white): number {
  const OFFSET = 0.05;
  const solidBg = bg.a < 1 ? composite(bg, base) : bg;
  const solidFg = fg.a < 1 ? composite(fg, solidBg) : fg;
  const l1 = luminance(solidFg);
  const l2 = luminance(solidBg);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + OFFSET) / (darker + OFFSET);
}

/** Two decimal places, rounded down, so a reported ratio never overstates. */
export function reportRatio(ratio: number): number {
  const SCALE = 100;
  return Math.floor(ratio * SCALE) / SCALE;
}
