import assert from "node:assert/strict";
import { test } from "node:test";

import { fileBookmarkMap, findPathBookmark, folderTitle, normalizeBookmarkPath } from "../src/fileBookmarks.ts";
import type { Bookmark } from "../src/types.ts";

const bm = (id: number, server_id: number, path: string): Bookmark => ({
  id,
  server_id,
  name: `b${id}`,
  path,
  startup_cmd: "",
});

test("normalizes trailing slash like the backend", () => {
  assert.equal(normalizeBookmarkPath("/srv/a/"), "/srv/a");
  assert.equal(normalizeBookmarkPath("/"), "/");
  assert.equal(normalizeBookmarkPath("~"), "~");
});

test("groups sidebar paths per server, sorted and unique", () => {
  const map = fileBookmarkMap([bm(1, 1, "/b"), bm(2, 1, "/a/"), bm(3, 2, "/c"), bm(4, 1, "/a")]);
  assert.deepEqual(map, { "1": ["/a", "/b"], "2": ["/c"] });
});

test("finds the sidebar path for a folder on the same server only", () => {
  const list = [bm(1, 1, "/srv/a"), bm(2, 2, "/srv/b")];
  assert.equal(findPathBookmark(list, 1, "/srv/a/")?.id, 1);
  assert.equal(findPathBookmark(list, 1, "/srv/b"), undefined);
  assert.equal(findPathBookmark(list, 3, "/srv/a"), undefined);
});

test("titles folders by their last segment", () => {
  assert.equal(folderTitle("/"), "Root (/)");
  assert.equal(folderTitle("/var/www"), "www");
  assert.equal(folderTitle("/var/www/"), "www");
  assert.equal(folderTitle("~"), "~");
});
