import { describe, expect, it } from "vitest";
import {
  contextSearchTerms,
  rankContextPaths,
} from "./contextSuggestions";

describe("context suggestions", () => {
  it("extracts a bounded set of useful local search terms", () => {
    expect(contextSearchTerms(
      "Please review the authentication context window and authentication errors",
    )).toEqual(["authentication", "context", "window", "errors"]);
    expect(contextSearchTerms("the and for with")).toEqual([]);
  });

  it("ranks files across term groups without counting duplicate lines twice", () => {
    const paths = rankContextPaths([
      [
        { relativePath: "src/auth.ts", line: 4, preview: "authentication" },
        { relativePath: "src/auth.ts", line: 9, preview: "authentication" },
        { relativePath: "README.md", line: 2, preview: "authentication" },
      ],
      [
        { relativePath: "src/auth.ts", line: 14, preview: "context" },
        { relativePath: "src/context.ts", line: 3, preview: "context" },
      ],
    ], 2);
    expect(paths).toEqual(["src/auth.ts", "README.md"]);
  });
});
