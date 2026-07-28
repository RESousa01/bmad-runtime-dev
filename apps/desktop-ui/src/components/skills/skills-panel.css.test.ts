/**
 * Every token the skills panel spends is a token something defines.
 *
 * FE-25 shipped `font-family: var(--font-mono)` into this sheet. Nothing
 * defines `--font-mono`, so the declaration was invalid at computed-value time
 * and dropped in silence: the recorded state pair rendered in the UI face while
 * every other identifier beside it rendered in the code face. A stylesheet
 * cannot fail loudly, and jsdom loads no CSS, so the only place this class of
 * defect can be caught is here — by reading the sheet and the token contract
 * and comparing them.
 *
 * Deliberately scoped to this one component sheet. The shared stylesheets are
 * another task's territory, and a repo-wide sweep here would fail on somebody
 * else's file while saying nothing about this one.
 */

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "../../../../..");
const read = (relative: string) => readFileSync(path.join(repoRoot, relative), "utf8");

/** Custom properties a sheet *defines*, from any selector block in it. */
function definedTokens(css: string): ReadonlySet<string> {
  return new Set([...css.matchAll(/(--[\w-]+)\s*:/gu)].map((match) => match[1] ?? ""));
}

/** Custom properties a sheet *reads*, including inside fallbacks. */
function referencedTokens(css: string): ReadonlySet<string> {
  return new Set([...css.matchAll(/var\(\s*(--[\w-]+)/gu)].map((match) => match[1] ?? ""));
}

describe("skills-panel.css", () => {
  it("reads only custom properties the shell defines", () => {
    const defined = new Set([
      ...definedTokens(read("packages/ui/src/tokens.css")),
      ...definedTokens(read("apps/desktop-ui/src/styles.css")),
      ...definedTokens(read("apps/desktop-ui/src/forge-v2.css")),
    ]);
    const used = referencedTokens(read("apps/desktop-ui/src/components/skills/skills-panel.css"));

    // Guards against the assertion passing because the sheet was renamed or the
    // regex stopped matching: an empty set trivially satisfies every subset.
    expect(used.size).toBeGreaterThan(5);
    expect([...used].filter((token) => !defined.has(token))).toEqual([]);
  });
});
