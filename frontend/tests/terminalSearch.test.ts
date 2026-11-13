import assert from "node:assert/strict";
import { test } from "node:test";

import { findBufferMatches, stepMatchIndex } from "../src/terminalSearch.ts";

test("findBufferMatches: returns empty array for empty query", () => {
  assert.deepEqual(findBufferMatches(["hello world", "foo bar"], ""), []);
});

test("findBufferMatches: finds case-insensitive matches across multiple lines and same line", () => {
  const lines = [
    "root@srv:~# cargo test",
    "Compiling Cargo and cargo-util",
    "Finished dev profile",
  ];
  const matches = findBufferMatches(lines, "cargo");
  assert.deepEqual(matches, [
    { row: 0, col: 12, length: 5 },
    { row: 1, col: 10, length: 5 },
    { row: 1, col: 20, length: 5 },
  ]);
});

test("stepMatchIndex: wraps forward and backward safely", () => {
  assert.equal(stepMatchIndex(0, 0, 1), -1);
  assert.equal(stepMatchIndex(0, 3, 1), 1);
  assert.equal(stepMatchIndex(2, 3, 1), 0);
  assert.equal(stepMatchIndex(0, 3, -1), 2);
  assert.equal(stepMatchIndex(-1, 3, 1), 0);
  assert.equal(stepMatchIndex(-1, 3, -1), 2);
});
