import assert from "node:assert/strict";
import { test } from "node:test";

import {
  buildReplaceTargets,
  countSelectedMatches,
  DEFAULT_SEARCH_EXCLUDES,
  parsePatternList,
  splitMatchLine
} from "../src/fileSearch.ts";
import type { FileSearchFileResult } from "../src/types.ts";

test("default excludes include common dependency and VCS folders", () => {
  assert.ok(DEFAULT_SEARCH_EXCLUDES.includes("node_modules"));
  assert.ok(DEFAULT_SEARCH_EXCLUDES.includes(".git"));
  assert.ok(DEFAULT_SEARCH_EXCLUDES.includes("target"));
  assert.ok(DEFAULT_SEARCH_EXCLUDES.includes("dist"));
});

test("parsePatternList splits commas and newlines, trims, and deduplicates", () => {
  assert.deepEqual(
    parsePatternList(" *.ts, *.tsx \n *.rs , , *.ts "),
    ["*.ts", "*.tsx", "*.rs"]
  );
  assert.deepEqual(parsePatternList("   "), []);
});

test("splitMatchLine slices before, match, and after safely", () => {
  assert.deepEqual(splitMatchLine("const foo = 1;", 6, 9), {
    before: "const ",
    match: "foo",
    after: " = 1;"
  });
  // Clamps out-of-range offsets without throwing
  assert.deepEqual(splitMatchLine("abc", -2, 10), {
    before: "",
    match: "abc",
    after: ""
  });
});

test("buildReplaceTargets and countSelectedMatches only include checked files", () => {
  const files: FileSearchFileResult[] = [
    {
      path: "/app/a.ts",
      rel_path: "a.ts",
      mtime: 100.5,
      match_count: 2,
      matches: []
    },
    {
      path: "/app/b.ts",
      rel_path: "b.ts",
      mtime: 200.25,
      match_count: 3,
      matches: []
    }
  ];
  const selected = new Set<string>(["/app/b.ts"]);
  assert.deepEqual(buildReplaceTargets(files, selected), [
    { path: "/app/b.ts", expected_mtime: 200.25 }
  ]);
  assert.equal(countSelectedMatches(files, selected), 3);
});
