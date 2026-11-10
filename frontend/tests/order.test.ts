import assert from "node:assert/strict";
import { test } from "node:test";

import { moveBeside, moveWithinGroup } from "../src/order.ts";

type S = { id: number; group_name: string };
const list: S[] = [
  { id: 1, group_name: "" },
  { id: 2, group_name: "Klien" },
  { id: 3, group_name: "" },
  { id: 4, group_name: "Klien" },
  { id: 5, group_name: "Klien" },
];

test("moves one step within the group, skipping other groups' servers", () => {
  assert.deepEqual(moveWithinGroup(list, 3, -1), [3, 2, 1, 4, 5]);
  assert.deepEqual(moveWithinGroup(list, 2, 1), [1, 4, 3, 2, 5]);
  assert.deepEqual(moveWithinGroup(list, 5, -1), [1, 2, 3, 5, 4]);
});

test("returns null at the edge of the group", () => {
  assert.equal(moveWithinGroup(list, 1, -1), null);
  assert.equal(moveWithinGroup(list, 5, 1), null);
  assert.equal(moveWithinGroup(list, 99, 1), null);
});

test("drag places the server before or after the target in the same group", () => {
  assert.deepEqual(moveBeside(list, 5, 2, false), [1, 5, 2, 3, 4]);
  assert.deepEqual(moveBeside(list, 2, 5, true), [1, 3, 4, 5, 2]);
  assert.deepEqual(moveBeside(list, 3, 1, false), [3, 1, 2, 4, 5]);
});

test("drag across groups or onto itself does nothing", () => {
  assert.equal(moveBeside(list, 1, 2, false), null);
  assert.equal(moveBeside(list, 2, 2, true), null);
  assert.equal(moveBeside(list, 2, 4, false), null);
});
