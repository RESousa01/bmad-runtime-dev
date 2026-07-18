import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { parseProjectionEvent } from "./projectionProtocol";

const testDirectory = dirname(fileURLToPath(import.meta.url));

// TypeScript side of the shared renderer↔host projection-event golden
// fixtures; the Rust side lives in
// crates/desktop-runtime/tests/projection_event_fixtures.rs.
const fixtures = JSON.parse(
  readFileSync(
    join(testDirectory, "../../../../../tests/ipc-fixtures/projection-events.json"),
    "utf8",
  ),
) as readonly unknown[];

describe("projection event golden fixtures", () => {
  it("covers every projection event variant exactly once", () => {
    const types = fixtures.map(
      (fixture) => ((fixture as { event: { type: string } }).event).type,
    );
    expect(new Set(types).size).toBe(types.length);
    expect(types).toHaveLength(10);
  });

  it.each(fixtures.map((fixture, index) => [index, fixture] as const))(
    "parses fixture %i without loss",
    (_index, fixture) => {
      const parsed = parseProjectionEvent(fixture);
      expect(parsed).toEqual(fixture);
    },
  );
});
