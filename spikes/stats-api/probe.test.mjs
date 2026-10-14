import { test } from "node:test";
import assert from "node:assert/strict";
import { createJsonFramer, decodeMessage, shapeOf } from "./probe.mjs";

test("framer splits concatenated objects across arbitrary chunk boundaries", () => {
  const out = [];
  const feed = createJsonFramer((t) => out.push(JSON.parse(t)));
  const stream = '{"Event":"A","Data":{"x":1}}{"Event":"B","Data":"{\\"y\\":\\"}{\\"}"}';
  for (const ch of stream) feed(ch);
  assert.deepEqual(out.map((o) => o.Event), ["A", "B"]);
});

test("framer ignores whitespace between objects", () => {
  const out = [];
  const feed = createJsonFramer((t) => out.push(t));
  feed('{"a":1}\r\n  {"b":');
  feed("2}\n");
  assert.equal(out.length, 2);
});

test("decodeMessage unwraps string-encoded Data", () => {
  const { event, data } = decodeMessage('{"Event":"GoalScored","Data":"{\\"GoalSpeed\\":90.5}"}');
  assert.equal(event, "GoalScored");
  assert.equal(data.GoalSpeed, 90.5);
});

test("shapeOf flattens nested arrays and objects", () => {
  const shape = shapeOf({ Players: [{ Name: "a", Boost: 33 }], Game: { bOvertime: false } });
  assert.deepEqual(Object.fromEntries(shape), {
    "Players[].Name": "string",
    "Players[].Boost": "number",
    "Game.bOvertime": "boolean",
  });
});
