import { describe, expect, it } from "vitest";
import { parseBmadPersonaPerspective } from "./bmadProtocol";

const validPerspective = {
  schemaVersion: "sapphirus.bmad-persona-perspective.v1",
  agentCode: "bmad-agent-analyst",
  name: "Mary",
  title: "Business Analyst",
  icon: "📊",
  instructionMarkdown: "# Managed analyst persona guidance\n\nWorking stance.",
  instructionHash: `sha256:${"a".repeat(64)}`,
};

describe("persona perspective projection", () => {
  it("accepts the exact closed shape", () => {
    const parsed = parseBmadPersonaPerspective(validPerspective);
    expect(parsed.agentCode).toBe("bmad-agent-analyst");
    expect(parsed.name).toBe("Mary");
    expect(parsed.instructionMarkdown).toContain("Working stance");
  });

  it.each([
    ["extra field", { ...validPerspective, sourcePath: "runtime/x.md" }],
    ["wrong schema", { ...validPerspective, schemaVersion: "other.v1" }],
    ["foreign agent code", { ...validPerspective, agentCode: "builder-agent" }],
    ["empty markdown", { ...validPerspective, instructionMarkdown: "" }],
    [
      "oversized markdown",
      { ...validPerspective, instructionMarkdown: "x".repeat(20_000) },
    ],
    [
      "malformed hash",
      { ...validPerspective, instructionHash: "sha256:not-hex" },
    ],
    [
      "missing hash",
      (() => {
        const { instructionHash: _dropped, ...rest } = validPerspective;
        return rest;
      })(),
    ],
  ])("rejects %s", (_label, value) => {
    expect(() => parseBmadPersonaPerspective(value)).toThrow();
  });
});
