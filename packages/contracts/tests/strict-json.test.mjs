import assert from "node:assert/strict";
import test from "node:test";
import { parseStrictJson } from "../scripts/lib/strict-json.mjs";

test("parses nested strict JSON without changing null or empty arrays", () => {
  const value = parseStrictJson('{"items":[],"nested":{"value":null},"safe":9007199254740991}');
  assert.deepEqual([...value.items], []);
  assert.equal(value.nested.value, null);
  assert.equal(value.safe, 9007199254740991);
});

test("rejects duplicate decoded member names", () => {
  assert.throws(() => parseStrictJson('{"name":1,"na\\u006de":2}'), {
    code: "DUPLICATE_MEMBER",
  });
});

test("rejects unpaired Unicode surrogates", () => {
  assert.throws(() => parseStrictJson('{"value":"\\ud800"}'), {
    code: "INVALID_UNICODE",
  });
});

test("rejects integers outside the interoperable range", () => {
  assert.throws(() => parseStrictJson('{"value":9007199254740992}'), {
    code: "INTEGER_OUT_OF_RANGE",
  });
});
